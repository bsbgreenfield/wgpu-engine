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
    dense_handle_idx: u16,
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
        // select an open slot
        let slot_index = if let Some(free) = self.free_list.pop() {
            free
        } else {
            self.slots.push(Slot {
                archetype: data.id(),
                generation: 0,
                a_table_index: 0,    // NON-INIT
                dense_handle_idx: 0, // NON-INIT
            });

            (self.slots.len() - 1) as u16
        };

        let new_handle = InstanceHandle {
            instance_id: slot_index,
            generation: self.slots[slot_index as usize].generation,
        };
        // push a new handle
        self.handles.push(new_handle.clone());

        // set the handle idx for the selected slot to the be the location of the new handle
        self.slots[slot_index as usize].dense_handle_idx = (self.handles.len() - 1) as u16;

        new_handle
    }

    pub fn remove(&mut self, handle: InstanceHandle) {
        // get the slot indicated by the handle to be removed
        let slot = &mut self.slots[handle.instance_id as usize];

        assert!(slot.generation == handle.generation);

        let idx_of_goner = slot.dense_handle_idx as usize;
        let idx_of_replacement = self.handles.len() - 1;

        let instance_id_of_moved = self.handles[idx_of_replacement].instance_id;

        if idx_of_goner != idx_of_replacement {
            self.handles.swap(idx_of_goner, idx_of_replacement);
        }

        // remove the goner
        self.handles.pop();

        // invalidate the slot for the goner
        slot.generation += 1;

        self.free_list.push(handle.instance_id);

        // the slot for the data that moved adjusts its dense handle idx to match the new location
        self.slots[instance_id_of_moved as usize].dense_handle_idx = idx_of_goner as u16;
    }

    pub fn resolve(&self, handle: InstanceHandle) -> Option<(ArchetypeId, usize)> {
        let slot = &self.slots[handle.instance_id as usize];

        if slot.generation != handle.generation {
            return None;
        }
        Some((slot.archetype, slot.a_table_index as usize))
    }
}
