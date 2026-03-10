use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use super::scene::Scene;
use crate::{
    app::{render::renderer::RenderUpdateDelta, renderer_new::AllocationHandle},
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

#[derive(Debug)]
pub enum WorldInitError {
    AssetFailure(AssetLoadError),
    EntityFailure(EntityManagerError),
}

#[derive(Debug)]
pub enum WorldUpdateError {
    AssetLoadFailure(AssetLoadError),
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
    asset_load_jobs: Vec<Rc<AssetLoadJob>>,
}

struct AssetLoadJob {
    asset_handle: AssetHandle,
    allocation_handle: Option<AllocationHandle>,
}
struct AssetLoadQueue {
    entity_jobs: HashMap<EntityHandle, EntityLoadJob>,
    asset_jobs: HashMap<AssetHandle, Rc<AssetLoadJob>>,
}

impl AssetLoadQueue {
    fn new() -> Self {
        Self {
            entity_jobs: HashMap::new(),
            asset_jobs: HashMap::new(),
        }
    }
    fn new_entity_load(&mut self, entity: EntityHandle, assets: &HashSet<AssetHandle>) {
        self.entity_jobs.insert(
            entity,
            EntityLoadJob {
                asset_load_jobs: vec![],
            },
        );

        let mut jobs = Vec::<Rc<AssetLoadJob>>::new();
        for asset in assets {
            if let Some(job) = self.asset_jobs.get(asset) {
                jobs.push(job.clone());
            } else {
                let new_job = Rc::new(AssetLoadJob {
                    asset_handle: *asset,
                    allocation_handle: None,
                });
                jobs.push(new_job.clone());
                self.asset_jobs.insert(*asset, new_job);
            }
        }
    }

    fn poll_entity_job(&mut self, entity: EntityHandle) -> bool {
        if let Some(entity_job) = self.entity_jobs.get(&entity) {
            for asset_job in entity_job.asset_load_jobs.iter() {
                if asset_job.allocation_handle.is_none() {
                    return false;
                }
            }
            // entity job is done. remove from queue, and test asset jobs to see if they can be
            // removed from the queue
            let completed_entity_job = self.entity_jobs.remove(&entity).unwrap();
            for asset_job in completed_entity_job.asset_load_jobs.iter() {
                if Rc::strong_count(asset_job) == 1 {
                    self.asset_jobs.remove(&asset_job.asset_handle);
                }
            }
            return true;
        } else {
            panic!("no entity found")
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
    entity_manager: EntityManager,
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

    pub fn update(&mut self) -> Result<Vec<WorldUpdateDelta>, WorldUpdateError> {
        let mut deltas = Vec::<WorldUpdateDelta>::new();
        if self.scene.is_dirty() {
            if let Some(scene_event) = self.scene.pop_event() {
                deltas.extend(self.handle_scene_event(scene_event, self.scene.load_level)?);
            }
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
                    // get handles of the assets that need loading
                    let unallocated_assets =
                        self.entity_manager.unallocated_assets_of(entity_handle);
                    // create a new entity load job along with unique asset load jobs
                    self.asset_load_queue
                        .new_entity_load(entity_handle, &unallocated_assets);
                    for asset_handle in unallocated_assets {
                        match self
                            .asset_manager
                            .set_minumum_load_level(asset_handle, scene_load_level)?
                        {
                            AssetLoadResult::PendingCPU => todo!("handle async cpu load"),
                            AssetLoadResult::PendingGPU => {
                                deltas.push(WorldUpdateDelta::AssetDidLoad(asset_handle));
                            }
                            AssetLoadResult::LoadedCPU => {
                                // do nothing?
                            }
                            AssetLoadResult::LoadedGPU(allocation_handle) => {
                                //
                            }
                        }
                    }
                }
            }
        }
        Ok(deltas)
    }

    fn set_min_load_level(&mut self, assets: Vec<AssetHandle>, load_level: SceneLoadLevel) {
        todo!()
    }

    pub fn post_frame_update(&mut self, render_deltas: &[RenderUpdateDelta]) {
        for delta in render_deltas {
            match delta {
                RenderUpdateDelta::AssetGPULoaded(mesh_handle) => self
                    .asset_manager
                    .register_asset_gpu_residency(mesh_handle)
                    .expect("Asset not found"),
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
