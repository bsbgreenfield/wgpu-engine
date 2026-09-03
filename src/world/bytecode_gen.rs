use crate::{
    app::GPUAssetUploadJob,
    asset_manager::AssetHandle,
    renderer::{
        BufferType, GPUAllocationHandle, GPUBindings, GPUInstanceHandle, Instruction, Operations,
        RenderConstant,
    },
    util::types::{PNUJWVertex, PNUVertex, VIndex},
    world::{
        RenderKey,
        world::{
            CopiedInstanceData, JointTransforms, LocalTransforms, NewInstanceData, World,
            WorldUpdateDelta,
        },
    },
};

impl<'frame> BytecodeGenerator<'frame> for World {}

pub trait BytecodeGenerator<'frame> {
    fn emit_const_last(constants: &Vec<RenderConstant<'_>>, instructions: &mut Vec<Instruction>) {
        let idx: usize = constants.len() - 1;
        if constants.len() > 255 {
            instructions.push(Instruction::WideIdx((idx >> 8) as u8));
        }
        instructions.push(Instruction::ConstIdx(idx as u8));
    }

    fn emit_const(
        constants: &Vec<RenderConstant<'frame>>,
        instructions: &mut Vec<Instruction>,
        idx: usize,
    ) {
        if constants.len() > 255 {
            instructions.push(Instruction::WideIdx((idx >> 8) as u8));
        }
        instructions.push(Instruction::ConstIdx(idx as u8));
    }
    fn gen_bytecode(
        deltas: &'frame Vec<WorldUpdateDelta>,
        instructions: &mut Vec<Instruction>,
        constants: &mut Vec<RenderConstant<'frame>>,
    ) {
        for delta in deltas.iter() {
            match delta {
                WorldUpdateDelta::AssetDidLoad(asset_upload_job) => {
                    Self::asset_upload(asset_upload_job, instructions, constants)
                }
                WorldUpdateDelta::NewEntitySpawn(new_instance) => {
                    Self::new_entity_spawn(new_instance, instructions, constants)
                }
                WorldUpdateDelta::EntityInstanceSpawn(copied_instance) => {
                    Self::entity_instance_spawn(copied_instance, instructions, constants)
                }
                WorldUpdateDelta::InstanceDespawn(gpu_instance_handle) => {
                    Self::despawn_instance(gpu_instance_handle, instructions, constants)
                }
                WorldUpdateDelta::AssetUnload(asset_handle, alloc_handle) => {
                    Self::unload_asset(alloc_handle, asset_handle, instructions, constants)
                }
            }
        }
    }

    fn unload_asset(
        alloc_handle: &GPUAllocationHandle,
        asset_handle: &AssetHandle,
        instructions: &mut Vec<Instruction>,
        constants: &mut Vec<RenderConstant<'frame>>,
    ) {
        instructions.push(Instruction::Op(Operations::DespawnAsset));
        constants.push(RenderConstant::Key(asset_handle.as_key()));
        Self::emit_const_last(constants, instructions);
        constants.push(RenderConstant::Key(alloc_handle.as_key()));
        Self::emit_const_last(constants, instructions);
    }

    fn despawn_instance(
        gpu_instance_handle: &GPUInstanceHandle,
        instructions: &mut Vec<Instruction>,
        constants: &mut Vec<RenderConstant<'frame>>,
    ) {
        instructions.push(Instruction::Op(Operations::DespawnInstance));
        constants.push(RenderConstant::Key(gpu_instance_handle.as_key()));
        Self::emit_const_last(constants, instructions);
    }

    fn entity_instance_spawn(
        copied_instance: &CopiedInstanceData,
        instructions: &mut Vec<Instruction>,
        constants: &mut Vec<RenderConstant<'frame>>,
    ) {
        let mut bind_mask = GPUBindings::empty();
        constants.push(RenderConstant::Key(
            copied_instance.prototype_handle.as_key(),
        ));

        let prototype_idx = constants.len() - 1;
        bind_mask.insert(GPUBindings::LOCAL_TRANSFORM);
        let lt_instr = match copied_instance.local_transforms {
            LocalTransforms::NeedsCopy => Instruction::Op(Operations::CopyData),
            LocalTransforms::NeedsShared => Instruction::Op(Operations::ShareData),
            _ => panic!(),
        };
        let joint_instr = match copied_instance.joint_transforms {
            JointTransforms::None => None,
            JointTransforms::NeedsCopy => {
                bind_mask.insert(GPUBindings::JOINT_TRANSFORM);
                Some(Instruction::Op(Operations::CopyData))
            }
            JointTransforms::NeedsShared => {
                bind_mask.insert(GPUBindings::JOINT_TRANSFORM);
                Some(Instruction::Op(Operations::ShareData))
            }
            _ => panic!(),
        };

        for handle in copied_instance.handles.iter().cloned() {
            instructions.push(Instruction::Op(Operations::Push));
            Self::emit_const(constants, instructions, prototype_idx);
            instructions.push(Instruction::Op(Operations::SpawnFromPrototype));
            constants.push(RenderConstant::Key(handle.as_key()));
            Self::emit_const_last(constants, instructions);
            instructions.push(lt_instr);
            instructions.push(Instruction::Buffer(BufferType::LocalTransform));
            if let Some(joint_instr) = joint_instr {
                instructions.push(joint_instr);
                instructions.push(Instruction::Buffer(BufferType::JointTransform));
            }
            instructions.push(Instruction::Op(Operations::EmitEntitySpawn));
            instructions.push(Instruction::Byte(bind_mask.bits()));
        }
    }

    fn new_entity_spawn(
        new_instance: &'frame NewInstanceData,
        instructions: &mut Vec<Instruction>,
        constants: &mut Vec<RenderConstant<'frame>>,
    ) {
        let mut bind_mask = GPUBindings::empty();
        // prototype gen
        instructions.push(Instruction::Op(Operations::CreatePrototype));
        constants.push(RenderConstant::Key(new_instance.prototype.as_key()));
        Self::emit_const_last(constants, instructions);
        constants.push(RenderConstant::Key(new_instance.handle.as_key()));
        Self::emit_const_last(constants, instructions);

        instructions.push(Instruction::Op(Operations::SpawnEntityInstance));

        // local transforms
        bind_mask.insert(GPUBindings::LOCAL_TRANSFORM);
        instructions.push(Instruction::Op(Operations::LocalTransformUpload));
        let data_bytes: &[u8] = bytemuck::cast_slice(&new_instance.local_transforms);
        constants.push(RenderConstant::DataRef(data_bytes));
        Self::emit_const_last(constants, instructions);

        // joints and ibms
        if let Some(jt_bytes) = &new_instance.joint_transforms {
            bind_mask.insert(GPUBindings::JOINT_TRANSFORM);
            instructions.push(Instruction::Op(Operations::JointTransformUpload));
            let jt_bytes: &[u8] = bytemuck::cast_slice(jt_bytes);
            let ibm_bytes: &[u8] = if let Some(data) = &new_instance.ibms {
                bytemuck::cast_slice(data)
            } else {
                panic!("joint transforms must be accompanied by ibms");
            };
            constants.push(RenderConstant::DataRef(jt_bytes));
            Self::emit_const_last(constants, instructions);
            constants.push(RenderConstant::DataRef(ibm_bytes));
            Self::emit_const_last(constants, instructions);
        }

        instructions.push(Instruction::Op(Operations::EmitEntitySpawn));
        instructions.push(Instruction::Byte(bind_mask.bits()));
    }

    fn asset_upload(
        asset_upload_job: &'frame GPUAssetUploadJob,
        instructions: &mut Vec<Instruction>,
        constants: &mut Vec<RenderConstant<'frame>>,
    ) {
        instructions.push(Instruction::Op(Operations::AddAsset));
        constants.push(RenderConstant::Key(asset_upload_job.asset_handle.as_key()));
        Self::emit_const_last(constants, instructions);
        if let Some(pnu) = &asset_upload_job.pnu_vertices {
            instructions.push(Instruction::Op(Operations::PNUUpload));
            let pnu_data = bytemuck::cast_slice::<PNUVertex, u8>(&pnu);
            constants.push(RenderConstant::DataRef(pnu_data));
            Self::emit_const_last(constants, instructions);
        }
        if let Some(pnujw) = &asset_upload_job.pnujw_vertices {
            instructions.push(Instruction::Op(Operations::PNUJWUpload));
            let pnujw_data = bytemuck::cast_slice::<PNUJWVertex, u8>(&pnujw);
            constants.push(RenderConstant::DataRef(pnujw_data));
            Self::emit_const_last(constants, instructions);
        }
        if let Some(indices) = &asset_upload_job.indices {
            instructions.push(Instruction::Op(Operations::IndexUpload));
            let index_data = bytemuck::cast_slice::<VIndex, u8>(&indices);
            constants.push(RenderConstant::DataRef(index_data));
            Self::emit_const_last(constants, instructions);
        }
        if let Some(textures) = &asset_upload_job.textures {
            instructions.push(Instruction::Op(Operations::TextureUpload));
        }
        instructions.push(Instruction::Op(Operations::EmitAssetUpload));
    }
}
