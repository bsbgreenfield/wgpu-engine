use crate::world::entity_manager::{EntityHandle, EntityManager};

pub enum SceneLoadLevel {
    NotLoaded,
    CPU,
    GPU,
}

pub struct Scene {
    entitites: Vec<EntityHandle>,
    dirty: bool,
    load_level: SceneLoadLevel,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            entitites: vec![],
            dirty: false,
            load_level: SceneLoadLevel::NotLoaded,
        }
    }

    pub fn add_entity(&mut self, entity: EntityHandle) {
        self.entitites.push(entity);
    }

    pub fn set_load_level(&mut self, level: SceneLoadLevel) {
        self.load_level = level;
        self.dirty = true;
    }
}
