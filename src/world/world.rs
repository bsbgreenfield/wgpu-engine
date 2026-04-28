use std::{ops::Range, vec::Drain};

use super::scene::Scene;
use crate::{
    app::{
        GPUAssetUploadJob,
        renderer::{
            GPUAllocationHandle, InstanceUploadJob, Instruction, Operations, RenderUpdateDelta,
            VMValue,
        },
    },
    asset_manager_new::{Asset, AssetHandle, AssetLoadError, LoadableAsset},
    util::types::LocalTransform,
    world::{
        WorldInitError, WorldUpdateError,
        camera::Camera,
        entity_manager::{EntityHandle, EntityManager, Renderables},
        instance_manager::{Archetype, InstanceHandle, InstanceManager},
        load_queue::EntityLoadQueue,
        scene::SceneEvent,
    },
};

pub struct DrawSet {
    pub primtitive_ranges: Vec<Range<u32>>,
    pub index_ranges: Option<Vec<Range<u32>>>,
}

impl DrawSet {
    #[inline]
    pub const fn within(prim_range: &Range<u32>, range: &Range<u32>) -> Range<u32> {
        let start = range.start + prim_range.start;
        start..(start + (prim_range.end - prim_range.start) as u32)
    }

    pub fn from_ids_and_prims(
        data: Option<(Vec<u32>, Vec<Range<u32>>, Option<Vec<Range<u32>>>)>,
    ) -> Option<Self> {
        if let Some((_ids, prims, indices)) = data {
            Some(Self {
                primtitive_ranges: prims,
                index_ranges: indices,
            })
        } else {
            None
        }
    }
}

pub struct RenderView {
    pub gpu_handle: GPUAllocationHandle,
    pub pnujw_draws: Option<DrawSet>,
    pub pnu_draws: Option<DrawSet>,
}

pub struct RenderGroup {
    pub instance_handles: Vec<InstanceHandle>,
    pub views: Vec<RenderView>,
}
impl RenderGroup {
    pub fn new(instance_handle: InstanceHandle, views: Vec<RenderView>) -> Self {
        Self {
            instance_handles: vec![instance_handle],
            views,
        }
    }
}

#[derive(Debug)]
pub struct InstanceUploadData {
    pub instance_handle: InstanceHandle,
    pub local_transforms: Option<Vec<LocalTransform>>,
    // others
}

#[derive(Debug)]
pub enum WorldUpdateDelta {
    EntityDidSpawn(InstanceUploadData),
    EntityDidLoad(EntityHandle),
    AssetDidLoad(AssetHandle),
}

impl WorldUpdateDelta {
    pub fn gen_bytecode<'frame>(
        &'frame self,
        world: &'frame World,
        constants: &mut Vec<VMValue<'frame>>,
        instructions: &mut Vec<Instruction>,
    ) {
        match self {
            Self::AssetDidLoad(asset_handle) => {
                let gpu_upload_job = world.get_upload_job_for(asset_handle);
                instructions.push(Instruction::Op(Operations::AddAsset));
                constants.push(VMValue::UploadJob(gpu_upload_job));
                instructions.push(Instruction::ConstIdx((constants.len() - 1) as u8));
            }

            Self::EntityDidSpawn(upload_data) => {
                instructions.push(Instruction::Op(Operations::SpawnEntityInstance));
                constants.push(VMValue::InstanceDataUpload(upload_data));
                instructions.push(Instruction::ConstIdx((constants.len() - 1) as u8));
            }
            WorldUpdateDelta::EntityDidLoad(_) => {
                //TODO spawn based on user input
            }
        }
    }
}

pub struct World {
    pub camera: Camera,
    pub scene: Scene,
    pub entity_manager: EntityManager,
    load_queue: EntityLoadQueue,
    pub instance_manager: InstanceManager,
}

impl World {
    pub fn add_scene(&mut self, scene: Scene) {
        self.scene = scene;
    }

    pub fn register_asset<A>(&mut self, str_dir: &str) -> Result<AssetHandle, AssetLoadError>
    where
        A: Asset + LoadableAsset + 'static,
    {
        self.entity_manager
            .asset_manager
            .register_asset::<A>(str_dir)
    }

    pub fn new(
        aspect_ratio: f32,
        entity_manager: EntityManager,
        device: &wgpu::Device,
    ) -> Result<Self, WorldInitError> {
        let mut camera = crate::world::camera::get_camera_default();
        camera.build_camera_uniform(aspect_ratio, device);

        Ok(Self {
            camera,
            scene: Scene::new(),
            entity_manager,
            load_queue: EntityLoadQueue::new(),
            instance_manager: InstanceManager::new(),
        })
    }

