use std::mem::MaybeUninit;

use crate::world::components::MeshCollectionComponent;

pub struct EntityManager {
    mesh_collections: SparseSet<MeshCollectionComponent, 100>,
}

impl EntityManager {
    pub fn new_entity() -> EntityHandle {
        // add the entity to the list
        todo!()
    }
    pub fn add_component_to(entity_handle: EntityHandle) {
        // add link between entity and component
    }
}

pub struct EntityHandle(u32);
pub struct Entity {
    handle: EntityHandle,
}
const INVALID: usize = usize::MAX;
struct SparseSet<T, const N: usize> {
    dense: [MaybeUninit<T>; N],
    dense_ids: [usize; N],
    sparse: [usize; N],
    len: usize,
}

impl<T, const N: usize> SparseSet<T, N> {
    pub fn new() -> Self {
        Self {
            dense: unsafe { MaybeUninit::uninit().assume_init() },
            dense_ids: [INVALID; N],
            sparse: [INVALID; N],
            len: 0,
        }
    }
}
