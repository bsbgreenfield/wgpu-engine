use std::collections::HashMap;

use crate::{
    renderer::{
        BufferType, PrototypeHandle, StorageData,
        bind_groups::{
            instance_data::InstanceDataBindGroup, local_transforms::LocalTransformBindGroup,
            skinning::SkinningBindGroup,
        },
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
    fn despawn(&mut self, handle: &GPUInstanceHandle);
}

pub(super) trait SharedInstanceData: StorageData {}

#[derive(Debug)]
pub struct PrototypeEntry {
    pub(super) ref_count: usize,
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
                ref_count: 0,
                local_transforms_slot: 0,
                joint_transforms_slot: None,
            },
        );
    }
    pub(super) fn add_prototype_instance(&mut self, prototype: &PrototypeHandle) {
        self.prototypes
            .get_mut(prototype)
            .expect("should be prototype")
            .ref_count += 1;
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
    pub(super) fn despawn(&mut self, handle: &GPUInstanceHandle) {
        let entry = self
            .prototypes
            .get_mut(&handle.prototype)
            .expect("invalid instance");

        entry.ref_count -= 1;

        //if entry.ref_count == 0 {
        //    self.prototypes.remove(&handle.prototype);
        //}

        self.instance_data.despawn(handle);
        self.local_transforms.despawn(handle);
        self.skinning.despawn(handle);
    }
}
