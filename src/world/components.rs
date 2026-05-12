use std::marker::PhantomData;

use gltf::json::extensions::mesh::Mesh;

use crate::{
    animation::animation::EntityAnimation,
    app::renderer::GPUAllocationHandle,
    asset_manager_new::{Asset, AssetHandle, ProvidesAnimationData, ProvidesMeshData},
    world::{entity_upload_query::DataRequirement, world::RenderView},
};

#[derive(Debug)]
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
#[derive(Debug)]
pub struct MeshCollectionComponent<A: ProvidesMeshData + ?Sized> {
    pub resource_backing: ResourceBacking<A>,
    pub mesh_accessor: MeshAcessor,
    pub rigid_animation_mode: RigidAnimationMode,
}

pub struct MeshCollectionDescriptor<T: ProvidesMeshData> {
    pub resource_backing: ResourceBacking<T>,
    pub allocation_handle: Option<GPUAllocationHandle>,
    pub mesh_accessor: MeshAcessor,
    pub rigid_animation_mode: RigidAnimationMode,
}

impl<T: ProvidesMeshData> MeshCollectionComponent<T> {
    pub fn new(descriptor: MeshCollectionDescriptor<T>) -> Self {
        Self {
            resource_backing: descriptor.resource_backing,
            mesh_accessor: descriptor.mesh_accessor,
            rigid_animation_mode: descriptor.rigid_animation_mode,
        }
    }
}

pub trait Component {
    type AssetType: Asset + ?Sized;
    type Output;

    fn get_data_requirements<'a>(&'a self, is_instanced: bool) -> Vec<DataRequirement<'a>>;
    fn get_output_data(&self, asset: &Self::AssetType, is_instanced: bool) -> Self::Output;
}

impl<A: ProvidesMeshData + ?Sized> Component for MeshCollectionComponent<A> {
    type AssetType = A;
    type Output = Vec<RenderView>;
    fn get_data_requirements<'a>(&'a self, is_instanced: bool) -> Vec<DataRequirement<'a>> {
        let mut res = Vec::new();
        if !is_instanced {
            res.push(DataRequirement::MeshData(&self.mesh_accessor));
        }
        if !is_instanced || matches!(self.rigid_animation_mode, RigidAnimationMode::Independent) {
            res.push(DataRequirement::LocalTransformData {
                accessor: &self.mesh_accessor,
                mode: &self.rigid_animation_mode,
            });
        }
        res
    }

    fn get_output_data(&self, asset: &A, is_instanced: bool) -> Self::Output {
        if is_instanced {
            return vec![];
        }
        asset.render_view(&self.mesh_accessor, &self.rigid_animation_mode)
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
    mesh_accessor: MeshAcessor,
}

pub struct AnimationComponentDescriptor<A: ProvidesAnimationData> {
    pub resource_backing: ResourceBacking<A>,
    pub accessor: AnimationAccessor,
    pub mesh_accessor: MeshAcessor,
}

impl<A: ProvidesAnimationData> AnimationComponent<A> {
    pub fn new(desciptor: AnimationComponentDescriptor<A>) -> Self {
        Self {
            resource_backing: desciptor.resource_backing,
            animation_accessor: desciptor.accessor,
            mesh_accessor: desciptor.mesh_accessor,
        }
    }
}

impl<A: ProvidesAnimationData> Component for AnimationComponent<A> {
    type AssetType = A;
    type Output = Vec<EntityAnimation>;
    fn get_data_requirements<'a>(&'a self, is_instanced: bool) -> Vec<DataRequirement<'a>> {
        vec![DataRequirement::AnimationData {
            anim_accessor: &self.animation_accessor,
        }]
    }

    fn get_output_data(&self, asset: &Self::AssetType, is_instanced: bool) -> Self::Output {
        asset.entity_animation(&self.animation_accessor, &self.mesh_accessor)
    }
}
