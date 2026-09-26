use core::panic;
use std::sync::Arc;

use crate::{
    app::{GPUAssetUploadJob, GPUTextureBinding},
    asset_manager::AssetHandle,
    common::instance::InstanceHandle,
    renderer::{
        BufferType, GPUAllocationHandle, GPUBindings, GPUInstanceHandle, Instruction, Operations,
        PrototypeHandle, RenderProgram, TexDim,
    },
    util::types::AssetIndices,
    world::{
        FrameArena, RenderBytes, RenderKey,
        world::{
            CopiedInstanceData, InverseBindMatrices, JointTransforms, LocalTransforms,
            NewInstanceData, World, WorldUpdateDelta,
        },
    },
};

impl World {
    pub(crate) fn gen_bytecode(
        deltas: &[WorldUpdateDelta],
        render_program: &mut RenderProgram,
        frame_arena: &mut FrameArena,
    ) {
        for delta in deltas.iter() {
            match delta {
                WorldUpdateDelta::AssetDidLoad(asset_upload_job) => {
                    asset_upload(asset_upload_job, render_program, frame_arena)
                }
                WorldUpdateDelta::NewEntitySpawn(new_instance) => {
                    new_entity_spawn(new_instance, render_program, frame_arena)
                }
                WorldUpdateDelta::EntityInstanceSpawn(copied_instance) => {
                    entity_instance_spawn(copied_instance, render_program)
                }
                WorldUpdateDelta::InstanceDespawn(gpu_instance_handle) => {
                    despawn_instance(gpu_instance_handle, render_program)
                }
                WorldUpdateDelta::AssetUnload(asset_handle, alloc_handle) => {
                    unload_asset(alloc_handle, asset_handle, render_program)
                }
                WorldUpdateDelta::ReleasePrototype(prototype) => {
                    release_prototype(prototype, render_program)
                }
            }
        }
    }
}

fn push_data(data: Arc<dyn RenderBytes>, program: &mut RenderProgram, arena: &mut FrameArena) {
    let token = arena.add_data(data);
    program.push_data(token);
}

