use std::{fmt::Debug, marker::PhantomData};

use crate::{
    animation::EntityAnimationData,
    asset_manager::{
        Asset, AssetHandle, ProvidesAnimationData, ProvidesMaterialData, ProvidesMeshData,
        ProvidesTextureData,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub enum AnimationMode {
    Shared,
    Independent,
    None,
}

pub struct ResourceBacking<A: Asset + ?Sized> {
    pub asset_handle: AssetHandle,
    _t: PhantomData<A>,
}

impl<A: ProvidesMeshData + 'static> From<ResourceBacking<A>>
    for ResourceBacking<dyn ProvidesMeshData>
{
    fn from(value: ResourceBacking<A>) -> Self {
        ResourceBacking::new(value.asset_handle)
    }
}
impl<A: ProvidesAnimationData + 'static> From<ResourceBacking<A>>
    for ResourceBacking<dyn ProvidesAnimationData>
{
    fn from(value: ResourceBacking<A>) -> Self {
        ResourceBacking::new(value.asset_handle)
    }
}
impl<A: ProvidesMaterialData + 'static> From<ResourceBacking<A>>
    for ResourceBacking<dyn ProvidesMaterialData>
{
    fn from(value: ResourceBacking<A>) -> Self {
        ResourceBacking::new(value.asset_handle)
    }
}
impl<A: ProvidesTextureData + 'static> From<ResourceBacking<A>>
    for ResourceBacking<dyn ProvidesTextureData>
{
    fn from(value: ResourceBacking<A>) -> Self {
        ResourceBacking::new(value.asset_handle)
    }
}

impl<A: Asset + ?Sized> Debug for ResourceBacking<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ResourceBacking with an asset type of {:?} and handle: {:?}",
            self._t, self.asset_handle
        )
    }
}

impl<A: Asset + ?Sized> Clone for ResourceBacking<A> {
    fn clone(&self) -> Self {
        Self {
            asset_handle: self.asset_handle.clone(),
            _t: PhantomData,
        }
    }
}

impl<A: Asset + ?Sized> ResourceBacking<A> {
    pub fn new(handle: AssetHandle) -> Self {
        Self {
            asset_handle: handle,
            _t: PhantomData,
        }
    }

