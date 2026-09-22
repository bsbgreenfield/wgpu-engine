use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

use crate::{
    asset_manager::{AssetHandle, asset_manager::AssetManager},
    common::{entity::EntityHandle, instance::InstanceHandle},
    world::{
        entity_manager::entity_manager::EntityManager,
        scene::{Scene, SceneId, SceneLoadLevel},
    },
};

#[derive(Debug)]
pub enum DependencyGraphError {
    InvalidChild,
    ChildNotFound,
    SceneNotFound,
}

impl std::error::Error for DependencyGraphError {}
impl Display for DependencyGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidChild => f.write_str("invalid child node!"),
            Self::ChildNotFound => {
                f.write_str("could not find the dependency, although it is listed as a child")
            }
            Self::SceneNotFound => f.write_str("could not find this scene"),
        }
    }
}
pub struct SweepResult {
    pub required_assets: HashMap<AssetHandle, SceneLoadLevel>,
    pub achieved_scenes: HashMap<SceneId, SceneLoadLevel>,
}

struct EntityNode {
    assets: HashSet<AssetHandle>,
    demand: Demand,
    instances: HashMap<SceneId, Vec<InstanceHandle>>,
    live_instance_count: usize,
}
struct SceneNode {
    children: Vec<SceneId>,
    entities: Vec<EntityHandle>,
}

struct Demand {
    cpu: usize,
    gpu: usize,
}
impl Demand {
    fn apply(&mut self, prev: SceneLoadLevel, new: SceneLoadLevel) {
        if prev == SceneLoadLevel::CPU {
            self.cpu -= 1;
        }
        if prev == SceneLoadLevel::GPU {
            self.gpu -= 1;
        }
        if new == SceneLoadLevel::CPU {
            self.cpu += 1;
        }
        if new == SceneLoadLevel::GPU {
            self.gpu += 1;
        }
    }

    fn level(&self) -> SceneLoadLevel {
        if self.gpu >= 1 {
            return SceneLoadLevel::GPU;
        }
        if self.cpu >= 1 {
            return SceneLoadLevel::CPU;
        }
        SceneLoadLevel::NotLoaded
    }
}

pub struct DemandSweep {
    pub assets: Vec<AssetHandle>,
    pub entities: Vec<EntityHandle>,
}

pub struct DespawnAck {
    pub freed_assets: Vec<AssetHandle>,
    /// set when that was the last live instance of the entity
    pub freed_entity: Option<EntityHandle>,
}

struct AssetDemand {
    demand: Demand,
    holders: Vec<SceneId>,
    entities: Vec<EntityHandle>,
}

#[derive(Default)]
pub struct DependencyGraph {
    scenes: HashMap<SceneId, SceneNode>,
    entities: Vec<EntityNode>,
    asset_demand: HashMap<AssetHandle, AssetDemand>,
}

impl DependencyGraph {
    pub fn ack_despawn(&mut self, instance_handle: InstanceHandle) -> Option<DespawnAck> {
        let entity = instance_handle.entity_handle;
        let node = &mut self.entities[entity.0 as usize];
        node.live_instance_count -= 1;

        if node.live_instance_count > 0 {
            return None;
        } else {
            let freed_entity = (node.demand.level() < SceneLoadLevel::GPU).then_some(entity);

            let freed_assets: Vec<AssetHandle> = self.entities[entity.0 as usize]
                .assets
                .iter()
                .copied()
                .filter(|asset| {
                    self.asset_demand[asset]
                        .entities
                        .iter()
                        .all(|e| self.entities[e.0 as usize].live_instance_count == 0)
                })
                .collect();

            Some(DespawnAck {
                freed_assets,
                freed_entity,
            })
        }
    }
    pub fn holders_of(&self, asset_handle: &AssetHandle) -> &[SceneId] {
        &self
            .asset_demand
            .get(asset_handle)
            .map(|d| d.holders.as_slice())
            .unwrap_or(&[])
    }
    //pub fn required_assets_of(&self, scene_id: SceneId) -> Vec<AssetHandle> {
    //    let mut assets = HashSet::new();
    //    for entity in self.scenes.get(&scene_id).unwrap().entities.iter() {
    //        for asset in &self.entities.get(entity.0 as usize).unwrap().assets {
    //            assets.insert(*asset);
    //        }
    //    }
    //    assets.into_iter().collect()
    //}

