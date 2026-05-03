use crate::{
    app::renderer::GPUAllocationHandle, asset_manager_new::AssetHandle, world::InstanceUploadQuery,
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
}

pub enum AnimationAccessor {
    All,
    Index(usize),
}

pub struct AnimationComponent {
    resource_backing: AssetHandle,
    animation_accessor: AnimationAccessor,
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
        todo!()
    }
}
