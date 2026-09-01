use std::fmt::Debug;
use std::range::Range;

use crate::{
    app::{GPUAssetUploadJob, app::AppCommand},
    asset_manager::{
        Asset, AssetHandle, AssetLoadError, AssetResidency, asset_manager::AssetManager,
    },
    common::{entity::EntityHandle, instance::InstanceHandle},
    renderer::{GPUAllocationHandle, GPUInstanceHandle, PrototypeHandle, RenderUpdateDelta},
    util::types::{LocalTransform, Mat4F32},
    world::{
        RenderKey, WorldUpdateError,
        camera::Camera,
        entity_manager::{components::ResourceBacking, entity_manager::EntityManager},
        instance_manager::{archetypes::Archetype, instance_manager::InstanceManager},
        scene::{
            SceneId, SceneLoadLevel,
            manager::{SceneManager, SceneManagerError},
            scene::Spawn,
        },
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
        Range {
            start: start,
            end: start + (prim_range.end - prim_range.start) as u32,
        }
    }
}

pub(crate) struct RenderView {
    pub alloc_handle: GPUAllocationHandle,
    pub pnujw_draws: Option<DrawSet>,
    pub pnu_draws: Option<DrawSet>,
}

pub(crate) struct RenderGroup {
    _entity_handle: EntityHandle,
    views: Vec<RenderView>,
}

impl RenderGroup {
    pub fn views(&self) -> &[RenderView] {
        &self.views
    }
    pub(super) fn new(views: Vec<RenderView>, entity_handle: EntityHandle) -> Self {
        Self {
            _entity_handle: entity_handle,
            views,
        }
    }
}

#[derive(Debug, Clone)]
pub enum LocalTransforms {
    Uninit,
    Owned { data: Vec<LocalTransform> },
    CopiedFrom { donor: InstanceHandle },
    NeedsCopy,
    SharedWith { donor: InstanceHandle },
    NeedsShared,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

#[derive(Clone)]
pub(crate) enum WorldUpdateDelta {
    NewEntitySpawn(NewInstanceData),
    EntityInstanceSpawn(CopiedInstanceData),
    AssetDidLoad(GPUAssetUploadJob),
    AssetUnload(AssetHandle, GPUAllocationHandle),
    InstanceDespawn(GPUInstanceHandle),
}

impl<'frame> Debug for WorldUpdateDelta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorldUpdateDelta::NewEntitySpawn(_) => f.write_str("NewEntitySpawn"),
            WorldUpdateDelta::EntityInstanceSpawn(_) => f.write_str("EntityInstanceSpawn"),
            WorldUpdateDelta::AssetDidLoad(_) => f.write_str("AssetDidLoad"),
            WorldUpdateDelta::InstanceDespawn(handle) => write!(f, "despawn {:?}", handle),
            WorldUpdateDelta::AssetUnload(_asset_handle, alloc_handle) => {
                write!(f, "unload asset {:?}", alloc_handle)
            }
        }
    }
}

pub struct World {
    init: bool,
    pub camera: Camera,
    pub entity_manager: EntityManager,
    pub asset_manager: AssetManager,
    pub instance_manager: InstanceManager,
    pub scene_manager: SceneManager,
    pub(crate) deltas: Vec<WorldUpdateDelta>,
}

impl World {
    pub fn is_initialized(&self) -> bool {
        self.init
    }
    pub fn init(&mut self, aspect_ratio: f32, device: &wgpu::Device) {
        self.camera.build_camera_uniform(aspect_ratio, device);
        self.init = true;
    }

    pub fn register_asset<A>(&mut self, str_dir: &str) -> Result<ResourceBacking<A>, AssetLoadError>
    where
        A: Asset + 'static,
    {
        self.asset_manager.register_asset::<A>(str_dir)
    }

    pub fn new() -> Self {
        let camera = crate::world::camera::get_camera_default();
        //camera.build_camera_uniform(aspect_ratio, device);

        Self {
            deltas: Vec::<WorldUpdateDelta>::new(),
            init: false,
            camera,
            entity_manager: EntityManager::new(),
            asset_manager: AssetManager::new(),
            instance_manager: InstanceManager::new(),
            scene_manager: SceneManager::new(),
        }
    }

