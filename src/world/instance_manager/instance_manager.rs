use crate::{
    app::app::AppCommand,
    common::{entity::EntityHandle, instance::InstanceHandle},
    renderer::{GPUInstanceHandle, RenderPacket},
    world::{
        WorldUpdateError,
        instance_manager::{
            ArchetypeId, RenderFrame,
            animation_controller::AnimationController,
            archetype_table::{APositionTable, ArchetypeTable},
            gpu_bind_registry::GPUBindRegistry,
        },
        world::RenderGroup,
    },
};

pub struct InstanceManager {
    pub(super) gpu_bind_registry: GPUBindRegistry,
    pub(super) pos: APositionTable,
    pub(super) render_groups: Vec<Option<RenderGroup>>,
    pub(super) free_group_slots: Vec<usize>,
    pub(super) sparse_entity_group: Vec<usize>,
    pub animation_controller: AnimationController,
}

impl InstanceManager {
    pub fn new() -> Self {
        Self {
            gpu_bind_registry: GPUBindRegistry::default(),
            pos: APositionTable::new(),
            free_group_slots: Vec::new(),
            sparse_entity_group: Vec::from_iter(std::iter::repeat_n(usize::MAX, 100)),
            render_groups: Vec::new(),
            animation_controller: AnimationController::default(),
        }
    }

    pub fn activate_animation(
        &mut self,
        instance_handle: &InstanceHandle,
        anim_idx: usize,
        offset: Option<f32>,
    ) {
        self.animation_controller
            .activate_animations(instance_handle, anim_idx, offset);
    }
    pub fn resolve_idx(&self, handle: &InstanceHandle) -> Option<usize> {
        match handle.archetype {
            ArchetypeId::Position => self.pos.arena.resolve(handle),
        }
    }

    pub fn update(&mut self, commands: &mut Vec<AppCommand>) {
        if let Some(command) = commands.pop() {
            let mut idx: isize = -1;
            let dummy = match command {
                AppCommand::One => {
                    idx = 0;
                    Some(InstanceHandle {
                        archetype: ArchetypeId::Position,
                        entity_handle: EntityHandle(1),
                        instance_id: 0,
                        generation: 0,
                    })
                }

                AppCommand::Two => {
                    idx = 1;
                    Some(InstanceHandle {
                        archetype: ArchetypeId::Position,
                        entity_handle: EntityHandle(0),
                        instance_id: 1,
                        generation: 0,
                    })
                }

                AppCommand::Three => {
                    idx = 2;
                    Some(InstanceHandle {
                        archetype: ArchetypeId::Position,
                        entity_handle: EntityHandle(0),
                        instance_id: 2,
                        generation: 0,
                    })
                }
                _ => None,
            };
            if idx >= 0 && dummy.is_some() {
                match self.pos.query(dummy.as_ref().unwrap()) {
                    Some(_) => {
                        self.activate_animation(&dummy.unwrap(), idx as usize, None);
                    }
                    None => commands.push(command),
                }
            } else {
                commands.push(command);
            }
        }
        self.animation_controller.update();
    }

    pub fn despawn(
        &mut self,
        handle: InstanceHandle,
    ) -> Result<GPUInstanceHandle, WorldUpdateError> {
        let gpu_handle = self.gpu_bind_registry.unregister(&handle)?;
        self.animation_controller.clear_animation_for(&handle);

        match handle.archetype {
            ArchetypeId::Position => self.pos.remove(handle),
        }
        Ok(gpu_handle)
        // TODO: other tables
    }

    pub fn release_entity_render_state(&mut self, entity_handle: &EntityHandle) {
        debug_assert!(
            !self
                .pos
                .arena
                .handles
                .iter()
                .any(|h| &h.entity_handle == entity_handle),
            "released group for {entity_handle:?} with live instances in it"
        );
        self.animation_controller
            .registered_animations
            .remove(entity_handle);
        let Some(slot) = self
            .sparse_entity_group
            .get(entity_handle.0 as usize)
            .copied()
        else {
            return;
        };

        if slot == usize::MAX {
            return;
        }

        self.sparse_entity_group[entity_handle.0 as usize] = usize::MAX;
        if self.render_groups[slot].take().is_some() {
            self.free_group_slots.push(slot);
        }
    }

    pub fn prepare_render_frame<'frame>(
        &'frame self,
        render_packet: &'frame RenderPacket,
    ) -> RenderFrame<'frame> {
        let mut render_frame = RenderFrame::default();
        render_frame.indirection_list = &render_packet.draw_packet.indirection_list;
        render_frame.global_transforms = &render_packet.global_transforms;

        self.animation_controller.prepare_animation_frame(
            &mut render_frame,
            &self.gpu_bind_registry.registered_instances,
        );
        render_frame
    }
}
