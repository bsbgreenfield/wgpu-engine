use crate::{
    common::{entity::EntityHandle, instance::InstanceHandle},
    world::{
        instance_manager::archetypes::Archetype,
        scene::{SceneEvent, SceneId, SceneLoadLevel},
        world::InstanceUploadData,
    },
};

pub struct Scene {
    pub scene_id: SceneId,
    pub children: Vec<Scene>,
    pub entitites: Vec<EntityHandle>,
    pub instances: Vec<InstanceHandle>,
    dirty: bool,
    pub load_level: SceneLoadLevel,
    pub event_queue: Vec<SceneEvent>,
}
impl Scene {
    #[cfg(test)]
    pub fn new_with_id(id: usize) -> Self {
        Self {
            children: vec![],
            scene_id: SceneId(id),
            entitites: vec![],
            instances: vec![],
            dirty: false,
            load_level: SceneLoadLevel::NotLoaded,
            event_queue: Vec::new(),
        }
    }

    pub fn add_instances(&mut self, instance_upload_data: &InstanceUploadData) {
        self.instances.extend(instance_upload_data.handles());
    }

    pub fn new() -> Self {
        Self {
            children: vec![],
            scene_id: SceneId(0), // TODO: scene ids to keep track of loads, querys, etc??
            entitites: vec![],
            instances: vec![],
            dirty: false,
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

    fn despawn_all(&mut self) {
        todo!()
    }
    pub fn spawn(&mut self, instance_data: Vec<(EntityHandle, Box<dyn Archetype>)>) {
        self.dirty = true;
        self.event_queue.push(SceneEvent::Spawn(instance_data));
        if self.load_level < SceneLoadLevel::GPU {
            self.set_load_level(SceneLoadLevel::GPU);
        }
        self.event_queue.sort();
    }

    pub fn add_entity(&mut self, entity: EntityHandle) {
        self.entitites.push(entity);
    }

    fn set_load_level(&mut self, level: SceneLoadLevel) {
        self.event_queue
            .push(SceneEvent::LoadLevelChanged(self.load_level, level));
        if self.load_level == SceneLoadLevel::GPU && level < self.load_level {
            self.despawn_all();
        }
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
