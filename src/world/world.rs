use std::collections::HashMap;

use cgmath::vec3;

use crate::{
    app::{GPUAssetUploadJob, app::AppCommand},
    asset_manager::{Asset, AssetHandle, AssetLoadError, AssetSource, asset_manager::AssetManager},
    common::{entity::EntityHandle, instance::InstanceHandle},
    renderer::{RenderKey, RenderUpdateDelta, camera::Camera},
    world::{
        WorldUpdateDelta, WorldUpdateError,
        entity_manager::{components::ResourceBacking, entity_manager::EntityManager},
        instance_manager::{
            InstanceUploadData, NewInstanceData,
            archetypes::{APosition, Archetype},
            instance_manager::InstanceManager,
        },
        scene::{
            SceneId, SceneLoadLevel,
            manager::{SceneManager, SceneManagerError},
            scene::Spawn,
        },
    },
};

pub struct World {
    init: bool,
    pub camera: Camera,
    pub entity_manager: EntityManager,
    pub asset_manager: AssetManager,
    pub instance_manager: InstanceManager,
    pub scene_manager: SceneManager,
    pub(crate) deltas: Vec<WorldUpdateDelta>,
}

impl World {
    pub fn is_initialized(&self) -> bool {
        self.init
    }
    pub fn init(&mut self, aspect_ratio: f32, device: &wgpu::Device) {
        self.camera.build_camera_uniform(aspect_ratio, device);
        self.init = true;
    }

    pub fn register_asset<A>(&mut self, str_dir: &str) -> Result<ResourceBacking<A>, AssetLoadError>
    where
        A: Asset + AssetSource + 'static,
    {
        self.asset_manager.register_asset::<A>(str_dir)
    }

    pub fn new() -> Self {
        let camera = crate::renderer::camera::get_camera_default();
        //camera.build_camera_uniform(aspect_ratio, device);

        Self {
            deltas: Vec::<WorldUpdateDelta>::new(),
            init: false,
            camera,
            entity_manager: EntityManager::new(),
            asset_manager: AssetManager::new(),
            instance_manager: InstanceManager::new(),
            scene_manager: SceneManager::new(),
        }
    }

    pub fn add_instances(
        &mut self,
        scene_id: SceneId,
        spawn_data: Vec<Spawn<dyn Archetype>>,
    ) -> Result<(), SceneManagerError> {
        self.scene_manager.add_instances(scene_id, spawn_data)
    }

    pub fn spawn(
        &mut self,
        scene_spawns: HashMap<SceneId, Vec<Spawn<dyn Archetype>>>,
    ) -> Result<(), WorldUpdateError> {
        let mut new_this_frame: HashMap<EntityHandle, NewInstanceData> = HashMap::new();

        for (scene_id, spawns) in scene_spawns {
            // pull out anything for an entity we already promoted to New earlier this frame
            let (already_new, rest): (Vec<_>, Vec<_>) = spawns
                .into_iter()
                .partition(|s| new_this_frame.contains_key(&s.entity));

            for spawn in already_new {
                let handle = self
                    .instance_manager
                    .insert_archetypes(&spawn.entity, vec![spawn.data])
                    .remove(0);
                self.scene_manager
                    .add_instance_handle(scene_id, handle.clone())?;
                new_this_frame
                    .get_mut(&spawn.entity)
                    .unwrap()
                    .additional
                    .push(handle);
            }

            for iud in self.instance_manager.spawn_instances(
                &self.entity_manager,
                &self.asset_manager,
                rest,
            )? {
                match iud {
                    InstanceUploadData::New(new) => {
                        // primary + any additional generated within THIS scene's own batch
                        let mut handles = vec![new.handle.clone()];
                        handles.extend(new.additional.iter().cloned());
                        self.scene_manager
                            .add_multiple_instances_handles(scene_id, handles)?;
                        new_this_frame.insert(new.handle.entity_handle.clone(), new);
                    }
                    InstanceUploadData::Copied(copied) => {
                        self.scene_manager
                            .add_multiple_instances_handles(scene_id, copied.handles.clone())?;
                        self.deltas
                            .push(WorldUpdateDelta::EntityInstanceSpawn(copied));
                    }
                }
            }
        }

        for (_, new) in new_this_frame {
            self.deltas.push(WorldUpdateDelta::NewEntitySpawn(new));
        }
        Ok(())
    }

    pub fn despawn_instance(
        &mut self,
        instance_handle: InstanceHandle,
    ) -> Result<(), WorldUpdateError> {
        let gpu_instance_handle = self.instance_manager.despawn(instance_handle.clone())?;
        self.deltas.push(WorldUpdateDelta::InstanceDespawn(
            gpu_instance_handle.clone(),
        ));
        self.scene_manager
            .inflight_despawns
            .insert(gpu_instance_handle, instance_handle);
        Ok(())
    }

