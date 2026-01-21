use crate::{
    asset_manager::gltf_assets::gltf_loader::loader::GltfLoadError,
    util::types::{IndexType, ModelVertex, PNUJWVertex},
};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    ops::Range,
    rc::Rc,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetHandle {
    type_id: TypeId,
    id: u32,
}

#[derive(Debug)]
pub enum AssetLoadError {
    Gltf(GltfLoadError),
    AssetNotLoaded,
    AssetNotFound,
    ComponentNotFound,
}

impl From<GltfLoadError> for AssetLoadError {
    fn from(value: GltfLoadError) -> Self {
        Self::Gltf(value)
    }
}
pub trait Asset {
    type Builder: AssetBuilder;
    type Components;
    fn builder(dir_name: &str) -> Result<Self::Builder, AssetLoadError>;
}

#[derive(Clone, Copy)]
pub enum AssetResidencyLevel {
    Registered,
    CPU,
    GPU,
}

pub trait AssetBuilder {
    fn load_asset(self) -> Result<Box<dyn AssetBuilder>, AssetLoadError>;
    fn get_residency_level(&self) -> AssetResidencyLevel;
    fn get_components(&self) -> Result<LoadedAsset, AssetLoadError>;
}

#[derive(Debug)]
pub struct LoadedAsset {
    components: HashMap<TypeId, Vec<Rc<dyn Any>>>,
}

impl LoadedAsset {
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
        }
    }

    pub fn add_component(&mut self, component: Vec<Rc<dyn Any>>) {
        self.components.insert(component.type_id(), component);
    }

    pub fn get(&self, tid: &TypeId) -> Option<Vec<Rc<dyn Any>>> {
        let a: Option<Vec<Rc<dyn Any>>> = self.components.get(tid).cloned();
        a
    }
}

pub struct MeshPool<V: ModelVertex, I: IndexType> {
    pub cpu: CPUMeshPool<V, I>,
    gpu: GPUMeshBuffers,
}

impl<V: ModelVertex, I: IndexType> MeshPool<V, I> {
    pub fn get_vertices_mut(&mut self) -> &mut Vec<V> {
        &mut self.cpu.vertices
    }
    pub fn get_indices_mut(&mut self) -> &mut Vec<I> {
        &mut self.cpu.indices
    }

    pub fn push_vertices(&mut self, vertices: Vec<V>) {
        self.cpu.vertices.extend(vertices);
    }
    pub fn push_indices(&mut self, index_ranges: &Vec<Range<usize>>, bin: &Vec<u8>) {
        let mut index_vec: Vec<I> = Vec::new();
        for range in index_ranges.iter() {
            let indices_bytes: &[u8] = &bin[range.start..range.end];
            let indices: &[I] = bytemuck::cast_slice::<u8, I>(indices_bytes);
            index_vec.extend(indices.to_vec());
        }
        self.cpu.indices.extend(index_vec);
    }
}
struct CPUMeshPool<V: ModelVertex, I: IndexType> {
    pub vertices: Vec<V>,
    pub indices: Vec<I>,
}

struct GPUMeshBuffers {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

pub struct BufferPool {
    pub PNUJW: MeshPool<PNUJWVertex, u16>,
}

pub struct AssetManager {
    asset_registry: HashMap<u32, Box<dyn AssetBuilder>>,
    asset_data: HashMap<u32, LoadedAsset>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            asset_registry: HashMap::new(),
            asset_data: HashMap::new(),
        }
    }
    pub fn get_builder(&self, asset_handle: &AssetHandle) -> Option<&Box<dyn AssetBuilder>> {
        self.asset_registry.get(&asset_handle.id)
    }
    fn gen_handle<A: Asset + 'static>(&self) -> AssetHandle {
        AssetHandle {
            type_id: TypeId::of::<A>(),
            id: self.asset_registry.len() as u32,
        }
    }

    pub fn get_components_for(
        &mut self,
        asset_handle: &AssetHandle,
    ) -> Result<&LoadedAsset, AssetLoadError> {
        let builder = self
            .asset_registry
            .get(&asset_handle.id)
            .ok_or(AssetLoadError::AssetNotLoaded)?;

        let la = builder.get_components()?;
        self.asset_data.insert(asset_handle.id, la);
        Ok(self.asset_data.get(&asset_handle.id).unwrap())
    }

    fn register_with_asset<A: Asset + 'static>(
        &mut self,
        dir_name: &str,
    ) -> Result<AssetHandle, AssetLoadError>
    where
        A::Builder: AssetBuilder + 'static,
    {
        let builder = A::builder(dir_name)?;
        let handle = self.gen_handle::<A>();
        self.asset_registry.insert(handle.id, Box::new(builder));
        Ok(handle)
    }
}
