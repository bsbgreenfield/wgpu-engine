use std::{
    collections::{HashMap, HashSet},
    ops::Range,
};

use cgmath::SquareMatrix;

use super::scene::Scene;
use crate::{
    app::renderer_new::{GPUAllocationHandle, RenderUpdateDeltaNew},
    asset_manager::asset_manager::{
        AssetHandle, AssetLoadError, AssetLoadResult, AssetManager, GltfAsset, LoadedAsset,
    },
    util::types::Mat4F32,
    world::{
        camera::Camera,
        components::{MeshCollectionComponent, MeshCollectionDescriptor},
        entity_manager::{EntityHandle, EntityManager, EntityManagerError},
        scene::{SceneEvent, SceneLoadLevel},
    },
};

pub struct DrawSet {
    pub mesh_ids: Vec<u32>,
    pub primtitive_ranges: Vec<Range<u32>>,
}

impl DrawSet {
    #[inline]
    pub const fn within(prim_range: &Range<u32>, range: &Range<u32>) -> Range<u32> {
        let start = range.start + prim_range.start;
        start..(start + (prim_range.end - prim_range.start) as u32)
    }
}

pub struct RenderView {
    pub gpu_handle: GPUAllocationHandle,
    pub pnujw_draws: DrawSet,
    pub pnu_draws: DrawSet,
}

pub struct RenderGroup {
    entity: EntityHandle,
    pub views: Vec<RenderView>,
}
impl RenderGroup {
    pub fn new(entity: EntityHandle, views: Vec<RenderView>) -> Self {
        Self { entity, views }
    }
}

#[derive(Debug)]
pub enum WorldInitError {
    AssetFailure(AssetLoadError),
    EntityFailure(EntityManagerError),
}

#[derive(Debug)]
pub enum WorldUpdateError {
    AssetLoadFailure(AssetLoadError),
    AssetLoadNotComplete(AssetHandle),
    EntityLoadNotFound(EntityHandle),
    EntityLoadNotComplete(EntityHandle),
    EntityLoadAlreadyEnqeued(EntityHandle),
    SomethingIsWrong(String),
}

impl From<AssetLoadError> for WorldUpdateError {
    fn from(value: AssetLoadError) -> Self {
        Self::AssetLoadFailure(value)
    }
}

impl From<AssetLoadError> for WorldInitError {
    fn from(value: AssetLoadError) -> Self {
        Self::AssetFailure(value)
    }
}
impl From<EntityManagerError> for WorldInitError {
    fn from(value: EntityManagerError) -> Self {
        Self::EntityFailure(value)
    }
}

struct EntityLoadJob {
    load_level: SceneLoadLevel,
    asset_load_jobs: Vec<AssetHandle>,
}

enum AssetLoadJobState {
    Done(AssetLoadResult),
    Pending,
}
struct AssetLoadJob {
    state: AssetLoadJobState,
    ref_count: usize,
}
struct AssetLoadQueue {
    completed_queue: HashMap<EntityHandle, HashMap<AssetHandle, GPUAllocationHandle>>,
    entity_jobs: HashMap<EntityHandle, EntityLoadJob>,
    asset_jobs: HashMap<AssetHandle, AssetLoadJob>,
}

struct EntityLoadResult {
    entity: EntityHandle,
    allocations: HashMap<AssetHandle, GPUAllocationHandle>,
}

impl AssetLoadQueue {
    fn new() -> Self {
        Self {
            completed_queue: HashMap::new(),
            entity_jobs: HashMap::new(),
            asset_jobs: HashMap::new(),
        }
    }

    fn dequeue_completed(&mut self, entity: &EntityHandle) {
        let completed = self.completed_queue.remove(&entity).unwrap();
        // decrement all ref counts of asset jobs and remove any that are no longer referenced
        for asset in completed.keys() {
            let asset_job = self.asset_jobs.get_mut(asset).unwrap();
            asset_job.ref_count -= 1;
            if asset_job.ref_count == 0 {
                self.asset_jobs.remove(asset);
            }
        }
    }

