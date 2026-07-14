use std::{fmt::Debug, marker::PhantomData};

use crate::{
    animation::animation::EntityAnimationData,
    asset_manager::{Asset, AssetHandle, ProvidesAnimationData, ProvidesMeshData},
};

#[derive(Debug, Clone)]
pub enum MeshAcessor {
    GltfRootNode(u32),
    All,
}

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
#[derive(Debug)]
pub struct MeshCollectionComponent<A: ProvidesMeshData + ?Sized> {
    pub resource_backing: ResourceBacking<A>,
    pub mesh_accessor: MeshAcessor,
}

#[derive(Debug)]
pub struct MeshCollectionDescriptor {
    pub resource_backing: ResourceBacking<dyn ProvidesMeshData>,
    pub mesh_accessor: MeshAcessor,
    pub animation: Option<AnimationComponent<dyn ProvidesAnimationData>>,
}

impl MeshCollectionDescriptor {
    pub fn new<T: ProvidesMeshData>(
        resource: ResourceBacking<T>,
        mesh_accessor: MeshAcessor,
    ) -> Self {
        Self {
            mesh_accessor,
            resource_backing: resource.erase(),
            animation: None,
        }
    }

    pub fn with_animation<T: ProvidesAnimationData + 'static>(
        mut self,
        desc: AnimationComponentDescriptor<T>,
    ) -> Self {
        self.animation = Some(AnimationComponent {
            resource_backing: desc.resource_backing.erase(),
            animation_accessor: desc.accessor,
            rigid_animation_mode: desc.rigid_animation_mode,
            skinned_animation_mode: desc.skinned_animation_mode,
            mesh_accessor: self.mesh_accessor.clone(),
        });
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

#[derive(Debug)]
pub enum AnimationAccessor {
    All,
    Index(usize),
}

pub struct AnimationComponent<T: ProvidesAnimationData + ?Sized> {
    pub resource_backing: ResourceBacking<T>,
    pub animation_accessor: AnimationAccessor,
    pub rigid_animation_mode: AnimationMode,
    pub skinned_animation_mode: AnimationMode,

    pub mesh_accessor: MeshAcessor,
}

impl<T: ProvidesAnimationData + ?Sized> Debug for AnimationComponent<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnimationComponent")
            .field("Animation Accesor", &self.animation_accessor)
            .field("mesh accessor", &self.mesh_accessor)
            .finish()
    }
}

pub struct AnimationComponentDescriptor<A: ProvidesAnimationData + ?Sized> {
    pub resource_backing: ResourceBacking<A>,
    pub accessor: AnimationAccessor,
    pub rigid_animation_mode: AnimationMode,
    pub skinned_animation_mode: AnimationMode,
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
