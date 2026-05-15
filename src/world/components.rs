use std::marker::PhantomData;

use crate::{
    animation::animation::EntityAnimations,
    asset_manager::{Asset, AssetHandle, ProvidesAnimationData, ProvidesMeshData},
    world::entity_manager::MeshRenderables,
};

#[derive(Debug, Clone)]
pub enum MeshAcessor {
    GltfRootNode(u32),
    All,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RigidAnimationMode {
    Shared,
    Independent,
}

#[derive(Debug)]
pub struct ResourceBacking<A: Asset + ?Sized> {
    pub asset_handle: AssetHandle,
    _t: PhantomData<A>,
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
    pub rigid_animation_mode: RigidAnimationMode,
}

pub struct MeshCollectionDescriptor {
    pub resource_backing: ResourceBacking<dyn ProvidesMeshData>,
    pub mesh_accessor: MeshAcessor,
    pub animation: Option<AnimationComponent<dyn ProvidesAnimationData>>,
    pub rigid_animation_mode: RigidAnimationMode,
}

impl MeshCollectionDescriptor {
    pub fn new<T: ProvidesMeshData>(
        resource: ResourceBacking<T>,
        mesh_accessor: MeshAcessor,
        rigid: RigidAnimationMode,
    ) -> Self {
        Self {
            mesh_accessor,
            resource_backing: resource.erase(),
            rigid_animation_mode: rigid,
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
    type Output = MeshRenderables;
    type Erased = MeshCollectionComponent<dyn ProvidesMeshData>;

    fn get_output_data(&self, meshed_asset: &A) -> Self::Output {
        meshed_asset.render_mesh_data(&self.mesh_accessor, &self.rigid_animation_mode)
    }
    fn erase(self) -> Self::Erased {
        MeshCollectionComponent {
            mesh_accessor: self.mesh_accessor,
            resource_backing: self.resource_backing.erase(),
            rigid_animation_mode: self.rigid_animation_mode,
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
    pub mesh_accessor: MeshAcessor,
}

pub struct AnimationComponentDescriptor<A: ProvidesAnimationData + ?Sized> {
    pub resource_backing: ResourceBacking<A>,
    pub accessor: AnimationAccessor,
}

impl<A: ProvidesAnimationData> AnimationComponent<A> {
    pub fn from_mesh_component(
        mcc: &MeshCollectionComponent<impl ProvidesMeshData>,
        resource_backing: ResourceBacking<A>,
        animation_accessor: AnimationAccessor,
    ) -> Self {
        Self {
            resource_backing,
            animation_accessor,
            mesh_accessor: mcc.mesh_accessor.clone(),
        }
    }
}

impl<A: ProvidesAnimationData + ?Sized> Component for AnimationComponent<A> {
    type AssetType = A;
    type Output = EntityAnimations;
    type Erased = AnimationComponent<dyn ProvidesAnimationData>;

    fn erase(self) -> Self::Erased {
        AnimationComponent {
            resource_backing: self.resource_backing.erase(),
            animation_accessor: self.animation_accessor,
            mesh_accessor: self.mesh_accessor,
        }
    }

    fn get_output_data(&self, asset: &Self::AssetType) -> Self::Output {
        asset.entity_animation(&self.animation_accessor, &self.mesh_accessor)
    }
}