    fn new_entity_load(
        &mut self,
        entity: EntityHandle,
        load_level: SceneLoadLevel,
        assets: &HashSet<AssetHandle>,
    ) -> Result<&EntityLoadJob, WorldUpdateError> {
        let s = self.entity_jobs.insert(
            entity,
            EntityLoadJob {
                load_level,
                asset_load_jobs: assets.iter().map(|a| a.to_owned()).collect(),
            },
        );

        if s.is_some() {
            return Err(WorldUpdateError::EntityLoadAlreadyEnqeued(entity));
        }

        for asset in assets {
            if self.asset_jobs.get(asset).is_none() {
                self.asset_jobs.insert(
                    *asset,
                    AssetLoadJob {
                        ref_count: 1,
                        state: AssetLoadJobState::Pending,
                    },
                );
            }
        }
        Ok(self.entity_jobs.get(&entity).as_ref().unwrap())
    }

    fn poll_entity_jobs(&mut self) {
        if self.entity_jobs.len() == 0 {
            return;
        }
        let completed_jobs: Vec<(EntityHandle, HashMap<AssetHandle, GPUAllocationHandle>)> = self
            .entity_jobs
            .extract_if(|_, entity_job| {
                entity_job.asset_load_jobs.iter().all(|asset_handle| {
                    matches!(
                        self.asset_jobs.get(asset_handle).unwrap().state,
                        AssetLoadJobState::Done(_)
                    )
                })
            })
            .map(|(entity_handle, entity_job)| {
                let allocations = entity_job
                    .asset_load_jobs
                    .into_iter()
                    .map(|asset_handle| {
                        let alloc = match &self.asset_jobs.get(&asset_handle).unwrap().state {
                            AssetLoadJobState::Done(load_result) => match load_result {
                                AssetLoadResult::LoadedGPU(alloc_handle) => alloc_handle.clone(),
                                _ => todo!(),
                            },
                            _ => unreachable!(),
                        };

                        (asset_handle, alloc)
                    })
                    .collect();
                (entity_handle, allocations)
            })
            .collect();
        for job in completed_jobs {
            self.completed_queue.insert(job.0, job.1);
        }
    }
}

pub enum WorldUpdateDelta {
    EntityDidLoad(EntityHandle),
    AssetDidLoad(AssetHandle),
}

pub struct World {
    camera: Camera,
    scene: Scene,
    asset_manager: AssetManager,
    pub entity_manager: EntityManager,
    asset_load_queue: AssetLoadQueue,
}

impl World {
    pub fn get_loaded_asset_of(&self, asset_handle: &AssetHandle) -> Option<&LoadedAsset> {
        self.asset_manager.get_loaded_asset(asset_handle)
    }

    pub fn new(aspect_ratio: f32, device: &wgpu::Device) -> Result<Self, WorldInitError> {
        let mut camera = crate::world::camera::get_camera_default();
        camera.build_camera_uniform(aspect_ratio, device);

        let mut asset_manager = AssetManager::new();
        let mut entity_manager = EntityManager::new();

        let box_asset = asset_manager.register_asset::<GltfAsset>("box")?;

        let mesh = MeshCollectionComponent::new(MeshCollectionDescriptor {
            resource_backing: box_asset,
            allocation_handle: None,
            mesh_ids: &[0],
        });

        let box_entity = entity_manager.new_entity()?;

        entity_manager.add_mesh_collection_for_entity(box_entity, mesh);
        entity_manager.add_physical_position_for_entity(
            box_entity,
            cgmath::Matrix4::<f32>::identity().into(),
        );

        let mut scene = Scene::new();
        scene.add_entity(box_entity);
        scene.set_load_level(SceneLoadLevel::GPU);

        Ok(Self {
            camera,
            scene,
            asset_manager,
            entity_manager,
            asset_load_queue: AssetLoadQueue::new(),
        })
    }