    pub fn update<'frame>(
        &'frame mut self,
        commands: &mut Vec<AppCommand>,
    ) -> Result<(), WorldUpdateError> {
        for request in self.scene_manager.drain_asset_requests() {
            self.scene_manager
                .load_queue_new
                .add_load_job(request, &self.asset_manager);
        }
        for transition in self
            .scene_manager
            .load_queue_new
            .poll_jobs(&mut self.asset_manager)?
        {
            if matches!(transition.new, SceneLoadLevel::PendingGPU) {
                let job: GPUAssetUploadJob =
                    self.asset_manager.get_upload_job_for(transition.handle)?;
                self.deltas.push(WorldUpdateDelta::AssetDidLoad(job));
            } else if transition.old == SceneLoadLevel::GPU {
                let alloc_handle = self.asset_manager.alloc_handle_of(&transition.handle)?;
                self.deltas.push(WorldUpdateDelta::AssetUnload(
                    transition.handle,
                    alloc_handle,
                ));
            }
            self.scene_manager.on_asset_level_changed(transition);
        }

        self.scene_manager.process_scene_events()?;

        // TODO: this a regression from the normal pattern of request -> renderer -> ack -> do
        // the reason is because, in this case, its a bit of a reverse ack
        // as the gpu needs to know that the instance no longer exists on the world, not the
        // other way around, and its ok if the gpu data persists for a frame or two while
        // the instance manager simply doesnt draw it.
        // worth revisiting though
        for handle in std::mem::take(&mut self.scene_manager.despawn_queue) {
            self.despawn_instance(handle)?;
        }
        for entity in std::mem::take(&mut self.scene_manager.prototype_release_queue) {
            if let Some(prototype) = self.entity_manager.release_prototype(&entity) {
                self.instance_manager.release_entity_render_state(&entity);
                self.deltas
                    .push(WorldUpdateDelta::ReleasePrototype(prototype));
            }
        }

        if !self.scene_manager.spawn_queue.is_empty() {
            let spawn_data = std::mem::take(&mut self.scene_manager.spawn_queue);
            self.spawn(spawn_data)?;
        }
        match commands.pop() {
            Some(c) => match c {
                AppCommand::Despawn => {
                    self.scene_manager.set_load_level(
                        SceneId(0),
                        SceneLoadLevel::NotLoaded,
                        &self.asset_manager,
                    )?;
                }

                AppCommand::Spawn => {
                    self.add_instances(
                        SceneId(0),
                        vec![Spawn {
                            entity: EntityHandle(0),
                            data: Box::new(APosition {
                                position: cgmath::Matrix4::<f32>::from_translation(vec3(
                                    0., 3., 5.,
                                ))
                                .into(),
                            }),
                        }],
                    )?;
                    self.scene_manager.set_load_level(
                        SceneId(0),
                        SceneLoadLevel::GPU,
                        &self.asset_manager,
                    )?;
                }
                _ => commands.push(c),
            },
            None => {}
        }
        self.instance_manager.update(commands);

        Ok(())
    }

    pub(crate) fn post_frame_update(
        &mut self,
        render_deltas: Vec<RenderUpdateDelta>,
    ) -> Result<(), WorldUpdateError> {
        for delta in render_deltas {
            match delta {
                RenderUpdateDelta::AssetGPULoaded { key, alloc_handle } => {
                    self.asset_manager
                        .register_asset_gpu_residency(
                            AssetHandle::from_key(key),
                            alloc_handle.clone(),
                        )
                        .expect("Asset not found");
                }
                RenderUpdateDelta::AssetUnloaded { key } => {
                    let asset_handle = AssetHandle::from_key(key);
                    self.asset_manager
                        .register_asset_gpu_unloaded(asset_handle)?;
                }
                RenderUpdateDelta::InstanceDespawn(gpu_handle) => {
                    self.scene_manager.ack_despawn(gpu_handle);
                }
                RenderUpdateDelta::InstanceSpawn {
                    instance_key,
                    gpu_instance_handle,
                } => {
                    let instance_handle = InstanceHandle::from_key(instance_key);
                    self.instance_manager
                        .ack_instance_spawn(&instance_handle, gpu_instance_handle);
                }
                RenderUpdateDelta::PrototypeCreated {
                    entity_key,
                    prototype_handle,
                } => {
                    let entity_handle = EntityHandle::from_key(entity_key);
                    self.entity_manager
                        .ack_prototype(&entity_handle, prototype_handle);
                }
            }
        }
        Ok(())
    }
}
