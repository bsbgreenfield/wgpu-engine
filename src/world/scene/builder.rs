use crate::{
    common::entity::EntityHandle,
    world::{
        scene::{SceneDesc, SceneId, manager::SceneManagerError},
        world::World,
    },
};

pub struct SceneBuilder {
    pub(super) desc: SceneDesc,
}

impl SceneBuilder {
    pub fn new() -> Self {
        Self {
            desc: super::SceneDesc {
                children: vec![],
                entities: Vec::new(),
            },
        }
    }

    pub fn add_child(mut self, child: SceneId) -> Self {
        self.desc.children.push(child);
        self
    }

    pub fn add_entity(mut self, entity: EntityHandle) -> Self {
        self.desc.entities.push(entity);
        self
    }

    pub fn create(self, world: &mut World) -> Result<SceneId, SceneManagerError> {
        world.scene_manager.add_scene(self, &world.entity_manager)
    }
}
