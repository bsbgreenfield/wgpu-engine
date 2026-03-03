use std::{any::TypeId, fmt::Display, iter::Peekable, ops::Range, slice::Iter};

use crate::{
    app::{
        render::{Instruction, Operations, VMValue, renderer::RenderUpdateDelta},
        renderer_new::{
            renderer_new::{RenderUpdateError, RendererNew},
            vertex_arena::{MeshUploadable, UploadMeshJob},
        },
    },
    asset_manager::asset_manager::LoadedAsset,
    util::types::{PNUJWVertex, PNUVertex},
};

pub enum RenderUpdateDeltaNew {
    AssetGPULoaded,
}

impl<'frame> VMValue<'frame> {
    fn unwrap_loaded_asset(&self) -> &'frame LoadedAsset {
        match self {
            VMValue::LoadedAsset(la) => la,
            _ => panic!("value is not a loaded asset ref"),
        }
    }
}
type InstructionSet<'a> = Peekable<Iter<'a, Instruction>>;

impl MeshUploadable<PNUJWVertex> for LoadedAsset {
    fn as_mesh_job(&self, global_alloc_id: u32) -> super::vertex_arena::UploadMeshJob<PNUJWVertex> {
        let mut primitive_ranges = Vec::<Range<u32>>::new();
        let mut per_model_primitive_count = Vec::<u32>::new();
        for (idx, mesh_data) in self.gltf_mesh_data.mesh_data.iter().enumerate() {
            per_model_primitive_count.push(0);
            for mesh in mesh_data.meshes.iter() {
                let ranges: Vec<Range<u32>> = mesh
                    .primitives
                    .iter()
                    .filter(|primitive| primitive.vertex_type == TypeId::of::<PNUJWVertex>())
                    .map(|primitive| primitive.vertices.clone())
                    .collect();
                per_model_primitive_count[idx] = ranges.len() as u32;
                primitive_ranges.extend(ranges);
            }
        }
        UploadMeshJob {
            global_alloc_id,
            per_model_primitive_count,
            primitive_ranges,
            verts: &self.gltf_mesh_data.pnujw_vertices,
        }
    }
}
impl MeshUploadable<PNUVertex> for LoadedAsset {
    fn as_mesh_job(&self, global_alloc_id: u32) -> super::vertex_arena::UploadMeshJob<PNUVertex> {
        let mut primitive_ranges = Vec::<Range<u32>>::new();
        let mut per_model_primitive_count = Vec::<u32>::new();
        for (idx, mesh_data) in self.gltf_mesh_data.mesh_data.iter().enumerate() {
            per_model_primitive_count.push(0);
            for m in mesh_data.meshes.iter() {
                let ranges: Vec<Range<u32>> = m
                    .primitives
                    .iter()
                    .filter(|prim| prim.vertex_type == TypeId::of::<PNUVertex>())
                    .map(|prim| prim.vertices.clone())
                    .collect();
                per_model_primitive_count[idx] = ranges.len() as u32;
                primitive_ranges.extend(ranges);
            }
        }
        UploadMeshJob {
            global_alloc_id,
            per_model_primitive_count,
            primitive_ranges,
            verts: &self.gltf_mesh_data.pnu_vertices,
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
    ) -> Result<Vec<RenderUpdateDelta>, RenderUpdateError> {
        let mut res: Vec<RenderUpdateDelta> = Vec::new();
        let mut instr_peek = instructions.iter().peekable();

        while instr_peek.peek().is_some() {
            let instr = instr_peek.next().unwrap();
            match instr {
                Instruction::Op(op) => match op {
                    Operations::AddEntity => {
                        let const_idx = Self::get_constant_idx(&mut instr_peek);
                        let loaded_asset = constants[const_idx as usize].unwrap_loaded_asset();
                        // TODO: GET GLOBAL ALLOC ID WHICH WILL BE COMMON BETWEEN JOBS
                        // USE THIS TO INDEX INTO LOCAL TRANSFORM BUFFER PER MESH ID
                        let skinned_job: UploadMeshJob<PNUJWVertex> = loaded_asset.as_mesh_job();
                        let static_job: UploadMeshJob<PNUJWVertex> = loaded_asset.as_mesh_job();
                        let skinned_handle = self
                            .upload_mesh_data(skinned_job, queue)
                            .map_err(|e| RenderUpdateError::MeshUploadFailed(e.to_string()))?;
                        let static_handle = self
                            .upload_mesh_data(static_job, queue)
                            .map_err(|e| RenderUpdateError::MeshUploadFailed(e.to_string()));
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
