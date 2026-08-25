//! Tests for `SceneManager` / `DependencyGraph`.
//!
//! These target the `SceneNew` path only (`Scene` is on its way out), and they
//! describe the behaviour the graph is meant to have once loading is driven off
//! of it — so some of them are expected to fail against the current sketch.

#[cfg(test)]
mod util {
    use crate::{
        asset_manager::{AssetHandle, gltf_asset::GltfAsset},
        common::entity::EntityHandle,
        world::{
            entity_manager::components::{MeshAcessor, MeshCollectionDescriptor},
            instance_manager::archetypes::{APosition, Archetype},
            scene::{
                SceneDesc, SceneId, SceneNew,
                scene::{SceneRuntime, Spawn},
            },
            world::World,
        },
    };

    /// an entity backed by the gltf asset living in `res/<dir>`
    pub(super) fn asset_entity(world: &mut World, dir: &str) -> EntityHandle {
        entities_sharing_asset(world, dir, 1)[0]
    }

    /// `count` entities all backed by the *same* registered asset
    pub(super) fn entities_sharing_asset(
        world: &mut World,
        dir: &str,
        count: usize,
    ) -> Vec<EntityHandle> {
        let backing = world
            .register_asset::<GltfAsset>(dir)
            .expect("asset should register");
        (0..count)
            .map(|_| {
                let entity = world.entity_manager.new_entity().expect("entity handle");
                world.entity_manager.add_mesh_collection_for_entity(
                    &entity,
                    MeshCollectionDescriptor::new(backing.clone(), MeshAcessor::All),
                );
                entity
            })
            .collect()
    }

    /// the single asset behind an entity built by [`asset_entity`]
    pub(super) fn only_asset(world: &World, entity: EntityHandle) -> AssetHandle {
        let assets = world.entity_manager.rbcs_of(entity);
        assert_eq!(assets.len(), 1, "test entity should own exactly one asset");
        *assets.iter().next().unwrap()
    }

    pub(super) fn spawn_at(entity: EntityHandle, x: f32) -> Spawn<dyn Archetype> {
        Spawn {
            entity,
            data: Box::new(APosition {
                position: cgmath::Matrix4::<f32>::from_translation(cgmath::vec3(x, 0., 0.)).into(),
            }),
        }
    }

    /// hand roll a scene so the graph can be exercised without a SceneManager
    pub(super) fn scene_desc(
        id: usize,
        children: Vec<SceneId>,
        entities: Vec<EntityHandle>,
    ) -> SceneNew {
        SceneNew {
            id: SceneId(id),
            desc: SceneDesc {
                children,
                entities: entities.into_iter().map(|e| (e, vec![])).collect(),
            },
            runtime: SceneRuntime::new(),
        }
    }
}

#[cfg(test)]
mod scene_manager_tests {
    use super::util::{asset_entity, spawn_at};
    use crate::world::{
        scene::{SceneEvent, SceneId, SceneLoadLevel, builder::SceneBuilder},
        world::World,
    };

    #[test]
    fn add_scene_registers_the_scene_and_a_root() {
        let mut world = World::new();
        let entity = asset_entity(&mut world, "box");

        let scene_id = SceneBuilder::new(&mut world)
            .add_entity(entity)
            .create(&mut world)
            .expect("scene");

        assert_eq!(scene_id, SceneId(0));
        assert_eq!(world.scene_manager.scene_count(), 1);

        let scene = world.scene_manager.scene(scene_id).expect("scene exists");
        assert_eq!(scene.desc.entities.len(), 1);
        assert_eq!(scene.desc.entities[0].0, entity);

        assert_eq!(world.scene_manager.graph().root_ids(), vec![scene_id]);
    }

    #[test]
    fn set_load_level_marks_dirty_and_queues_an_event() {
        let mut world = World::new();
        let entity = asset_entity(&mut world, "box");
        let scene_id = SceneBuilder::new(&mut world)
            .add_entity(entity)
            .create(&mut world)
            .expect("scene");
        assert!(!world.scene_manager.is_dirty(scene_id));

        world
            .scene_manager
            .set_load_level(scene_id, SceneLoadLevel::GPU)
            .expect("load level should be settable");

        assert!(world.scene_manager.is_dirty(scene_id));
        let scene = world.scene_manager.scene(scene_id).expect("scene exists");
        assert_eq!(scene.runtime.requested_level, SceneLoadLevel::GPU);
        assert_eq!(scene.runtime.event_queue.len(), 1);
        assert!(matches!(
            scene.runtime.event_queue[0],
            SceneEvent::LoadLevelChanged(SceneLoadLevel::NotLoaded, SceneLoadLevel::GPU)
        ));
    }

