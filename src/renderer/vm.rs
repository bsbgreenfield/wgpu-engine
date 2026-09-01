use core::panic;
use std::{iter::Peekable, slice::Iter};

use crate::{
    renderer::{
        BufferType, GPUAllocationHandle, GPUBindings, GPUInstanceHandle, InstanceUploadJob,
        Instruction, Operations, PrototypeHandle, RenderConstant, RenderUpdateDelta,
        RenderUpdateError, StackValue, UploadMeshJob, VertexArenaSelector,
        gpu_allocator::{GPUUploadResult, UploadIndexJob},
        renderer::Renderer,
    },
    util::types::{InstanceRecordData, PNUJWVertex, PNUVertex},
    world::RenderKey,
};

type InstructionSet<'a> = Peekable<Iter<'a, Instruction>>;

impl<'frame> Renderer {
    fn get_constant_idx(instructions: &mut InstructionSet) -> usize {
        let res = match instructions.next().unwrap() {
            Instruction::WideIdx(high) => {
                if let Some(Instruction::ConstIdx(low)) = instructions.next() {
                    ((*high as usize) << 8) | (*low as usize)
                } else {
                    panic!("should be wide");
                }
            }
            Instruction::ConstIdx(idx) => *idx as usize,
            _ => panic!("expected a const idx"),
        };
        res
    }
    fn get_byte(instructions: &mut InstructionSet) -> u8 {
        let instr = instructions.next().expect("should define a byte");
        match instr {
            Instruction::Byte(number) => *number,
            _ => panic!("expected a byte"),
        }
    }
    pub(super) fn interpret(
        &mut self,
        constants: Vec<RenderConstant>,
        instructions: Vec<Instruction>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<Vec<RenderUpdateDelta>, RenderUpdateError> {
        let mut stack = Vec::<StackValue>::new();
        let mut res: Vec<RenderUpdateDelta> = Vec::new();
        let mut instr_peek = instructions.iter().peekable();

        while instr_peek.peek().is_some() {
            let instr = instr_peek.next().unwrap();
            match instr {
                Instruction::WideIdx(_) => {}
                Instruction::Buffer(_bt) => {
                    //
                }
                Instruction::Op(op) => match op {
                    Operations::Pop => {
                        stack.pop();
                    }
                    Operations::Push => {
                        let val_idx = Self::get_constant_idx(&mut instr_peek);
                        let val = constants[val_idx as usize].clone();
                        stack.push(val.into());
                    }

                    Operations::PNUUpload => {
                        let alloc_handle = stack.pop().expect("should be gac").as_alloc();
                        let pnu = constants[Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_data_ref();
                        self.upload_mesh(
                            UploadMeshJob::<PNUVertex>::new(pnu, alloc_handle.clone()),
                            queue,
                            device,
                        )?;
                        stack.push(StackValue::Alloc(alloc_handle));
                    }

                    Operations::PNUJWUpload => {
                        let alloc_handle = stack.pop().expect("should be gac").as_alloc();
                        let pnujw = constants[Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_data_ref();
                        self.upload_mesh(
                            UploadMeshJob::<PNUJWVertex>::new(pnujw, alloc_handle.clone()),
                            queue,
                            device,
                        )?;
                        stack.push(StackValue::Alloc(alloc_handle));
                    }
                    Operations::IndexUpload => {
                        let alloc_handle = stack.pop().expect("should be gac").as_alloc();
                        let indices = constants[Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_data_ref();
                        self.upload_indices(
                            UploadIndexJob {
                                indices,
                                alloc_handle: alloc_handle.clone(),
                            },
                            queue,
                            device,
                        )?;
                        stack.push(StackValue::Alloc(alloc_handle));
                    }
                    Operations::EmitAssetUpload => {
                        let alloc_handle = stack.pop().expect("should be gac").as_alloc();
                        let asset_key = stack.pop().expect("should be asset handle").as_raw_key();
                        res.push(RenderUpdateDelta::AssetGPULoaded {
                            key: asset_key,
                            alloc_handle: alloc_handle,
                        });
                    }
                    Operations::AddAsset => {
                        let asset_key = Self::get_constant_idx(&mut instr_peek);
                        stack.push(constants[asset_key as usize].clone().into()); // push asset handle to stack

                        let global_allocation_id = self.get_global_alloc_id();

                        stack.push(StackValue::Alloc(GPUAllocationHandle {
                            global_allocation_id,
                        }));
                    }
                    Operations::MoveEntity => todo!(),
                    Operations::EmitEntitySpawn => {
                        let bind_mask = GPUBindings::from_bits(Self::get_byte(&mut instr_peek))
                            .expect("should be a valid mask");
                        assert!(bind_mask.contains(GPUBindings::LOCAL_TRANSFORM));

                        let gpu_instance_handle =
                            stack.pop().expect("should be payload").as_instance_handle();
                        let joint_offset: Option<u32> =
                            if bind_mask.contains(GPUBindings::JOINT_TRANSFORM) {
                                Some(stack.pop().expect("should be offset").as_offset())
                            } else {
                                None
                            };
                        let lt_offset = stack.pop().expect("should be offset").as_offset();

                        let record_data: Vec<u8> = bytemuck::pod_collect_to_vec(&[
                            lt_offset,
                            joint_offset.unwrap_or_default(),
                            0,
                            0,
                        ]);
                        let record_job: InstanceUploadJob<InstanceRecordData> =
                            InstanceUploadJob::new(&record_data, gpu_instance_handle);
                        let GPUUploadResult::BindGroupUploadResult {
                            buffer_element_offset,
                            ..
                        } = self.upload_instance_record(record_job, queue, device)?
                        else {
                            panic!("unexpected upload result type")
                        };

                        let instance_key = stack.pop().expect("should be key").as_raw_key();
                        res.push(RenderUpdateDelta::EntitySpawned {
                            instance_key,
                            gpu_instance_handle,
                            record_offset: buffer_element_offset,
                        });
                    }
                    Operations::LocalTransformUpload => {
                        let gpu_instance_handle =
                            stack.pop().expect("should be payload").as_instance_handle();
                        let lt = constants[Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_data_ref();
                        let lt_upload_job = InstanceUploadJob::new(lt, gpu_instance_handle.clone());
                        let GPUUploadResult::BindGroupUploadResult {
                            buffer_element_offset,
                            alloc_meta_idx: _,
                        } = self.upload_local_transforms(lt_upload_job, queue, device)?
                        else {
                            panic!("expected bing group upload")
                        };

                        stack.push(StackValue::Offset(buffer_element_offset));
                        stack.push(StackValue::Instance(gpu_instance_handle));
                    }
                    Operations::CreatePrototype => {
                        let prototype_idx = Self::get_constant_idx(&mut instr_peek);
                        let prototype_handle = PrototypeHandle::from_key(
                            constants[prototype_idx as usize].unwrap_key(),
                        );

                        self.add_prototype(prototype_handle.clone());
                        let handle_idx = Self::get_constant_idx(&mut instr_peek);
                        let instance_handle_key = constants[handle_idx as usize].clone();

                        stack.push(instance_handle_key.into());
                        stack.push(StackValue::Key(prototype_handle.as_key()));
                    }
                    Operations::SpawnEntityInstance => {
                        let prototype_key =
                            stack.pop().expect("should be prototype key").as_raw_key();
                        let prototype_handle = PrototypeHandle::from_key(prototype_key);
                        self.add_prototype_instance(&prototype_handle);
                        let gpu_instance_handle = self.get_gpu_instance_handle(&prototype_handle);
                        // TODO: GPU instance handle should be a payload
                        stack.push(StackValue::Instance(gpu_instance_handle));
                    }
                    Operations::JointTransformUpload => {
                        let gpu_instance_handle =
                            stack.pop().expect("should be payload").as_instance_handle();
                        let jt = constants[Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_data_ref();
                        let ibms = constants[Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_data_ref();
                        let jt_upload_job = InstanceUploadJob::new(jt, gpu_instance_handle.clone());
                        let ibm_upload_job =
                            InstanceUploadJob::new(ibms, gpu_instance_handle.clone());
                        let GPUUploadResult::BindGroupUploadResult {
                            buffer_element_offset,
                            alloc_meta_idx: _,
                        } = self.upload_skin_data(jt_upload_job, ibm_upload_job, queue, device)?
                        else {
                            panic!("expected bin group upload");
                        };

                        // NOTE: ibm offset should always be the same as joint offset
                        stack.push(StackValue::Offset(buffer_element_offset));
                        stack.push(StackValue::Instance(gpu_instance_handle));
                    }
                    Operations::SpawnFromPrototype => {
                        let prototype_key =
                            stack.pop().expect("should be prototype key").as_raw_key();
                        let prototype_handle = PrototypeHandle::from_key(prototype_key);
                        self.add_prototype_instance(&prototype_handle);
                        let new_gpu_handle = self.get_gpu_instance_handle(&prototype_handle);

                        // instance handle
                        let const_idx = Self::get_constant_idx(&mut instr_peek);
                        stack.push(constants[const_idx as usize].clone().into());

                        // new -> donor
                        stack.push(StackValue::Instance(new_gpu_handle));
                    }
                    Operations::ShareData => {
                        let new_handle = stack
                            .pop()
                            .expect("should be gpu handle")
                            .as_instance_handle();

                        if let Some(Instruction::Buffer(bt)) = instr_peek.next() {
                            match bt {
                                BufferType::LocalTransform => {
                                    let slot =
                                        self.bind_groups.get_slot(&new_handle.prototype, *bt);
                                    let lt_offset = self
                                        .bind_groups
                                        .local_transforms
                                        .register_shared_binding(slot, &new_handle)
                                        .expect("register shared lt fail");
                                    stack.push(StackValue::Offset(lt_offset));
                                }
                                BufferType::JointTransform => {
                                    let slot =
                                        self.bind_groups.get_slot(&new_handle.prototype, *bt);
                                    let (jt_offset, _ibm_offset) = self
                                        .bind_groups
                                        .skinning
                                        .register_shared_binding(slot, &new_handle)
                                        .expect("register shared skin fail");
                                    stack.push(StackValue::Offset(jt_offset));
                                }
                            }
                            stack.push(StackValue::Instance(new_handle));
                        } else {
                            panic!("expected buffer type instr for share")
                        }
                    }
                    Operations::CopyData => {
                        let new_handle = stack
                            .pop()
                            .expect("should be gpu handle")
                            .as_instance_handle();
                        if let Some(Instruction::Buffer(bt)) = instr_peek.next() {
                            match bt {
                                BufferType::LocalTransform => {
                                    let slot =
                                        self.bind_groups.get_slot(&new_handle.prototype, *bt);
                                    let lt_offset = self
                                        .bind_groups
                                        .local_transforms
                                        .register_copy_binding(slot, &new_handle, queue, device)
                                        .expect("register shared lt fail");
                                    stack.push(StackValue::Offset(lt_offset));
                                    stack.push(StackValue::Instance(new_handle));
                                }
                                BufferType::JointTransform => {
                                    let slot =
                                        self.bind_groups.get_slot(&new_handle.prototype, *bt);
                                    let (jt_offset, _ibm_offset) = self
                                        .bind_groups
                                        .skinning
                                        .register_copy_binding(slot, &new_handle, queue, device)?;
                                    stack.push(StackValue::Offset(jt_offset));
                                    stack.push(StackValue::Instance(new_handle));
                                }
                            }
                        } else {
                            panic!("expected buffer type instr for share")
                        }
                    }
                    Operations::DespawnInstance => {
                        let gpu_instance_handle_idx = Self::get_constant_idx(&mut instr_peek);
                        let gpu_instance_handle_key = &constants[gpu_instance_handle_idx];
                        let gpu_instance_handle =
                            GPUInstanceHandle::from_key(gpu_instance_handle_key.unwrap_key());
                        self.despawn_instance(&gpu_instance_handle);
                        if let Some(delta) = res.pop()
                            && let RenderUpdateDelta::InstanceDespawns(mut handles) = delta
                        {
                            handles.push(gpu_instance_handle);
                        } else {
                        }
                        res.push(RenderUpdateDelta::InstanceDespawns(vec![
                            gpu_instance_handle,
                        ]));
                    }
                    Operations::DespawnAsset => {
                        let asset_key_idx = Self::get_constant_idx(&mut instr_peek);
                        let asset_key = constants[asset_key_idx].unwrap_key();
                        let gpu_alloc_handle_idx = Self::get_constant_idx(&mut instr_peek);
                        let gpu_alloc_handle_key = &constants[gpu_alloc_handle_idx];
                        let gpu_alloc_handle =
                            GPUAllocationHandle::from_key(gpu_alloc_handle_key.unwrap_key());
                        self.unload_asset(gpu_alloc_handle.clone());
                        res.push(RenderUpdateDelta::AssetUnloaded {
                            key: asset_key,
                            alloc_handle: gpu_alloc_handle,
                        })
                    }
                },
                Instruction::Byte(_byte) => {}
                Instruction::ConstIdx(_idx) => {}
            }
        }

        Ok(res)
    }
}
