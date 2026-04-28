use core::panic;
use std::{iter::Peekable, slice::Iter};

use crate::{
    app::{
        GPUAssetUploadJob,
        renderer::{
            GPUAllocationHandle, InstanceUploadJob, Instruction, Operations, RenderUpdateDelta,
            RenderUpdateError, UploadMeshJob, VMValue, VertexArenaSelector,
            gpu_allocator::UploadIndexJob, renderer::Renderer,
        },
    },
    util::types::{PNUJWVertex, PNUVertex},
    world::{instance_manager::InstanceGPUBindings, world::InstanceUploadData},
};

impl<'frame> VMValue<'frame> {
    fn unwrap_gpu_upload_job(&'frame self) -> &'frame GPUAssetUploadJob<'frame> {
        match self {
            VMValue::UploadJob(gpu_job) => gpu_job,
            _ => panic!("value is not a gpu upload job, it is {:?}", self),
        }
    }
    fn unwrap_instance_upload_jobs(&'frame self) -> &'frame InstanceUploadData {
        match self {
            VMValue::InstanceDataUpload(data) => data,
            _ => panic!("value is not a gpu upload job, it is {:?}", self),
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
        constants: Vec<VMValue>,
        instructions: Vec<Instruction>,
        queue: &wgpu::Queue,
    ) -> Result<Vec<RenderUpdateDelta>, RenderUpdateError> {
        let mut res: Vec<RenderUpdateDelta> = Vec::new();
        let mut instr_peek = instructions.iter().peekable();

        while instr_peek.peek().is_some() {
            let instr = instr_peek.next().unwrap();
            match instr {
                Instruction::Op(op) => match op {
                    Operations::AddAsset => {
                        let const_idx = Self::get_constant_idx(&mut instr_peek);
                        let gpu_upload_job: &GPUAssetUploadJob =
                            constants[const_idx as usize].unwrap_gpu_upload_job();

                        let global_allocation_id = self.get_global_alloc_id();

                        let maybe_skinned_job: Option<UploadMeshJob<'_, PNUJWVertex>> =
                            gpu_upload_job.pnujw_vertices.map(|x| UploadMeshJob {
                                verts: x,
                                global_alloc_id: global_allocation_id,
                            });

                        let maybe_static_job: Option<UploadMeshJob<'_, PNUVertex>> =
                            gpu_upload_job.pnu_vertices.map(|x| UploadMeshJob {
                                verts: x,
                                global_alloc_id: global_allocation_id,
                            });

                        let maybe_index_job: Option<UploadIndexJob<'_>> =
                            gpu_upload_job.indices.map(|x| UploadIndexJob {
                                indices: x,
                                global_alloc_id: global_allocation_id,
                            });

                        if let Some(static_job) = maybe_static_job {
                            self.upload_mesh(static_job, queue)?;
                        }
                        if let Some(skinned_job) = maybe_skinned_job {
                            self.upload_mesh(skinned_job, queue)?;
                        }

                        if let Some(index_job) = maybe_index_job {
                            self.upload_indices(index_job, queue)?;
                        }

                        res.push(RenderUpdateDelta::AssetGPULoaded(
                            *gpu_upload_job.asset_handle,
                            GPUAllocationHandle {
                                global_allocation_id,
                            },
                        ));
                    }
                    Operations::AddEntity => {
                        // TODO
                    }
                    Operations::MoveEntity => todo!(),
                    Operations::SpawnEntityInstance => {
                        let const_idx = Self::get_constant_idx(&mut instr_peek);
                        let instance_upload_jobs =
                            constants[const_idx as usize].unwrap_instance_upload_jobs();

                        if let Some(ref local_transform_data) =
                            instance_upload_jobs.local_transforms
                        {
                            let lt_upload_job = InstanceUploadJob::new(
                                local_transform_data,
                                instance_upload_jobs.instance_handle.clone(),
                            );
                            let lt_offset = self.upload_local_transforms(lt_upload_job, queue)?;
                            res.push(RenderUpdateDelta::EntitySpawned((
                                instance_upload_jobs.instance_handle.clone(),
                                InstanceGPUBindings { lt_offset },
                            )));
                        }
                    }
                },
                Instruction::Byte(_byte) => {}
                Instruction::ConstIdx(_idx) => {}
            }
        }

        Ok(res)
    }
}
