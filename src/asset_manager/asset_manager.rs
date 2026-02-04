use crate::{
    asset_manager::gltf_assets::{
        gltf_loader::loader::{BinarySource, GltfLoadError, GltfLoader},
        model_builder_new::{GltfBuilder, MeshCollectionAssetData, ModelBuilderError},
    },
    util::types::{IndexType, ModelVertex, PNUJWVertex},
};
use std::{
    collections::HashMap,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct AssetHandle {
    id: u32,
}

#[derive(Debug)]
pub enum AssetLoadError {
    Gltf(GltfLoadError),
    AssetNotLoaded,
    AssetNotFound,
    ComponentNotFound,
}

impl From<ModelBuilderError> for AssetLoadError {
    fn from(value: ModelBuilderError) -> Self {
        Self::Gltf(GltfLoadError::ModelBuilderError(Box::new(value)))
    }
}

impl From<GltfLoadError> for AssetLoadError {
    fn from(value: GltfLoadError) -> Self {
        Self::Gltf(value)
    }
}

#[derive(Clone, Copy)]
pub enum AssetResidencyLevel {
    Registered,
    CPU,
    GPU,
}

#[derive(Debug)]
pub struct LoadedAsset {
    mesh_collections: Vec<MeshCollectionAssetData>,
}

impl LoadedAsset {
    pub fn new() -> Self {
        Self {
            mesh_collections: Vec::new(),
        }
    }

    pub fn add_mesh_collections(&mut self, mesh_collections: Vec<MeshCollectionAssetData>) {
        self.mesh_collections.extend(mesh_collections);
    }
}

struct CPUVertexData<V: ModelVertex> {
    vertices: Vec<V>,
}
struct CPUIndexData<I: IndexType> {
    indices: Vec<I>,
}

impl<V: ModelVertex> Deref for CPUVertexData<V> {
    type Target = Vec<V>;
    fn deref(&self) -> &Self::Target {
        &self.vertices
    }
}
impl<V: ModelVertex> DerefMut for CPUVertexData<V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.vertices
    }
}

impl<I: IndexType> Deref for CPUIndexData<I> {
    type Target = Vec<I>;
    fn deref(&self) -> &Self::Target {
        &self.indices
    }
}
impl<I: IndexType> DerefMut for CPUIndexData<I> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.indices
    }
}

impl<V: ModelVertex> CPUVertexData<V> {
    fn new() -> Self {
        Self {
            vertices: Vec::new(),
        }
    }
}
impl<I: IndexType> CPUIndexData<I> {
    fn new() -> Self {
        Self {
            indices: Vec::new(),
        }
    }
}

pub struct AssetManager {
    registered_handles: Vec<AssetHandle>,
    PNUJW_vertex_data: CPUVertexData<PNUJWVertex>,
    U16_index_data: CPUIndexData<u16>,
    registered_assets: HashMap<AssetHandle, RegisteredAsset>,
    loaded_assets: Vec<LoadedAsset>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            registered_handles: Vec::new(),
            PNUJW_vertex_data: CPUVertexData::<PNUJWVertex>::new(),
            U16_index_data: CPUIndexData::<u16>::new(),
            loaded_assets: Vec::new(),
            registered_assets: HashMap::new(),
        }
    }
    fn gen_handle(&self) -> AssetHandle {
        AssetHandle {
            id: self.registered_handles.len() as u32,
        }
    }

    pub fn set_minumum_load_level(&mut self, assets: Vec<AssetHandle>) {
        for asset in assets {
            let (_, ra) = self.registered_assets.remove_entry(&asset).unwrap();
            let new_entry = ra.load_asset(self);
            self.registered_assets.insert(asset, new_entry);
        }
    }

    pub fn register_asset<A: Asset + 'static>(
        &mut self,
        source: &str,
    ) -> Result<AssetHandle, AssetLoadError>
    where
        AssetManager: DataSelector<A::V, A::I>,
    {
        let a = A::new(source)?;
        let handle = self.gen_handle();
        self.registered_assets.insert(
            handle,
            RegisteredAsset {
                residency: AssetResidencyLevel::Registered,
                build: Some(Box::new(move |manager: &mut AssetManager| {
                    a.build(manager);
                })),
            },
        );
        todo!("rest of the function goes here")
    }

    fn data_for<A>(&self) -> (&CPUVertexData<A::V>, &CPUIndexData<A::I>)
    where
        A: Asset,
        AssetManager: DataSelector<A::V, A::I>,
    {
        <Self as DataSelector<A::V, A::I>>::get_data(self)
    }
    fn data_for_mut<A>(&mut self) -> (&mut CPUVertexData<A::V>, &mut CPUIndexData<A::I>)
    where
        A: Asset,
        AssetManager: DataSelector<A::V, A::I>,
    {
        <Self as DataSelector<A::V, A::I>>::get_data_mut(self)
    }
}

