use std::{fmt::Debug, ops::Range};

use super::scene::Scene;
use crate::{
    app::{
        GPUAssetUploadJob,
        app::AppCommand,
        renderer::{
            BufferType, GPUAllocationHandle, GPUBindings, Instruction, Operations, PrototypeHandle,
            RenderConstant, RenderUpdateDelta, renderer::GPUInstanceHandle,
        },
    },
    asset_manager::{Asset, AssetLoadError},
    util::types::{LocalTransform, Mat4F32, PNUJWVertex, PNUVertex, VIndex},
    world::{
        RenderKey, WorldUpdateError,
        camera::Camera,
        entity_manager::{
            EntityHandle, components::ResourceBacking, entity_manager::EntityManager,
        },
        instance_manager::{Archetype, InstanceHandle, instance_manager::InstanceManager},
        load_queue::EntityLoadQueue,
        scene::SceneEvent,
    },
};

pub struct DrawSet {
    /// for use while iterating over primitives
    /// mesh_map[primitive_slot_index] = mesh_slot_index
    pub mesh_map: Vec<u32>,
    pub primtitive_ranges: Vec<Range<u32>>,
    pub index_ranges: Option<Vec<Range<u32>>>,
    pub joint_map: Vec<u32>,
}

impl DrawSet {
    #[inline]
    pub const fn within(prim_range: &Range<u32>, range: &Range<u32>) -> Range<u32> {
        let start = range.start + prim_range.start;
        start..(start + (prim_range.end - prim_range.start) as u32)
    }
}

pub struct RenderView {
    pub alloc_handle: GPUAllocationHandle,
    pub pnujw_draws: Option<DrawSet>,
    pub pnu_draws: Option<DrawSet>,
}

pub struct RenderGroup {
    pub entity_handle: EntityHandle,
    pub views: Vec<RenderView>,
}

#[derive(Debug)]
pub enum LocalTransforms {
    Uninit,
    Owned { data: Vec<LocalTransform> },
    CopiedFrom { donor: InstanceHandle },
    NeedsCopy,
    SharedWith { donor: InstanceHandle },
    NeedsShared,
}

#[derive(Debug)]
pub enum JointTransforms {
    None,
    Owned { data: Vec<Mat4F32> },
    NeedsCopy,
    NeedsShared,
}

#[derive(Debug)]
pub enum InverseBindMatrices {
    None,
    Owned { data: Vec<Mat4F32> },
    NeedsCopy,
    NeedsShared,
}

#[derive(Debug)]
pub struct NewInstanceData {
    pub handle: InstanceHandle,
    pub prototype: PrototypeHandle,
    pub local_transforms: Vec<LocalTransform>,
    pub joint_transforms: Option<Vec<Mat4F32>>,
    pub ibms: Option<Vec<Mat4F32>>,
}

impl NewInstanceData {
    pub fn new(handle: InstanceHandle, prototype: PrototypeHandle) -> Self {
        Self {
            handle,
            prototype,
            local_transforms: Vec::new(),
            joint_transforms: None,
            ibms: None,
        }
    }
}

#[derive(Debug)]
pub struct CopiedInstanceData {
    pub handles: Vec<InstanceHandle>,
    pub prototype_handle: PrototypeHandle,
    pub local_transforms: LocalTransforms,
    pub joint_transforms: JointTransforms,
}

#[derive(Debug)]
pub enum InstanceUploadData {
    New(NewInstanceData),
    Copied(CopiedInstanceData),
}

impl InstanceUploadData {
    pub(super) fn handles(&self) -> Vec<InstanceHandle> {
        let mut handles: Vec<InstanceHandle> = Vec::new();
        match self {
            InstanceUploadData::New(new) => {
                handles.push(new.handle.clone());
            }
            InstanceUploadData::Copied(copied) => {
                handles.extend(copied.handles.iter().cloned());
            }
        }
        handles
    }
}

pub enum WorldUpdateDelta {
    NewEntitySpawn(NewInstanceData),
    EntityInstanceSpawn(CopiedInstanceData),
    AssetDidLoad(GPUAssetUploadJob),
    InstanceDespawn(GPUInstanceHandle),
}

