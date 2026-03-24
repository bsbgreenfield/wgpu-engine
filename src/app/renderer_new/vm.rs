use core::panic;
use std::{iter::Peekable, slice::Iter};

use crate::{
    app::renderer_new::{
        GPUAllocationHandle, Instruction, Operations, RenderUpdateDeltaNew, VMValue,
        renderer_new::{RenderUpdateError, RendererNew},
        vertex_arena::LocalTransformUploadJob,
    },
    asset_manager::asset_manager::LoadedAsset,
    util::types::{ModelVertex, PNUJWVertex, PNUVertex},
    world::{
        components::MeshCollectionComponent,
        entity_manager::Renderables,
        instance_manager::InstanceHandle,
        world::{DrawSet, RenderView},
    },
};

impl<'frame> VMValue<'frame> {
    fn unwrap_loaded_asset(&self) -> &'frame LoadedAsset {
        match self {
            VMValue::LoadedAsset(la) => la,
            _ => panic!("value is not a loaded asset ref"),
        }
    }
    fn unwrap_mesh_collection(&self) -> &'frame MeshCollectionComponent {
        match self {
            VMValue::MeshCollectionComponent(mc) => mc,
            _ => panic!("value is not a MCC ref"),
        }
    }

    fn unwrap_renderables(&'frame self) -> &Renderables<'frame> {
        match self {
            VMValue::Renderables(renderables) => renderables,
            _ => panic!("value is not renderables"),
        }
    }

    fn unwrap_instance_handle(&'frame self) -> &'frame InstanceHandle {
        match self {
            VMValue::InstanceHandle(handle) => handle,
            _ => panic!("value is not an instance handle"),
        }
    }
}

pub struct UploadMeshJob<'frame, V: ModelVertex> {
    pub verts: &'frame [V],
    pub(super) global_alloc_id: u32,
}

pub trait MeshUploadable<V: ModelVertex> {
    fn as_mesh_job<'frame>(verts: &'frame [V], global_alloc_id: u32) -> UploadMeshJob<'frame, V>;
}
type InstructionSet<'a> = Peekable<Iter<'a, Instruction>>;

impl<V: ModelVertex> MeshUploadable<V> for LoadedAsset {
    fn as_mesh_job<'frame>(verts: &'frame [V], global_alloc_id: u32) -> UploadMeshJob<'frame, V> {
        // REMOVE
        UploadMeshJob {
            global_alloc_id,
            verts,
        }
    }
}

impl<'frame> RendererNew {
    unsafe fn get_asset_ref(instr_peek: &mut Peekable<Iter<'_, Instruction>>) {
        let a: &Instruction = instr_peek.next().unwrap().try_into().unwrap();
    }

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
    ) -> Result<Vec<RenderUpdateDeltaNew>, RenderUpdateError> {
        let mut res: Vec<RenderUpdateDeltaNew> = Vec::new();
        let mut instr_peek = instructions.iter().peekable();

        while instr_peek.peek().is_some() {
            let instr = instr_peek.next().unwrap();
            match instr {
                Instruction::Op(op) => match op {
                    Operations::AddAsset => {
                        let const_idx = Self::get_constant_idx(&mut instr_peek);
                        let loaded_asset = constants[const_idx as usize].unwrap_loaded_asset();

                        let global_allocation_id = self.get_global_alloc_id();

                        let skinned_job: UploadMeshJob<'_, PNUJWVertex> = LoadedAsset::as_mesh_job(
                            &loaded_asset.gltf_mesh_data.pnujw_vertices,
                            global_allocation_id,
                        );
                        let static_job: UploadMeshJob<'_, PNUVertex> = LoadedAsset::as_mesh_job(
                            &loaded_asset.gltf_mesh_data.pnu_vertices,
                            global_allocation_id,
                        );

                        let lt_job: LocalTransformUploadJob = LocalTransformUploadJob {
                            local_transforms: &loaded_asset.gltf_mesh_data.local_transforms,
                            global_alloc_id: global_allocation_id,
                        };

                        self.upload_local_transform_data(lt_job, queue)?;
                        self.upload_mesh_data(skinned_job, queue)?;
                        self.upload_mesh_data(static_job, queue)?;

                        res.push(RenderUpdateDeltaNew::AssetGPULoaded(GPUAllocationHandle {
                            asset_handle: loaded_asset.handle,
                            global_allocation_id,
                        }));
                    }
                    Operations::AddEntity => {
                        // TODO
                    }
                    Operations::MoveEntity => todo!(),
                    Operations::SpawnEntityInstance => {
                        let const_idx = Self::get_constant_idx(&mut instr_peek);
                        let instance_handle =
                            constants[const_idx as usize].unwrap_instance_handle();

                        let renderables_idx = Self::get_constant_idx(&mut instr_peek);
                        let renderables = constants[renderables_idx as usize].unwrap_renderables();

                        if let Some(mesh_collection) = renderables.mesh_collection {
                            let la_const_idx = Self::get_constant_idx(&mut instr_peek);
                            let la = constants[la_const_idx as usize].unwrap_loaded_asset();

                            let (pnujw_ids, pnujw_prims) =
                                la.mesh_ids_and_prim_ranges_of::<PNUJWVertex>();
                            let (pnu_ids, pnu_prims) =
                                la.mesh_ids_and_prim_ranges_of::<PNUVertex>();
                            let view = RenderView {
                                gpu_handle: mesh_collection.allocation_handle.to_owned().unwrap(),
                                pnu_draws: DrawSet {
                                    mesh_ids: pnu_ids,
                                    primtitive_ranges: pnu_prims,
                                },
                                pnujw_draws: DrawSet {
                                    mesh_ids: pnujw_ids,
                                    primtitive_ranges: pnujw_prims,
                                },
                            };
                            self.add_render_group(vec![view], instance_handle.clone());
                        }
                    }
                },
                Instruction::Byte(byte) => {}
                Instruction::ConstIdx(idx) => {}
            }
        }

        Ok(res)
    }
}
