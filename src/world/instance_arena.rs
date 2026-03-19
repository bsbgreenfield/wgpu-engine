use std::clone;

use crate::util::types::GlobalTransform;

struct InstanceData {
    global_transform: GlobalTransform,
}

#[derive(Clone)]
struct InstanceHandle {
    instance_id: u32,
    generation: u32,
}

struct Slot {
    dense_index: u32,
    generation: u32,
}

struct InstanceArena {
    dense: Vec<InstanceData>,
    dense_to_handle: Vec<InstanceHandle>,
    slots: Vec<Slot>,
    free_list: Vec<u32>,
}

impl InstanceArena {
    fn insert(&mut self, data: InstanceData) -> InstanceHandle {
        let slot_index = if let Some(free) = self.free_list.pop() {
            free
        } else {
            self.slots.push(Slot {
                dense_index: 0, // NON INIT
                generation: 0,
            });
            (self.slots.len() - 1) as u32
        };

        let new_dense_index = self.dense.len() as u32;

        self.dense.push(data);
        self.dense_to_handle.push(InstanceHandle {
            instance_id: slot_index,
            generation: self.slots[slot_index as usize].generation,
        });
        self.slots[slot_index as usize].dense_index = new_dense_index;

        self.dense_to_handle[self.dense_to_handle.len() - 1].clone()
    }
    fn remove(&mut self, handle: InstanceHandle) {
        // get the slot indicated by the handle to be removed
        let slot = &mut self.slots[handle.instance_id as usize];

        assert!(slot.generation == handle.generation);

        let old_dense_index = slot.dense_index as usize;
        let new_dense_index = self.dense.len() - 1;

        let instance_id_of_moved = self.dense_to_handle[new_dense_index].instance_id;
        // swap the last handle into the old handles place, remove old handle
        if new_dense_index != old_dense_index {
            self.dense.swap(old_dense_index, new_dense_index);
            self.dense_to_handle.swap(old_dense_index, new_dense_index);
        }

        // swap the actual data, remove the old data
        self.dense.pop();
        self.dense_to_handle.pop();

        // old slot gets invalidated
        slot.generation += 1;
        // old slot index gets marked as free
        self.free_list.push(handle.instance_id);

        // the slot for the moved data (last index) its dense index set to new location
        // which is were the removed data used to be
        self.slots[instance_id_of_moved as usize].dense_index = old_dense_index as u32;
    }