impl<'frame> Debug for WorldUpdateDelta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorldUpdateDelta::NewEntitySpawn(_) => f.write_str("NewEntitySpawn"),
            WorldUpdateDelta::EntityInstanceSpawn(_) => f.write_str("EntityInstanceSpawn"),
            WorldUpdateDelta::AssetDidLoad(_) => f.write_str("AssetDidLoad"),
            WorldUpdateDelta::InstanceDespawn(handle) => write!(f, "despawn {:?}", handle),
        }
    }
}

pub struct World {
    init: bool,
    pub camera: Camera,
    pub scene: Scene,
    pub entity_manager: EntityManager,
    load_queue: EntityLoadQueue,
    pub instance_manager: InstanceManager,
    pub deltas: Vec<WorldUpdateDelta>,
}

impl World {
    pub fn is_initialized(&self) -> bool {
        self.init
    }
    pub fn init(&mut self, aspect_ratio: f32, device: &wgpu::Device) {
        self.camera.build_camera_uniform(aspect_ratio, device);
        self.init = true;
    }
    fn emit_const_last(constants: &Vec<RenderConstant<'_>>, instructions: &mut Vec<Instruction>) {
        let idx: usize = constants.len() - 1;
        if constants.len() > 255 {
            instructions.push(Instruction::WideIdx((idx >> 8) as u8));
        }
        instructions.push(Instruction::ConstIdx(idx as u8));
    }