    pub fn recompute_levels(
        &mut self,
        scene_id: SceneId,
        prev: SceneLoadLevel,
        new: SceneLoadLevel,
    ) -> DemandSweep {
        let Self {
            scenes,
            entities: entity_nodes,
            asset_demand,
        } = self;

        let scene = scenes.get(&scene_id).expect("scene");
        let mut assets = HashSet::<AssetHandle>::new();
        let mut entities = Vec::<EntityHandle>::with_capacity(scene.entities.len());

        for entity in scene.entities.iter() {
            let node = &mut entity_nodes[entity.0 as usize];
            if prev != new {
                node.demand.apply(prev, new);
            }
            assets.extend(node.assets.iter().copied());
            entities.push(*entity);
        }

        let assets: Vec<AssetHandle> = assets.into_iter().collect();
        if prev != new {
            for asset in assets.iter() {
                asset_demand.get_mut(asset).unwrap().demand.apply(prev, new);
            }
        }

        DemandSweep { assets, entities }
    }

    pub fn required_asset_level(&self, asset_handle: &AssetHandle) -> SceneLoadLevel {
        self.asset_demand.get(asset_handle).unwrap().demand.level()
    }
    pub fn required_entity_level(&self, entity_handle: &EntityHandle) -> SceneLoadLevel {
        self.entities
            .get(entity_handle.0 as usize)
            .unwrap()
            .demand
            .level()
    }

    pub fn live_instances_of(&self, entity: &EntityHandle) -> usize {
        self.entities
            .get(entity.0 as usize)
            .unwrap()
            .live_instance_count
    }
    pub fn add_scene(
        &mut self,
        scene: &Scene,
        entity_manager: &EntityManager,
        asset_manager: &AssetManager,
    ) -> Result<(), DependencyGraphError> {
        let mut children: Vec<SceneId> = Vec::new();
        for child in scene.desc.children.iter() {
            children.push(*child);
        }

        let mut entities = Vec::<EntityHandle>::new();
        for entity in scene.desc.entities.iter() {
            entities.push(*entity);
            if self.entities.len() <= entity.0 as usize {
                self.entities
                    .resize_with((entity.0 + 1) as usize, || EntityNode {
                        assets: HashSet::new(),
                        instances: HashMap::new(),
                        demand: Demand { cpu: 0, gpu: 0 },
                        live_instance_count: 0,
                    });
            }
            self.entities[entity.0 as usize].assets =
                entity_manager.rbcs_of(*entity, asset_manager);
            for asset in self.entities.get(entity.0 as usize).unwrap().assets.iter() {
                if !self.asset_demand.contains_key(asset) {
                    self.asset_demand.insert(
                        *asset,
                        AssetDemand {
                            demand: Demand { cpu: 0, gpu: 0 },
                            holders: vec![],
                            entities: vec![*entity],
                        },
                    );
                } else {
                    self.asset_demand
                        .get_mut(asset)
                        .unwrap()
                        .entities
                        .push(*entity);
                }
            }
        }

        let scene_assets: HashSet<AssetHandle> = entities
            .iter()
            .flat_map(|e| self.entities[e.0 as usize].assets.iter().copied())
            .collect();

        for asset in scene_assets {
            self.asset_demand
                .entry(asset)
                .and_modify(|ad| ad.holders.push(scene.id));
        }
        let new_scene = SceneNode { children, entities };

        self.scenes.insert(scene.id, new_scene);

        Ok(())
    }
    pub fn children_of(&self, scene_id: SceneId) -> &[SceneId] {
        self.scenes
            .get(&scene_id)
            .map(|s| s.children.as_slice())
            .unwrap_or(&[])
    }

    pub fn add_instance_handles(
        &mut self,
        scene_id: SceneId,
        handles: impl IntoIterator<Item = InstanceHandle>,
    ) {
        for handle in handles {
            let entity_node = self
                .entities
                .get_mut(handle.entity_handle.0 as usize)
                .unwrap();
            entity_node
                .instances
                .entry(scene_id)
                .and_modify(|instances| instances.push(handle.clone()))
                .or_insert(vec![handle]);
            entity_node.live_instance_count += 1;
        }
    }

    pub fn drain_instances_of(
        &mut self,
        scene_id: SceneId,
    ) -> impl IntoIterator<Item = InstanceHandle> {
        let mut handles = Vec::new();

        let scene = self.scenes.get(&scene_id).expect("scene");

        for entity in scene.entities.iter() {
            let entity_node = self.entities.get_mut(entity.0 as usize).expect("entity");
            if let Some(instances) = entity_node.instances.get_mut(&scene_id) {
                handles.extend(instances.drain(..));
            }
        }

        handles
    }

    #[cfg(test)]
    pub fn clone_instances_of(
        &self,
        scene_id: SceneId,
    ) -> impl IntoIterator<Item = InstanceHandle> {
        let mut handles = Vec::new();

        let scene = self.scenes.get(&scene_id).expect("scene");

        for entity in scene.entities.iter() {
            let entity_node = self.entities.get(entity.0 as usize).expect("entity");
            if let Some(instances) = entity_node.instances.get(&scene_id) {
                handles.extend(instances.clone());
            }
        }

        handles
    }
}
