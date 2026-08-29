use core::panic;
use std::{iter::Peekable, slice::Iter};

use crate::{
    asset_manager::AssetHandle,
    common::instance::{GPUInstanceHandle, InstanceHandle},
    renderer::{
        BufferType, GPUAllocationHandle, GPUBindings, InstanceUploadJob, Instruction, Operations,
        PrototypeHandle, RenderConstant, RenderUpdateDelta, RenderUpdateError, UploadMeshJob,
        VertexArenaSelector, gpu_allocator::UploadIndexJob, renderer::Renderer,
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
    pub fn interpret(
        &mut self,
        constants: Vec<RenderConstant>,
        instructions: Vec<Instruction>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<Vec<RenderUpdateDelta>, RenderUpdateError> {
        let mut stack = Vec::<RenderConstant>::new();
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
                        stack.push(val);
                    }

                    Operations::PNUUpload => {
                        let global_alloc_key = stack.pop().expect("should be gac");
                        let global_alloc_id = global_alloc_key.unwrap_key() as u32;
                        let pnu = constants[Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_data_ref();
                        self.upload_mesh(
                            UploadMeshJob::<PNUVertex>::new(pnu, global_alloc_id.clone()),
                            queue,
                            device,
                        )?;
                        stack.push(global_alloc_key);
                    }

                    Operations::PNUJWUpload => {
                        let global_alloc_key = stack.pop().expect("should be gac");
                        let global_alloc_id = global_alloc_key.unwrap_key() as u32;
                        let pnujw = constants[Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_data_ref();
                        self.upload_mesh(
                            UploadMeshJob::<PNUJWVertex>::new(pnujw, global_alloc_id.clone()),
                            queue,
                            device,
                        )?;
                        stack.push(global_alloc_key);
                    }
                    Operations::IndexUpload => {
                        let global_alloc_key = stack.pop().expect("should be gac");
                        let global_alloc_id = global_alloc_key.unwrap_key() as u32;
                        let indices = constants[Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_data_ref();
                        self.upload_indices(
                            UploadIndexJob {
                                indices,
                                global_alloc_id,
                            },
                            queue,
                            device,
                        )?;
                        stack.push(global_alloc_key);
                    }
                    Operations::EmitAssetUpload => {
                        let global_alloc_id = stack.pop().expect("should be gac").unwrap_key();
                        let asset_handle =
                            stack.pop().expect("should be asset handle").unwrap_key();
                        res.push(RenderUpdateDelta::AssetGPULoaded(
                            AssetHandle::from_key(asset_handle),
                            GPUAllocationHandle::from_key(global_alloc_id),
                        ));
                    }
                    Operations::AddAsset => {
                        let handle_idx = Self::get_constant_idx(&mut instr_peek);
                        stack.push(constants[handle_idx as usize].clone()); // push asset handle to stack

                        let global_allocation_id = self.get_global_alloc_id();

                        stack.push(RenderConstant::Key(
                            GPUAllocationHandle {
                                global_allocation_id,
                            }
                            .as_key(),
                        ));
                    }
                    Operations::MoveEntity => todo!(),
                    Operations::EmitEntitySpawn => {
                        let bind_mask = GPUBindings::from_bits(Self::get_byte(&mut instr_peek))
                            .expect("should be a valid mask");
                        assert!(bind_mask.contains(GPUBindings::LOCAL_TRANSFORM));

                        let gpu_instance_handle_key =
                            stack.pop().expect("should be key").unwrap_key();
                        let gpu_instance_handle =
                            GPUInstanceHandle::from_key(gpu_instance_handle_key);
                        let joint_offset: Option<u32> =
                            if bind_mask.contains(GPUBindings::JOINT_TRANSFORM) {
                                Some(stack.pop().expect("should be offset").unwrap_offset() as u32)
                            } else {
                                None
                            };
                        let lt_offset =
                            stack.pop().expect("should be offset").unwrap_offset() as u32;

                        println!("JT OFFSET: {:?}", joint_offset);
                        println!("LT OFFSET: {:?}", lt_offset);
                        let record_data: Vec<u8> = bytemuck::pod_collect_to_vec(&[
                            lt_offset,
                            joint_offset.unwrap_or_default(),
                            0,
                            0,
                        ]);
                        let record_job: InstanceUploadJob<InstanceRecordData> =
                            InstanceUploadJob::new(&record_data, gpu_instance_handle);
                        let record_offset =
                            self.upload_instance_record(record_job, queue, device)?;

                        let instance_handle_key = stack.pop().expect("should be key").unwrap_key();
                        res.push(RenderUpdateDelta::EntitySpawned {
                            instance_handle: InstanceHandle::from_key(instance_handle_key),
                            gpu_instance_handle,
                            record_offset: record_offset,
                        });
                    }
                    Operations::LocalTransformUpload => {
                        let gpu_handle_key = stack.pop().expect("should be key");
                        let gpu_instance_handle =
                            GPUInstanceHandle::from_key(gpu_handle_key.unwrap_key());
                        let lt = constants[Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_data_ref();
                        let lt_upload_job = InstanceUploadJob::new(lt, gpu_instance_handle.clone());
                        let lt_offset =
                            self.upload_local_transforms(lt_upload_job, queue, device)?;

                        stack.push(RenderConstant::Offset(lt_offset as u64));
                        stack.push(gpu_handle_key);
                    }
                    Operations::CreatePrototype => {
                        let prototype_idx = Self::get_constant_idx(&mut instr_peek);
                        let prototype_handle = PrototypeHandle::from_key(
                            constants[prototype_idx as usize].unwrap_key(),
                        );

                        self.add_prototype(prototype_handle.clone());
                        let handle_idx = Self::get_constant_idx(&mut instr_peek);
                        let instance_handle_key = constants[handle_idx as usize].clone();

                        let instance_handle =
                            InstanceHandle::from_key(instance_handle_key.unwrap_key());
                        res.push(RenderUpdateDelta::ProtypeCreated {
                            instance_handle: instance_handle.clone(),
                            prototype: prototype_handle,
                        });
                        stack.push(instance_handle_key);
                        stack.push(RenderConstant::Key(prototype_handle.as_key()));
                    }
                    Operations::SpawnEntityInstance => {
                        let prototype_key = stack.pop().expect("should be prototype key");
                        let prototype_handle =
                            PrototypeHandle::from_key(prototype_key.unwrap_key());
                        self.add_prototype_instance(&prototype_handle);
                        let gpu_instance_handle = self.get_gpu_instance_handle(&prototype_handle);
                        stack.push(RenderConstant::Key(gpu_instance_handle.as_key()));
                    }
                    Operations::JointTransformUpload => {
                        let gpu_handle_key = stack.pop().expect("should be key");
                        let gpu_instance_handle =
                            GPUInstanceHandle::from_key(gpu_handle_key.unwrap_key());
                        let jt = constants[Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_data_ref();
                        let ibms = constants[Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_data_ref();
                        let jt_upload_job = InstanceUploadJob::new(jt, gpu_instance_handle.clone());
                        let ibm_upload_job =
                            InstanceUploadJob::new(ibms, gpu_instance_handle.clone());
                        let jt_offset =
                            self.upload_skin_data(jt_upload_job, ibm_upload_job, queue, device)?;

                        // NOTE: ibm offset should always be the same as joint offset
                        stack.push(RenderConstant::Offset(jt_offset as u64));
                        stack.push(gpu_handle_key);
                    }
                    Operations::SpawnFromPrototype => {
                        let prototype_key = stack.pop().expect("should be prototype key");
                        let prototype_handle =
                            PrototypeHandle::from_key(prototype_key.unwrap_key());
                        self.add_prototype_instance(&prototype_handle);
                        let new_gpu_handle = self.get_gpu_instance_handle(&prototype_handle);

                        // instance handle
                        let const_idx = Self::get_constant_idx(&mut instr_peek);
                        stack.push(constants[const_idx as usize].clone());

                        // new -> donor
                        stack.push(RenderConstant::Key(new_gpu_handle.as_key()));
                    }
                    Operations::ShareData => {
                        let new_handle_key = stack.pop().expect("should be gpu handle");
                        let new_handle = GPUInstanceHandle::from_key(new_handle_key.unwrap_key());

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
                                    stack.push(RenderConstant::Offset(lt_offset as u64));
                                }
                                BufferType::JointTransform => {
                                    let slot =
                                        self.bind_groups.get_slot(&new_handle.prototype, *bt);
                                    let (jt_offset, _ibm_offset) = self
                                        .bind_groups
                                        .skinning
                                        .register_shared_binding(slot, &new_handle)
                                        .expect("register shared skin fail");
                                    stack.push(RenderConstant::Offset(jt_offset as u64));
                                }
                            }
                            stack.push(new_handle_key);
                        } else {
                            panic!("expected buffer type instr for share")
                        }
                    }
                    Operations::CopyData => {
                        let new_handle_key = stack.pop().expect("should be gpu handle");
                        let new_handle = GPUInstanceHandle::from_key(new_handle_key.unwrap_key());
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
                                    stack.push(RenderConstant::Offset(lt_offset as u64));
                                    stack.push(new_handle_key);
                                }
                                BufferType::JointTransform => {
                                    let slot =
                                        self.bind_groups.get_slot(&new_handle.prototype, *bt);
                                    let (jt_offset, _ibm_offset) = self
                                        .bind_groups
                                        .skinning
                                        .register_copy_binding(slot, &new_handle, queue, device)?;
                                    stack.push(RenderConstant::Offset(jt_offset as u64));
                                    stack.push(new_handle_key);
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
                        self.despawn(&gpu_instance_handle);
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
                        todo!()
                    }
                },
                Instruction::Byte(_byte) => {}
                Instruction::ConstIdx(_idx) => {}
            }
        }

        Ok(res)
    }
}
