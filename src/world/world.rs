use std::ops::Range;

use cgmath::{SquareMatrix, Vector3};

use super::scene::Scene;
use crate::{
    app::{
        app::App,
        renderer::{GPUAllocationHandle, Instruction, Operations, RenderUpdateDelta, VMValue},
    },
    asset_manager::{
        AssetHandle, LoadedAsset, asset_manager::AssetManager, gltf_assets::GltfAsset,
    },
    world::{
        WorldInitError, WorldUpdateError,
        camera::Camera,
        components::{MeshCollectionComponent, MeshCollectionDescriptor},
        entity_manager::{EntityHandle, EntityManager},
        instance_manager::{APosition, Archetype, InstanceHandle, InstanceManager},
        load_queue::EntityLoadQueue,
        scene::{SceneEvent, SceneLoadLevel},
    },
};

pub struct DrawSet {
    pub mesh_ids: Vec<u32>,
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
        if let Some((ids, prims, indices)) = data {
            Some(Self {
                mesh_ids: ids,
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
    pub instance_handle: InstanceHandle,
    pub views: Vec<RenderView>,
    pub indexed: bool,
}
impl RenderGroup {
    pub fn new(instance_handle: InstanceHandle, views: Vec<RenderView>, is_indexed: bool) -> Self {
        Self {
            instance_handle,
            views,
            indexed: is_indexed,
        }
    }
}

pub enum WorldUpdateDelta {
    EntityDidSpawn(InstanceHandle),
    EntityDidLoad(EntityHandle),
    AssetDidLoad(AssetHandle),
}

impl WorldUpdateDelta {
    pub fn gen_bytecode<'frame>(
        &self,
        world: &'frame World,
    ) -> (Vec<VMValue<'frame>>, Vec<Instruction>) {
        let mut constants = Vec::<VMValue<'frame>>::new();
        let mut instructions = Vec::<Instruction>::new();
        match self {
            Self::AssetDidLoad(asset_handle) => {
                let la = world
                    .get_loaded_asset_of(&asset_handle)
                    .expect("loaded asset should be exactly CPU resident!");
                // generate bytecode for renderer VM to load an asset
                constants.push(VMValue::LoadedAsset(la));
                instructions.push(Instruction::Op(Operations::AddAsset));
                instructions.push(Instruction::ConstIdx((constants.len() - 1) as u8));
            }

            Self::EntityDidSpawn(instance_handle) => {
                let entity_handle = instance_handle.entity_handle.clone();
                let renderables = world
                    .entity_manager
                    .get_renderables(&entity_handle, &world.asset_manager);

                instructions.push(Instruction::Op(Operations::SpawnEntityInstance));
                let assets = App::get_ordered_assets(&renderables);
                constants.push(VMValue::InstanceHandle(instance_handle.clone()));
                instructions.push(Instruction::ConstIdx((constants.len() - 1) as u8));
                constants.push(VMValue::Renderables(renderables));
                instructions.push(Instruction::ConstIdx((constants.len() - 1) as u8));
                // TODO: renderables can have a variable number of associated assets, this
                // affects the indices of the constants
                for asset_handle in assets {
                    constants.push(VMValue::LoadedAsset(
                        world
                            .get_loaded_asset_of(&asset_handle)
                            .expect("should be a registered asset"),
                    ));
                    instructions.push(Instruction::ConstIdx((constants.len() - 1) as u8));
                }
            }
            WorldUpdateDelta::EntityDidLoad(_) => {
                //TODO spawn based on user input
            }
        }
        (constants, instructions)
    }
}

pub struct World {
    pub camera: Camera,
    scene: Scene,
    pub asset_manager: AssetManager,
    pub entity_manager: EntityManager,
    load_queue: EntityLoadQueue,
    pub instance_manager: InstanceManager,
}

impl World {
    pub fn get_loaded_asset_of(&self, asset_handle: &AssetHandle) -> Option<&LoadedAsset> {
        self.asset_manager.get_loaded_asset(asset_handle)
    }

    pub fn add_scene(&mut self, scene: Scene) {
        self.scene = scene;
    }

    pub fn new(
        aspect_ratio: f32,
        asset_manager: AssetManager,
        entity_manager: EntityManager,
        device: &wgpu::Device,
    ) -> Result<Self, WorldInitError> {
        let mut camera = crate::world::camera::get_camera_default();
        camera.build_camera_uniform(aspect_ratio, device);

        Ok(Self {
            camera,
            scene: Scene::new(),
            asset_manager,
            entity_manager,
            load_queue: EntityLoadQueue::new(),
            instance_manager: InstanceManager::new(),
        })
    }

    pub fn spawn<A: Archetype>(
        instance_manager: &mut InstanceManager,
        entity_handle: EntityHandle,
        archetype: A,
    ) -> Result<&Vec<InstanceHandle>, WorldUpdateError> {
        Ok(instance_manager.spawn(entity_handle, archetype))
    }

    pub fn update(&mut self) -> Result<Vec<WorldUpdateDelta>, WorldUpdateError> {
        let mut deltas = Vec::<WorldUpdateDelta>::new();
        // check scenes
        if self.scene.is_dirty() {
            self.handle_scene_event()?; // TODO: allow for multiple scenes
        }
        if let Some(updates) = self.load_queue.poll_entity_jobs(&mut self.asset_manager)? {
            deltas.extend(updates);
        }
        for (i, completed) in self.load_queue.completed_queue.iter().enumerate() {
            match completed.1.load_level {
                // TODO
            }
            deltas.push(WorldUpdateDelta::EntityDidSpawn(instances[0].clone()));
        }
        self.load_queue.dequeue_completed();

        // TODO: emit EntityDidSpawn event when necessary

        Ok(deltas)
    }

    fn handle_scene_event(&mut self) -> Result<(), WorldUpdateError> {
        loop {
            let maybe_event = self.scene.pop_event();
            if maybe_event.is_none() {
                break;
            } else {
                match maybe_event.unwrap() {
                    SceneEvent::EntitiesAdded(_) => {
                        todo!()
                        // for entity_handle in entities {
                        //     // TODO: handle failed job enqueue?
                        //     self.enqueue_entity_load(entity_handle, scene_load_level)?;
                        //     self.poll_assets_for_job(&entity_handle, &mut deltas);
                        // }
                    }
                    SceneEvent::LoadLevelChanged(old, new) => {
                        if new > old {
                            let entities = self.scene.entitites.clone();
                            for entity in entities {
                                let _ = self.load_queue.new_entity_load(
                                    entity,
                                    self.scene.load_level,
                                    &self.entity_manager.rbcs_of(entity),
                                );
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn post_frame_update(&mut self, render_deltas: &[RenderUpdateDelta]) {
        for delta in render_deltas {
            println!("{delta:?}");
            match delta {
                RenderUpdateDelta::AssetGPULoaded(allocation_handle) => {
                    self.asset_manager
                        .register_asset_gpu_residency(allocation_handle)
                        .expect("Asset not found");
                }
                RenderUpdateDelta::EntityGPULoaded(_) => {
                    // TODO wait to dequeue until GPU reports it has successfully loaded entity?
                }
            }
        }
    }
}