    fn get_upload_job_for<'frame>(
        &'frame self,
        asset_handle: &'frame AssetHandle,
    ) -> GPUAssetUploadJob<'frame> {
        self.entity_manager
            .asset_manager
            .get_upload_job_for(asset_handle)
            .expect("should be uploadable")
    }
    pub fn spawn(
        &mut self,
        entity_handle: EntityHandle,
        archetype: Box<dyn Archetype>,
    ) -> Result<InstanceUploadData, WorldUpdateError> {
        if let Some(renderables) = self.entity_manager.get_renderables(&entity_handle) {
            return Ok(self.instance_manager.spawn(renderables, archetype));
        }
        Err(WorldUpdateError::InstanceSpawnFailure)
    }

    pub fn update(&mut self) -> Result<Vec<WorldUpdateDelta>, WorldUpdateError> {
        let mut deltas = Vec::<WorldUpdateDelta>::new();
        // check scenes
        if self.scene.is_dirty() {
            self.handle_scene_event(&mut deltas)?; // TODO: allow for multiple scenes
        }
        // TODO: emit EntityDidSpawn event when necessary

        Ok(deltas)
    }

    fn try_handle_scene_load(
        &mut self,
        deltas: &mut Vec<WorldUpdateDelta>,
    ) -> Result<bool, WorldUpdateError> {
        self.load_queue.poll_scene_job(
            self.scene.scene_id,
            &mut self.entity_manager.asset_manager,
            deltas,
        )?;
        if self
            .load_queue
            .completed_queue
            .get(&self.scene.scene_id)
            .is_some()
        {
            self.scene.pop_event();
            self.load_queue.dequeue_spawned_scene(self.scene.scene_id);
            return Ok(true);
        }

        Ok(false)
    }

    fn handle_scene_event(
        &mut self,
        deltas: &mut Vec<WorldUpdateDelta>,
    ) -> Result<(), WorldUpdateError> {
        loop {
            let scene_event = self.scene.current_event();
            if scene_event.is_some() {
                match scene_event.unwrap() {
                    SceneEvent::EntitiesAdded(_) => {
                        todo!()
                        // for entity_handle in entities {
                        //     // TODO: handle failed job enqueue?
                        //     self.enqueue_entity_load(entity_handle, scene_load_level)?;
                        //     self.poll_assets_for_job(&entity_handle, &mut deltas);
                        // }
                    }
                    SceneEvent::LoadLevelChanged(old, new) => {
                        if self.load_queue.has_pending_scene_job(self.scene.scene_id) {
                            if !self.try_handle_scene_load(deltas)? {
                                break;
                            }
                        } else if new > old {
                            self.load_queue
                                .new_scene_job(&self.scene, &self.entity_manager)?;
                            if !self.try_handle_scene_load(deltas)? {
                                break;
                            }
                        } else {
                            //TODO: continue?
                        }
                    }
                    SceneEvent::Spawn(_) => match self.scene.pop_event().unwrap() {
                        SceneEvent::Spawn(mut instance_data) => {
                            for (entity_handle, archetype) in instance_data.drain(..) {
                                let instance_upload_jobs = self.spawn(entity_handle, archetype)?;
                                deltas.push(WorldUpdateDelta::EntityDidSpawn(instance_upload_jobs));
                            }
                        }
                        _ => unreachable!(),
                    },
                }
            } else {
                self.scene.mark_clean();
                break;
            }
        }
        Ok(())
    }

    pub fn post_frame_update(&mut self, render_deltas: Vec<RenderUpdateDelta>) {
        for delta in render_deltas {
            match delta {
                RenderUpdateDelta::AssetGPULoaded(asset_handle, allocation_handle) => {
                    self.entity_manager
                        .asset_manager
                        .register_asset_gpu_residency(&asset_handle, allocation_handle.clone())
                        .expect("Asset not found");
                }
                RenderUpdateDelta::EntityGPULoaded(_) => {
                    // TODO wait to dequeue until GPU reports it has successfully loaded entity?
                }
                RenderUpdateDelta::EntitySpawned(gpu_bindings) => {
                    self.instance_manager.update_gpu_bindings(gpu_bindings);
                }
            }
        }
    }
}