fn asset_upload(job: &GPUAssetUploadJob, program: &mut RenderProgram, arena: &mut FrameArena) {
    match job {
        GPUAssetUploadJob::ModelData {
            asset_handle,
            pnu_vertices,
            pnujw_vertices,
            indices,
            embedded_materials,
        } => {
            program.push_op(Operations::AddAsset);
            program.push_key(asset_handle.as_key());
            if let Some(pnu) = &pnu_vertices {
                program.push_op(Operations::PNUUpload);
                push_data(pnu.clone(), program, arena);
            }
            if let Some(pnujw) = &pnujw_vertices {
                program.push_op(Operations::PNUJWUpload);
                push_data(pnujw.clone(), program, arena);
            }
            if let Some(indices) = &indices {
                match indices {
                    AssetIndices::U16(_) => program.push_op(Operations::Index16Upload),
                    AssetIndices::U32(_) => program.push_op(Operations::Index32Upload),
                };
                push_data(Arc::new(indices.clone()), program, arena);
            }
            if !embedded_materials.records.is_empty() {
                let mut alloc_indices = Vec::<u8>::with_capacity(embedded_materials.records.len());
                let mut next_embedded: u8 = 0;
                for tex_binding in embedded_materials.textures.iter() {
                    match tex_binding {
                        GPUTextureBinding::Embedded(tex) => {
                            program.push_op(Operations::TextureUpload);
                            push_data(tex.clone(), program, arena);
                            program.push_instruction(Instruction::TexDim(TexDim::from_u32(
                                tex.height,
                            )));
                            alloc_indices.push(next_embedded);
                            next_embedded += 1;
                        }
                        GPUTextureBinding::Resolved(_) => alloc_indices.push(0),
                        GPUTextureBinding::None => alloc_indices.push(0),
                    }
                }
                for (texture, alloc_idx) in
                    embedded_materials.textures.iter().zip(alloc_indices).rev()
                {
                    match texture {
                        GPUTextureBinding::Resolved(handle) => {
                            program.push_op(Operations::PushAlloc);
                            program.push_key(handle.as_key());
                            program.push_op(Operations::TextureAcquire);
                            program.push_instruction(Instruction::Byte(alloc_idx));
                            program.push_op(Operations::Pop);
                            program.push_op(Operations::Swap);
                        }
                        GPUTextureBinding::Embedded(_) => {
                            program.push_op(Operations::TextureAcquire);
                            program.push_instruction(Instruction::Byte(alloc_idx));
                        }
                        GPUTextureBinding::None => {
                            program.push_op(Operations::TexureDefault);
                        }
                    }
                }
                program.push_op(Operations::MaterialUpload);
                push_data(embedded_materials.records.clone(), program, arena);
            }

            program.push_op(Operations::EmitAssetUpload);
        }
        GPUAssetUploadJob::MaterialData { .. } => todo!(),
        GPUAssetUploadJob::TextureData {
            asset_handle,
            data: gpu_texture_data,
        } => {
            program.push_op(Operations::AddAsset);
            program.push_key(asset_handle.as_key());
            program.push_op(Operations::TextureUpload);
            push_data(gpu_texture_data.clone(), program, arena);
            program.push_instruction(Instruction::TexDim(TexDim::from_u32(
                gpu_texture_data.height,
            )));
            program.push_op(Operations::EmitAssetUpload);
        }
    }
}
fn new_entity_spawn(job: &NewInstanceData, program: &mut RenderProgram, arena: &mut FrameArena) {
    let mut bind_mask = GPUBindings::empty();

    // create the prototype, associated with this entity
    program.push_op(Operations::CreatePrototype);
    program.push_key(job.handle.entity_handle.as_key());

    // spawn a new instance
    program.push_op(Operations::SpawnInstance);
    program.push_key(job.handle.as_key());

    // local transforms
    bind_mask.insert(GPUBindings::LOCAL_TRANSFORM);
    program.push_op(Operations::LocalTransformUpload);
    if let LocalTransforms::OwnedCopy { data } | LocalTransforms::OwnedShared { data } =
        &job.local_transforms
    {
        push_data(data.clone(), program, arena);
    } else {
        panic!("must be lt data")
    }
    match &job.local_transforms {
        LocalTransforms::OwnedShared { .. } => {
            program.push_op(Operations::ShareData);
            program.push_instruction(Instruction::Buffer(BufferType::LocalTransform));
        }
        LocalTransforms::OwnedCopy { .. } => {
            program.push_op(Operations::CopyData);
            program.push_instruction(Instruction::Buffer(BufferType::LocalTransform));
        }
        _ => panic!("no local transforms given: {:?}", job.local_transforms),
    }

    if let JointTransforms::OwnedShared { data } | JointTransforms::OwnedCopy { data } =
        &job.joint_transforms
    {
        bind_mask.insert(GPUBindings::JOINT_TRANSFORM);
        let ibm_bytes = if let InverseBindMatrices::Owned { data } = &job.ibms {
            data.clone()
        } else {
            panic!("joint transforms must be accompanied by ibms");
        };
        program.push_op(Operations::JointTransformUpload);
        push_data(data.clone(), program, arena);
        push_data(ibm_bytes, program, arena);
        match &job.joint_transforms {
            JointTransforms::OwnedShared { .. } => {
                program.push_op(Operations::ShareData);
            }
            JointTransforms::OwnedCopy { .. } => {
                program.push_op(Operations::CopyData);
            }
            _ => unreachable!(),
        }
        program.push_instruction(Instruction::Buffer(BufferType::JointTransform));
    }

    program.push_op(Operations::EmitInstanceSpawn);
    program.push_instruction(Instruction::Byte(bind_mask.bits()));

    if !job.additional.is_empty() {
        entity_instance_spawn_ex(
            program,
            &job.local_transforms,
            &job.joint_transforms,
            &job.additional,
            &mut bind_mask,
        );
    }

    program.push_op(Operations::EmitPrototypeSpawn);
}

