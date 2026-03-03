use std::{fmt::Display, iter::Peekable, slice::Iter};

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
    fn to_raw_vertices(&self) -> super::vertex_arena::UploadMeshJob<PNUJWVertex> {
        UploadMeshJob {
            verts: &self.gltf_mesh_data.pnujw_vertices,
        }
    }
}
impl MeshUploadable<PNUVertex> for LoadedAsset {
    fn to_raw_vertices(&self) -> super::vertex_arena::UploadMeshJob<PNUVertex> {
        UploadMeshJob {
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
                        let skinned_job: UploadMeshJob<PNUJWVertex> =
                            loaded_asset.to_raw_vertices();
                        let static_job: UploadMeshJob<PNUJWVertex> = loaded_asset.to_raw_vertices();
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
