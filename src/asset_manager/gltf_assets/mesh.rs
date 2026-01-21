use std::ops::Range;

#[derive(Debug)]
pub struct Mesh {
    pub id: u32,
    pub primitives: Vec<Primitive>,
}

#[derive(Debug)]
pub struct Primitive {
    pub vertices: Range<u32>,
    pub indices: Range<u32>,
}

impl Primitive {
    pub fn new(vertices: Range<u32>, indices: Range<u32>) -> Self {
        Self { vertices, indices }
    }
}
