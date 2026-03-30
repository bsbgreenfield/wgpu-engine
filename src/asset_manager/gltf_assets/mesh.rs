use std::{any::TypeId, ops::Range};

use crate::util::types::ModelVertex;

#[derive(Debug)]
pub(super) struct Mesh {
    pub id: u32,
    pub primitives: Vec<Primitive>,
}

#[derive(Debug)]
pub(super) struct Primitive {
    pub vertex_type: TypeId,
    pub vertices: Range<u32>,
    pub indices: Range<u32>,
}

impl Primitive {
    pub fn new<V: ModelVertex>(vertices: Range<u32>, indices: Range<u32>) -> Self {
        Self {
            vertices,
            indices,
            vertex_type: TypeId::of::<V>(),
        }
    }
}