    #[test]
    fn add_instances_records_spawn_data_against_the_entity() {
        let mut world = World::new();
        let entity = asset_entity(&mut world, "box");
        let scene_id = SceneBuilder::new(&mut world)
            .add_entity(entity)
            .create(&mut world)
            .expect("scene");

        world
            .add_instances(scene_id, vec![spawn_at(entity, 0.), spawn_at(entity, 3.)])
            .expect("spawns should be accepted");

        let scene = world.scene_manager.scene(scene_id).expect("scene exists");
        assert_eq!(scene.desc.entities[0].1.len(), 2);
    }
}

#[cfg(test)]
mod dependency_graph_tests {
    use super::util::{asset_entity, entities_sharing_asset, scene_desc};
    use crate::world::{
        entity_manager::entity_manager::EntityManager,
        scene::{SceneId, dependency_graph::DependencyGraph},
        world::World,
    };

    #[test]
    fn nested_scenes_hang_off_their_parent() {
        let mut graph = DependencyGraph::new();
        let entity_manager = EntityManager::new();

        graph
            .add_scene(&scene_desc(0, vec![], vec![]), &entity_manager)
            .expect("child scene");
        graph
            .add_scene(&scene_desc(1, vec![], vec![]), &entity_manager)
            .expect("sibling scene");
        graph
            .add_scene(&scene_desc(2, vec![SceneId(0)], vec![]), &entity_manager)
            .expect("parent scene");

        let mut roots = graph.root_ids();
        roots.sort_by_key(|id| id.0);
        assert_eq!(roots, vec![SceneId(1), SceneId(2)]);
        assert_eq!(graph.child_ids_of(SceneId(2)), Some(vec![SceneId(0)]));
        assert_eq!(
            graph.child_ids_of(SceneId(0)),
            None,
            "a nested scene is no longer a root"
        );
    }

    #[test]
    fn scene_entities_and_their_assets_get_nodes() {
        let mut world = World::new();
        let entity = asset_entity(&mut world, "box");
        assert_eq!(
            world.entity_manager.rbcs_of(entity).len(),
            1,
            "test entity should own exactly one asset"
        );
        let scene = scene_desc(0, vec![], vec![entity]);

        world
            .scene_manager
            .graph_mut()
            .add_scene(&scene, &world.entity_manager)
            .expect("scene should be addable");

        let graph = world.scene_manager.graph();
        // node vecs are indexed by handle, so they have to reach past the handle itself
        assert!(
            graph.entity_node_count() > entity.0 as usize,
            "expected a node for entity {:?}, got {} nodes",
            entity,
            graph.entity_node_count()
        );
        assert!(
            graph.asset_node_count() >= 1,
            "the box asset should be tracked by the graph"
        );
    }

    #[test]
    fn an_asset_shared_by_two_entities_is_a_single_node() {
        let mut world = World::new();
        let entities = entities_sharing_asset(&mut world, "box", 2);
        let scene = scene_desc(0, vec![], entities);

        world
            .scene_manager
            .graph_mut()
            .add_scene(&scene, &world.entity_manager)
            .expect("scene should be addable");

        assert_eq!(
            world.scene_manager.graph().asset_node_count(),
            1,
            "one AssetHandle should mean one node, however many entities point at it"
        );
    }
}

/// What a load level request is supposed to do to the graph.
///
/// ASSUMPTION baked into these: `rc` is `(cpu_holders, gpu_holders)`, and a
/// holder counts in exactly one tier — so the residency an asset actually needs
/// is GPU if `rc.1 > 0`, CPU if `rc.0 > 0`, otherwise nothing. If you'd rather
/// have GPU imply a CPU ref too, the expected pairs below are the only thing
/// that changes.
///
/// These also assume the refs land during `set_load_level`. If propagation ends
/// up deferred to a flush/update pass instead, these need that call added.
#[cfg(test)]
mod load_level_ref_count_tests {
    use super::util::{asset_entity, only_asset};
    use crate::world::{
        scene::{SceneLoadLevel, builder::SceneBuilder},
        world::World,
    };