    pub fn add_instances(
        &mut self,
        scene_id: SceneId,
        spawn_data: Vec<Spawn<dyn Archetype>>,
    ) -> Result<(), SceneManagerError> {
        self.scene_manager.add_instances(scene_id, spawn_data)
    }

    pub fn spawn(
        &mut self,
        scene_id: SceneId,
        instance_data: Vec<Spawn<dyn Archetype>>,
    ) -> Result<Vec<InstanceUploadData>, WorldUpdateError> {
        let upload_data = self.instance_manager.spawn_instances(
            &self.entity_manager,
            &self.asset_manager,
            instance_data,
        )?;

        for iud in upload_data.iter() {
            self.scene_manager
                .add_instance_handles(scene_id, iud.handles())?;
        }
        Ok(upload_data)
    }

    pub fn despawn_instance(
        &mut self,
        instance_handle: InstanceHandle,
    ) -> Result<(), WorldUpdateError> {
        let gpu_instance_handle = self.instance_manager.despawn(instance_handle.clone())?;
        self.deltas.push(WorldUpdateDelta::InstanceDespawn(
            gpu_instance_handle.clone(),
        ));
        self.scene_manager
            .inflight_despawns
            .insert(gpu_instance_handle, instance_handle);
        Ok(())
    }

    pub fn update<'frame>(
        &'frame mut self,
        commands: &mut Vec<AppCommand>,
    ) -> Result<(), WorldUpdateError> {
        for request in self.scene_manager.asset_requests() {
            self.scene_manager
                .load_queue_new
                .add_load_job(request, &self.asset_manager);
        }
        for transition in self
            .scene_manager
            .load_queue_new
            .poll_jobs(&mut self.asset_manager)?
        {
            if matches!(transition.new, SceneLoadLevel::PendingGPU) {
                let job: GPUAssetUploadJob =
                    self.asset_manager.get_upload_job_for(transition.handle)?;
                self.deltas.push(WorldUpdateDelta::AssetDidLoad(job));
            } else if transition.old == SceneLoadLevel::GPU {
                let alloc_handle = self.asset_manager.alloc_handle_of(&transition.handle)?;
                self.deltas.push(WorldUpdateDelta::AssetUnload(
                    transition.handle,
                    alloc_handle,
                ));
            }
            self.scene_manager.on_asset_level_changed(transition);
        }

        self.scene_manager.process_scene_events()?;
        for handle in std::mem::take(&mut self.scene_manager.despawn_queue) {
            self.despawn_instance(handle)?;
        }

        if !self.scene_manager.spawn_queue.is_empty() {
            let spawn_data = std::mem::take(&mut self.scene_manager.spawn_queue);
            for (scene_id, spawns) in spawn_data {
                for instance_data in self.spawn(scene_id, spawns)? {
                    match instance_data {
                        InstanceUploadData::New(new) => {
                            self.deltas.push(WorldUpdateDelta::NewEntitySpawn(new))
                        }
                        InstanceUploadData::Copied(copied) => self
                            .deltas
                            .push(WorldUpdateDelta::EntityInstanceSpawn(copied)),
                    }
                }
            }
        }
        self.instance_manager.update(commands);

        Ok(())
    }

    pub(crate) fn post_frame_update(&mut self, render_deltas: Vec<RenderUpdateDelta>) {
        for delta in render_deltas {
            match delta {
                RenderUpdateDelta::AssetGPULoaded { key, alloc_handle } => {
                    self.asset_manager
                        .register_asset_gpu_residency(
                            AssetHandle::from_key(key),
                            alloc_handle.clone(),
                        )
                        .expect("Asset not found");
                }
                RenderUpdateDelta::AssetUnloaded {
                    alloc_handle: _,
                    key,
                } => {
                    let asset_handle = AssetHandle::from_key(key);
                    self.asset_manager.register_asset_gpu_unloaded(asset_handle);
                }
                RenderUpdateDelta::EntitySpawned {
                    instance_key,
                    gpu_instance_handle,
                    record_offset,
                } => {
                    let instance_handle = InstanceHandle::from_key(instance_key);
                    self.instance_manager.add_record_index(
                        &instance_handle,
                        record_offset,
                        gpu_instance_handle,
                    );
                }
                RenderUpdateDelta::InstanceDespawns(gpu_handles) => {
                    self.scene_manager.ack_despawns(gpu_handles);
                }
            }
        }
    }
}
