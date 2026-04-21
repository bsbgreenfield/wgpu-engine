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
    asset_manager::LoadedAsset,
    util::types::{ModelVertex, PNUJWVertex, PNUVertex, VIndex},
    world::{
        entity_manager::Renderables,
        instance_manager::InstanceHandle,
        world::{DrawSet, RenderView},
    },
};

impl<'frame> VMValue<'frame> {
    fn unwrap_gpu_upload_job(&'frame self) -> &'frame GPUUploadJob<'frame> {
        match self {
            VMValue::UploadJob(gpu_job) => gpu_job,
            _ => panic!("value is not a gpu upload job, it is {:?}", self),
        }
    }

    fn unwrap_renderables(&'frame self) -> &'frame Renderables<'frame> {
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

trait MeshUploadable<V: ModelVertex> {
    fn as_mesh_job<'frame>(
        verts: &'frame [V],
        global_alloc_id: u32,
    ) -> Option<UploadMeshJob<'frame, V>>;
}

trait IndexUploadable {
    fn as_index_job<'frame>(
        indices: Option<&'frame [VIndex]>,
        global_alloc_id: u32,
    ) -> Option<UploadIndexJob<'frame>>;
}
type InstructionSet<'a> = Peekable<Iter<'a, Instruction>>;

impl<V: ModelVertex> MeshUploadable<V> for LoadedAsset {
    fn as_mesh_job<'frame>(
        verts: &'frame [V],
        global_alloc_id: u32,
    ) -> Option<UploadMeshJob<'frame, V>> {
        if verts.len() > 0 {
            Some(UploadMeshJob {
                global_alloc_id,
                verts,
            })
        } else {
            None
        }
    }
}

impl IndexUploadable for LoadedAsset {
    fn as_index_job<'frame>(
        indices: Option<&'frame [VIndex]>,
        global_alloc_id: u32,
    ) -> Option<UploadIndexJob<'frame>> {
        if let Some(indices) = indices {
            Some(UploadIndexJob {
                global_alloc_id,
                indices,
            })
        } else {
            None
        }
    }
}

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

                        let lt_job: Option<LocalTransformUploadJob> = gpu_upload_job
                            .local_transforms
                            .map(|x| LocalTransformUploadJob {
                                local_transforms: x,
                                global_alloc_id: global_allocation_id,
                            });
                        self.upload_local_transform_data(lt_job, queue)?;
                        if let Some(static_job) = maybe_static_job {
                            self.upload_mesh(static_job, queue)?;
                        }
                        if let Some(skinned_job) = maybe_skinned_job {
                            self.upload_mesh(skinned_job, queue)?;
                        }

                        if let Some(index_job) = maybe_index_job {
                            self.upload_indices(index_job, queue)?;
                        }

                        res.push(RenderUpdateDelta::AssetGPULoaded(GPUAllocationHandle {
                            global_allocation_id,
                        }));
                    }
                    Operations::AddEntity => {
                        // TODO
                    }
                    Operations::MoveEntity => todo!(),
                    Operations::SpawnEntityInstance => {
                        //    let instance_idx = Self::get_constant_idx(&mut instr_peek);
                        //    let instance_handle =
                        //        constants[instance_idx as usize].unwrap_instance_handle();

                        //    let renderables_idx = Self::get_constant_idx(&mut instr_peek);
                        //    let renderables = constants[renderables_idx as usize].unwrap_renderables();

                        //    if let Some(mesh_collection_renderable) = &renderables.mesh_collection {
                        //        let la_const_idx = Self::get_constant_idx(&mut instr_peek);
                        //        let la = constants[la_const_idx as usize].unwrap_loaded_asset();

                        //        let pnu_data = la.mesh_ids_and_alloc_ranges_of::<PNUVertex>();
                        //        let pnujw_data = la.mesh_ids_and_alloc_ranges_of::<PNUJWVertex>();
                        let view = RenderView {
                            gpu_handle: mesh_collection_renderable.0.to_owned(),
                            pnu_draws: DrawSet::from_ids_and_prims(pnu_data),
                            pnujw_draws: DrawSet::from_ids_and_prims(pnujw_data),
                        };
                        self.add_render_group(
                            vec![view],
                            instance_handle.clone(),
                            la.gltf_mesh_data.indices.is_some(),
                        );
                    }
                },
                Instruction::Byte(_byte) => {}
                Instruction::ConstIdx(_idx) => {}
            }
        }

        Ok(res)
    }
}