    /// a node that was never created counts as unreferenced
    fn rc(node: Option<(usize, usize)>) -> (usize, usize) {
        node.unwrap_or((0, 0))
    }

    #[test]
    fn loading_a_scene_ref_counts_its_entities_and_assets() {
        let mut world = World::new();
        let entity = asset_entity(&mut world, "box");
        let asset = only_asset(&world, entity);
        let scene_id = SceneBuilder::new(&mut world)
            .add_entity(entity)
            .create(&mut world)
            .expect("scene");

        let graph = world.scene_manager.graph();
        assert_eq!(
            rc(graph.scene_rc(scene_id)),
            (0, 0),
            "nothing asked for yet"
        );
        assert_eq!(rc(graph.entity_rc(entity)), (0, 0));
        assert_eq!(rc(graph.asset_rc(asset)), (0, 0));

        world
            .scene_manager
            .set_load_level(scene_id, SceneLoadLevel::CPU)
            .expect("load level should be settable");

        let graph = world.scene_manager.graph();
        assert_eq!(rc(graph.scene_rc(scene_id)), (1, 0), "scene holds itself");
        assert_eq!(
            rc(graph.entity_rc(entity)),
            (1, 0),
            "the scene should hold its entity"
        );
        assert_eq!(
            rc(graph.asset_rc(asset)),
            (1, 0),
            "the entity should hold its asset"
        );
    }

    #[test]
    fn raising_the_level_moves_the_refs_to_the_gpu_tier() {
        let mut world = World::new();
        let entity = asset_entity(&mut world, "box");
        let asset = only_asset(&world, entity);
        let scene_id = SceneBuilder::new(&mut world)
            .add_entity(entity)
            .create(&mut world)
            .expect("scene");

        world
            .scene_manager
            .set_load_level(scene_id, SceneLoadLevel::CPU)
            .expect("load level should be settable");
        world
            .scene_manager
            .set_load_level(scene_id, SceneLoadLevel::GPU)
            .expect("load level should be settable");

        let graph = world.scene_manager.graph();
        assert_eq!(rc(graph.scene_rc(scene_id)), (0, 1));
        assert_eq!(rc(graph.entity_rc(entity)), (0, 1));
        assert_eq!(
            rc(graph.asset_rc(asset)),
            (0, 1),
            "one holder shouldnt be counted at two levels at once"
        );
    }

    #[test]
    fn unloading_a_scene_releases_every_ref() {
        let mut world = World::new();
        let entity = asset_entity(&mut world, "box");
        let asset = only_asset(&world, entity);
        let scene_id = SceneBuilder::new(&mut world)
            .add_entity(entity)
            .create(&mut world)
            .expect("scene");

        world
            .scene_manager
            .set_load_level(scene_id, SceneLoadLevel::GPU)
            .expect("load level should be settable");
        world
            .scene_manager
            .set_load_level(scene_id, SceneLoadLevel::NotLoaded)
            .expect("load level should be settable");

        let graph = world.scene_manager.graph();
        assert_eq!(rc(graph.scene_rc(scene_id)), (0, 0));
        assert_eq!(rc(graph.entity_rc(entity)), (0, 0));
        assert_eq!(
            rc(graph.asset_rc(asset)),
            (0, 0),
            "the asset has no holders left and should be evictable"
        );
    }

    #[test]
    fn two_scenes_sharing_an_entity_each_hold_a_ref() {
        let mut world = World::new();
        let shared = asset_entity(&mut world, "box");
        let asset = only_asset(&world, shared);

        let scene_a = SceneBuilder::new(&mut world)
            .add_entity(shared)
            .create(&mut world)
            .expect("scene a");
        let scene_b = SceneBuilder::new(&mut world)
            .add_entity(shared)
            .create(&mut world)
            .expect("scene b");

        world
            .scene_manager
            .set_load_level(scene_a, SceneLoadLevel::CPU)
            .expect("load level should be settable");
        world
            .scene_manager
            .set_load_level(scene_b, SceneLoadLevel::CPU)
            .expect("load level should be settable");

        assert_eq!(rc(world.scene_manager.graph().entity_rc(shared)), (2, 0));

        // dropping one scene must not pull the shared entity out from under the other
        world
            .scene_manager
            .set_load_level(scene_a, SceneLoadLevel::NotLoaded)
            .expect("load level should be settable");

        let graph = world.scene_manager.graph();
        assert_eq!(rc(graph.scene_rc(scene_a)), (0, 0));
        assert_eq!(
            rc(graph.entity_rc(shared)),
            (1, 0),
            "scene b still needs it"
        );
        assert!(
            rc(graph.asset_rc(asset)).0 >= 1,
            "the asset is still in use and must stay resident"
        );

        world
            .scene_manager
            .set_load_level(scene_b, SceneLoadLevel::NotLoaded)
            .expect("load level should be settable");

        let graph = world.scene_manager.graph();
        assert_eq!(rc(graph.entity_rc(shared)), (0, 0));
        assert_eq!(rc(graph.asset_rc(asset)), (0, 0));
    }

