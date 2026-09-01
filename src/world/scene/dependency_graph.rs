use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

use crate::{
    asset_manager::AssetHandle,
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
    instances: HashMap<SceneId, Vec<InstanceHandle>>,
}
struct SceneNode {
    children: Vec<SceneId>,
    entities: Vec<EntityHandle>,
}

struct AssetDemand {
    cpu: usize,
    gpu: usize,
    holders: Vec<SceneId>,
    live_instance_count: usize,
}

#[derive(Default)]
pub struct DependencyGraph {
    scenes: HashMap<SceneId, SceneNode>,
    entities: Vec<EntityNode>,
    asset_demand: HashMap<AssetHandle, AssetDemand>,
}

impl DependencyGraph {
    pub fn ack_despawn(&mut self, instance_handle: InstanceHandle) -> Vec<AssetHandle> {
        let mut free_assets = Vec::<AssetHandle>::new();
        for asset in self.entities[instance_handle.entity_handle.0 as usize]
            .assets
            .iter()
        {
            let d = self
                .asset_demand
                .get_mut(asset)
                .expect("asset is not registered");
            d.live_instance_count -= 1;
            if d.live_instance_count == 0 {
                free_assets.push(*asset);
            }
        }
        free_assets
    }
    pub fn holders_of(&self, asset_handle: &AssetHandle) -> &[SceneId] {
        &self
            .asset_demand
            .get(asset_handle)
            .map(|d| d.holders.as_slice())
            .unwrap_or(&[])
    }
    pub fn required_assets_of(&self, scene_id: SceneId) -> Vec<AssetHandle> {
        let mut assets = HashSet::new();
        for entity in self.scenes.get(&scene_id).unwrap().entities.iter() {
            assets.extend(
                self.entities
                    .get(entity.0 as usize)
                    .unwrap()
                    .assets
                    .iter()
                    .copied(),
            );
        }
        assets.into_iter().collect()
    }
    pub fn recompute_asset_levels(
        &mut self,
        scene_id: SceneId,
        prev: SceneLoadLevel,
        new: SceneLoadLevel,
    ) -> Vec<AssetHandle> {
        let mut assets = self.required_assets_of(scene_id);
        if prev == new {
            return assets;
        }
        for asset in assets.iter_mut() {
            let d = self.asset_demand.get_mut(&asset).unwrap();
            if prev == SceneLoadLevel::CPU {
                d.cpu -= 1;
            }
            if prev == SceneLoadLevel::GPU {
                d.gpu -= 1;
            }
            if new == SceneLoadLevel::CPU {
                d.cpu += 1;
            }
            if new == SceneLoadLevel::GPU {
                d.gpu += 1;
            }
        }
        assets
    }

    pub fn required_asset_level(&self, asset_handle: &AssetHandle) -> SceneLoadLevel {
        let demand = self.asset_demand.get(asset_handle).unwrap();
        if demand.gpu >= 1 {
            return SceneLoadLevel::GPU;
        }
        if demand.cpu >= 1 {
            return SceneLoadLevel::CPU;
        }
        return SceneLoadLevel::NotLoaded;
    }
    pub fn add_scene(
        &mut self,
        scene: &Scene,
        entity_manager: &EntityManager,
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
                    });
            }
            self.entities[entity.0 as usize].assets = entity_manager.rbcs_of(*entity);
            for asset in self.entities.get(entity.0 as usize).unwrap().assets.iter() {
                if !self.asset_demand.contains_key(asset) {
                    self.asset_demand.insert(
                        *asset,
                        AssetDemand {
                            cpu: 0,
                            gpu: 0,
                            holders: vec![],
                            live_instance_count: 0,
                        },
                    );
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
            for asset in entity_node.assets.iter() {
                self.asset_demand
                    .get_mut(asset)
                    .unwrap()
                    .live_instance_count += 1;
            }
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
