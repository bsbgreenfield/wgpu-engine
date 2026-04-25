use crate::{
    app::renderer::GPUAllocationHandle,
    asset_manager_new::{AssetHandle, LoadableAsset},
    util::types::{GlobalTransform, LocalTransform},
    world::{InstanceUploadQuery, entity_manager::Renderables, instance_manager::InstanceManager},
};

#[derive(Debug)]
pub struct ResourceBacking {
    pub asset_handle: AssetHandle,
    pub resource_index: u8,
}

impl ResourceBacking {
    pub fn new(asset_handle: AssetHandle, resource_index: u8) -> Self {
        Self {
            asset_handle,
            resource_index,
        }
    }
}

#[derive(Debug)]
pub enum MeshAcessor {
    GltfRootNode(u32),
    All,
}

#[derive(Debug)]
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
    fn modify_query<'a>(&'a self, query: &mut InstanceUploadQuery<'a>);
}

impl Component for MeshCollectionComponent {
    fn modify_query<'a>(&'a self, query: &mut InstanceUploadQuery<'a>) {
        query.mesh_accesor = Some(&self.mesh_accessor)
    }
}