struct RegisteredAsset {
    residency: AssetResidencyLevel,
    build: Option<Box<dyn FnOnce(&mut AssetManager)>>,
}
impl RegisteredAsset {
    fn load_asset(self, manager: &mut AssetManager) -> Self {
        (self.build.unwrap())(manager);
        RegisteredAsset {
            residency: AssetResidencyLevel::CPU,
            build: None,
        }
    }
}

pub struct GLTFAsset<V: ModelVertex, I: IndexType> {
    _v: PhantomData<V>,
    _i: PhantomData<I>,
    gltf: gltf::Gltf,
    bin: BinarySource,
}

impl<V: ModelVertex, I: IndexType> Asset for GLTFAsset<V, I> {
    type V = V;
    type I = I;

    fn from(dir_name: &str) -> Self {
        todo!("implement get generic params from the source")
    }
    fn build<'a>(&self, manager: &'a mut AssetManager)
    where
        AssetManager: DataSelector<Self::V, Self::I>,
    {
        let (vertices, indices) = manager.data_for_mut::<Self>();
        Self::load_gltf::<V, I>(&self.gltf, &self.bin, vertices, indices);
    }

    fn new(dir_name: &str) -> Result<Self, AssetLoadError> {
        let (gltf, bin) = GltfLoader::load_gltf_from_resource(dir_name)?;
        Ok(Self {
            _v: PhantomData,
            _i: PhantomData,
            gltf,
            bin,
        })
    }
}

impl<V: ModelVertex, I: IndexType> GltfBuilder for GLTFAsset<V, I> {}

trait Asset {
    type V: ModelVertex;
    type I: IndexType;
    fn from(dir_name: &str) -> Self;
    fn build<'a>(&self, manager: &'a mut AssetManager)
    where
        AssetManager: DataSelector<Self::V, Self::I>;
    fn new(dir_name: &str) -> Result<Self, AssetLoadError>
    where
        Self: Sized;
}
//struct Asset<A: AssetType> {
//    res_level: AssetResidencyLevel,
//    asset_type: A,
//}

//impl<A: AssetType> Asset<A> {
//    pub fn new(dir_name: &str) -> Result<Self, AssetLoadError> {
//        let t = A::new(dir_name)?;
//
//        Ok(Self {
//            res_level: AssetResidencyLevel::Registered,
//            asset_type: t,
//        })
//    }
//}
//
//struct CPUDataMut<'a, V: ModelVertex, I: IndexType> {
//    vertex: &'a mut CPUVertexData<V>,
//    index: &'a mut CPUIndexData<I>,
//}
//struct CPUDataRef<'a, V: ModelVertex, I: IndexType> {
//    vertex: &'a CPUVertexData<V>,
//    index: &'a CPUIndexData<I>,
//}

//impl<'a, V: ModelVertex, I: IndexType> CPUDataRef<'a, V, I> {
//    fn new(vertex: &'a CPUVertexData<V>, index: &'a CPUIndexData<I>) -> Self {
//        Self { vertex, index }
//    }
//}
//impl<'a, V: ModelVertex, I: IndexType> CPUDataMut<'a, V, I> {
//    fn new(vertex: &'a mut CPUVertexData<V>, index: &'a mut CPUIndexData<I>) -> Self {
//        Self { vertex, index }
//    }
//}

trait DataSelector<V: ModelVertex, I: IndexType> {
    fn get_data(&self) -> (&CPUVertexData<V>, &CPUIndexData<I>);
    fn get_data_mut(&mut self) -> (&mut CPUVertexData<V>, &mut CPUIndexData<I>);
}

impl DataSelector<PNUJWVertex, u16> for AssetManager {
    fn get_data(&self) -> (&CPUVertexData<PNUJWVertex>, &CPUIndexData<u16>) {
        (&self.PNUJW_vertex_data, &self.U16_index_data)
    }
    fn get_data_mut(&mut self) -> (&mut CPUVertexData<PNUJWVertex>, &mut CPUIndexData<u16>) {
        (&mut self.PNUJW_vertex_data, &mut self.U16_index_data)
    }
}
