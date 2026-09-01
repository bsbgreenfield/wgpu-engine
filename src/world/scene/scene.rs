use crate::{
    common::entity::EntityHandle,
    world::{
        instance_manager::archetypes::{APosition, Archetype},
        scene::{SceneEvent, SceneId, SceneLoadLevel},
    },
};

pub struct Spawn<T: Archetype + ?Sized> {
    pub entity: EntityHandle,
    pub data: Box<T>,
}

impl From<(EntityHandle, Box<APosition>)> for Spawn<dyn Archetype> {
    fn from(value: (EntityHandle, Box<APosition>)) -> Self {
        Self {
            entity: value.0,
            data: value.1,
        }
    }
}

//TODO: move archetype data to runtime?
pub(super) struct SceneDesc {
    pub(super) children: Vec<SceneId>,
    pub(super) entities: Vec<EntityHandle>,
}

#[allow(unused)]
#[derive(Default)]
pub(super) struct SceneRuntime {
    pub(super) current_state: SceneLoadLevel,
    pub(super) requested_level: SceneLoadLevel,
    pub(super) event_queue: Vec<SceneEvent>,
    pub(super) spawn_queue: Vec<Spawn<dyn Archetype>>,
}

impl SceneRuntime {
    pub(super) fn ready_to_spawn(&self) -> bool {
        self.current_state == SceneLoadLevel::GPU
    }
}
