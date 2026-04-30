use core::panic;
use std::{clone, iter::Peekable, slice::Iter};

use crate::{
    app::{
        GPUAssetUploadJob,
        renderer::{
            GPUAllocationHandle, InstanceUploadJob, Instruction, Operations, RenderConstant,
            RenderUpdateDelta, RenderUpdateError, UploadMeshJob, VMValue, VertexArenaSelector,
            gpu_allocator::UploadIndexJob, renderer::Renderer,
        },
    },
    asset_manager_new::AssetHandle,
    util::types::{LocalTransform, Mat4F32, PNUJWVertex, PNUVertex},
    world::{
        RenderKey,
        instance_manager::{InstanceGPUBindings, InstanceHandle},
        world::InstanceUploadData,
    },
};

impl<'frame> VMValue<'frame> {
    fn unwrap_gpu_upload_job(&'frame self) -> &'frame GPUAssetUploadJob<'frame> {
        match self {
            VMValue::UploadJob(gpu_job) => gpu_job,
            _ => panic!("value is not a gpu upload job, it is {:?}", self),
        }
    }
    fn unwrap_transform_set(&'frame self) -> &'frame Vec<Mat4F32> {
        match self {
            VMValue::TransformSet(transforms) => transforms,
            _ => panic!("value is not transforms, it is {:?}", self),
        }
    }
    fn unwrap_lt_set(&'frame self) -> &'frame Vec<LocalTransform> {
        match self {
            VMValue::LocalTransformSet(transforms) => transforms,
            _ => panic!("value is not transforms, it is {:?}", self),
        }
    }
    fn unwrap_instance_handle_ref(&'frame self) -> &'frame InstanceHandle {
        match self {
            VMValue::InstanceHandle(handle) => handle,
            _ => panic!("value is not instance handle, it is {:?}", self),
        }
    }
    fn unwrap_instance_handle(self) -> InstanceHandle {
        match self {
            VMValue::InstanceHandle(handle) => handle,
            _ => panic!("value is not instance handle, it is {:?}", self),
        }
    }
    fn clone(&self) -> Self {
        match self {
            Self::Transform(transform) => Self::Transform(*transform),
            Self::InstanceHandle(handle) => Self::InstanceHandle(handle.clone()),
            Self::UploadJob(job) => Self::UploadJob(job.clone()),
            _ => panic!("cannot clone {:?}", self),
        }
    }
}

type InstructionSet<'a> = Peekable<Iter<'a, Instruction>>;

impl<'frame> Renderer {
    fn get_constant_idx(instructions: &mut InstructionSet) -> u8 {
        let instr = instructions.next().expect("should define a constant idx");
        match instr {
            Instruction::ConstIdx(idx) => *idx,
            _ => panic!("expected a constant idx"),
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
                            .unwrap_data();
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
                            .unwrap_data();
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
                            .unwrap_data();
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
                        let instance_handle_key = stack.pop().expect("should be key").unwrap_key();
                        let lt_offset = stack.pop().expect("should be offset").unwrap_offset();

                        res.push(RenderUpdateDelta::EntitySpawned((
                            InstanceHandle::from_key(instance_handle_key),
                            InstanceGPUBindings {
                                lt_offset: lt_offset as u32,
                            },
                        )));
                    }
                    Operations::LocalTransformUpload => {
                        let instance_handle_key = stack.pop().expect("should be key");
                        let instance_handle =
                            InstanceHandle::from_key(instance_handle_key.unwrap_key());
                        let lt = constants[Self::get_constant_idx(&mut instr_peek) as usize]
                            .unwrap_data();
                        let lt_upload_job = InstanceUploadJob::new(lt, instance_handle.clone());
                        let lt_offset = self.upload_local_transforms(lt_upload_job, queue)?;
                        stack.push(instance_handle_key);
                        res.push(RenderUpdateDelta::EntitySpawned((
                            instance_handle.clone(),
                            InstanceGPUBindings { lt_offset },
                        )));
                    }
                    Operations::SpawnEntityInstance => {
                        let const_idx = Self::get_constant_idx(&mut instr_peek);
                        stack.push(constants[const_idx as usize].clone());
                    }
                },
                Instruction::Byte(_byte) => {}
                Instruction::ConstIdx(_idx) => {}
            }
        }

        Ok(res)
    }
}