    #[test]
    fn a_scene_load_reaches_through_to_its_children() {
        let mut world = World::new();
        let child_entity = asset_entity(&mut world, "box");
        let child_asset = only_asset(&world, child_entity);

        let child = SceneBuilder::new(&mut world)
            .add_entity(child_entity)
            .create(&mut world)
            .expect("child scene");
        let parent = SceneBuilder::new(&mut world)
            .add_child(child)
            .create(&mut world)
            .expect("parent scene");

        world
            .scene_manager
            .set_load_level(parent, SceneLoadLevel::GPU)
            .expect("load level should be settable");

        let graph = world.scene_manager.graph();
        assert_eq!(rc(graph.scene_rc(parent)), (0, 1));
        assert_eq!(
            rc(graph.scene_rc(child)),
            (0, 1),
            "loading a scene should load the scenes it depends on"
        );
        assert_eq!(rc(graph.entity_rc(child_entity)), (0, 1));
        assert_eq!(rc(graph.asset_rc(child_asset)), (0, 1));
    }

    /// an asset's refs should follow the *entity's* effective level, not the
    /// level of whichever scene happened to change
    #[test]
    fn lowering_one_holder_keeps_the_asset_at_the_level_another_still_needs() {
        let mut world = World::new();
        let shared = asset_entity(&mut world, "box");
        let asset = only_asset(&world, shared);

        let scene_a = SceneBuilder::new(&mut world)
            .add_entity(shared)
            .create(&mut world)
            .expect("scene a");
        let scene_b = SceneBuilder::new(&mut world)
            .add_entity(shared)
            .create(&mut world)
            .expect("scene b");

        for scene in [scene_a, scene_b] {
            world
                .scene_manager
                .set_load_level(scene, SceneLoadLevel::CPU)
                .expect("load level should be settable");
        }
        assert_eq!(rc(world.scene_manager.graph().asset_rc(asset)), (1, 0));

        // a alone goes to GPU: the entity is now needed on the gpu
        world
            .scene_manager
            .set_load_level(scene_a, SceneLoadLevel::GPU)
            .expect("load level should be settable");
        let graph = world.scene_manager.graph();
        assert_eq!(rc(graph.entity_rc(shared)), (1, 1));
        assert_eq!(rc(graph.asset_rc(asset)), (0, 1));

        // a drops out entirely, but b still holds the entity at CPU
        world
            .scene_manager
            .set_load_level(scene_a, SceneLoadLevel::NotLoaded)
            .expect("load level should be settable");
        let graph = world.scene_manager.graph();
        assert_eq!(rc(graph.entity_rc(shared)), (1, 0));
        assert_eq!(
            rc(graph.asset_rc(asset)),
            (1, 0),
            "b still needs this asset on the cpu, it must not be evicted"
        );
    }

