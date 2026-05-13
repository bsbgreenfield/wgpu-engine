use std::ops::Range;

use super::scene::Scene;
use crate::{
    app::{
        GPUAssetUploadJob,
        app::AppCommand,
        renderer::{
            GPUAllocationHandle, Instruction, Operations, RenderConstant, RenderUpdateDelta,
        },
    },
    asset_manager_new::{Asset, AssetHandle, AssetLoadError, LoadableAsset},
    util::types::{LocalTransform, PNUJWVertex, PNUVertex, VIndex},
    world::{
        RenderKey, WorldInitError, WorldUpdateError,
        camera::Camera,
        components::ResourceBacking,
        entity_manager::{EntityHandle, EntityManager},
        instance_manager::{Archetype, InstanceHandle, InstanceManager},
        load_queue::EntityLoadQueue,
        scene::SceneEvent,
    },
};

pub struct DrawSet {
    /// for use while iterating over primitives
    /// mesh_map[primitive_slot_index] = mesh_slot_index
    pub mesh_map: Vec<u32>,
    pub primtitive_ranges: Vec<Range<u32>>,
    pub index_ranges: Option<Vec<Range<u32>>>,
}

impl DrawSet {
    #[inline]
    pub const fn within(prim_range: &Range<u32>, range: &Range<u32>) -> Range<u32> {
        let start = range.start + prim_range.start;
        start..(start + (prim_range.end - prim_range.start) as u32)
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
pub enum LocalTransformsNew {
    Uninit,
    Owned { data: Vec<LocalTransform> },
    CopiedFrom { donor: InstanceHandle },
    NeedsCopy,
    SharedWith { donor: InstanceHandle },
    NeedsShared,
}

#[derive(Debug)]
pub enum LocalTransformData {
    None,
    NeedsDonor,
    FromShared {
        donor: InstanceHandle,
    },
    New {
        local_transforms: Vec<LocalTransform>,
        buffer_slot_map: Vec<usize>,
    },
    Copy(Vec<LocalTransform>),
}

#[derive(Debug)]
pub struct InstanceUploadData {
    pub instance_handle: InstanceHandle,
    pub local_transforms: LocalTransformsNew,
    // others
}

#[derive(Debug)]
pub enum WorldUpdateDelta<'frame> {
    EntityDidSpawn(InstanceUploadData),
    AssetDidLoad(GPUAssetUploadJob<'frame>),
}

pub struct World {
    pub camera: Camera,
    pub scene: Scene,
    pub entity_manager: EntityManager,
    load_queue: EntityLoadQueue,
    pub instance_manager: InstanceManager,
}

impl World {
    fn const_last(constants: &Vec<RenderConstant<'_>>) -> Instruction {
        Instruction::ConstIdx((constants.len() - 1) as u8)
    }
    pub fn gen_bytecode<'frame>(
        deltas: Vec<WorldUpdateDelta<'frame>>,
        instructions: &mut Vec<Instruction>,
        constants: &mut Vec<RenderConstant<'frame>>,
    ) {
        for delta in deltas {
            match delta {
                WorldUpdateDelta::AssetDidLoad(asset_upload_job) => {
                    instructions.push(Instruction::Op(Operations::AddAsset));
                    constants.push(RenderConstant::Key(asset_upload_job.asset_handle.as_key()));
                    instructions.push(Self::const_last(constants));
                    if let Some(pnu) = asset_upload_job.pnu_vertices {
                        instructions.push(Instruction::Op(Operations::PNUUpload));
                        let pnu_data = bytemuck::cast_slice::<PNUVertex, u8>(pnu);
                        constants.push(RenderConstant::DataRef(pnu_data));
                        instructions.push(Self::const_last(constants));
                    }
                    if let Some(pnujw) = asset_upload_job.pnujw_vertices {
                        instructions.push(Instruction::Op(Operations::PNUJWUpload));
                        let pnujw_data = bytemuck::cast_slice::<PNUJWVertex, u8>(pnujw);
                        constants.push(RenderConstant::DataRef(pnujw_data));
                        instructions.push(Self::const_last(constants));
                    }
                    if let Some(indices) = asset_upload_job.indices {
                        instructions.push(Instruction::Op(Operations::IndexUpload));
                        let index_data = bytemuck::cast_slice::<VIndex, u8>(indices);
                        constants.push(RenderConstant::DataRef(index_data));
                        instructions.push(Self::const_last(constants));
                    }
                    instructions.push(Instruction::Op(Operations::EmitAssetUpload));
                }
                WorldUpdateDelta::EntityDidSpawn(instance_upload_data) => {
                    instructions.push(Instruction::Op(Operations::SpawnEntityInstance));
                    constants.push(RenderConstant::Key(
                        instance_upload_data.instance_handle.as_key(),
                    ));
                    instructions.push(Self::const_last(constants));
                    match instance_upload_data.local_transforms {
                        // LocalTransformData::Copy(mut local_transforms) => {
                        //     instructions.push(Instruction::Op(Operations::LocalTransformUpload));
                        //     let lt_bytes: Vec<u8> = {
                        //         let ptr = local_transforms.as_mut_ptr() as *mut u8;
                        //         let len =
                        //             local_transforms.len() * std::mem::size_of::<LocalTransform>();
                        //         let cap = local_transforms.capacity()
                        //             * std::mem::size_of::<LocalTransform>();
                        //         std::mem::forget(local_transforms);
                        //         unsafe { Vec::from_raw_parts(ptr, len, cap) }
                        //     };
                        //     constants.push(RenderConstant::DataOwned(lt_bytes));
                        //     instructions.push(Self::const_last(constants));
                        // }
                        // LocalTransformData::FromShared { donor } => {
                        //     instructions.push(Instruction::Op(Operations::ResolveSharedLTBinding));
                        //     constants.push(RenderConstant::Key(donor.as_key()));
                        //     instructions.push(Self::const_last(constants));
                        // }
                        // LocalTransformData::NeedsDonor => panic!(
                        //     "instance manager is responsible for providing the instance upload data with a donor handle"
                        // ),
                        // LocalTransformData::None => panic!("not supported yet"),
                        // _ => panic!(),
                        LocalTransformsNew::Uninit => todo!(),
                        LocalTransformsNew::Owned { data } => todo!(),
                        LocalTransformsNew::CopiedFrom { donor } => todo!(),
                        LocalTransformsNew::NeedsCopy => todo!(),
                        LocalTransformsNew::SharedWith { donor } => todo!(),
                        LocalTransformsNew::NeedsShared => todo!(),
                    }
                    instructions.push(Instruction::Op(Operations::EmitEntitySpawn));
                }
            }
        }
    }
    pub fn add_scene(&mut self, scene: Scene) {
        self.scene = scene;
    }

    pub fn register_asset<A>(&mut self, str_dir: &str) -> Result<ResourceBacking<A>, AssetLoadError>
    where
        A: Asset + 'static,
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

    pub fn spawn(
        &mut self,
        entity_handle: &EntityHandle,
        archetype: Box<dyn Archetype>,
    ) -> InstanceUploadData {
        self.instance_manager
            .spawn(entity_handle, &self.entity_manager, archetype)
            .unwrap_or_else(|e| panic!("error handle for spawn fail! {:?}", e))
    }

    pub fn update<'frame>(
        &'frame mut self,
        commands: &mut Vec<AppCommand>,
    ) -> Result<Vec<WorldUpdateDelta<'frame>>, WorldUpdateError> {
        let mut deltas = Vec::<WorldUpdateDelta>::new();
        // check scenes
        if self.scene.is_dirty() {
            self.handle_scene_event(&mut deltas)?; // TODO: allow for multiple scenes
        }
        let pending_assets = self.load_queue.pending_asset_uploads.drain(..);
        for handle in pending_assets {
            let job: GPUAssetUploadJob = self
                .entity_manager
                .asset_manager
                .get_upload_job_for(handle)?;
            deltas.push(WorldUpdateDelta::AssetDidLoad(job));
        }

        self.instance_manager.update(commands);

        Ok(deltas)
    }

    fn try_handle_scene_load(&mut self) -> Result<bool, WorldUpdateError> {
        self.load_queue
            .poll_scene_job(self.scene.scene_id, &mut self.entity_manager.asset_manager)?;
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
        'outer: loop {
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
                            if !self.try_handle_scene_load()? {
                                break;
                            }
                        } else if new > old {
                            self.load_queue
                                .new_scene_job(&self.scene, &self.entity_manager)?;
                            if !self.try_handle_scene_load()? {
                                break 'outer;
                            }
                        } else {
                            //TODO: continue?
                        }
                    }
                    SceneEvent::Spawn(_) => match self.scene.pop_event().unwrap() {
                        SceneEvent::Spawn(mut instance_data) => {
                            for (entity_handle, archetype) in instance_data.drain(..) {
                                let instance_upload_data = self.spawn(&entity_handle, archetype);
                                deltas.push(WorldUpdateDelta::EntityDidSpawn(instance_upload_data));
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