    fn emit_const<'frame>(
        constants: &Vec<RenderConstant<'frame>>,
        instructions: &mut Vec<Instruction>,
        idx: usize,
    ) {
        if constants.len() > 255 {
            instructions.push(Instruction::WideIdx((idx >> 8) as u8));
        }
        instructions.push(Instruction::ConstIdx(idx as u8));
    }
    pub fn gen_bytecode<'frame>(
        deltas: &'frame mut Vec<WorldUpdateDelta>,
        instructions: &mut Vec<Instruction>,
        constants: &mut Vec<RenderConstant<'frame>>,
    ) {
        for delta in deltas.iter_mut() {
            match delta {
                WorldUpdateDelta::AssetDidLoad(asset_upload_job) => {
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
                    instructions.push(Instruction::Op(Operations::EmitAssetUpload));
                }
                WorldUpdateDelta::NewEntitySpawn(new_instance) => {
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
                WorldUpdateDelta::EntityInstanceSpawn(copied_instance) => {
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
                WorldUpdateDelta::InstanceDespawn(gpu_instance_handle) => {
                    instructions.push(Instruction::Op(Operations::DespawnInstance));
                    constants.push(RenderConstant::Key(gpu_instance_handle.as_key()));
                    Self::emit_const_last(constants, instructions);
                }
            }
        }
    }
    pub fn add_scene(&mut self, scene: Scene) {
        self.scene = scene;
    }

    pub fn register_asset<A>(&mut self, str_dir: &str) -> Result<ResourceBacking<A>, AssetLoadError>
    where
        A: Asset + 'static,
    {
        self.entity_manager
            .asset_manager
            .register_asset::<A>(str_dir)
    }

    pub fn new() -> Self {
        let camera = crate::world::camera::get_camera_default();
        //camera.build_camera_uniform(aspect_ratio, device);

        Self {
            deltas: Vec::<WorldUpdateDelta>::new(),
            init: false,
            camera,
            scene: Scene::new(),
            entity_manager: EntityManager::new(),
            load_queue: EntityLoadQueue::new(),
            instance_manager: InstanceManager::new(),
        }
    }

    pub fn spawn(
        &mut self,
        instance_data: Vec<(EntityHandle, Box<dyn Archetype>)>,
    ) -> Vec<InstanceUploadData> {
        let instance_upload_data = self
            .instance_manager
            .spawn_instances(&self.entity_manager, instance_data)
            .unwrap_or_else(|e| panic!("error handle for spawn fail! {:?}", e));

        for upload_data in instance_upload_data.iter() {
            self.scene.add_instances(&upload_data);
        }
        instance_upload_data
    }

    pub fn despawn(
        &mut self,
        instance_handle: InstanceHandle,
        deltas: &mut Vec<WorldUpdateDelta>,
    ) -> Result<(), WorldUpdateError> {
        let gpu_instance_handle = self.instance_manager.despawn(instance_handle)?;
        deltas.push(WorldUpdateDelta::InstanceDespawn(gpu_instance_handle));
        Ok(())
    }

    pub fn update<'frame>(
        &'frame mut self,
        commands: &mut Vec<AppCommand>,
    ) -> Result<(), WorldUpdateError> {
        // check scenes
        if self.scene.is_dirty() {
            self.handle_scene_event()?; // TODO: allow for multiple scenes
        }
        let pending_assets = self.load_queue.pending_asset_uploads.drain(..);
        for handle in pending_assets {
            let job: GPUAssetUploadJob = self
                .entity_manager
                .asset_manager
                .get_upload_job_for(handle)?;
            self.deltas.push(WorldUpdateDelta::AssetDidLoad(job));
        }

        self.instance_manager.update(commands);

        Ok(())
    }

    fn try_handle_scene_load(&mut self) -> Result<bool, WorldUpdateError> {
        self.load_queue
            .poll_scene_job(self.scene.scene_id, &mut self.entity_manager.asset_manager)?;
        if self
            .load_queue
            .completed_queue
            .get(&self.scene.scene_id)
            .is_some()
        {
            self.scene.pop_event();
            self.load_queue.dequeue_spawned_scene(self.scene.scene_id);
            return Ok(true);
        }

        Ok(false)
    }

    fn handle_scene_event(&mut self) -> Result<(), WorldUpdateError> {
        'outer: loop {
            let scene_event = self.scene.current_event();
            if scene_event.is_some() {
                match scene_event.unwrap() {
                    SceneEvent::LoadLevelChanged(old, new) => {
                        if self.load_queue.has_pending_scene_job(self.scene.scene_id) {
                            if !self.try_handle_scene_load()? {
                                break;
                            }
                        } else if new > old {
                            self.load_queue
                                .new_scene_job(&self.scene, &self.entity_manager)?;
                            if !self.try_handle_scene_load()? {
                                break 'outer;
                            }
                        } else {
                            //TODO: continue?
                        }
                    }
                    SceneEvent::Spawn(_) => match self.scene.pop_event().unwrap() {
                        SceneEvent::Spawn(instance_data) => {
                            let upload_data = self.spawn(instance_data);
                            for datum in upload_data {
                                match datum {
                                    InstanceUploadData::New(new_instance) => {
                                        self.deltas
                                            .push(WorldUpdateDelta::NewEntitySpawn(new_instance));
                                    }
                                    InstanceUploadData::Copied(copied_instance) => {
                                        self.deltas.push(WorldUpdateDelta::EntityInstanceSpawn(
                                            copied_instance,
                                        ));
                                    }
                                }
                            }
                        }
                        _ => unreachable!(),
                    },
                }
            } else {
                self.scene.mark_clean();
                break;
            }
        }
        Ok(())
    }

    pub fn post_frame_update(&mut self, render_deltas: Vec<RenderUpdateDelta>) {
        for delta in render_deltas {
            match delta {
                RenderUpdateDelta::AssetGPULoaded(asset_handle, allocation_handle) => {
                    self.entity_manager
                        .asset_manager
                        .register_asset_gpu_residency(&asset_handle, allocation_handle.clone())
                        .expect("Asset not found");
                }
                RenderUpdateDelta::EntityGPULoaded(_) => {
                    // TODO wait to dequeue until GPU reports it has successfully loaded entity?
                }
                RenderUpdateDelta::EntitySpawned {
                    instance_handle,
                    gpu_instance_handle,
                    record_offset,
                } => {
                    self.instance_manager.add_record_index(
                        &instance_handle,
                        record_offset,
                        gpu_instance_handle,
                    );
                }
                RenderUpdateDelta::ProtypeCreated {
                    instance_handle,
                    prototype,
                } => self
                    .instance_manager
                    .register_prototype(instance_handle.entity_handle, prototype),
            }
        }
    }
}
