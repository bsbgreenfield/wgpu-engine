use std::{iter::Peekable, ops::Range, slice::Iter};

use crate::{
    app::renderer_new::{
        AllocationHandle, GPUAllocationHandle, Instruction, Operations, RenderUpdateDeltaNew,
        VMValue,
        renderer_new::{RenderCategory, RenderUpdateError, RendererNew},
        vertex_arena::LocalTransformUploadJob,
    },
    asset_manager::{asset_manager::LoadedAsset, gltf_assets::model_builder_new::GltfMeshData},
    util::types::{Mat4F32, ModelVertex, PNUJWVertex, PNUVertex},
};

impl<'frame> VMValue<'frame> {
    fn unwrap_loaded_asset(&self) -> &'frame LoadedAsset {
        match self {
            VMValue::LoadedAsset(la) => la,
            _ => panic!("value is not a loaded asset ref"),
        }
    }
}

pub struct UploadMeshJob<'frame, V: ModelVertex> {
    pub verts: &'frame [V],
    pub(super) primitive_ranges: Vec<Range<u32>>,
    pub(super) global_alloc_id: u32,
    pub(super) mesh_ids: Vec<u32>,
}

pub trait MeshUploadable<V: ModelVertex> {
    fn as_mesh_job<'frame>(
        verts: &'frame [V],
        mesh_data: &'frame [GltfMeshData],
        global_alloc_id: u32,
    ) -> UploadMeshJob<'frame, V>;
}
type InstructionSet<'a> = Peekable<Iter<'a, Instruction>>;

impl<V: ModelVertex> MeshUploadable<V> for LoadedAsset {
    fn as_mesh_job<'frame>(
        verts: &'frame [V],
        mesh_data: &'frame [GltfMeshData],
        global_alloc_id: u32,
    ) -> UploadMeshJob<'frame, V> {
        // REMOVE
        let (mesh_ids, primitive_ranges) = Self::mesh_ids_and_prim_ranges_of::<V>(mesh_data);
        UploadMeshJob {
            mesh_ids,
            global_alloc_id,
            primitive_ranges,
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
                            &loaded_asset.gltf_mesh_data.mesh_data,
                            global_allocation_id,
                        );
                        let static_job: UploadMeshJob<'_, PNUVertex> = LoadedAsset::as_mesh_job(
                            &loaded_asset.gltf_mesh_data.pnu_vertices,
                            &loaded_asset.gltf_mesh_data.mesh_data,
                            global_allocation_id,
                        );

                        let lt_job: LocalTransformUploadJob = LocalTransformUploadJob {
                            local_transforms: &loaded_asset.gltf_mesh_data.local_transforms,
                            global_alloc_id: global_allocation_id,
                        };
                        self.upload_local_transform_data(lt_job, queue)?;

                        // TODO: map global alloc id to pipeline alloc handle
                        let _ = self.upload_mesh_data(skinned_job, queue)?;
                        let _ = self.upload_mesh_data(static_job, queue)?;

                        res.push(RenderUpdateDeltaNew::AssetGPULoaded(GPUAllocationHandle {
                            asset_handle: loaded_asset.handle,
                            global_allocation_id,
                        }));
                    }
                    Operations::AddEntity => {
                        let global_alloc_id = 0;
                    }
                    Operations::MoveEntity => todo!(),
                },
                Instruction::Byte(byte) => {}
                Instruction::ConstIdx(idx) => {}
            }
        }

        Ok(res)
    }
}

struct RenderGroup {}

struct RenderView {
    global_alloc_id: u32,
    category: RenderCategory,
    draw_ranges: Vec<Range<usize>>,
}
