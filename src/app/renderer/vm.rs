use core::panic;
use std::{iter::Peekable, slice::Iter};

use crate::{
    app::{
        GPUUploadJob,
        renderer::{
            GPUAllocationHandle, Instruction, Operations, RenderUpdateDelta, RenderUpdateError,
            UploadMeshJob, VMValue, VertexArenaSelector,
            gpu_allocator::{LocalTransformUploadJob, UploadIndexJob},
            renderer::Renderer,
        },
    },
    util::types::{ModelVertex, PNUJWVertex, PNUVertex, VIndex},
    world::{
        entity_manager::{InstanceRenderData, Renderables},
        instance_manager::InstanceHandle,
        world::{DrawSet, RenderGroup, RenderView},
    },
};

impl<'frame> VMValue<'frame> {
    fn unwrap_gpu_upload_job(&'frame self) -> &'frame GPUUploadJob<'frame> {
        match self {
            VMValue::UploadJob(gpu_job) => gpu_job,
            _ => panic!("value is not a gpu upload job, it is {:?}", self),
        }
    }

    fn unwrap_renderables(&'frame self) -> &'frame Renderables {
        match self {
            VMValue::Renderables(renderables) => renderables,
            _ => panic!("value is not renderables. it is {:?}", self),
        }
    }

    fn unwrap_instance_handle(&'frame self) -> &'frame InstanceHandle {
        match self {
            VMValue::InstanceHandle(handle) => handle,
            _ => panic!("value is not an instance handle. it is {:?}", self),
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
                        let gpu_upload_job: &GPUUploadJob =
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

                        // let lt_job: Option<LocalTransformUploadJob> = gpu_upload_job
                        //     .local_transforms
                        //     .map(|x| LocalTransformUploadJob {
                        //         local_transforms: x,
                        //         global_alloc_id: global_allocation_id,
                        //     });
                        // self.upload_local_transform_data(lt_job, queue)?;
                        // if let Some(static_job) = maybe_static_job {
                        //     self.upload_mesh(static_job, queue)?;
                        // }
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

                        match &constants[const_idx as usize] {
                            VMValue::InstanceHandle(instance_handle) => {
                                let group_idx = self
                                    .entity_group_index
                                    .get(&instance_handle.entity_handle)
                                    .expect("group should exist");

                                let group =
                                    self.groups.get_mut(*group_idx).expect("group should exist");

                                group.instance_handles.push(instance_handle.clone());
                            }
                            VMValue::Renderables(renderables) => {
                                let mut views: Vec<RenderView> =
                                    Vec::with_capacity(renderables.0.len());
                                for instance_data in renderables.0.iter() {
                                    match instance_data {
                                        InstanceRenderData::MeshRenderable {
                                            gpu_alloc_handle,
                                            pnu_vertex_ranges,
                                            pnujw_vertex_ranges,
                                            index_ranges,
                                        } => {
                                            views.push(RenderView {
                                                gpu_handle: gpu_alloc_handle.clone(),
                                                pnu_draws: pnu_vertex_ranges.map(|pnu| todo!()),
                                                pnujw_draws: pnujw_vertex_ranges
                                                    .map(|pnujw| todo!()),
                                            });
                                        }
                                    }
                                }
                            }
                            _ => panic!("unexpected constant for spawn entity"),
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
