#[cfg(test)]
use crate::world::{WorldInitError, world::World};
use crate::{
    asset_manager::{AssetLoadResult, gltf_asset::GltfAsset},
    world::{
        components::{AnimationComponentDescriptor, MeshAcessor, RigidAnimationMode},
        entity_manager::EntityHandle,
        instance_manager::Archetype,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum SceneLoadLevel {
    NotLoaded,
    CPU,
    GPU,
}

impl From<&AssetLoadResult> for SceneLoadLevel {
    fn from(value: &AssetLoadResult) -> Self {
        match value {
            AssetLoadResult::PendingCPU => SceneLoadLevel::NotLoaded,
            AssetLoadResult::LoadedCPU => SceneLoadLevel::CPU,
            AssetLoadResult::PendingGPU => SceneLoadLevel::CPU,
            AssetLoadResult::LoadedGPU(_) => SceneLoadLevel::GPU,
        }
    }
}

pub enum SceneEvent {
    LoadLevelChanged(SceneLoadLevel, SceneLoadLevel),
    Spawn(Vec<(EntityHandle, Box<dyn Archetype>)>),
}

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct SceneId(usize);
pub struct Scene {
    pub scene_id: SceneId,
    pub entitites: Vec<EntityHandle>,
    dirty: bool,
    pub spawn_count: usize,
    pub load_level: SceneLoadLevel,
    event_queue: Vec<SceneEvent>,
}

impl Scene {
    #[cfg(test)]
    pub fn new_with_id(id: usize) -> Self {
        Self {
            scene_id: SceneId(id),
            entitites: vec![],
            dirty: false,
            spawn_count: 0,
            load_level: SceneLoadLevel::NotLoaded,
            event_queue: Vec::new(),
        }
    }

    pub fn new() -> Self {
        Self {
            scene_id: SceneId(0), // TODO: scene ids to keep track of loads, querys, etc??
            entitites: vec![],
            dirty: false,
            spawn_count: 0,
            load_level: SceneLoadLevel::NotLoaded,
            event_queue: Vec::new(),
        }
    }
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn current_event(&self) -> Option<&SceneEvent> {
        self.event_queue.last()
    }

    pub fn spawn(&mut self, instance_data: Vec<(EntityHandle, Box<dyn Archetype>)>) {
        self.dirty = true;
        self.event_queue.push(SceneEvent::Spawn(instance_data));
        if self.load_level < SceneLoadLevel::GPU {
            self.set_load_level(SceneLoadLevel::GPU);
        }
        self.spawn_count += 1;
    }

    pub fn add_entity(&mut self, entity: EntityHandle) {
        self.entitites.push(entity);
    }

    fn set_load_level(&mut self, level: SceneLoadLevel) {
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

    pub fn fox_box(
        world: &mut crate::world::world::World,
    ) -> Result<Self, crate::world::WorldInitError> {
        use cgmath::SquareMatrix;

        use crate::world::{components::MeshCollectionDescriptor, instance_manager::APosition};

        let box_asset = world.register_asset::<GltfAsset>("box")?;
        let fox_asset = world.register_asset::<GltfAsset>("fox")?;

        let box_entity = world.entity_manager.new_entity()?;
        let fox_entity = world.entity_manager.new_entity()?;

        world.entity_manager.add_mesh_collection_for_entity(
            &box_entity,
            MeshCollectionDescriptor::new(box_asset, MeshAcessor::All, RigidAnimationMode::Shared),
        ); // mesh
        world.entity_manager.add_mesh_collection_for_entity(
            &fox_entity,
            MeshCollectionDescriptor::new(fox_asset, MeshAcessor::All, RigidAnimationMode::Shared),
        ); // mesh

        let mut scene = Scene::new();
        scene.add_entity(box_entity);
        scene.add_entity(fox_entity);
        scene.spawn(vec![
            (
                box_entity,
                Box::new(APosition {
                    position: cgmath::Matrix4::<f32>::identity().into(),
                }),
            ),
            (
                fox_entity,
                Box::new(APosition {
                    position: (cgmath::Matrix4::<f32>::from_translation(cgmath::Vector3::new(
                        1.5, 0.0, 0.0,
                    )) * cgmath::Matrix4::<f32>::from_scale(0.05))
                    .into(),
                }),
            ),
        ]);
        Ok(scene)
    }

    pub fn box_animated(
        world: &mut crate::world::world::World,
    ) -> Result<Self, crate::world::WorldInitError> {
        use cgmath::SquareMatrix;

        use crate::world::{
            components::{AnimationAccessor, MeshCollectionDescriptor},
            instance_manager::APosition,
        };

        let box_anim_asset = world.register_asset::<GltfAsset>("box_animated")?;
        let box_anim_entity = world.entity_manager.new_entity()?;
        world.entity_manager.add_mesh_collection_for_entity(
            &box_anim_entity,
            MeshCollectionDescriptor::new(
                box_anim_asset.clone(),
                MeshAcessor::All,
                RigidAnimationMode::Shared,
            )
            .with_animation(AnimationComponentDescriptor {
                accessor: AnimationAccessor::All,
                resource_backing: box_anim_asset,
            }),
        );

        let mut scene = Scene::new();
        scene.add_entity(box_anim_entity);

        scene.spawn(vec![(
            box_anim_entity.clone(),
            Box::new(APosition {
                position: cgmath::Matrix4::<f32>::identity().into(),
            }),
        )]);

        Ok(scene)
    }

    #[cfg(test)]
    pub fn box_scene(world: &mut World) -> Result<Self, WorldInitError> {
        use cgmath::SquareMatrix;

        use crate::world::{components::MeshCollectionDescriptor, instance_manager::APosition};

        let box_asset = world.register_asset::<GltfAsset>("box")?; // asset

        let box_entity = world.entity_manager.new_entity()?;

        world.entity_manager.add_mesh_collection_for_entity(
            &box_entity,
            MeshCollectionDescriptor {
                rigid_animation_mode: RigidAnimationMode::Shared,
                mesh_accessor: MeshAcessor::All,
                resource_backing: box_asset.erase(),
                animation: None,
            },
        ); // mesh

        let mut scene = Scene::new();
        scene.add_entity(box_entity);
        scene.spawn(vec![(
            EntityHandle(0),
            Box::new(APosition {
                position: cgmath::Matrix4::<f32>::identity().into(),
            }),
        )]);
        Ok(scene)
    }
    #[cfg(test)]
    pub fn fox_scene(world: &mut World) -> Result<Self, WorldInitError> {
        use crate::world::{components::MeshCollectionDescriptor, instance_manager::APosition};

        let fox_asset = world.register_asset::<GltfAsset>("fox")?; // asset

        let fox_entity = world.entity_manager.new_entity()?;

        world.entity_manager.add_mesh_collection_for_entity(
            &fox_entity,
            MeshCollectionDescriptor {
                resource_backing: fox_asset.erase(),
                animation: None,
                mesh_accessor: MeshAcessor::All,
                rigid_animation_mode: RigidAnimationMode::Shared,
            },
        ); // mesh

        let mut scene = Scene::new();
        scene.add_entity(fox_entity);
        scene.spawn(vec![(
            EntityHandle(0),
            Box::new(APosition {
                position: cgmath::Matrix4::<f32>::from_scale(0.05).into(),
            }),
        )]);
        Ok(scene)
    }
}
