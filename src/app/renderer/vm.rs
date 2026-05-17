use core::panic;
use std::{iter::Peekable, slice::Iter};

use crate::{
    app::renderer::{
        GPUAllocationHandle, GPUBindings, InstanceUploadJob, Instruction, Operations,
        RenderConstant, RenderUpdateDelta, RenderUpdateError, UploadMeshJob, VertexArenaSelector,
        gpu_allocator::UploadIndexJob, renderer::Renderer,
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
    ) -> Result<Vec<RenderUpdateDelta>, RenderUpdateError> {
        let mut stack = Vec::<RenderConstant>::new();
        let mut res: Vec<RenderUpdateDelta> = Vec::new();
        let mut instr_peek = instructions.iter().peekable();

        while instr_peek.peek().is_some() {
            let instr = instr_peek.next().unwrap();
            match instr {
                Instruction::Op(op) => match op {
                    Operations::Pop => {
                        stack.pop();
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
                        let instance_handle_key = stack.pop().expect("should be key").unwrap_key();
                        let joint_offset: Option<u32> =
                            if bind_mask.contains(GPUBindings::JOINT_TRANSFORM) {
                                Some(stack.pop().expect("should be offset").unwrap_offset() as u32)
                            } else {
                                None
                            };
                        let lt_offset = stack.pop().expect("should be offset").unwrap_offset();

                        res.push(RenderUpdateDelta::EntitySpawned((
                            InstanceHandle::from_key(instance_handle_key),
                            InstanceGPUBindings {
                                lt_offset: lt_offset as u32,
                                joint_offset,
                            },
                        )));
                    }
                    Operations::LocalTransformUpload => {
                        let instance_handle_key = stack.pop().expect("should be key");
                        let instance_handle =
                            InstanceHandle::from_key(instance_handle_key.unwrap_key());
                        let lt = constants[Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_data_owned();
                        let lt_upload_job = InstanceUploadJob::new(lt, instance_handle.clone());
                        let lt_offset = self.upload_local_transforms(lt_upload_job, queue)?;
                        stack.push(RenderConstant::Offset(lt_offset as u64));
                        stack.push(instance_handle_key);
                    }
                    Operations::ResolveSharedLTBinding => {
                        let donor_handle = constants
                            [Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_key();
                        let new_handle = stack.pop().expect("should be key");
                        let lt_offset = self.resolve_shared_lt_binding(
                            &InstanceHandle::from_key(donor_handle),
                            &InstanceHandle::from_key(new_handle.unwrap_key()),
                        )?;
                        stack.push(RenderConstant::Offset(lt_offset as u64));
                        stack.push(new_handle);
                    }
                    Operations::SpawnEntityInstance => {
                        // push instance handle
                        let const_idx = Self::get_constant_idx(&mut instr_peek);
                        stack.push(constants[const_idx as usize].clone());
                    }
                    Operations::JointTransformUpload => {
                        let instance_handle_key = stack.pop().expect("should be key");
                        let instance_handle =
                            InstanceHandle::from_key(instance_handle_key.unwrap_key());
                        let jt = constants[Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_data_owned();
                        let jt_upload_job = InstanceUploadJob::new(jt, instance_handle.clone());
                        let jt_offset = self.upload_joint_transforms(jt_upload_job, queue)?;
                        stack.push(RenderConstant::Offset(jt_offset as u64));
                        stack.push(instance_handle_key);
                    }
                    Operations::ResolveSharedJTBinding => {
                        let donor_handle = constants
                            [Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_key();
                        let new_handle = stack.pop().expect("should be key");
                        let jt_offset = self.resolve_shared_joint_binding(
                            &InstanceHandle::from_key(donor_handle),
                            &InstanceHandle::from_key(new_handle.unwrap_key()),
                        )?;
                        stack.push(RenderConstant::Offset(jt_offset as u64));
                        stack.push(new_handle);
                    }
                },
                Instruction::Byte(_byte) => {}
                Instruction::ConstIdx(_idx) => {}
            }
        }

        Ok(res)
    }
}
