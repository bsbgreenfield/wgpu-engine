use std::{
    fmt::{Debug, Display},
    marker::PhantomData,
    ops::Deref,
    path::PathBuf,
    range::Range,
    sync::Arc,
};

use image::DynamicImage;

use crate::{
    animation::EntityAnimationData,
    app::GPUAssetUploadJob,
    asset_manager::{
        asset_manager::AssetManager,
        gltf_asset::{AssetSources, GltfAsset, GltfLoadError, GltfValidationError, TextureSource},
        texture::TextureAsset,
    },
    renderer::GPUAllocationHandle,
    util::types::{GPUMaterialData, InverseBindMatrix, JointTransform, LocalTransform},
    world::{RenderKey, entity_manager::components::ComponentAccessor, scene::SceneLoadLevel},
};

pub mod asset_manager;
pub mod gltf_asset;
pub mod material;
mod range_splicer;
pub mod texture;
#[derive(Debug)]
pub enum AssetLoadError {
    Gltf(GltfLoadError),
    AssetNotLoaded(String),
    AssetNotFound,
    ComponentNotFound,
    NoVertexData,
    InstanceUploadFailure(String),
}

pub struct MeshRenderables {
    pub pnu_vertex_ranges: Option<Vec<Range<u32>>>,
    pub pnu_mesh_map: Vec<u32>,
    pub pnujw_vertex_ranges: Option<Vec<Range<u32>>>,
    pub pnujw_mesh_map: Vec<u32>,
    pub joint_transforms: Option<Vec<JointTransform>>,
    pub joint_map: Vec<u32>,
    pub ibms: Option<Vec<InverseBindMatrix>>,
    pub index_ranges: Option<Vec<Range<u32>>>,
    pub local_transforms: Vec<LocalTransform>,
    pub pnu_materials: Vec<Option<u32>>,
    pub pnujw_materials: Vec<Option<u32>>,
}
impl Display for AssetLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gltf(gltf_error) => {
                write!(f, "Asset Load Failure: {}", gltf_error)
            }
            Self::AssetNotLoaded(s) => write!(f, "The asset is not yet loaded. Message: {}", s),
            Self::AssetNotFound => f.write_str("No such asset exists"),
            Self::ComponentNotFound => {
                f.write_str("The component associated with this asset does not exist")
            }
            Self::NoVertexData => f.write_str("This Asset has no vertices to upload"),
            Self::InstanceUploadFailure(str) => f.write_str(str.as_str()),
        }
    }
}

impl std::error::Error for AssetLoadError {}

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

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct AssetHandle(u32);

#[cfg(test)]
impl AssetHandle {
    pub fn mock(id: u32) -> Self {
        Self(id)
    }
}

impl RenderKey for AssetHandle {
    fn as_key(&self) -> u64 {
        self.0 as u64
    }

    fn from_key(key: u64) -> Self {
        Self(key as u32)
    }
}

pub struct BinaryData {
    buffer_offsets: Vec<usize>,
    data: Vec<u8>,
}
pub enum UnloadedAssetData {
    Gltf {
        sources: AssetSources,
        gltf: gltf::Gltf,
        extenal_textures: Vec<Option<AssetHandle>>,
    },
    Texture(PathBuf),

    #[cfg(test)]
    Mock,
}
impl Debug for UnloadedAssetData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnloadedAssetData::Gltf { .. } => write!(f, "Gltf Asset"),
            UnloadedAssetData::Texture(path) => write!(f, "Texture in path {:?}", path),
            #[cfg(test)]
            UnloadedAssetData::Mock => write!(f, "mock"),
        }
    }
}

struct ExternalResource<A: Asset + AssetSource + ?Sized> {
    path: PathBuf,
    _t: PhantomData<A>,
}

impl UnloadedAssetData {
    fn set_external_paths(&mut self, handles: Vec<Option<AssetHandle>>) {
        if handles.is_empty() {
            return;
        }
        match self {
            UnloadedAssetData::Gltf {
                extenal_textures, ..
            } => *extenal_textures = handles,
            UnloadedAssetData::Texture(_path_buf) => todo!(),
            #[cfg(test)]
            UnloadedAssetData::Mock => {}
        }
    }

