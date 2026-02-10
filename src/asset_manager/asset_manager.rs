use crate::{
    asset_manager::gltf_assets::{
        gltf_loader::loader::{BinarySource, GltfLoadError, GltfLoader},
        model_builder_new::{GltfBuilder, GltfLoadResult, ModelBuilderError},
    },
    util::types::{IndexType, ModelVertex, PNUJWVertex, PNUVertex},
    world::scene::SceneLoadLevel,
};
use std::{
    any::TypeId,
    collections::HashMap,
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

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub enum AssetResidencyLevel {
    Registered,
    CPU,
    GPU,
}
impl PartialEq<SceneLoadLevel> for AssetResidencyLevel {
    fn eq(&self, other: &SceneLoadLevel) -> bool {
        match self {
            AssetResidencyLevel::Registered => {
                if *other == SceneLoadLevel::NotLoaded {
                    return true;
                }
            }
            AssetResidencyLevel::CPU => {
                if *other == SceneLoadLevel::CPU {
                    return true;
                }
            }
            AssetResidencyLevel::GPU => {
                if *other == SceneLoadLevel::GPU {
                    return true;
                }
            }
        }
        return false;
    }
}

impl PartialOrd<SceneLoadLevel> for AssetResidencyLevel {
    fn partial_cmp(&self, other: &SceneLoadLevel) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering;
        match self {
            AssetResidencyLevel::Registered => match other {
                SceneLoadLevel::NotLoaded => return Some(Ordering::Equal),
                SceneLoadLevel::CPU | SceneLoadLevel::GPU => return Some(Ordering::Less),
            },
            AssetResidencyLevel::CPU => match other {
                SceneLoadLevel::NotLoaded => return Some(Ordering::Greater),
                SceneLoadLevel::CPU => return Some(Ordering::Equal),
                SceneLoadLevel::GPU => return Some(Ordering::Less),
            },
            AssetResidencyLevel::GPU => match other {
                SceneLoadLevel::NotLoaded | SceneLoadLevel::CPU => return Some(Ordering::Greater),
                SceneLoadLevel::GPU => return Some(Ordering::Equal),
            },
        }
    }
}

type LoadedMeshData<'la, V: ModelVertex> = Vec<Vec<&'la [V]>>;
#[derive(Debug)]
pub struct LoadedAsset<'la> {
    pnuj_mesh_data: LoadedMeshData<'la, PNUJWVertex>,
    pnu_mesh_data: LoadedMeshData<'la, PNUVertex>,
}