    #[test]
    fn releasing_holders_at_different_levels_clears_the_asset() {
        let mut world = World::new();
        let shared = asset_entity(&mut world, "box");
        let asset = only_asset(&world, shared);

        let scene_a = SceneBuilder::new(&mut world)
            .add_entity(shared)
            .create(&mut world)
            .expect("scene a");
        let scene_b = SceneBuilder::new(&mut world)
            .add_entity(shared)
            .create(&mut world)
            .expect("scene b");

        world
            .scene_manager
            .set_load_level(scene_a, SceneLoadLevel::CPU)
            .expect("load level should be settable");
        world
            .scene_manager
            .set_load_level(scene_b, SceneLoadLevel::GPU)
            .expect("load level should be settable");

        let graph = world.scene_manager.graph();
        assert_eq!(rc(graph.entity_rc(shared)), (1, 1));
        assert_eq!(
            rc(graph.asset_rc(asset)),
            (0, 1),
            "one entity holds this asset, and it needs it on the gpu"
        );

        for scene in [scene_a, scene_b] {
            world
                .scene_manager
                .set_load_level(scene, SceneLoadLevel::NotLoaded)
                .expect("load level should be settable");
        }

        let graph = world.scene_manager.graph();
        assert_eq!(rc(graph.entity_rc(shared)), (0, 0));
        assert_eq!(
            rc(graph.asset_rc(asset)),
            (0, 0),
            "nothing holds the asset anymore, no refs may be left behind"
        );
    }

    #[test]
    fn setting_the_same_level_twice_doesnt_double_count() {
        let mut world = World::new();
        let entity = asset_entity(&mut world, "box");
        let asset = only_asset(&world, entity);
        let scene_id = SceneBuilder::new(&mut world)
            .add_entity(entity)
            .create(&mut world)
            .expect("scene");

        for _ in 0..2 {
            world
                .scene_manager
                .set_load_level(scene_id, SceneLoadLevel::GPU)
                .expect("load level should be settable");
        }

        let graph = world.scene_manager.graph();
        assert_eq!(rc(graph.scene_rc(scene_id)), (0, 1));
        assert_eq!(rc(graph.entity_rc(entity)), (0, 1));
        assert_eq!(rc(graph.asset_rc(asset)), (0, 1));
    }

    #[test]
    fn a_nested_scene_can_be_loaded_without_its_parent() {
        let mut world = World::new();
        let child_entity = asset_entity(&mut world, "box");
        let child_asset = only_asset(&world, child_entity);

        let child = SceneBuilder::new(&mut world)
            .add_entity(child_entity)
            .create(&mut world)
            .expect("child scene");
        let parent = SceneBuilder::new(&mut world)
            .add_child(child)
            .create(&mut world)
            .expect("parent scene");

        // addressing a scene that is no longer a root
        world
            .scene_manager
            .set_load_level(child, SceneLoadLevel::GPU)
            .expect("a nested scene should still be addressable by id");

        let graph = world.scene_manager.graph();
        assert_eq!(rc(graph.scene_rc(child)), (0, 1));
        assert_eq!(rc(graph.entity_rc(child_entity)), (0, 1));
        assert_eq!(rc(graph.asset_rc(child_asset)), (0, 1));
        assert_eq!(
            rc(graph.scene_rc(parent)),
            (0, 0),
            "loading a scene shouldnt load the scene that depends on it"
        );
    }

    #[test]
    fn propagation_reaches_grandchildren() {
        let mut world = World::new();
        let leaf_entity = asset_entity(&mut world, "box");
        let leaf_asset = only_asset(&world, leaf_entity);

        let leaf = SceneBuilder::new(&mut world)
            .add_entity(leaf_entity)
            .create(&mut world)
            .expect("leaf scene");
        let middle = SceneBuilder::new(&mut world)
            .add_child(leaf)
            .create(&mut world)
            .expect("middle scene");
        let top = SceneBuilder::new(&mut world)
            .add_child(middle)
            .create(&mut world)
            .expect("top scene");

        world
            .scene_manager
            .set_load_level(top, SceneLoadLevel::CPU)
            .expect("load level should be settable");

        let graph = world.scene_manager.graph();
        assert_eq!(rc(graph.scene_rc(middle)), (1, 0));
        assert_eq!(
            rc(graph.scene_rc(leaf)),
            (1, 0),
            "walk must not stop at depth 1"
        );
        assert_eq!(rc(graph.entity_rc(leaf_entity)), (1, 0));
        assert_eq!(rc(graph.asset_rc(leaf_asset)), (1, 0));

        world
            .scene_manager
            .set_load_level(top, SceneLoadLevel::NotLoaded)
            .expect("load level should be settable");

        let graph = world.scene_manager.graph();
        assert_eq!(rc(graph.scene_rc(leaf)), (0, 0));
        assert_eq!(rc(graph.entity_rc(leaf_entity)), (0, 0));
        assert_eq!(rc(graph.asset_rc(leaf_asset)), (0, 0));
    }
}