    fn resolve(&self, handle: InstanceHandle) -> Option<u32> {
        let slot = &self.slots[handle.instance_id as usize];

        if slot.generation != handle.generation {
            return None;
        }
        Some(slot.dense_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_transform(val: f32) -> GlobalTransform {
        GlobalTransform {
            transform: [
                [val, 0.0, 0.0, 0.0],
                [0.0, val, 0.0, 0.0],
                [0.0, 0.0, val, 0.0],
                [0.0, 0.0, 0.0, val],
            ],
        }
    }

    fn make_instance(val: f32) -> InstanceData {
        InstanceData {
            global_transform: make_transform(val),
        }
    }
    fn dummy_instance() -> InstanceData {
        InstanceData {
            global_transform: unsafe { std::mem::zeroed() }, // replace if needed
        }
    }

    fn new_arena() -> InstanceArena {
        InstanceArena {
            dense: Vec::new(),
            dense_to_handle: Vec::new(),
            slots: Vec::new(),
            free_list: Vec::new(),
        }
    }

    #[test]
    fn insert_returns_valid_handle() {
        let mut arena = new_arena();

        let handle = arena.insert(dummy_instance());

        let resolved = arena.resolve(handle.clone());
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap(), 0);
    }

    #[test]
    fn multiple_inserts_have_unique_indices() {
        let mut arena = new_arena();

        let h1 = arena.insert(dummy_instance());
        let h2 = arena.insert(dummy_instance());
        let h3 = arena.insert(dummy_instance());

        assert_eq!(arena.resolve(h1).unwrap(), 0);
        assert_eq!(arena.resolve(h2).unwrap(), 1);
        assert_eq!(arena.resolve(h3).unwrap(), 2);
    }

    #[test]
    fn remove_invalidates_handle() {
        let mut arena = new_arena();

        let handle = arena.insert(dummy_instance());
        arena.remove(handle.clone());

        assert!(arena.resolve(handle).is_none());
    }

    #[test]
    fn remove_swaps_dense_correctly() {
        let mut arena = new_arena();

        let h1 = arena.insert(dummy_instance());
        let h2 = arena.insert(dummy_instance());
        let h3 = arena.insert(dummy_instance());

        // Remove middle element
        arena.remove(h2.clone());

        // h2 should be invalid
        assert!(arena.resolve(h2).is_none());

        // h1 should still resolve to index 0
        assert_eq!(arena.resolve(h1).unwrap(), 0);

        // h3 should now occupy index 1 (swapped)
        assert_eq!(arena.resolve(h3).unwrap(), 1);
    }

    #[test]
    fn slot_reuse_uses_free_list() {
        let mut arena = new_arena();

        let h1 = arena.insert(dummy_instance());
        arena.remove(h1.clone());

        let h2 = arena.insert(dummy_instance());

        // Should reuse same slot index
        assert_eq!(h1.instance_id, h2.instance_id);

        // But generation must differ
        assert_ne!(h1.generation, h2.generation);

        // Old handle invalid
        assert!(arena.resolve(h1).is_none());

        // New handle valid
        assert!(arena.resolve(h2).is_some());
    }

    #[test]
    fn remove_last_element_does_not_break() {
        let mut arena = new_arena();

        let h1 = arena.insert(dummy_instance());
        let h2 = arena.insert(dummy_instance());

        arena.remove(h2.clone());

        assert!(arena.resolve(h2).is_none());
        assert_eq!(arena.resolve(h1).unwrap(), 0);
    }

    #[test]
    #[should_panic]
    fn removing_with_wrong_generation_panics() {
        let mut arena = new_arena();

        let mut handle = arena.insert(dummy_instance());
        handle.generation += 1; // corrupt it

        arena.remove(handle);
    }

    #[test]
    fn dense_and_handle_arrays_stay_in_sync() {
        let mut arena = new_arena();

        let handles: Vec<_> = (0..10).map(|_| arena.insert(dummy_instance())).collect();

        // Remove a few
        arena.remove(handles[3].clone());
        arena.remove(handles[7].clone());

        // All remaining valid handles should resolve to valid indices
        for handle in handles {
            if let Some(idx) = arena.resolve(handle.clone()) {
                assert!(idx < arena.dense.len() as u32);
                let mapped_handle = &arena.dense_to_handle[idx as usize];
                assert_eq!(mapped_handle.instance_id, handle.instance_id);
                assert_eq!(mapped_handle.generation, handle.generation);
            }
        }
    }
    #[test]
    fn resolve_returns_correct_transform_after_insert() {
        let mut arena = new_arena();

        let h1 = arena.insert(make_instance(1.0));
        let h2 = arena.insert(make_instance(2.0));
        let h3 = arena.insert(make_instance(3.0));

        let i1 = arena.resolve(h1.clone()).unwrap();
        let i2 = arena.resolve(h2.clone()).unwrap();
        let i3 = arena.resolve(h3.clone()).unwrap();

        assert_eq!(
            arena.dense[i1 as usize].global_transform.transform[0][0],
            1.0
        );
        assert_eq!(
            arena.dense[i2 as usize].global_transform.transform[0][0],
            2.0
        );
        assert_eq!(
            arena.dense[i3 as usize].global_transform.transform[0][0],
            3.0
        );
    }
    #[test]
    fn swap_remove_preserves_correct_transform_mapping() {
        let mut arena = new_arena();

        let h1 = arena.insert(make_instance(1.0));
        let h2 = arena.insert(make_instance(2.0));
        let h3 = arena.insert(make_instance(3.0));

        // Remove middle (h2), causing h3 to move
        arena.remove(h2.clone());

        // h2 should be gone
        assert!(arena.resolve(h2).is_none());

        let i1 = arena.resolve(h1.clone()).unwrap();
        let i3 = arena.resolve(h3.clone()).unwrap();

        // h1 should still point to transform 1.0
        assert_eq!(
            arena.dense[i1 as usize].global_transform.transform[0][0],
            1.0
        );

        // h3 should now point to its original data (3.0), even if moved
        assert_eq!(
            arena.dense[i3 as usize].global_transform.transform[0][0],
            3.0
        );
    }
    #[test]
    fn multiple_removals_keep_transforms_correct() {
        let mut arena = new_arena();

        let handles: Vec<_> = (0..5)
            .map(|i| arena.insert(make_instance(i as f32)))
            .collect();

        // Remove a couple in non-trivial order
        arena.remove(handles[1].clone());
        arena.remove(handles[3].clone());

        for (i, handle) in handles.into_iter().enumerate() {
            if let Some(idx) = arena.resolve(handle.clone()) {
                let expected = i as f32;
                let actual = arena.dense[idx as usize].global_transform.transform[0][0];
                assert_eq!(actual, expected, "handle {} mapped to wrong transform", i);
            }
        }
    }

    #[test]
    fn reuse_slot_keeps_new_transform_correct() {
        let mut arena = new_arena();

        let h1 = arena.insert(make_instance(1.0));
        arena.remove(h1.clone());

        let h2 = arena.insert(make_instance(42.0));

        let idx = arena.resolve(h2.clone()).unwrap();
        let val = arena.dense[idx as usize].global_transform.transform[0][0];

        assert_eq!(val, 42.0);

        // Old handle must not resolve
        assert!(arena.resolve(h1).is_none());
    }

    #[test]
    fn stress_insert_remove_transform_integrity() {
        let mut arena = new_arena();

        let mut handles = Vec::new();

        // Insert 20
        for i in 0..20 {
            handles.push(arena.insert(make_instance(i as f32)));
        }

        // Remove evens
        for i in (0..20).step_by(2) {
            arena.remove(handles[i].clone());
        }

        // Check odds still map correctly
        for i in (0..20).step_by(2).map(|x| x + 1) {
            let handle = handles[i].clone();
            let idx = arena.resolve(handle.clone()).unwrap();
            let val = arena.dense[idx as usize].global_transform.transform[0][0];
            assert_eq!(val, i as f32);
        }
    }
}
