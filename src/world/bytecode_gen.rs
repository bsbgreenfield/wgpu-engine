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
