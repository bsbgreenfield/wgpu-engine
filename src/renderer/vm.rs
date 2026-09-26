use core::panic;
use std::{iter::Peekable, slice::Iter};

use crate::{
    renderer::{
        AllocationMask, BufferType, GPUAllocationHandle, GPUBindings, GPUInstanceHandle,
        InstanceUploadJob, Instruction, Operations, PrototypeHandle, RenderConstant, RenderProgram,
        RenderUpdateDelta, RenderUpdateError, StackValue, TexDim, TexLayer, UploadMeshJob,
        VertexArenaSelector,
        bind_groups::SharedInstanceBindGroup,
        gpu_allocator::{
            UploadIndexJob, UploadMaterialJob, UploadTextureJob,
            gpu_arena::InstanceAllocationResult,
        },
        renderer::Renderer,
    },
    util::types::{GPUMaterialData, InstanceRecordData, PNUJWVertex, PNUVertex},
    world::{FrameArena, RenderKey},
};

type InstructionSet<'a> = Peekable<Iter<'a, Instruction>>;

impl<'frame> Renderer {
    fn get_constant_idx(instructions: &mut InstructionSet) -> usize {
        let res = match instructions.next().unwrap() {
            Instruction::WideIdx(high) => {
                if let Some(Instruction::ConstIdx(low)) = instructions.next() {
                    ((*high as usize) << 8) | (*low as usize)
                } else {
                    panic!("should be wide");
                }
            }
            Instruction::ConstIdx(idx) => *idx as usize,
            _ => panic!("expected a const idx"),
        };
        res
    }
    fn get_byte(instructions: &mut InstructionSet) -> u8 {
        let instr = instructions.next().expect("should define a byte");
        match instr {
            Instruction::Byte(number) => *number,
            _ => panic!("expected a byte"),
        }
    }
    fn get_tex_dim(instructions: &mut InstructionSet) -> TexDim {
        let instr = instructions.next().expect("should define a tex dim");
        match instr {
            Instruction::TexDim(dim) => *dim,
            _ => panic!("expected a byte"),
        }
    }

    fn resolve_next_token(
        arena: &'frame FrameArena,
        render_program: &RenderProgram,
        instructions: &mut InstructionSet,
    ) -> Option<&'frame [u8]> {
        let token =
            render_program.constants[Self::get_constant_idx(instructions) as usize].unwrap_token();
        arena.resolve(token)
    }
    pub(super) fn interpret(
        &mut self,
        render_program: &RenderProgram,
        frame_arena: &FrameArena,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<Vec<RenderUpdateDelta>, RenderUpdateError> {
        //for i in instructions.iter() {
        //    println!("{i:?}");
        //}
        //println!("------------------------");
        let mut stack = Vec::<StackValue>::new();
        let mut res: Vec<RenderUpdateDelta> = Vec::new();
        let mut instr_peek = render_program.instructions.iter().peekable();

        while instr_peek.peek().is_some() {
            let instr = instr_peek.next().unwrap();
            //println!("instr: {:?}, stack: {:?}", instr, stack);
            match instr {
                Instruction::WideIdx(_) => {}
                Instruction::Buffer(_bt) => {
                    //
                }
                Instruction::Op(op) => match op {
                    Operations::Pop => {
                        stack.pop();
                    }
                    Operations::PushPrototype => {
                        let val_idx = Self::get_constant_idx(&mut instr_peek);
                        let val = render_program.constants[val_idx as usize].clone();
                        stack.push(StackValue::Prototype(PrototypeHandle::from_key(
                            val.unwrap_key(),
                        )));
                    }
                    Operations::PushAlloc => {
                        let val_idx = Self::get_constant_idx(&mut instr_peek);
                        let val = render_program.constants[val_idx as usize].clone();
                        stack.push(StackValue::Alloc(GPUAllocationHandle::from_key(
                            val.unwrap_key(),
                        )));
                    }
                    Operations::PushKey | Operations::PushInstance => {
                        todo!()
                    }
                    Operations::Swap => {
                        let first = stack.pop().unwrap();
                        let second = stack.pop().unwrap();
                        stack.push(first);
                        stack.push(second);
                    }
                    Operations::TextureUpload => {
                        let mut alloc_handle = stack.pop().unwrap().as_alloc();
                        alloc_handle.alloc_mask.insert(AllocationMask::TEX);
                        let data =
                            Self::resolve_next_token(frame_arena, render_program, &mut instr_peek)
                                .expect("should be texture data");
                        let dim = Self::get_tex_dim(&mut instr_peek);

                        let job = UploadTextureJob {
                            pixels: data,
                            dim,
                            texture_handle: alloc_handle.clone(),
                        };
                        self.upload_texture(job, device, queue)?;
                        stack.push(StackValue::Alloc(alloc_handle));
                    }
                    Operations::TexureDefault => {
                        let gac = stack.pop().unwrap();
                        stack.push(StackValue::TextureSlot(TexLayer {
                            bucket: 0,
                            layer: 0,
                        }));
                        stack.push(gac);
                    }
                    Operations::TextureAcquire => {
                        let texture_alloc_handle = stack.pop().unwrap().as_alloc(); // get alloc
                        let alloc_index = Self::get_byte(&mut instr_peek) as usize; // get idx

                        let (bucket, layer) = self
                            .bind_groups
                            .material_bind_group
                            .resolve_texture_slot(&texture_alloc_handle, alloc_index)
                            .unwrap();
                        stack.push(StackValue::TextureSlot(TexLayer {
                            bucket: bucket as u16,
                            layer: layer as u16,
                        }));
                        stack.push(StackValue::Alloc(texture_alloc_handle));
                    }
                    Operations::MaterialUpload => {
                        let mut gac = stack.pop().expect("should be gac").as_alloc();
                        gac.alloc_mask.insert(AllocationMask::MATERIAL);
                        let material_data =
                            Self::resolve_next_token(frame_arena, render_program, &mut instr_peek)
                                .expect("should be materials");
                        let mut records = material_data.to_vec();
                        for material_chunk in
                            records.chunks_exact_mut(std::mem::size_of::<GPUMaterialData>())
                        {
                            let tex_mod = stack
                                .pop()
                                .expect("should be texture slot")
                                .as_texture_slot();
                            material_chunk[24..28].copy_from_slice(bytemuck::bytes_of(&tex_mod));
                        }
                        self.upload_materials(
                            UploadMaterialJob {
                                data: &records,
                                alloc_handle: gac.clone(),
                            },
                            queue,
                            device,
                        )?;
                        stack.push(StackValue::Alloc(gac));
                    }
                    Operations::PNUUpload => {
                        let mut alloc_handle = stack.pop().expect("should be gac").as_alloc();
                        alloc_handle.alloc_mask.insert(AllocationMask::PNU_VERTEX);
                        let pnu =
                            Self::resolve_next_token(frame_arena, render_program, &mut instr_peek)
                                .expect("should be pnu data");
                        self.upload_mesh(
                            UploadMeshJob::<PNUVertex>::new(pnu, alloc_handle.clone()),
                            queue,
                            device,
                        )?;
                        stack.push(StackValue::Alloc(alloc_handle));
                    }

                    Operations::PNUJWUpload => {
                        let mut alloc_handle = stack.pop().expect("should be gac").as_alloc();
                        alloc_handle.alloc_mask.insert(AllocationMask::PNUJW_VERTEX);
                        let pnujw =
                            Self::resolve_next_token(frame_arena, render_program, &mut instr_peek)
                                .expect("should be pnujw");
                        self.upload_mesh(
                            UploadMeshJob::<PNUJWVertex>::new(pnujw, alloc_handle.clone()),
                            queue,
                            device,
                        )?;
                        stack.push(StackValue::Alloc(alloc_handle));
                    }
                    Operations::Index16Upload => {
                        let mut alloc_handle = stack.pop().expect("should be gac").as_alloc();
                        alloc_handle.alloc_mask.insert(AllocationMask::INDEX16);

                        let indices =
                            Self::resolve_next_token(frame_arena, render_program, &mut instr_peek)
                                .expect("should be indices");
                        self.upload_indices_16(
                            UploadIndexJob {
                                indices,
                                alloc_handle: alloc_handle.clone(),
                            },
                            queue,
                            device,
                        )?;
                        stack.push(StackValue::Alloc(alloc_handle));
                    }
                    Operations::Index32Upload => {
                        let mut alloc_handle = stack.pop().expect("should be gac").as_alloc();
                        alloc_handle.alloc_mask.insert(AllocationMask::INDEX32);

                        let indices =
                            Self::resolve_next_token(frame_arena, render_program, &mut instr_peek)
                                .expect("should be indices");
                        self.upload_indices_32(
                            UploadIndexJob {
                                indices,
                                alloc_handle: alloc_handle.clone(),
                            },
                            queue,
                            device,
                        )?;
                        stack.push(StackValue::Alloc(alloc_handle));
                    }
                    Operations::EmitAssetUpload => {
                        let alloc_handle = stack.pop().expect("should be gac").as_alloc();
                        let asset_key = stack.pop().expect("should be asset handle").as_raw_key();
                        res.push(RenderUpdateDelta::AssetGPULoaded {
                            key: asset_key,
                            alloc_handle: alloc_handle,
                        });
                    }
                    Operations::AddAsset => {
                        let asset_key = Self::get_constant_idx(&mut instr_peek);
                        stack.push(render_program.constants[asset_key as usize].clone().into());

                        let global_allocation_id = self.get_global_alloc_id();

                        stack.push(StackValue::Alloc(GPUAllocationHandle {
                            global_allocation_id,
                            alloc_mask: AllocationMask::empty(),
                        }));
                    }
                    Operations::EmitPrototypeSpawn => {
                        let prototype = stack.pop().expect("should be prottoype").as_prototype();
                        let entity_key = stack.pop().expect("should be entity key").as_raw_key();

                        res.push(RenderUpdateDelta::PrototypeCreated {
                            entity_key,
                            prototype_handle: prototype,
                        });
                    }

                    Operations::EmitInstanceSpawn => {
                        let bind_mask = GPUBindings::from_bits(Self::get_byte(&mut instr_peek))
                            .expect("should be a valid mask");
                        assert!(bind_mask.contains(GPUBindings::LOCAL_TRANSFORM));

                        let mut gpu_instance_handle =
                            stack.pop().expect("should be payload").as_instance_handle();
                        let joint_result: Option<(u32, u32)> = if bind_mask
                            .contains(GPUBindings::JOINT_TRANSFORM)
                        {
                            let jt_offset = stack.pop().expect("should be data offset").as_offset();
                            let chunk_index =
                                stack.pop().expect("should be chunk offset").as_offset();
                            Some((chunk_index, jt_offset))
                        } else {
                            None
                        };
                        let lt_offset = stack.pop().expect("should be offset").as_offset();
                        let lt_buffer_index =
                            stack.pop().expect("should be chunk offset").as_offset();

                        let record_data: Vec<u8> = bytemuck::pod_collect_to_vec(&[
                            lt_offset,
                            joint_result.map(|j| j.1).unwrap_or(0),
                            0,
                            0,
                        ]);
                        let reserved_node_id = stack.pop().expect("should be node id").as_offset();
                        let record_job: InstanceUploadJob<InstanceRecordData> =
                            InstanceUploadJob::new(&record_data, gpu_instance_handle);
                        self.upload_instance_record(record_job, reserved_node_id, queue, device)?;

                        let bind_id = self.bind_groups.set_bindings([
                            lt_buffer_index as u8,
                            joint_result.map(|j| j.0 as u8).unwrap_or(0),
                            0,
                            0,
                            0,
                            0,
                            0,
                            0,
                        ]);
                        gpu_instance_handle.bind_id = bind_id;

                        let instance_key = stack.pop().expect("should be key").as_raw_key();
                        res.push(RenderUpdateDelta::InstanceSpawn {
                            instance_key,
                            gpu_instance_handle,
                        });
                    }
                    //Operations::EmitEntitySpawn => {
                    //    let bind_mask = GPUBindings::from_bits(Self::get_byte(&mut instr_peek))
                    //        .expect("should be a valid mask");
                    //    assert!(bind_mask.contains(GPUBindings::LOCAL_TRANSFORM));

                    //    let gpu_instance_handle =
                    //        stack.pop().expect("should be payload").as_instance_handle();
                    //    let joint_result: Option<(u32, u32)> = if bind_mask
                    //        .contains(GPUBindings::JOINT_TRANSFORM)
                    //    {
                    //        let jt_offset = stack.pop().expect("should be data offset").as_offset();
                    //        let chunk_index =
                    //            stack.pop().expect("should be chunk offset").as_offset();
                    //        Some((chunk_index, jt_offset))
                    //    } else {
                    //        None
                    //    };
                    //    let lt_offset = stack.pop().expect("should be offset").as_offset();
                    //    let lt_buffer_index =
                    //        stack.pop().expect("should be chunk offset").as_offset();

                    //    let record_data: Vec<u8> = bytemuck::pod_collect_to_vec(&[
                    //        lt_offset,
                    //        joint_result.map(|j| j.1).unwrap_or(0),
                    //        0,
                    //        0,
                    //    ]);
                    //    let record_job: InstanceUploadJob<InstanceRecordData> =
                    //        InstanceUploadJob::new(&record_data, gpu_instance_handle);
                    //    let GPUUploadResult::RecordData { element_slot } = self
                    //        .upload_instance_record(
                    //            record_job,
                    //            gpu_instance_handle.instance_id,
                    //            queue,
                    //            device,
                    //        )?
                    //    else {
                    //        panic!("unexpected upload result type")
                    //    };

                    //    let instance_key = stack.pop().expect("should be key").as_raw_key();
                    //    res.push(RenderUpdateDelta::EntitySpawned {
                    //        instance_key,
                    //        gpu_instance_handle,
                    //        record_offset: element_slot,
                    //        binding_key: InstanceBindKey {
                    //            lt: lt_buffer_index as u16,
                    //            jt: joint_result.map(|jr| jr.0).unwrap_or(0) as u16,
                    //        },
                    //    });
                    //}
                    Operations::LocalTransformUpload => {
                        let gpu_instance_handle =
                            stack.pop().expect("should be payload").as_instance_handle();
                        let lt =
                            Self::resolve_next_token(frame_arena, render_program, &mut instr_peek)
                                .expect("should be lt data");
                        let lt_upload_job = InstanceUploadJob::new(lt, gpu_instance_handle.clone());
                        self.upload_local_transforms(lt_upload_job, queue, device)?;

                        stack.push(StackValue::Instance(gpu_instance_handle));
                    }
                    Operations::CreatePrototype => {
                        let entity_key_idx = Self::get_constant_idx(&mut instr_peek);
                        let entity_key =
                            render_program.constants[entity_key_idx as usize].unwrap_key();
                        let prototype_handle = PrototypeHandle::from_key(entity_key.clone());

                        stack.push(StackValue::Key(entity_key));
                        stack.push(StackValue::Prototype(prototype_handle));
                    }
                    Operations::ReleasePrototype => {
                        let idx = Self::get_constant_idx(&mut instr_peek);
                        let prototype =
                            PrototypeHandle::from_key(render_program.constants[idx].unwrap_key());
                        self.release_prototypes(&prototype)?;
                    }
                    Operations::JointTransformUpload => {
                        let gpu_instance_handle =
                            stack.pop().expect("should be payload").as_instance_handle();
                        let jt =
                            Self::resolve_next_token(frame_arena, render_program, &mut instr_peek)
                                .expect("should be joints");
                        let ibms =
                            Self::resolve_next_token(frame_arena, render_program, &mut instr_peek)
                                .expect("should be ibs");
                        let jt_upload_job = InstanceUploadJob::new(jt, gpu_instance_handle.clone());
                        let ibm_upload_job =
                            InstanceUploadJob::new(ibms, gpu_instance_handle.clone());
                        self.upload_skin_data(jt_upload_job, ibm_upload_job, queue, device)?;

                        stack.push(StackValue::Instance(gpu_instance_handle));
                    }
                    Operations::SpawnInstance => {
                        let instance_key_idx = Self::get_constant_idx(&mut instr_peek);
                        let instance_key = render_program.constants[instance_key_idx].unwrap_key();
                        let prototype_handle =
                            stack.pop().expect("should be prototype key").as_prototype();
                        let (reserved_node_id, gpu_instance_handle) = self
                            .get_gpu_instance_handle(queue, device, &prototype_handle)
                            .map_err(|e| RenderUpdateError::GpuUploadFailure(Box::new(e)))?;
                        stack.push(StackValue::Prototype(prototype_handle));
                        stack.push(StackValue::Key(instance_key));
                        stack.push(StackValue::Offset(reserved_node_id));
                        stack.push(StackValue::Instance(gpu_instance_handle));
                    }
                    Operations::ShareData => {
                        let new_handle = stack
                            .pop()
                            .expect("should be gpu handle")
                            .as_instance_handle();

                        if let Some(Instruction::Buffer(bt)) = instr_peek.next() {
                            match bt {
                                BufferType::LocalTransform => {
                                    let InstanceAllocationResult {
                                        data_offset: lt_offset,
                                        chunk_index,
                                    } = self
                                        .bind_groups
                                        .local_transforms
                                        .register_shared_binding(&new_handle)
                                        .expect("register shared lt fail");
                                    stack.push(StackValue::Offset(chunk_index));
                                    stack.push(StackValue::Offset(lt_offset));
                                }
                                BufferType::JointTransform => {
                                    let jt_result = self
                                        .bind_groups
                                        .skinning
                                        .register_shared_binding(&new_handle)
                                        .expect("register shared skin fail");
                                    stack.push(StackValue::Offset(jt_result.chunk_index));
                                    stack.push(StackValue::Offset(jt_result.data_offset));
                                }
                            }
                            stack.push(StackValue::Instance(new_handle));
                        } else {
                            panic!("expected buffer type instr for share")
                        }
                    }
                    Operations::CopyData => {
                        let new_handle = stack
                            .pop()
                            .expect("should be gpu handle")
                            .as_instance_handle();
                        if let Some(Instruction::Buffer(bt)) = instr_peek.next() {
                            match bt {
                                BufferType::LocalTransform => {
                                    let InstanceAllocationResult {
                                        data_offset: lt_offset,
                                        chunk_index,
                                    } = self
                                        .bind_groups
                                        .local_transforms
                                        .register_copy_binding(&new_handle, queue, device)
                                        .expect("register shared lt fail");
                                    stack.push(StackValue::Offset(chunk_index));
                                    stack.push(StackValue::Offset(lt_offset));
                                    stack.push(StackValue::Instance(new_handle));
                                }
                                BufferType::JointTransform => {
                                    let jt_result = self
                                        .bind_groups
                                        .skinning
                                        .register_copy_binding(&new_handle, queue, device)?;
                                    stack.push(StackValue::Offset(jt_result.chunk_index));
                                    stack.push(StackValue::Offset(jt_result.data_offset));
                                    stack.push(StackValue::Instance(new_handle));
                                }
                            }
                        } else {
                            panic!("expected buffer type instr for share")
                        }
                    }
                    Operations::DespawnInstance => {
                        let gpu_instance_handle_idx = Self::get_constant_idx(&mut instr_peek);
                        let gpu_instance_handle_key =
                            &render_program.constants[gpu_instance_handle_idx];
                        let gpu_instance_handle =
                            GPUInstanceHandle::from_key(gpu_instance_handle_key.unwrap_key());
                        self.despawn_instance(&gpu_instance_handle);
                        res.push(RenderUpdateDelta::InstanceDespawn(gpu_instance_handle));
                    }
                    Operations::DespawnAsset => {
                        let asset_key_idx = Self::get_constant_idx(&mut instr_peek);
                        let asset_key = render_program.constants[asset_key_idx].unwrap_key();
                        let gpu_alloc_handle_idx = Self::get_constant_idx(&mut instr_peek);
                        let gpu_alloc_handle_key = &render_program.constants[gpu_alloc_handle_idx];
                        let gpu_alloc_handle =
                            GPUAllocationHandle::from_key(gpu_alloc_handle_key.unwrap_key());
                        self.unload_asset(gpu_alloc_handle.clone())?;
                        res.push(RenderUpdateDelta::AssetUnloaded { key: asset_key })
                    }
                },
                Instruction::Byte(_byte) => {}
                Instruction::TexDim(_dim) => {}
                Instruction::ConstIdx(_idx) => {}
            }
        }

        Ok(res)
    }
}