impl<'am, 'la> LoadedAsset<'la>
where
    'am: 'la,
{
    pub fn load_gltf_data(asset_manager: &'am mut AssetManager, gltf_data: GltfLoadResult) -> Self {
        let pnujw_offset = asset_manager.pnujw_vertex_data.len();
        let pnu_offset = asset_manager.pnu_vertex_data.len();
        asset_manager
            .pnujw_vertex_data
            .extend(gltf_data.pnujw_vertices);
        asset_manager.pnu_vertex_data.extend(gltf_data.pnu_vertices);
        for model in gltf_data.mesh_data.iter() {
            // TODO: deal with local transorms
            let mut slices_vec_pnujw = Vec::<Vec<&[PNUJWVertex]>>::new();
            let mut slices_vec_pnu = Vec::<Vec<&[PNUVertex]>>::new();
            for mesh in model.meshes.iter() {
                let mut slices_pnujw = Vec::<&[PNUJWVertex]>::new();
                let mut slices_pnu = Vec::<&[PNUVertex]>::new();
                for primitive in mesh.primitives.iter() {
                    if primitive.vertex_type == TypeId::of::<PNUJWVertex>() {
                        slices_pnujw.push(
                            &asset_manager.pnujw_vertex_data[pnujw_offset
                                + primitive.vertices.start as usize
                                ..pnujw_offset + primitive.vertices.end as usize],
                        );
                    } else {
                        slices_pnu.push(
                            &asset_manager.pnu_vertex_data[pnu_offset
                                + primitive.vertices.start as usize
                                ..pnujw_offset + primitive.vertices.end as usize],
                        );
                    }
                }
            }
        }
        todo!()
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

pub struct AssetManager<'am> {
    gpu_upload_queue: Vec<AssetHandle>,
    registered_handles: Vec<AssetHandle>,
    pnujw_vertex_data: CPUVertexData<PNUJWVertex>,
    pnu_vertex_data: CPUVertexData<PNUVertex>,
    u16_index_data: CPUIndexData<u16>,
    registered_assets: HashMap<AssetHandle, Box<dyn AssetNew<BuildOutput = GltfLoadResult>>>,
    loaded_assets: Vec<LoadedAsset<'am>>,
}

impl<'am> AssetManager<'am> {
    pub fn new() -> Self {
        Self {
            gpu_upload_queue: Vec::new(),
            registered_handles: Vec::new(),
            pnujw_vertex_data: CPUVertexData::<PNUJWVertex>::new(),
            pnu_vertex_data: CPUVertexData::<PNUVertex>::new(),
            u16_index_data: CPUIndexData::<u16>::new(),
            loaded_assets: Vec::new(),
            registered_assets: HashMap::new(),
        }
    }
    fn gen_handle(&self) -> AssetHandle {
        AssetHandle {
            id: self.registered_handles.len() as u32,
        }
    }

    pub fn set_minumum_load_level(
        &mut self,
        assets: Vec<AssetHandle>,
        load_level: SceneLoadLevel,
    ) -> Result<(), AssetLoadError> {
        for asset in assets {
            let asset_residency = self
                .registered_assets
                .get(&asset)
                .ok_or(AssetLoadError::AssetNotFound)?
                .get_residency_level();

            if asset_residency < load_level {
                if load_level == SceneLoadLevel::GPU
                    && asset_residency == AssetResidencyLevel::Registered
                {
                    let (_, mut registered_asset) =
                        self.registered_assets.remove_entry(&asset).unwrap();
                    let load_result = registered_asset.load_asset()?;
                    registered_asset.set_residency_level(AssetResidencyLevel::CPU);
                    self.registered_assets.insert(asset, registered_asset);
                }
            }
        }
        Ok(())
    }

    pub fn register_asset<A: AssetNew + 'static>(
        &mut self,
        source: &str,
    ) -> Result<AssetHandle, AssetLoadError> {
        let a = A::new(source)?;
        let handle = self.gen_handle();
        todo!("rest of the function goes here")
    }

    // fn data_for<A>(&self) -> (&CPUVertexData<A::V>, &CPUIndexData<A::I>)
    // where
    //     A: Asset,
    //     AssetManager: DataSelector<A::V, A::I>,
    // {
    //     <Self as DataSelector<A::V, A::I>>::get_data(self)
    // }
    // fn data_for_mut<A>(&mut self) -> (&mut CPUVertexData<A::V>, &mut CPUIndexData<A::I>)
    // where
    //     A: Asset,
    //     AssetManager: DataSelector<A::V, A::I>,
    // {
    //     <Self as DataSelector<A::V, A::I>>::get_data_mut(self)
    // }
}

pub trait AssetNew {
    type BuildOutput;
    fn new(dir_name: &str) -> Result<Self, AssetLoadError>
    where
        Self: Sized;
    fn load_asset(&self) -> Result<Self::BuildOutput, AssetLoadError>;
    fn get_residency_level(&self) -> AssetResidencyLevel;
    fn set_residency_level(&mut self, level: AssetResidencyLevel);
}

pub struct GltfAsset {
    gltf: gltf::Gltf,
    bin: BinarySource,
    res_level: AssetResidencyLevel,
}
impl GltfBuilder for GltfAsset {}

impl AssetNew for GltfAsset {
    type BuildOutput = GltfLoadResult;
    fn get_residency_level(&self) -> AssetResidencyLevel {
        self.res_level
    }
    fn set_residency_level(&mut self, level: AssetResidencyLevel) {
        self.res_level = level;
    }
    fn new(dir_name: &str) -> Result<Self, AssetLoadError>
    where
        Self: Sized,
    {
        let (gltf, bin) = GltfLoader::load_gltf_from_resource(dir_name)?;
        Ok(Self {
            gltf,
            bin,
            res_level: AssetResidencyLevel::Registered,
        })
    }

    fn load_asset(&self) -> Result<Self::BuildOutput, AssetLoadError> {
        self.load_asset()
    }
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

//pub trait DataSelector<V: ModelVertex, I: IndexType> {
//    fn get_data(&self) -> (&CPUVertexData<V>, &CPUIndexData<I>);
//    fn get_data_mut(&mut self) -> (&mut CPUVertexData<V>, &mut CPUIndexData<I>);
//}
//
//impl DataSelector<PNUJWVertex, u16> for AssetManager {
//    fn get_data(&self) -> (&CPUVertexData<PNUJWVertex>, &CPUIndexData<u16>) {
//        (&self.PNUJW_vertex_data, &self.U16_index_data)
//    }
//    fn get_data_mut(&mut self) -> (&mut CPUVertexData<PNUJWVertex>, &mut CPUIndexData<u16>) {
//        (&mut self.PNUJW_vertex_data, &mut self.U16_index_data)
//    }
//}
