#[cfg(test)]
use crate::world::{WorldInitError, world::World};
use crate::world::{entity_manager::EntityHandle, instance_manager::Archetype};

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub enum SceneLoadLevel {
    NotLoaded,
    CPU,
    GPU,
}

#[derive(Clone)]
pub enum SceneEvent {
    EntitiesAdded(Vec<EntityHandle>),
    LoadLevelChanged(SceneLoadLevel, SceneLoadLevel),
}

pub struct Scene {
    pub entitites: Vec<EntityHandle>,
    dirty: bool,
    spawn_count: usize,
    pub load_level: SceneLoadLevel,
    event_queue: Vec<SceneEvent>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            entitites: vec![],
            dirty: false,
            spawn_count: 0,
            load_level: SceneLoadLevel::NotLoaded,
            event_queue: Vec::new(),
        }
    }

    pub fn add_entity(&mut self, entity: EntityHandle) {
        self.entitites.push(entity);
    }

    pub fn set_load_level(&mut self, level: SceneLoadLevel) {
        self.event_queue
            .push(SceneEvent::LoadLevelChanged(self.load_level, level));
        self.load_level = level;
        self.dirty = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn pop_event(&mut self) -> Option<SceneEvent> {
        self.event_queue.pop()
    }
    #[cfg(test)]
    pub fn fox_box(world: &mut World) -> Result<Self, WorldInitError> {
        use crate::{
            asset_manager::gltf_assets::GltfAsset,
            world::components::{MeshCollectionComponent, MeshCollectionDescriptor},
        };

        let box_asset = world.asset_manager.register_asset::<GltfAsset>("box")?; // asset
        let fox_asset = world.asset_manager.register_asset::<GltfAsset>("fox")?;

        let box_entity = world.entity_manager.new_entity()?;
        let fox_entity = world.entity_manager.new_entity()?;

        let box_mesh = MeshCollectionComponent::new(MeshCollectionDescriptor {
            // MeshCollection
            resource_backing: box_asset,
            allocation_handle: None,
            mesh_ids: &[0],
        });
        let fox_mesh = MeshCollectionComponent::new(MeshCollectionDescriptor {
            resource_backing: fox_asset,
            allocation_handle: None,
            mesh_ids: &[0],
        });
        world
            .entity_manager
            .add_mesh_collection_for_entity(box_entity, box_mesh); // mesh
        world
            .entity_manager
            .add_mesh_collection_for_entity(fox_entity, fox_mesh); // mesh
        world
            .entity_manager
            .add_physical_position_for_entity(box_entity); // position

        let mut scene = Scene::new();
        scene.add_entity(box_entity);
        scene.add_entity(fox_entity);
        Ok(scene)
    }

    #[cfg(test)]
    pub fn box_scene(world: &mut World) -> Result<Self, WorldInitError> {
        use crate::{
            asset_manager::gltf_assets::GltfAsset,
            world::components::{MeshCollectionComponent, MeshCollectionDescriptor},
        };

        let box_asset = world.asset_manager.register_asset::<GltfAsset>("box")?; // asset

        let box_entity = world.entity_manager.new_entity()?;

        let box_mesh = MeshCollectionComponent::new(MeshCollectionDescriptor {
            // MeshCollection
            resource_backing: box_asset,
            allocation_handle: None,
            mesh_ids: &[0],
        });
        world
            .entity_manager
            .add_mesh_collection_for_entity(box_entity, box_mesh); // mesh

        let mut scene = Scene::new();
        scene.add_entity(box_entity);
        Ok(scene)
    }
    #[cfg(test)]
    pub fn fox_scene(world: &mut World) -> Result<Self, WorldInitError> {
        use crate::{
            asset_manager::gltf_assets::GltfAsset,
            world::components::{MeshCollectionComponent, MeshCollectionDescriptor},
        };

        let fox_asset = world.asset_manager.register_asset::<GltfAsset>("fox")?; // asset

        let fox_entity = world.entity_manager.new_entity()?;

        let fox_mesh = MeshCollectionComponent::new(MeshCollectionDescriptor {
            // MeshCollection
            resource_backing: fox_asset,
            allocation_handle: None,
            mesh_ids: &[0],
        });
        world
            .entity_manager
            .add_mesh_collection_for_entity(fox_entity, fox_mesh); // mesh

        let mut scene = Scene::new();
        scene.add_entity(fox_entity);
        Ok(scene)
    }
}
