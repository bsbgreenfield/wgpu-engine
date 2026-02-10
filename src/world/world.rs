use std::collections::HashSet;

use super::scene::Scene;
use crate::{
    asset_manager::asset_manager::{
        AssetHandle, AssetLoadError, AssetManager, GltfAsset, LoadedAsset,
    },
    util::types::Mat4F32,
    world::{
        camera::Camera,
        components::{MeshCollectionComponent, ResourceBacking},
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
}

impl WorldUpdateDelta {
    fn from_loaded_asset(la: &LoadedAsset, entity_handle: EntityHandle) -> Self {
        Self::EntityDidLoad(entity_handle)
    }
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

        // TODO: remove requirement for specifying vertex index type
        // remove ability to create assets separate from asset handles
        let box_asset = asset_manager.register_asset::<GltfAsset>("box")?;

        let mesh = MeshCollectionComponent::new(ResourceBacking::new(box_asset, 0));

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

    pub fn update(&mut self) -> Result<(), WorldUpdateError> {
        if self.scene.is_dirty() {
            if let Some(scene_event) = self.scene.pop_event() {
                self.handle_scene_event(scene_event, self.scene.load_level);
            }
        }
        Ok(())
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
                    let assets = self.entity_manager.assets_of(entity_handle);
                    self.asset_manager
                        .set_minumum_load_level(assets.into_iter().collect(), scene_load_level)?
                        .iter()
                        .for_each(|la_ref| {
                            deltas.push(WorldUpdateDelta::from_loaded_asset(la_ref, entity_handle));
                        });
                }
                // AKA load if needed
            }
        }
        Ok(deltas)
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
