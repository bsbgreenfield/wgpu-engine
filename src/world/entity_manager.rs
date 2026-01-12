use std::{error::Error, fmt::Display, mem::MaybeUninit};

use crate::{
    asset_manager::{
        self,
        asset_manager::{AssetHandle, AssetManager},
    },
    world::components::{ExtractComponents, MeshCollectionComponent},
};

#[derive(Debug)]
pub enum EntityManagerError {
    MaxEntitiesExceeded,
}
impl Display for EntityManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        return f.write_str(&self.to_string());
    }
}
impl Error for EntityManagerError {}

pub struct EntityManager {
    available_ids: Vec<std::ops::Range<u32>>,
    mesh_collections: SparseSet<MeshCollectionComponent, 100>,
}

impl EntityManager {
    pub fn new_entity(&mut self) -> Result<EntityHandle, EntityManagerError> {
        // return the lowest number available
        let first_range = self
            .available_ids
            .first_mut()
            .ok_or(EntityManagerError::MaxEntitiesExceeded)?;
        let res = EntityHandle(first_range.start);
        if first_range.len() > 1 {
            first_range.start = first_range.start + 1;
        } else {
            self.available_ids.remove(0);
        }
        return Ok(res);
    }
    pub fn create_entity_with_components<C: ExtractComponents>(
        asset_manager: &mut AssetManager,
        asset_handle: &AssetHandle,
    ) {
    }
}
#[derive(PartialEq, Eq, PartialOrd, Ord)]
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
