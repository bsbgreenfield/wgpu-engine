use crate::{
    app::renderer::GPUAllocationHandle, asset_manager_new::AssetHandle,
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
    ) -> Option<(Vec<u16>, Vec<&'frame [Self]>)> {
        let map: Vec<u16> = Vec::with_capacity(instance_manager.next_id as usize);
        let data: Vec<&'frame [Self]> = Vec::new();
        Some((map, data))
    }

    // fn get_instance_data<'frame>(
    //     _: &'frame InstanceManager,
    // ) -> Option<(Vec<u16>, Vec<&'frame [Self]>)> {
    //     None
    // }
}

impl ComponentData for GlobalTransform {
    fn get_data_type() -> ComponentDataType {
        ComponentDataType::PhysicalPosition
    }

    // fn get_instance_data<'frame>(
    //     instance_manager: &'frame InstanceManager,
    // ) -> Option<(Vec<u16>, Vec<&'frame [Self]>)> {
    //     let (mut map, mut data_slices) = Self::get_instance_buffers(instance_manager).unwrap();
    //     data_slices.push(&instance_manager.pos.positions[..]);
    //     for (i, handle) in instance_manager.pos.arena.handles.iter().enumerate() {
    //         map.insert(handle.global_id as usize, i as u16);
    //     }
    //     return Some((map, data_slices));
    // }
}
pub struct VoidComponentData {}
impl ComponentData for VoidComponentData {
    fn get_data_type() -> ComponentDataType {
        ComponentDataType::Void
    }
    fn get_instance_buffers<'frame>(
        _: &'frame InstanceManager,
    ) -> Option<(Vec<u16>, Vec<&'frame [Self]>)> {
        None
    }
}

impl Component for MeshCollectionComponent {
    type ComponentData = VoidComponentData;
}
