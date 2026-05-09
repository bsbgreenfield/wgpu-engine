use std::marker::PhantomData;

use crate::{
    app::renderer::GPUAllocationHandle,
    asset_manager_new::{AssetHandle, LoadedAsset, ProvidesMeshData},
    world::{
        InstanceUploadQuery,
        entity_upload_query::{DataRequirement, InstanceUploadQueryNew},
    },
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
pub struct MeshCollectionComponent {
    pub resource_backing: AssetHandle,
    pub mesh_accessor: MeshAcessor,
    pub rigid_animation_mode: RigidAnimationMode,
}

pub struct MeshCollectionDescriptor {
    pub resource_backing: AssetHandle,
    pub allocation_handle: Option<GPUAllocationHandle>,
    pub mesh_accessor: MeshAcessor,
    pub rigid_animation_mode: RigidAnimationMode,
}

impl MeshCollectionComponent {
    pub fn new(descriptor: MeshCollectionDescriptor) -> Self {
        Self {
            resource_backing: descriptor.resource_backing,
            mesh_accessor: descriptor.mesh_accessor,
            rigid_animation_mode: descriptor.rigid_animation_mode,
        }
    }
}

pub trait Component {
    fn modify_query<'a>(&'a self, query: &mut InstanceUploadQuery<'a>, is_instanced: bool);
    fn get_data_requirements<'a>(&'a self, is_instanced: bool) -> Vec<DataRequirement<'a>>;
}

impl Component for MeshCollectionComponent {
    fn modify_query<'a>(&'a self, query: &mut InstanceUploadQuery<'a>, is_instanced: bool) {
        if is_instanced {
            if matches!(self.rigid_animation_mode, RigidAnimationMode::Independent) {
                query.needs_local_transforms = true;
                query.mesh_accesor = Some(&self.mesh_accessor);
            }
        } else {
            query.needs_meshes = true;
            query.needs_local_transforms = true;
            query.mesh_accesor = Some(&self.mesh_accessor);
        }
        query.rigid_animation_mode = Some(&self.rigid_animation_mode)
    }

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
}

#[derive(Debug)]
pub enum AnimationAccessor {
    All,
    Index(usize),
}

pub struct AnimationComponent {
    pub resource_backing: AssetHandle,
    pub animation_accessor: AnimationAccessor,
}

pub struct AnimationComponentDescriptor {
    pub resource_backing: AssetHandle,
    pub accessor: AnimationAccessor,
}

impl AnimationComponent {
    pub fn new(desciptor: AnimationComponentDescriptor) -> Self {
        Self {
            resource_backing: desciptor.resource_backing,
            animation_accessor: desciptor.accessor,
        }
    }
}

impl Component for AnimationComponent {
    fn modify_query<'a>(&'a self, query: &mut InstanceUploadQuery<'a>, is_instanced: bool) {
        query.needs_animations = true;
        query.animation_accessor = Some(&self.animation_accessor);
    }
    fn get_data_requirements<'a>(&'a self, is_instanced: bool) -> Vec<DataRequirement<'a>> {
        vec![DataRequirement::AnimationData {
            anim_accessor: &self.animation_accessor,
        }]
    }
}
struct ResourceBacking<A: LoadedAsset + ?Sized> {
    asset_handle: AssetHandle,
    _t: PhantomData<A>,
}

pub struct TestMeshComponent<A: ProvidesMeshData + ?Sized> {
    resource_backing: ResourceBacking<A>,
    mesh_accessor: MeshAcessor,
    rigid_mode: RigidAnimationMode,
}

pub struct TestEntityManager {
    meshes: Vec<TestMeshComponent<dyn ProvidesMeshData>>,
}
