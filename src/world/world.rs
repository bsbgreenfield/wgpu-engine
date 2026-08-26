use std::range::Range;
use std::{collections::HashMap, fmt::Debug};

use crate::{
    app::{GPUAssetUploadJob, app::AppCommand},
    asset_manager::{Asset, AssetLoadError, asset_manager::AssetManager},
    common::{
        entity::{EntityHandle, PrototypeHandle},
        instance::{GPUInstanceHandle, InstanceHandle},
    },
    renderer::{GPUAllocationHandle, RenderUpdateDelta},
    util::types::{LocalTransform, Mat4F32},
    world::{
        WorldUpdateError,
        camera::Camera,
        entity_manager::{components::ResourceBacking, entity_manager::EntityManager},
        instance_manager::{archetypes::Archetype, instance_manager::InstanceManager},
        load_queue_new::LoadQueueNew,
        scene::{
            SceneEvent, SceneId, SceneLoadLevel,
            manager::{SceneManager, SceneManagerError},
            scene::{Scene, Spawn},
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

pub struct RenderView {
    pub alloc_handle: GPUAllocationHandle,
    pub pnujw_draws: Option<DrawSet>,
    pub pnu_draws: Option<DrawSet>,
}

pub struct RenderGroup {
    pub entity_handle: EntityHandle,
    pub views: Vec<RenderView>,
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
    pub entity_manager: EntityManager,
    pub asset_manager: AssetManager,
    load_queue_new: LoadQueueNew,
    pub instance_manager: InstanceManager,
    pub scene_manager: SceneManager,
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
            load_queue_new: LoadQueueNew::default(),
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

    pub fn spawn(&mut self, instance_data: Vec<Spawn<dyn Archetype>>) -> Vec<InstanceUploadData> {
        let instance_upload_data = self
            .instance_manager
            .spawn_instances(&self.entity_manager, &self.asset_manager, instance_data)
            .unwrap_or_else(|e| panic!("error handle for spawn fail! {:?}", e));

        for iud in instance_upload_data.iter() {
            todo!("add instance handles to scenes?")
        }

        instance_upload_data
    }

    pub fn despawn_instance(
        &mut self,
        instance_handle: InstanceHandle,
    ) -> Result<(), WorldUpdateError> {
        let gpu_instance_handle = self.instance_manager.despawn(instance_handle)?;
        self.deltas
            .push(WorldUpdateDelta::InstanceDespawn(gpu_instance_handle));
        Ok(())
    }

    // pub fn despawn_scene(&mut self, scene_id: SceneId) {
    //     //TODO: find the actual scene
    //     let scene = std::mem::replace(&mut self.scene, Scene::new());
    //     for instance in scene.instances.iter() {
    //         let dead_list = self.despawn_instance(instance.clone());
    //     }
    // }

    pub fn update<'frame>(
        &'frame mut self,
        commands: &mut Vec<AppCommand>,
    ) -> Result<(), WorldUpdateError> {
        self.scene_manager.process_scene_events()?;
        for update in self.scene_manager.asset_updates().drain(..) {
            self.load_queue_new
                .add_load_job(update, &self.asset_manager);
        }
        self.load_queue_new.poll_jobs(&mut self.asset_manager)?;
        for handle in self.load_queue_new.pending_gpu.drain(..) {
            let job: GPUAssetUploadJob = self.asset_manager.get_upload_job_for(handle)?;
            self.deltas.push(WorldUpdateDelta::AssetDidLoad(job));
        }
        if !self.scene_manager.spawn_queue.is_empty() {
            let sq = std::mem::take(&mut self.scene_manager.spawn_queue);
            self.spawn(sq);
        }
        self.instance_manager.update(commands);

        Ok(())
    }

    // pub fn update<'frame>(
    //     &'frame mut self,
    //     commands: &mut Vec<AppCommand>,
    // ) -> Result<(), WorldUpdateError> {
    //     // check scenes
    //     if self.scene.is_dirty() {
    //         self.handle_scene_event()?; // TODO: allow for multiple scenes
    //     }
    //     let pending_assets = self.load_queue.pending_asset_uploads.drain(..);
    //     for handle in pending_assets {
    //         let job: GPUAssetUploadJob = self.asset_manager.get_upload_job_for(handle)?;
    //         self.deltas.push(WorldUpdateDelta::AssetDidLoad(job));
    //     }

    //     let completed_assets = self.load_queue_new.completed.drain(..);
    //     for handle in completed_assets {}

    //     self.instance_manager.update(commands);

    //     Ok(())
    // }

    // fn try_handle_scene_load(&mut self) -> Result<bool, WorldUpdateError> {
    //     self.load_queue
    //         .poll_scene_job(self.scene.scene_id, &mut self.asset_manager)?;
    //     if self
    //         .load_queue
    //         .completed_queue
    //         .get(&self.scene.scene_id)
    //         .is_some()
    //     {
    //         self.scene.pop_event();
    //         self.load_queue.dequeue_spawned_scene(self.scene.scene_id);
    //         return Ok(true);
    //     }

    //     Ok(false)
    // }

    // fn handle_scene_event(&mut self) -> Result<(), WorldUpdateError> {
    //     'outer: loop {
    //         let scene_event = self.scene.current_event();
    //         if scene_event.is_some() {
    //             match scene_event.unwrap() {
    //                 SceneEvent::LoadLevelChanged(old, new) => {
    //                     if self.load_queue.has_pending_scene_job(self.scene.scene_id) {
    //                         if !self.try_handle_scene_load()? {
    //                             break;
    //                         }
    //                     } else if new > old {
    //                         self.load_queue
    //                             .new_scene_job(&self.scene, &self.entity_manager)?;
    //                         if !self.try_handle_scene_load()? {
    //                             break 'outer;
    //                         }
    //                     } else if new < old {
    //                         match (old, new) {
    //                             (SceneLoadLevel::GPU, SceneLoadLevel::CPU) => {
    //                                 todo!("remove gpu residencies")
    //                             }
    //                             (SceneLoadLevel::GPU, SceneLoadLevel::NotLoaded) => {
    //                                 todo!("remove gpu and cpu residencies")
    //                             }
    //                             (SceneLoadLevel::CPU, SceneLoadLevel::NotLoaded) => {
    //                                 todo!("remove cpu residencies")
    //                             }
    //                             _ => unreachable!(),
    //                         }
    //                     }
    //                 }
    //                 SceneEvent::Spawn(_) => match self.scene.pop_event().unwrap() {
    //                     SceneEvent::Spawn(instance_data) => {
    //                         let upload_data = self.spawn(instance_data);
    //                         for datum in upload_data {
    //                             match datum {
    //                                 InstanceUploadData::New(new_instance) => {
    //                                     self.deltas
    //                                         .push(WorldUpdateDelta::NewEntitySpawn(new_instance));
    //                                 }
    //                                 InstanceUploadData::Copied(copied_instance) => {
    //                                     self.deltas.push(WorldUpdateDelta::EntityInstanceSpawn(
    //                                         copied_instance,
    //                                     ));
    //                                 }
    //                             }
    //                         }
    //                     }
    //                     _ => unreachable!(),
    //                 },
    //                 SceneEvent::SpawnNew => {}
    //             }
    //         } else {
    //             self.scene.mark_clean();
    //             break;
    //         }
    //     }
    //     Ok(())
    // }

    pub fn post_frame_update(&mut self, render_deltas: Vec<RenderUpdateDelta>) {
        for delta in render_deltas {
            match delta {
                RenderUpdateDelta::AssetGPULoaded(asset_handle, allocation_handle) => {
                    self.asset_manager
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