    pub fn erase<T: Asset + ?Sized>(self) -> ResourceBacking<T> {
        ResourceBacking {
            asset_handle: self.asset_handle,
            _t: PhantomData,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ComponentAccessor {
    All,
    Indices(Vec<usize>),
    Index(usize),
}

#[derive(Debug)]
pub struct MeshCollectionComponent<A: ProvidesMeshData + ?Sized> {
    pub resource_backing: ResourceBacking<A>,
    pub mesh_accessor: ComponentAccessor,
}

#[derive(Debug)]
pub struct MeshCollectionDescriptor {
    pub resource_backing: ResourceBacking<dyn ProvidesMeshData>,
    pub mesh_accessor: ComponentAccessor,
    pub animation: Option<AnimationComponent<dyn ProvidesAnimationData>>,
    pub materials: Option<MaterialPalleteComponent<dyn ProvidesMaterialData>>,
}

impl MeshCollectionDescriptor {
    pub fn new(
        resource_backing: ResourceBacking<dyn ProvidesMeshData>,
        accessor: ComponentAccessor,
    ) -> Self {
        Self {
            resource_backing: resource_backing.into(),
            mesh_accessor: accessor,
            animation: None,
            materials: None,
        }
    }
    pub fn with_material(mut self, material_descriptor: MaterialComponentDescriptor) -> Self {
        let resource: ResourceBacking<dyn ProvidesMaterialData> = match material_descriptor {
            MaterialComponentDescriptor::Embedded => self.resource_backing.clone().erase(),
            MaterialComponentDescriptor::External {
                resource_backing: _,
                material_accessor: _,
            } => todo!(),
        };
        let _ = self.materials.insert(MaterialPalleteComponent {
            resource_backing: resource,
            material_accessor: ComponentAccessor::All,
        });
        self
    }

    pub fn with_animation(
        mut self,
        animation_descriptor: AnimationComponentDescriptor<dyn ProvidesAnimationData>,
    ) -> Self {
        match animation_descriptor {
            AnimationComponentDescriptor::Embedded {
                accessor,
                rigid_animation_mode,
                skinned_animation_mode,
            } => {
                self.animation = Some(AnimationComponent {
                    resource_backing: self.resource_backing.clone().erase(),
                    animation_accessor: accessor,
                    rigid_animation_mode,
                    skinned_animation_mode,
                    mesh_accessor: self.mesh_accessor.clone(),
                });
            }
            AnimationComponentDescriptor::External {
                resource_backing,
                accessor,
                rigid_animation_mode,
                skinned_animation_mode,
            } => todo!(),
        }
        self
    }
}

pub trait Component {
    type AssetType: Asset + ?Sized;
    type Output;
    type Erased: Component;

    fn erase(self) -> Self::Erased;
    fn get_output_data(&self, asset: &Self::AssetType) -> Self::Output;
}

impl<A: ProvidesMeshData + ?Sized> Component for MeshCollectionComponent<A> {
    type AssetType = A;
    type Output = crate::asset_manager::MeshRenderables;
    type Erased = MeshCollectionComponent<dyn ProvidesMeshData>;

    fn get_output_data(&self, meshed_asset: &A) -> Self::Output {
        meshed_asset.render_mesh_data(&self.mesh_accessor)
    }
    fn erase(self) -> Self::Erased {
        MeshCollectionComponent {
            mesh_accessor: self.mesh_accessor,
            resource_backing: self.resource_backing.erase(),
        }
    }
}

pub struct AnimationComponent<T: ProvidesAnimationData + ?Sized> {
    pub resource_backing: ResourceBacking<T>,
    pub animation_accessor: ComponentAccessor,
    pub rigid_animation_mode: AnimationMode,
    pub skinned_animation_mode: AnimationMode,
    pub mesh_accessor: ComponentAccessor,
}

impl<T: ProvidesAnimationData + ?Sized> Debug for AnimationComponent<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnimationComponent")
            .field("Animation Accesor", &self.animation_accessor)
            .field("mesh accessor", &self.mesh_accessor)
            .finish()
    }
}

pub enum AnimationComponentDescriptor<A: ProvidesAnimationData + ?Sized> {
    Embedded {
        accessor: ComponentAccessor,
        rigid_animation_mode: AnimationMode,
        skinned_animation_mode: AnimationMode,
    },
    External {
        resource_backing: ResourceBacking<A>,
        accessor: ComponentAccessor,
        rigid_animation_mode: AnimationMode,
        skinned_animation_mode: AnimationMode,
    },
}

impl<A: ProvidesAnimationData + ?Sized> Component for AnimationComponent<A> {
    type AssetType = A;
    type Output = EntityAnimationData;
    type Erased = AnimationComponent<dyn ProvidesAnimationData>;

    fn erase(self) -> Self::Erased {
        AnimationComponent {
            resource_backing: self.resource_backing.erase(),
            animation_accessor: self.animation_accessor,
            mesh_accessor: self.mesh_accessor,
            rigid_animation_mode: self.rigid_animation_mode,
            skinned_animation_mode: self.skinned_animation_mode,
        }
    }

    fn get_output_data(&self, asset: &Self::AssetType) -> Self::Output {
        asset.entity_animation(&self.animation_accessor, &self.mesh_accessor)
    }
}

pub struct MaterialPalleteComponent<T: ProvidesMaterialData + ?Sized> {
    pub resource_backing: ResourceBacking<T>,
    pub material_accessor: ComponentAccessor,
}

pub enum MaterialComponentDescriptor {
    Embedded,
    External {
        material_accessor: ComponentAccessor,
        resource_backing: ResourceBacking<dyn ProvidesMaterialData>,
    },
}

impl<T: ProvidesMaterialData + ?Sized> Debug for MaterialPalleteComponent<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialComponent")
            .field("Material Accessor", &self.material_accessor)
            .finish()
    }
}

impl<M: ProvidesMaterialData + ?Sized> Component for MaterialPalleteComponent<M> {
    type AssetType = M;

    type Output = Vec<u32>;

    type Erased = MaterialPalleteComponent<dyn ProvidesMaterialData>;

    fn erase(self) -> Self::Erased {
        MaterialPalleteComponent {
            resource_backing: self.resource_backing.erase(),
            material_accessor: self.material_accessor,
        }
    }

    fn get_output_data(&self, asset: &Self::AssetType) -> Self::Output {
        asset.material_palette(&self.material_accessor)
    }
}