fn entity_instance_spawn_ex(
    program: &mut RenderProgram,
    local_transforms: &LocalTransforms,
    joint_transforms: &JointTransforms,
    handles: &[InstanceHandle],
    bind_mask: &mut GPUBindings,
) {
    bind_mask.insert(GPUBindings::LOCAL_TRANSFORM);
    let lt_instr = match local_transforms {
        LocalTransforms::NeedsCopy | LocalTransforms::OwnedCopy { .. } => {
            Instruction::Op(Operations::CopyData)
        }
        LocalTransforms::NeedsShared | LocalTransforms::OwnedShared { .. } => {
            Instruction::Op(Operations::ShareData)
        }
        _ => panic!(),
    };
    let joint_instr = match joint_transforms {
        JointTransforms::None => None,
        JointTransforms::NeedsCopy | JointTransforms::OwnedCopy { .. } => {
            bind_mask.insert(GPUBindings::JOINT_TRANSFORM);
            Some(Instruction::Op(Operations::CopyData))
        }
        JointTransforms::NeedsShared | JointTransforms::OwnedShared { .. } => {
            bind_mask.insert(GPUBindings::JOINT_TRANSFORM);
            Some(Instruction::Op(Operations::ShareData))
        }
    };

    for handle in handles.iter().cloned() {
        program.push_op(Operations::SpawnInstance);
        program.push_key(handle.as_key());
        program.push_instruction(lt_instr);
        program.push_instruction(Instruction::Buffer(BufferType::LocalTransform));
        if let Some(joint_instr) = joint_instr {
            program.push_instruction(joint_instr);
            program.push_instruction(Instruction::Buffer(BufferType::JointTransform));
        }
        program.push_op(Operations::EmitInstanceSpawn);
        program.push_instruction(Instruction::Byte(bind_mask.bits()));
    }
}

fn entity_instance_spawn(job: &CopiedInstanceData, program: &mut RenderProgram) {
    let mut bind_mask = GPUBindings::empty();

    program.push_op(Operations::PushPrototype);
    program.push_key(job.prototype_handle.as_key());

    entity_instance_spawn_ex(
        program,
        &job.local_transforms,
        &job.joint_transforms,
        &job.handles,
        &mut bind_mask,
    );
}
fn despawn_instance(handle: &GPUInstanceHandle, program: &mut RenderProgram) {
    program.push_op(Operations::DespawnInstance);
    program.push_key(handle.as_key());
}
fn unload_asset(
    alloc_handle: &GPUAllocationHandle,
    asset_handle: &AssetHandle,
    program: &mut RenderProgram,
) {
    program.push_op(Operations::DespawnAsset);
    program.push_key(asset_handle.as_key());
    program.push_key(alloc_handle.as_key());
}
fn release_prototype(handle: &PrototypeHandle, program: &mut RenderProgram) {
    program.push_op(Operations::ReleasePrototype);
    program.push_key(handle.as_key());
}

