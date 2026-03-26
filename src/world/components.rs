use crate::{
    app::renderer_new::GPUAllocationHandle, asset_manager::asset_manager::AssetHandle,
    util::types::GlobalTransform, world::instance_manager::InstanceManager,
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
pub struct MeshCollectionComponent {
    pub resource_backing: AssetHandle,
    pub allocation_handle: Option<GPUAllocationHandle>,
    mesh_ids: Vec<u32>,
}

pub struct MeshCollectionDescriptor<'a> {
    pub resource_backing: AssetHandle,
    pub allocation_handle: Option<GPUAllocationHandle>,
    pub mesh_ids: &'a [u32],
}

impl MeshCollectionComponent {
    pub fn new(descriptor: MeshCollectionDescriptor) -> Self {
        Self {
            resource_backing: descriptor.resource_backing,
            allocation_handle: descriptor.allocation_handle,
            mesh_ids: descriptor.mesh_ids.to_vec(),
        }
    }
}

pub trait Component {
    type ComponentData: ComponentData;
}

impl Component for PhysicalPositionComponent {
    type ComponentData = GlobalTransform;
}
pub struct PhysicalPositionComponent;

#[derive(Debug)]
pub enum ComponentDataType {
    PhysicalPosition,
    Physics,
    Void,
}

pub trait ComponentData: Sized {
    fn get_data_type() -> ComponentDataType;

    fn get_instance_buffers<'frame>(
        instance_manager: &'frame InstanceManager,
    ) -> Option<(Vec<u16>, Vec<&'frame Self>)> {
        let map: Vec<u16> = Vec::with_capacity(instance_manager.next_id as usize);
        let data: Vec<&'frame Self> = Vec::new();
        Some((map, data))
    }

    fn get_instance_data<'frame>(
        instance_manager: &'frame InstanceManager,
    ) -> Option<(Vec<u16>, Vec<&'frame Self>)>;
}

impl ComponentData for GlobalTransform {
    fn get_data_type() -> ComponentDataType {
        ComponentDataType::PhysicalPosition
    }

    fn get_instance_data<'frame>(
        instance_manager: &'frame InstanceManager,
    ) -> Option<(Vec<u16>, Vec<&'frame Self>)> {
        let mut buffers = Self::get_instance_buffers(instance_manager).unwrap();
        for (i, (pos, handle)) in instance_manager
            .pos
            .positions
            .iter()
            .zip(instance_manager.pos.arena.handles.iter())
            .enumerate()
        {
            buffers.0.insert(handle.global_id as usize, i as u16);
            buffers.1.push(pos);
        }
        return Some((buffers.0, buffers.1));
    }
}
pub struct VoidComponentData {}
impl ComponentData for VoidComponentData {
    fn get_data_type() -> ComponentDataType {
        ComponentDataType::Void
    }
    fn get_instance_buffers<'frame>(
        instance_manager: &'frame InstanceManager,
    ) -> Option<(Vec<u16>, Vec<&'frame Self>)> {
        None
    }
    fn get_instance_data<'frame>(
        instance_manager: &'frame InstanceManager,
    ) -> Option<(Vec<u16>, Vec<&'frame Self>)> {
        None
    }
}

pub struct DummyPhysicsData {
    velocity: u32,
    mass: u32,
}
impl ComponentData for DummyPhysicsData {
    fn get_data_type() -> ComponentDataType {
        ComponentDataType::Physics
    }

    fn get_instance_buffers<'frame>(
        instance_manager: &'frame InstanceManager,
    ) -> Option<(Vec<u16>, Vec<&'frame Self>)> {
        None
    }
    fn get_instance_data<'frame>(
        instance_manager: &'frame InstanceManager,
    ) -> Option<(Vec<u16>, Vec<&'frame Self>)> {
        None
    }
}

pub struct DummyPhysicsComponent;
impl Component for DummyPhysicsComponent {
    type ComponentData = DummyPhysicsData;
}

impl Component for MeshCollectionComponent {
    type ComponentData = VoidComponentData;
}
