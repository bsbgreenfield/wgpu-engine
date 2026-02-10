use std::{any::TypeId, marker::PhantomData, ops::Range};

use crate::util::types::ModelVertex;

#[derive(Debug)]
pub struct Mesh {
    pub id: u32,
    pub primitives: Vec<Primitive>,
}

#[derive(Debug)]
pub struct Primitive {
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