//impl<'frame> BytecodeGenerator<'frame> for World {}
//
//pub trait BytecodeGenerator<'frame> {
//    fn emit_const_last(constants: &Vec<RenderConstant<'_>>, instructions: &mut Vec<Instruction>) {
//        let idx: usize = constants.len() - 1;
//        if constants.len() > 255 {
//            instructions.push(Instruction::WideIdx((idx >> 8) as u8));
//        }
//        instructions.push(Instruction::ConstIdx(idx as u8));
//    }
//
//    fn emit_const(
//        constants: &Vec<RenderConstant<'frame>>,
//        instructions: &mut Vec<Instruction>,
//        idx: usize,
//    ) {
//        if constants.len() > 255 {
//            instructions.push(Instruction::WideIdx((idx >> 8) as u8));
//        }
//        instructions.push(Instruction::ConstIdx(idx as u8));
//    }
//    fn gen_bytecode(
//        deltas: &'frame Vec<WorldUpdateDelta>,
//        instructions: &mut Vec<Instruction>,
//        constants: &mut Vec<RenderConstant<'frame>>,
//    ) {
//        for delta in deltas.iter() {
//            match delta {
//                WorldUpdateDelta::AssetDidLoad(asset_upload_job) => {
//                    Self::asset_upload(asset_upload_job, instructions, constants)
//                }
//                WorldUpdateDelta::NewEntitySpawn(new_instance) => {
//                    Self::new_entity_spawn(new_instance, instructions, constants)
//                }
//                WorldUpdateDelta::EntityInstanceSpawn(copied_instance) => {
//                    Self::entity_instance_spawn(copied_instance, instructions, constants)
//                }
//                WorldUpdateDelta::InstanceDespawn(gpu_instance_handle) => {
//                    Self::despawn_instance(gpu_instance_handle, instructions, constants)
//                }
//                WorldUpdateDelta::AssetUnload(asset_handle, alloc_handle) => {
//                    Self::unload_asset(alloc_handle, asset_handle, instructions, constants)
//                }
//                WorldUpdateDelta::ReleasePrototype(prototype) => {
//                    Self::release_prototype(prototype, instructions, constants)
//                }
//            }
//        }
//    }
//
//    fn release_prototype(
//        prototype: &PrototypeHandle,
//
//        instructions: &mut Vec<Instruction>,
//        constants: &mut Vec<RenderConstant<'frame>>,
//    ) {
//        instructions.push(Instruction::Op(Operations::ReleasePrototype));
//        constants.push(RenderConstant::Key(prototype.as_key()));
//        Self::emit_const_last(constants, instructions);
//    }
//    fn unload_asset(
//        alloc_handle: &GPUAllocationHandle,
//        asset_handle: &AssetHandle,
//        instructions: &mut Vec<Instruction>,
//        constants: &mut Vec<RenderConstant<'frame>>,
//    ) {
//        instructions.push(Instruction::Op(Operations::DespawnAsset));
//        constants.push(RenderConstant::Key(asset_handle.as_key()));
//        Self::emit_const_last(constants, instructions);
//        constants.push(RenderConstant::Key(alloc_handle.as_key()));
//        Self::emit_const_last(constants, instructions);
//    }
//
//    fn despawn_instance(
//        gpu_instance_handle: &GPUInstanceHandle,
//        instructions: &mut Vec<Instruction>,
//        constants: &mut Vec<RenderConstant<'frame>>,
//    ) {
//        instructions.push(Instruction::Op(Operations::DespawnInstance));
//        constants.push(RenderConstant::Key(gpu_instance_handle.as_key()));
//        Self::emit_const_last(constants, instructions);
//    }
//
//    fn entity_instance_spawn(
//        copied_instance: &CopiedInstanceData,
//        instructions: &mut Vec<Instruction>,
//        constants: &mut Vec<RenderConstant<'frame>>,
//    ) {
//        let mut bind_mask = GPUBindings::empty();
//
//        instructions.push(Instruction::Op(Operations::PushPrototype));
//        constants.push(RenderConstant::Key(
//            copied_instance.prototype_handle.as_key(),
//        ));
//        Self::emit_const_last(constants, instructions);
//
//        Self::entity_instance_spawn_ex(
//            instructions,
//            constants,
//            &copied_instance.local_transforms,
//            &copied_instance.joint_transforms,
//            &copied_instance.handles,
//            &mut bind_mask,
//        );
//    }
//
//    fn entity_instance_spawn_ex(
//        instructions: &mut Vec<Instruction>,
//        constants: &mut Vec<RenderConstant<'frame>>,
//        local_transforms: &LocalTransforms,
//        joint_transforms: &JointTransforms,
//        handles: &[InstanceHandle],
//        bind_mask: &mut GPUBindings,
//    ) {
//        bind_mask.insert(GPUBindings::LOCAL_TRANSFORM);
//        let lt_instr = match local_transforms {
//            LocalTransforms::NeedsCopy | LocalTransforms::OwnedCopy { .. } => {
//                Instruction::Op(Operations::CopyData)
//            }
//            LocalTransforms::NeedsShared | LocalTransforms::OwnedShared { .. } => {
//                Instruction::Op(Operations::ShareData)
//            }
//            _ => panic!(),
//        };
//        let joint_instr = match joint_transforms {
//            JointTransforms::None => None,
//            JointTransforms::NeedsCopy | JointTransforms::OwnedCopy { .. } => {
//                bind_mask.insert(GPUBindings::JOINT_TRANSFORM);
//                Some(Instruction::Op(Operations::CopyData))
//            }
//            JointTransforms::NeedsShared | JointTransforms::OwnedShared { .. } => {
//                bind_mask.insert(GPUBindings::JOINT_TRANSFORM);
//                Some(Instruction::Op(Operations::ShareData))
//            }
//        };
//
//        for handle in handles.iter().cloned() {
//            instructions.push(Instruction::Op(Operations::SpawnInstance));
//            constants.push(RenderConstant::Key(handle.as_key()));
//            Self::emit_const_last(constants, instructions);
//            instructions.push(lt_instr);
//            instructions.push(Instruction::Buffer(BufferType::LocalTransform));
//            if let Some(joint_instr) = joint_instr {
//                instructions.push(joint_instr);
//                instructions.push(Instruction::Buffer(BufferType::JointTransform));
//            }
//            instructions.push(Instruction::Op(Operations::EmitInstanceSpawn));
//            instructions.push(Instruction::Byte(bind_mask.bits()));
//        }
//    }
//
//    fn new_entity_spawn(
//        new_instance: &'frame NewInstanceData,
//        instructions: &mut Vec<Instruction>,
//        constants: &mut Vec<RenderConstant<'frame>>,
//    ) {
//        let mut bind_mask = GPUBindings::empty();
//
//        // create the prototype, associated with this entity
//        instructions.push(Instruction::Op(Operations::CreatePrototype));
//        constants.push(RenderConstant::Key(
//            new_instance.handle.entity_handle.as_key(),
//        ));
//        Self::emit_const_last(constants, instructions);
//
//        // spawn a new instance
//        instructions.push(Instruction::Op(Operations::SpawnInstance));
//        constants.push(RenderConstant::Key(new_instance.handle.as_key()));
//        Self::emit_const_last(constants, instructions);
//
//        // local transforms
//        bind_mask.insert(GPUBindings::LOCAL_TRANSFORM);
//        instructions.push(Instruction::Op(Operations::LocalTransformUpload));
//        if let LocalTransforms::OwnedCopy { data } | LocalTransforms::OwnedShared { data } =
//            &new_instance.local_transforms
//        {
//            let data_bytes: &[u8] = bytemuck::cast_slice(data);
//            constants.push(RenderConstant::DataRef(data_bytes));
//            Self::emit_const_last(constants, instructions);
//        } else {
//            panic!("must be lt data")
//        }
//        match &new_instance.local_transforms {
//            LocalTransforms::OwnedShared { .. } => {
//                instructions.push(Instruction::Op(Operations::ShareData));
//                instructions.push(Instruction::Buffer(BufferType::LocalTransform));
//            }
//            LocalTransforms::OwnedCopy { .. } => {
//                instructions.push(Instruction::Op(Operations::CopyData));
//                instructions.push(Instruction::Buffer(BufferType::LocalTransform));
//            }
//            _ => panic!(
//                "no local transforms given: {:?}",
//                new_instance.local_transforms
//            ),
//        }
//
//        if let JointTransforms::OwnedShared { data } | JointTransforms::OwnedCopy { data } =
//            &new_instance.joint_transforms
//        {
//            bind_mask.insert(GPUBindings::JOINT_TRANSFORM);
//            let ibm_bytes = if let InverseBindMatrices::Owned { data } = &new_instance.ibms {
//                bytemuck::cast_slice(data)
//            } else {
//                panic!("joint transforms must be accompanied by ibms");
//            };
//            instructions.push(Instruction::Op(Operations::JointTransformUpload));
//            let jt_bytes: &[u8] = bytemuck::cast_slice(data);
//            constants.push(RenderConstant::DataRef(jt_bytes));
//            Self::emit_const_last(constants, instructions);
//            constants.push(RenderConstant::DataRef(ibm_bytes));
//            Self::emit_const_last(constants, instructions);
//            match &new_instance.joint_transforms {
//                JointTransforms::OwnedShared { .. } => {
//                    instructions.push(Instruction::Op(Operations::ShareData))
//                }
//                JointTransforms::OwnedCopy { .. } => {
//                    instructions.push(Instruction::Op(Operations::CopyData))
//                }
//                _ => unreachable!(),
//            }
//            instructions.push(Instruction::Buffer(BufferType::JointTransform));
//        }
//
//        instructions.push(Instruction::Op(Operations::EmitInstanceSpawn));
//        instructions.push(Instruction::Byte(bind_mask.bits()));
//
//        if !new_instance.additional.is_empty() {
//            Self::entity_instance_spawn_ex(
//                instructions,
//                constants,
//                &new_instance.local_transforms,
//                &new_instance.joint_transforms,
//                &new_instance.additional,
//                &mut bind_mask,
//            );
//        }
//
//        instructions.push(Instruction::Op(Operations::EmitPrototypeSpawn));
//    }
//
//    fn asset_upload(
//        asset_upload_job: &'frame GPUAssetUploadJob,
//        instructions: &mut Vec<Instruction>,
//        constants: &mut Vec<RenderConstant<'frame>>,
//    ) {
//        match asset_upload_job {
//            GPUAssetUploadJob::ModelData {
//                asset_handle,
//                pnu_vertices,
//                pnujw_vertices,
//                indices,
//                embedded_materials,
//            } => {
//                instructions.push(Instruction::Op(Operations::AddAsset));
//                constants.push(RenderConstant::Key(asset_handle.as_key()));
//                Self::emit_const_last(constants, instructions);
//                if let Some(pnu) = &pnu_vertices {
//                    instructions.push(Instruction::Op(Operations::PNUUpload));
//                    let pnu_data = bytemuck::cast_slice::<PNUVertex, u8>(&pnu);
//                    constants.push(RenderConstant::DataRef(pnu_data));
//                    Self::emit_const_last(constants, instructions);
//                }
//                if let Some(pnujw) = &pnujw_vertices {
//                    instructions.push(Instruction::Op(Operations::PNUJWUpload));
//                    let pnujw_data = bytemuck::cast_slice::<PNUJWVertex, u8>(&pnujw);
//                    constants.push(RenderConstant::DataRef(pnujw_data));
//                    Self::emit_const_last(constants, instructions);
//                }
//                if let Some(indices) = &indices {
//                    let op = match indices {
//                        AssetIndices::U16(_) => Operations::Index16Upload,
//                        AssetIndices::U32(_) => Operations::Index32Upload,
//                    };
//                    instructions.push(Instruction::Op(op));
//                    constants.push(RenderConstant::DataRef(indices.as_bytes()));
//                    Self::emit_const_last(constants, instructions);
//                }
//                if !embedded_materials.records.is_empty() {
//                    let mut alloc_indices =
//                        Vec::<u8>::with_capacity(embedded_materials.records.len());
//                    let mut next_embedded: u8 = 0;
//                    for tex_binding in embedded_materials.textures.iter() {
//                        match tex_binding {
//                            GPUTextureBinding::Embedded(tex) => {
//                                instructions.push(Instruction::Op(Operations::TextureUpload));
//                                constants.push(RenderConstant::DataRef(tex.pixels.as_ref()));
//                                Self::emit_const_last(constants, instructions);
//                                instructions
//                                    .push(Instruction::TexDim(TexDim::from_u32(tex.height)));
//                                alloc_indices.push(next_embedded);
//                                next_embedded += 1;
//                            }
//                            GPUTextureBinding::Resolved(_) => alloc_indices.push(0),
//                            GPUTextureBinding::None => alloc_indices.push(0),
//                        }
//                    }
//                    for (texture, alloc_idx) in
//                        embedded_materials.textures.iter().zip(alloc_indices).rev()
//                    {
//                        match texture {
//                            GPUTextureBinding::Resolved(handle) => {
//                                instructions.push(Instruction::Op(Operations::PushAlloc));
//                                constants.push(RenderConstant::Key(handle.as_key()));
//                                Self::emit_const_last(constants, instructions);
//                                instructions.push(Instruction::Op(Operations::TextureAcquire));
//                                instructions.push(Instruction::Byte(alloc_idx));
//                                instructions.push(Instruction::Op(Operations::Pop));
//                                instructions.push(Instruction::Op(Operations::Swap));
//                            }
//                            GPUTextureBinding::Embedded(_) => {
//                                instructions.push(Instruction::Op(Operations::TextureAcquire));
//                                instructions.push(Instruction::Byte(alloc_idx));
//                            }
//                            GPUTextureBinding::None => {
//                                instructions.push(Instruction::Op(Operations::TexureDefault))
//                            }
//                        }
//                    }
//                    instructions.push(Instruction::Op(Operations::MaterialUpload));
//                    let material_bytes =
//                        bytemuck::cast_slice::<GPUMaterialData, u8>(&embedded_materials.records);
//                    constants.push(RenderConstant::DataRef(material_bytes));
//                    Self::emit_const_last(constants, instructions);
//                }
//
//                instructions.push(Instruction::Op(Operations::EmitAssetUpload));
//            }
//            GPUAssetUploadJob::MaterialData { .. } => todo!(),
//            GPUAssetUploadJob::TextureData {
//                asset_handle,
//                data: gpu_texture_data,
//            } => {
//                instructions.push(Instruction::Op(Operations::AddAsset));
//                constants.push(RenderConstant::Key(asset_handle.as_key()));
//                Self::emit_const_last(constants, instructions);
//                instructions.push(Instruction::Op(Operations::TextureUpload));
//                constants.push(RenderConstant::DataRef(&gpu_texture_data.pixels));
//                Self::emit_const_last(constants, instructions);
//                instructions.push(Instruction::TexDim(TexDim::from_u32(
//                    gpu_texture_data.height,
//                )));
//                instructions.push(Instruction::Op(Operations::EmitAssetUpload));
//            }
//        }
//    }
//}
