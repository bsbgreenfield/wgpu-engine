use crate::world::{instance_arena::InstanceHandle, instance_manager::InstanceManager};

pub trait Archetype {
    fn id(&self) -> ArchetypeId;

    fn insert_self(self, table: &mut InstanceManager, handle: InstanceHandle) -> u16;
}

#[derive(Debug, Clone, Copy)]
pub enum ArchetypeId {
    Position = 0,
}

struct Slot {
    archetype: ArchetypeId,
    generation: u16,
    a_table_index: u16,
}

#[derive(Default)]
pub struct InstanceArenaNew {
    slots: Vec<Slot>,
    free_list: Vec<u16>,
    handles: Vec<InstanceHandle>,
}

impl InstanceArenaNew {
    #[inline]
    pub fn set_index(&mut self, index: u16, id: u16) {
        self.slots[id as usize].a_table_index = index;
    }
    pub fn insert(&mut self, data: &impl Archetype) -> InstanceHandle {
        let slot_index = if let Some(free) = self.free_list.pop() {
            free
        } else {
            self.slots.push(Slot {
                archetype: data.id(),
                generation: 0,
                a_table_index: 0, // TODO
            });

            (self.slots.len() - 1) as u16
        };

        self.handles.push(InstanceHandle {
            instance_id: slot_index,
            generation: self.slots[slot_index as usize].generation,
        });

        self.slots[slot_index as usize].a_table_index = todo!("GET Archetype table index");
        self.handles[self.handles.len() - 1].clone()
    }

    pub fn remove(&mut self, handle: InstanceHandle) {
        // get the slot indicated by the handle to be removed
        let slot = &mut self.slots[handle.instance_id as usize];

        assert!(slot.generation == handle.generation);
    }
}
