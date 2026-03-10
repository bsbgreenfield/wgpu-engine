use std::collections::HashSet;

use super::scene::Scene;
use crate::{
    app::render::renderer::RenderUpdateDelta,
    asset_manager::asset_manager::{
        AssetHandle, AssetLoadError, AssetLoadResult, AssetManager, GltfAsset, LoadedAsset,
    },
    util::types::Mat4F32,
    world::{
        camera::Camera,
        components::{MeshCollectionComponent, MeshCollectionDescriptor, ResourceBacking},
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

pub enum WorldUpdateDelta {
    EntityDidLoad(EntityHandle),
    AssetDidLoad(AssetHandle),
}

pub struct World {
    camera: Camera,
    scene: Scene,
    asset_manager: AssetManager,
    entity_manager: EntityManager,
}

impl World {
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
        })
    }

    pub fn get_loaded_assets_for(&self, entity_handle: EntityHandle) -> Vec<&LoadedAsset> {
        let assets = self.entity_manager.assets_of(entity_handle);
        let loaded_asset_refs = self.asset_manager.get_loaded_assets(assets);
        loaded_asset_refs
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
                    let mut entity_done_loading = true;
                    let assets = self.entity_manager.assets_of(entity_handle);
                    for asset_handle in assets {
                        match self
                            .asset_manager
                            .set_minumum_load_level(asset_handle, scene_load_level)?
                        {
                            AssetLoadResult::PendingCPU => todo!("handle async cpu load"),
                            AssetLoadResult::PendingGPU => {
                                entity_done_loading = false;
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
                    if entity_done_loading {
                        deltas.push(WorldUpdateDelta::EntityDidLoad(entity_handle));
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
