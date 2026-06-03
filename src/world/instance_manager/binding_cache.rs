use std::collections::HashMap;

use crate::{
    app::renderer::GPUInstanceHandle,
    world::{
        entity_manager::{EntityHandle, components::AnimationMode},
        instance_manager::InstanceHandle,
        world::RenderGroupEntry,
    },
};

enum InstanceGPUResidency {
    Shared(GPUInstanceHandle),
    Independant(HashMap<InstanceHandle, GPUInstanceHandle>),
    None,
    Pending,
}

struct EntityRegistration {
    local_transform: InstanceGPUResidency,
    joint_transform: InstanceGPUResidency,
}

struct PendingInstanceRegistration {
    local_transforms: RegistrationType,
    joint_transforms: RegistrationType,
}
enum RegistrationType {
    NewShared,
    NewIndependant,
    None,
}

struct GPUInstanceCacheNew {
    pending: HashMap<InstanceHandle, PendingInstanceRegistration>,
    lt_generation: u32,
    jt_generation: u32,
    entity_register: HashMap<EntityHandle, EntityRegistration>,
    binding_map: HashMap<GPUInstanceHandle, usize>,
    cached_offsets: Vec<u32>,
}

impl GPUInstanceCacheNew {
    fn spawn(
        &mut self,
        instance_handle: InstanceHandle,
        rigid_mode: AnimationMode,
        joint_mode: AnimationMode,
    ) {
        if self
            .entity_register
            .get(&instance_handle.entity_handle)
            .is_some()
        {
            let new_joint_reg = match joint_mode {
                AnimationMode::Independent => RegistrationType::NewIndependant,
                AnimationMode::Shared => RegistrationType::None,
            };
            let new_lt_reg = match rigid_mode {
                AnimationMode::Independent => RegistrationType::NewIndependant,
                AnimationMode::Shared => RegistrationType::None,
            };

            self.pending.insert(
                instance_handle.clone(),
                PendingInstanceRegistration {
                    local_transforms: new_lt_reg,
                    joint_transforms: new_joint_reg,
                },
            );
        } else {
            let joint_reg = match joint_mode {
                AnimationMode::Shared => RegistrationType::NewShared,
                AnimationMode::Independent => RegistrationType::NewIndependant,
            };
            let lt_reg = match rigid_mode {
                AnimationMode::Shared => RegistrationType::NewShared,
                AnimationMode::Independent => RegistrationType::NewIndependant,
            };
            self.pending.insert(
                instance_handle.clone(),
                PendingInstanceRegistration {
                    local_transforms: lt_reg,
                    joint_transforms: joint_reg,
                },
            );

            self.entity_register.insert(
                instance_handle.entity_handle.clone(),
                EntityRegistration {
                    local_transform: InstanceGPUResidency::Pending,
                    joint_transform: InstanceGPUResidency::Pending,
                },
            );
        }
    }

    fn cache_offset(&mut self, gpu_handle: GPUInstanceHandle, offset_val: u32) {
        self.cached_offsets.push(offset_val);
        self.binding_map
            .insert(gpu_handle, self.cached_offsets.len() - 1);
    }

    fn register_instance(
        &mut self,
        instance_handle: InstanceHandle,
        lt_handle: Option<GPUInstanceHandle>,
        lt_offset: Option<u32>,
        joint_handle: Option<GPUInstanceHandle>,
        jt_offset: Option<u32>,
        relative_instance_idx: u32,
    ) -> RenderGroupEntry {
        let pending = self
            .pending
            .remove(&instance_handle)
            .expect("must be a pending instance");
        {
            if let Some(ref lt_handle) = lt_handle {
                self.cache_offset(lt_handle.clone(), lt_offset.expect("offset"));
            }
            if let Some(ref jt_handle) = joint_handle {
                self.cache_offset(jt_handle.clone(), jt_offset.expect("offset"));
            }
        }
        let entity_reg = self
            .entity_register
            .get_mut(&instance_handle.entity_handle)
            .expect("should be registered");
        let is_new = matches!(entity_reg.local_transform, InstanceGPUResidency::Pending);
        if is_new {
            entity_reg.local_transform = match pending.local_transforms {
                RegistrationType::NewShared => {
                    InstanceGPUResidency::Shared(lt_handle.expect("needs lt handle"))
                }
                RegistrationType::NewIndependant => {
                    let mut map = HashMap::new();
                    map.insert(instance_handle.clone(), lt_handle.expect("needs lt handle"));
                    InstanceGPUResidency::Independant(map)
                }
                RegistrationType::None => panic!("instances must have lt data"),
            };
            entity_reg.joint_transform = match pending.joint_transforms {
                RegistrationType::NewShared => {
                    InstanceGPUResidency::Shared(joint_handle.expect("needs joint handle"))
                }
                RegistrationType::NewIndependant => {
                    let mut map = HashMap::new();
                    map.insert(
                        instance_handle.clone(),
                        joint_handle.expect("needs joint handle"),
                    );
                    InstanceGPUResidency::Independant(map)
                }
                RegistrationType::None => InstanceGPUResidency::None,
            }
        } else {
            match pending.local_transforms {
                RegistrationType::NewShared => {
                    panic!("entity was already registered with shared binding for lt")
                }
                RegistrationType::NewIndependant => {
                    let InstanceGPUResidency::Independant(map) = &mut entity_reg.local_transform
                    else {
                        panic!("should have a registered lt map");
                    };
                    map.insert(instance_handle.clone(), lt_handle.expect("needs lt handle"));
                }
                RegistrationType::None => assert!(lt_handle.is_none()),
            }
            match pending.joint_transforms {
                RegistrationType::NewShared => {
                    panic!("entity was already registered with shared binding for joints")
                }
                RegistrationType::NewIndependant => {
                    let InstanceGPUResidency::Independant(map) = &mut entity_reg.joint_transform
                    else {
                        panic!("should have a registered joint map");
                    };
                    map.insert(
                        instance_handle.clone(),
                        joint_handle.expect("needs joint handle"),
                    );
                }
                RegistrationType::None => assert!(joint_handle.is_none()),
            }
        }

        let lt_offset_base = match &self
            .entity_register
            .get(&instance_handle.entity_handle)
            .unwrap()
            .local_transform
        {
            InstanceGPUResidency::Shared(gpu_handle) => {
                let idx = self.binding_map.get(gpu_handle).unwrap();
                self.cached_offsets[*idx]
            }
            InstanceGPUResidency::Independant(map) => {
                let gpu_handle = map.get(&instance_handle).unwrap();
                let idx = self.binding_map.get(gpu_handle).unwrap();
                self.cached_offsets[*idx]
            }
            InstanceGPUResidency::None => panic!("instances must have lt offset"),
            InstanceGPUResidency::Pending => panic!(),
        };

        let joint_offset_base = match &self
            .entity_register
            .get(&instance_handle.entity_handle)
            .unwrap()
            .joint_transform
        {
            InstanceGPUResidency::Shared(gpu_handle) => {
                let idx = self.binding_map.get(gpu_handle).unwrap();
                Some(self.cached_offsets[*idx])
            }
            InstanceGPUResidency::Independant(map) => {
                let gpu_handle = map.get(&instance_handle).unwrap();
                let idx = self.binding_map.get(gpu_handle).unwrap();
                Some(self.cached_offsets[*idx])
            }
            InstanceGPUResidency::None => None,
            InstanceGPUResidency::Pending => panic!(),
        };
        RenderGroupEntry {
            instance_handle,
            relative_instance_idx,
            lt_offset_base,
            joint_offset_base,
        }
    }
}