    //TODO: either make this generic over A, or add other methods to get other types of external resource
    fn external_textures(&self) -> Vec<Option<ExternalResource<TextureAsset>>> {
        match self {
            Self::Gltf { sources, .. } => {
                let mut res = Vec::<Option<ExternalResource<TextureAsset>>>::new();
                for source in sources.textures.iter() {
                    match source {
                        TextureSource::ExternalFile(path) => {
                            res.push(Some(ExternalResource::<TextureAsset> {
                                path: path.clone(),
                                _t: PhantomData,
                            }));
                        }
                        TextureSource::BinarySource(_) => res.push(None),
                    }
                }
                res
            }
            Self::Texture(_path) => {
                return vec![];
            }
            #[cfg(test)]
            Self::Mock => return vec![],
        }
    }

    fn get_external_asset_deps(&self) -> Option<&[Option<AssetHandle>]> {
        match self {
            UnloadedAssetData::Gltf {
                extenal_textures, ..
            } => Some(&extenal_textures),
            UnloadedAssetData::Texture(_path_buf) => None,
            #[cfg(test)]
            UnloadedAssetData::Mock => None,
        }
    }
    fn load_binary(&self) -> Result<BinaryData, AssetLoadError> {
        match self {
            UnloadedAssetData::Gltf { sources, gltf, .. } => {
                return GltfAsset::load_binary_data(gltf, sources)
                    .map_err(|e| AssetLoadError::Gltf(e));
            }
            UnloadedAssetData::Texture(_) => Ok(BinaryData {
                buffer_offsets: vec![],
                data: vec![],
            }),
            #[cfg(test)]
            UnloadedAssetData::Mock => todo!(),
        }
    }
    fn load(&self, bin: &BinaryData) -> Result<Box<dyn Asset>, ModelBuilderError> {
        match self {
            Self::Gltf {
                sources: _,
                gltf,
                extenal_textures,
            } => GltfAsset::load(gltf, bin, &extenal_textures),
            Self::Texture(path) => TextureAsset::load(path),
            #[cfg(test)]
            Self::Mock => Ok(Box::new(
                crate::asset_manager::asset_manager::asset_mocks::MockAsset,
            )),
        }
    }
}

pub trait AssetSource {
    fn new(dir_name: &str) -> Result<UnloadedAssetData, AssetLoadError>
    where
        Self: Sized;
}

pub enum InternPayload {
    MaterialPayload(Arc<[GPUMaterialData]>),
}

pub trait Asset {
    fn get_upload_job(
        &self,
        asset_handle: AssetHandle,
        asset_manager: &AssetManager,
    ) -> Result<GPUAssetUploadJob, AssetLoadError>;

