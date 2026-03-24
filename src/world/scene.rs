use crate::world::entity_manager::EntityHandle;

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
    pub load_level: SceneLoadLevel,
    event_queue: Vec<SceneEvent>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            entitites: vec![],
            dirty: false,
            load_level: SceneLoadLevel::NotLoaded,
            event_queue: Vec::new(),
        }
    }

    pub fn add_entity(&mut self, entity: EntityHandle) {
        self.entitites.push(entity);
    }

    pub fn set_load_level(&mut self, level: SceneLoadLevel) {
        self.load_level = level;
        self.dirty = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn pop_event(&mut self) -> Option<SceneEvent> {
        self.event_queue.pop()
    }
}