    fn enqueue_entity_load(
        &mut self,
        entity_handle: EntityHandle,
        scene_load_level: SceneLoadLevel,
    ) -> Result<(), WorldUpdateError> {
        let unallocated_assets = self.entity_manager.unallocated_assets_of(entity_handle);
        if unallocated_assets.is_empty() {
            return Err(WorldUpdateError::EntityLoadAlreadyEnqeued(entity_handle));
        }
        // create a new entity load job along with unique asset load jobs
        let job_ref = self.asset_load_queue.new_entity_load(
            entity_handle,
            scene_load_level,
            &unallocated_assets,
        )?;
        Ok(())
    }

    fn poll_assets_for_job(
        &mut self,
        entity_handle: &EntityHandle,
        deltas: &mut Vec<WorldUpdateDelta>, // this may not work with async
    ) -> Result<(), WorldUpdateError> {
        let entity_job = self
            .asset_load_queue
            .entity_jobs
            .get(entity_handle)
            .unwrap();
        for asset in entity_job.asset_load_jobs.iter() {
            match self
                .asset_manager
                .set_minumum_load_level(*asset, entity_job.load_level)?
            {
                AssetLoadResult::PendingCPU => todo!("handle async cpu load"),
                AssetLoadResult::PendingGPU => {
                    deltas.push(WorldUpdateDelta::AssetDidLoad(*asset));
                }
                AssetLoadResult::LoadedCPU => {
                    // do nothing?
                }
                AssetLoadResult::LoadedGPU(allocation_handle) => {
                    //
                }
            }
        }
        Ok(())
    }
    pub fn update(&mut self) -> Result<Vec<WorldUpdateDelta>, WorldUpdateError> {
        let mut deltas = Vec::<WorldUpdateDelta>::new();
        // check scenes
        if self.scene.is_dirty() {
            if let Some(scene_event) = self.scene.pop_event() {
                deltas.extend(self.handle_scene_event(scene_event, self.scene.load_level)?);
            }
        }
        self.asset_load_queue.poll_entity_jobs();
        for entity in self.asset_load_queue.completed_queue.keys() {
            deltas.push(WorldUpdateDelta::EntityDidLoad(*entity));
        }

        Ok(deltas)
    }

    fn handle_scene_event(
        &mut self,
        event: SceneEvent,
        scene_load_level: SceneLoadLevel,
    ) -> Result<Vec<WorldUpdateDelta>, WorldUpdateError> {
        let mut deltas: Vec<WorldUpdateDelta> = Vec::new();
        match event {
            SceneEvent::EntitiesAdded(entities) => {
                for entity_handle in entities {
                    // TODO: handle failed job enqueue?
                    self.enqueue_entity_load(entity_handle, scene_load_level)?;
                    self.poll_assets_for_job(&entity_handle, &mut deltas);
                }
            }
        }
        Ok(deltas)
    }

    pub fn post_frame_update(&mut self, render_deltas: &[RenderUpdateDeltaNew]) {
        for delta in render_deltas {
            match delta {
                RenderUpdateDeltaNew::AssetGPULoaded(allocation_handle) => {
                    self.asset_manager
                        .register_asset_gpu_residency(allocation_handle)
                        .expect("Asset not found");
                }
                RenderUpdateDeltaNew::EntityGPULoaded(entity_handle) => {
                    self.asset_load_queue.dequeue_completed(entity_handle);
                }
            }
        }
    }
}

pub struct EntityBuilder<'m> {
    entity_manager: &'m mut EntityManager,
    asset_manger: &'m mut AssetManager,
}

impl<'m> EntityBuilder<'m> {
    pub fn create_physical_entity(
        &mut self,
        mesh: MeshCollectionComponent,
        physical_position: Mat4F32,
    ) -> Result<EntityHandle, EntityManagerError> {
        let entity = self.entity_manager.new_entity()?;
        self.entity_manager
            .add_mesh_collection_for_entity(entity, mesh);
        Ok(entity)
    }
}