    fn as_mesh_provider(&self) -> Option<&dyn ProvidesMeshData>;
    fn as_animation_provider(&self) -> Option<&dyn ProvidesAnimationData>;
    fn as_materials_provider(&self) -> Option<&dyn ProvidesMaterialData>;
    fn as_texture_provider(&self) -> Option<&dyn ProvidesTextureData>;
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AssetResidency {
    Registered,
    PendingCPU,
    CPU(usize),
    PendingGPU(usize),
    PendingUnloadGPU(GPUAllocationHandle, usize),
    GPU(GPUAllocationHandle, usize),
}

impl AssetResidency {
    fn update_la_idx(&mut self, new_idx: usize) {
        match self {
            Self::Registered | Self::PendingCPU => {}
            Self::CPU(idx)
            | Self::PendingGPU(idx)
            | Self::GPU(_, idx)
            | Self::PendingUnloadGPU(_, idx) => *idx = new_idx,
        }
    }
}
impl PartialEq<SceneLoadLevel> for AssetResidency {
    fn eq(&self, other: &SceneLoadLevel) -> bool {
        match self {
            AssetResidency::Registered | AssetResidency::PendingCPU => {
                if *other == SceneLoadLevel::NotLoaded {
                    return true;
                }
            }
            AssetResidency::CPU(_) | AssetResidency::PendingGPU(_) => {
                if *other == SceneLoadLevel::CPU {
                    return true;
                }
            }
            AssetResidency::GPU(_, _) | AssetResidency::PendingUnloadGPU(..) => {
                if *other == SceneLoadLevel::GPU {
                    return true;
                }
            }
        }
        return false;
    }
}

impl PartialOrd<SceneLoadLevel> for AssetResidency {
    fn partial_cmp(&self, other: &SceneLoadLevel) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering;
        match self {
            AssetResidency::Registered | AssetResidency::PendingCPU => match other {
                SceneLoadLevel::NotLoaded | SceneLoadLevel::PendingCPU => {
                    return Some(Ordering::Equal);
                }
                SceneLoadLevel::CPU | SceneLoadLevel::GPU | SceneLoadLevel::PendingGPU => {
                    return Some(Ordering::Less);
                }
            },
            AssetResidency::CPU(_) | AssetResidency::PendingGPU(_) => match other {
                SceneLoadLevel::NotLoaded | SceneLoadLevel::PendingCPU => {
                    return Some(Ordering::Greater);
                }
                SceneLoadLevel::CPU | SceneLoadLevel::PendingGPU => return Some(Ordering::Equal),
                SceneLoadLevel::GPU => return Some(Ordering::Less),
            },
            AssetResidency::GPU(_, _) | AssetResidency::PendingUnloadGPU(..) => match other {
                SceneLoadLevel::NotLoaded
                | SceneLoadLevel::CPU
                | SceneLoadLevel::PendingCPU
                | SceneLoadLevel::PendingGPU => return Some(Ordering::Greater),
                SceneLoadLevel::GPU => return Some(Ordering::Equal),
            },
        }
    }
}
#[derive(Debug)]
pub enum ModelBuilderError {
    NodeNotFound(usize),
    MeshNotFound(usize),
    ValidationError(GltfValidationError),
    BinarySourceNotFound,
    IndexRangeError,
}

impl Display for ModelBuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NodeNotFound(node_id) => write!(f, "Node {} not found", node_id),
            Self::MeshNotFound(mesh_id) => write!(f, "Could not resolve mesh {}", mesh_id),
            Self::ValidationError(err) => err.fmt(f),
            Self::BinarySourceNotFound => f.write_str("binary source not found"),
            Self::IndexRangeError => f.write_str("index range out of bounds"),
        }
    }
}

impl std::error::Error for ModelBuilderError {}

impl From<GltfValidationError> for ModelBuilderError {
    fn from(value: GltfValidationError) -> Self {
        Self::ValidationError(value)
    }
}

pub trait ProvidesMeshData: Asset {
    fn render_mesh_data<'a>(&self, mesh_accessor: &'a ComponentAccessor) -> MeshRenderables;
}

pub trait ProvidesAnimationData: Asset {
    fn entity_animation<'a>(
        &self,
        animation_accessor: &ComponentAccessor,
        mesh_accessor: &ComponentAccessor,
    ) -> EntityAnimationData;
}

pub trait ProvidesMaterialData: Asset {
    fn material_palette<'a>(&self, material_accessor: &'a ComponentAccessor) -> Vec<u32>;
}

pub trait ProvidesTextureData: Asset {
    fn texture_data(&self, texture_accessor: &ComponentAccessor) -> DynamicImage;
}

pub struct LoadedAsset<'a> {
    pub asset: &'a Box<dyn Asset>,
    alloc_handle: GPUAllocationHandle,
}

impl<'a> LoadedAsset<'a> {
    pub(crate) fn alloc_handle(&self) -> &GPUAllocationHandle {
        &self.alloc_handle
    }
}

impl<'a> Deref for LoadedAsset<'a> {
    type Target = &'a Box<dyn Asset>;

    fn deref(&self) -> &Self::Target {
        &self.asset
    }
}
