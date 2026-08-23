use std::collections::HashMap;

use crate::{
    common::entity::EntityHandle,
    world::{
        scene::{SceneDesc, SceneId, SceneLoadLevel, SceneNew},
        world::World,
    },
};

pub struct SceneBuilder {
    pub desc: SceneDesc,
}

impl SceneBuilder {
    pub fn new(world: &mut World) -> Self {
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
        self.desc.entities.push((entity, vec![]));
        self
    }

    pub fn create(self, world: &mut World) -> SceneId {
        world.scene_manager.add_scene(self)
    }
}
