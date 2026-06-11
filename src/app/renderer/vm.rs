use core::panic;
use std::{iter::Peekable, slice::Iter};

use crate::{
    app::renderer::{
        BufferType, GPUAllocationHandle, GPUBindings, InstanceUploadJob, Instruction, Operations,
        PrototypeHandle, RenderConstant, RenderUpdateDelta, RenderUpdateError, UploadMeshJob,
        VertexArenaSelector,
        gpu_allocator::UploadIndexJob,
        renderer::{GPUInstanceHandle, Renderer},
    },
    asset_manager::AssetHandle,
    util::types::{PNUJWVertex, PNUVertex},
    world::{
        RenderKey,
        instance_manager::{InstanceGPUBindings, InstanceHandle},
    },
};

type InstructionSet<'a> = Peekable<Iter<'a, Instruction>>;

impl<'frame> Renderer {
    fn get_constant_idx(instructions: &mut InstructionSet) -> u8 {
        let instr = instructions.next().expect("should define a constant idx");
        match instr {
            Instruction::ConstIdx(idx) => *idx,
            _ => panic!("expected a constant idx"),
        }
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
        let mut stack = Vec::<RenderConstant>::new();
        let mut res: Vec<RenderUpdateDelta> = Vec::new();
        let mut instr_peek = instructions.iter().peekable();

        while instr_peek.peek().is_some() {
            let instr = instr_peek.next().unwrap();
            match instr {
                Instruction::Buffer(bt) => {
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
                        let gpu_instance_handle = GPUInstanceHandle(gpu_instance_handle_key as u32);
                        let joint_offset: Option<u32> =
                            if bind_mask.contains(GPUBindings::JOINT_TRANSFORM) {
                                Some(stack.pop().expect("should be offset").unwrap_offset() as u32)
                            } else {
                                None
                            };
                        let lt_offset = stack.pop().expect("should be offset").unwrap_offset();

                        let instance_handle_key = stack.pop().expect("should be key").unwrap_key();
                        res.push(RenderUpdateDelta::EntitySpawned {
                            instance_handle: InstanceHandle::from_key(instance_handle_key),
                            gpu_instance_handle,
                            bindings: InstanceGPUBindings {
                                lt_offset: lt_offset as u32,
                                joint_offset,
                            },
                        });
                    }
                    Operations::LocalTransformUpload => {
                        let gpu_handle_key = stack.pop().expect("should be key");
                        let gpu_instance_handle =
                            GPUInstanceHandle(gpu_handle_key.unwrap_key() as u32);
                        let lt = constants[Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_data_owned();
                        let lt_upload_job = InstanceUploadJob::new(lt, gpu_instance_handle.clone());
                        let lt_offset = self.upload_local_transforms(lt_upload_job, queue)?;

                        stack.push(RenderConstant::Offset(lt_offset as u64));
                        stack.push(gpu_handle_key);
                    }
                    Operations::CreatePrototype => {
                        let gpu_handle_key = stack.pop().expect("should be key");
                        let gpu_instance_handle =
                            GPUInstanceHandle(gpu_handle_key.unwrap_key() as u32);
                        let prototype_handle = self.add_prototype(gpu_instance_handle);
                        let instance_handle_key =
                            stack.pop().expect("should be instance handle key");
                        let instance_handle =
                            InstanceHandle::from_key(instance_handle_key.unwrap_key());
                        res.push(RenderUpdateDelta::ProtypeCreated {
                            instance_handle: instance_handle.clone(),
                            prototype: prototype_handle.clone(),
                        });
                        stack.push(RenderConstant::Key(prototype_handle.0 as u64));
                        stack.push(instance_handle_key);
                        stack.push(gpu_handle_key);
                    }
                    Operations::SpawnEntityInstance => {
                        // push instance handle
                        let const_idx = Self::get_constant_idx(&mut instr_peek);
                        stack.push(constants[const_idx as usize].clone());

                        let gpu_instance_handle = self.get_gpu_instance_handle();
                        stack.push(RenderConstant::Key(gpu_instance_handle.0 as u64));
                    }
                    Operations::JointTransformUpload => {
                        let gpu_handle_key = stack.pop().expect("should be key");
                        let gpu_instance_handle =
                            GPUInstanceHandle(gpu_handle_key.unwrap_key() as u32);
                        let jt = constants[Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_data_owned();
                        let ibms = constants[Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_data_owned();
                        let jt_upload_job = InstanceUploadJob::new(jt, gpu_instance_handle.clone());
                        let ibm_upload_job =
                            InstanceUploadJob::new(ibms, gpu_instance_handle.clone());
                        let jt_offset =
                            self.upload_skin_data(jt_upload_job, ibm_upload_job, queue)?;

                        // NOTE: ibm offset should always be the same as joint offset
                        stack.push(RenderConstant::Offset(jt_offset as u64));
                        stack.push(gpu_handle_key);
                    }
                    Operations::SpawnFromPrototype => {
                        let prototype_key = stack.pop().expect("should be prototype key");
                        let prototype_handle =
                            PrototypeHandle::from_key(prototype_key.unwrap_key());
                        let donor_gpu_handle = self
                            .instance_arenas
                            .get_prototype_gpu_handle(&prototype_handle);
                        let new_gpu_handle = self.get_gpu_instance_handle();

                        let const_idx = Self::get_constant_idx(&mut instr_peek);
                        stack.push(constants[const_idx as usize].clone());

                        stack.push(RenderConstant::Key(new_gpu_handle.0 as u64));
                        stack.push(RenderConstant::Key(donor_gpu_handle.0 as u64));
                    }
                    Operations::ShareData => {
                        let donor_handle_key = stack.pop().expect("should be donor handle");
                        let new_handle_key = stack.pop().expect("should be gpu handle");
                        let donor_handle = GPUInstanceHandle(donor_handle_key.unwrap_key() as u32);
                        let new_handle = GPUInstanceHandle(new_handle_key.unwrap_key() as u32);

                        if let Some(Instruction::Buffer(bt)) = instr_peek.next() {
                            match bt {
                                BufferType::LocalTransform => {
                                    let lt_offset = self
                                        .instance_arenas
                                        .local_transforms
                                        .register_shared_binding(&donor_handle, &new_handle)
                                        .expect("register shared lt fail");
                                    stack.push(RenderConstant::Offset(lt_offset as u64));
                                    stack.push(new_handle_key);
                                    stack.push(donor_handle_key);
                                }
                                BufferType::JointTransform => {
                                    let (jt_offset, ibm_offset) = self
                                        .instance_arenas
                                        .skinning
                                        .register_shared_binding(&donor_handle, &new_handle)
                                        .expect("register shared skin fail");
                                    stack.push(RenderConstant::Offset(jt_offset as u64));
                                    stack.push(new_handle_key);
                                    stack.push(donor_handle_key);
                                }
                            }
                        } else {
                            panic!("expected buffer type instr for share")
                        }
                    }
                    Operations::CopyData => {
                        let donor_handle_key = stack.pop().expect("should be donor handle");
                        let new_handle_key = stack.pop().expect("should be gpu handle");
                        let donor_handle = GPUInstanceHandle(donor_handle_key.unwrap_key() as u32);
                        let new_handle = GPUInstanceHandle(new_handle_key.unwrap_key() as u32);
                        if let Some(Instruction::Buffer(bt)) = instr_peek.next() {
                            match bt {
                                BufferType::LocalTransform => {
                                    let lt_offset = self
                                        .instance_arenas
                                        .local_transforms
                                        .register_copy_binding(
                                            &donor_handle,
                                            &new_handle,
                                            queue,
                                            device,
                                        )
                                        .expect("register shared lt fail");
                                    stack.push(RenderConstant::Offset(lt_offset as u64));
                                    stack.push(new_handle_key);
                                    stack.push(donor_handle_key);
                                }
                                BufferType::JointTransform => {
                                    let (jt_offset, ibm_offset) =
                                        self.instance_arenas.skinning.register_copy_binding(
                                            &donor_handle,
                                            &new_handle,
                                            queue,
                                            device,
                                        )?;
                                    stack.push(RenderConstant::Offset(jt_offset as u64));
                                    stack.push(new_handle_key);
                                    stack.push(donor_handle_key);
                                }
                            }
                        } else {
                            panic!("expected buffer type instr for share")
                        }
                    }
                    Operations::LoadPrototype => todo!(),
                },
                Instruction::Byte(_byte) => {}
                Instruction::ConstIdx(_idx) => {}
            }
        }

        Ok(res)
    }
}
