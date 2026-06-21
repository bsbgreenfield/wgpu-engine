use std::collections::HashMap;

use crate::{
    app::renderer::{
        BufferType, PrototypeHandle,
        bind_groups::{
            instance_data::InstanceDataBindGroup, local_transforms::LocalTransformBindGroup,
            skinning::SkinningBindGroup,
        },
        gpu_allocator::VertexArenaError,
        renderer::GPUInstanceHandle,
    },
    world::instance_manager::InstanceHandle,
};

pub(super) mod instance_data;
pub(super) mod local_transforms;
pub(super) mod skinning;

pub(super) trait BindGroupProvider {
    fn get_bind_group(&self, alloc_handle: &InstanceHandle) -> &wgpu::BindGroup;
    fn get_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout;
    fn add_bind_group(&mut self, device: &wgpu::Device);
    fn new() -> Self;
}
pub(super) trait SharedInstanceData {
    fn register_shared_binding(
        &mut self,
        slot_index: usize,
        new_handle: &GPUInstanceHandle,
    ) -> Result<u32, VertexArenaError>;

    fn register_copy_binding(
        &mut self,
        slot_index: usize,
        new_handle: &GPUInstanceHandle,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) -> Result<u32, VertexArenaError>;
}
pub struct PrototypeEntry {
    ref_count: usize,
    pub local_transforms_slot: usize,
    pub joint_transforms_slot: Option<usize>,
}

pub struct BindGroupUploadResult {
    pub buffer_offset: u32,
    pub alloc_meta_idx: usize,
}
pub(super) struct BindGroupCollection {
    next_handle: u32,
    pub prototypes: HashMap<PrototypeHandle, PrototypeEntry>,
    pub local_transforms: LocalTransformBindGroup,
    pub skinning: SkinningBindGroup,
    pub instance_data: InstanceDataBindGroup,
}

impl BindGroupCollection {
    pub fn get_slot(&self, prototype: &PrototypeHandle, buffer_type: BufferType) -> usize {
        match buffer_type {
            BufferType::LocalTransform => {
                self.prototypes
                    .get(prototype)
                    .expect("prototype not registered")
                    .local_transforms_slot
            }
            BufferType::JointTransform => self
                .prototypes
                .get(prototype)
                .expect("prototype not registered")
                .joint_transforms_slot
                .expect("joints dont exist for prototype"),
        }
    }
    pub(super) fn gen_gpu_instance_handle(
        &mut self,
        prototype: &PrototypeHandle,
    ) -> GPUInstanceHandle {
        self.next_handle += 1;
        GPUInstanceHandle {
            instance_id: self.next_handle - 1,
            prototype: prototype.clone(),
        }
    }

    pub(super) fn add_prototype(&mut self, prototype: PrototypeHandle) {
        self.prototypes.insert(
            prototype,
            PrototypeEntry {
                ref_count: 1,
                local_transforms_slot: 0,
                joint_transforms_slot: None,
            },
        );
    }

    pub(super) fn new() -> Self {
        Self {
            next_handle: 0,
            prototypes: HashMap::new(),
            local_transforms: LocalTransformBindGroup::new(),
            skinning: SkinningBindGroup::new(),
            instance_data: InstanceDataBindGroup::new(),
        }
    }
}
