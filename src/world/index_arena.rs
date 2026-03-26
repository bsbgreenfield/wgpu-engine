use std::marker::PhantomData;

use crate::world::instance_manager::{Archetype, InstanceHandle};

struct Slot {
    generation: u16,
    dense_index: u16,
}

pub struct InstanceArenaNew<A: Archetype> {
    slots: Vec<Slot>,
    free_list: Vec<u16>,
    pub(super) handles: Vec<InstanceHandle>,
    _t: PhantomData<A>,
}

impl<A: Archetype> InstanceArenaNew<A> {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_list: Vec::new(),
            handles: Vec::new(),
            _t: PhantomData,
        }
    }
    pub fn insert(&mut self, global_id: u16) -> InstanceHandle {
        // select an open slot
        let slot_index = if let Some(free) = self.free_list.pop() {
            free
        } else {
            self.slots.push(Slot {
                generation: 0,
                dense_index: 0, // NON-INIT
            });

            (self.slots.len() - 1) as u16
        };

        let new_handle = InstanceHandle {
            global_id,
            instance_id: slot_index,
            generation: self.slots[slot_index as usize].generation,
            archetype_id: A::id(),
        };
        // push a new handle
        self.handles.push(new_handle.clone());

        // set the handle idx for the selected slot to the be the location of the new handle
        self.slots[slot_index as usize].dense_index = (self.handles.len() - 1) as u16;

        new_handle
    }

    pub fn remove(&mut self, handle: InstanceHandle) -> Option<usize> {
        // get the slot indicated by the handle to be removed
        let slot = &mut self.slots[handle.instance_id as usize];

        assert!(slot.generation == handle.generation);

        let idx_of_goner = slot.dense_index as usize;
        let idx_of_replacement = self.handles.len() - 1;

        let instance_id_of_moved = self.handles[idx_of_replacement].instance_id;

        let res = if idx_of_goner != idx_of_replacement {
            self.handles.swap(idx_of_goner, idx_of_replacement);
            Some(idx_of_goner)
        } else {
            None
        };

        // remove the goner
        self.handles.pop();

        // invalidate the slot for the goner
        slot.generation += 1;

        self.free_list.push(handle.instance_id);

        // the slot for the data that moved adjusts its dense handle idx to match the new location
        self.slots[instance_id_of_moved as usize].dense_index = idx_of_goner as u16;

        res
    }

    pub fn resolve(&self, handle: InstanceHandle) -> Option<usize> {
        let slot = &self.slots[handle.instance_id as usize];

        if slot.generation != handle.generation {
            return None;
        }
        Some(slot.dense_index as usize)
    }
}
