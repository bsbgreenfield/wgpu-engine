use cgmath::{SquareMatrix, Vector3};

#[cfg(test)]
use crate::world::{WorldInitError, world::World};
use crate::{
    asset_manager::gltf_asset::GltfAsset,
    common::entity::EntityHandle,
    world::{
        entity_manager::components::{
            AnimationAccessor, AnimationComponentDescriptor, AnimationMode, MeshAcessor,
            MeshCollectionDescriptor,
        },
        instance_manager::archetypes::{APosition, Archetype},
        scene::{
            SceneLoadLevel, SceneNew,
            builder::SceneBuilder,
            scene::{Scene, Spawn},
        },
    },
};

impl SceneNew {
    pub fn create_buggy(
        world: &mut crate::world::world::World,
    ) -> Result<(), crate::world::WorldInitError> {
        let buggy_asset = world.register_asset::<GltfAsset>("buggy")?;
        let buggy_entity = world.entity_manager.new_entity()?;
        world.entity_manager.add_mesh_collection_for_entity(
            &buggy_entity,
            MeshCollectionDescriptor::new(buggy_asset, MeshAcessor::All),
        );

        let mut builder = SceneBuilder::new(world);
        builder = builder.add_entity(buggy_entity);

        let brain_asset = world.register_asset::<GltfAsset>("brain")?;
        let brain_entity = world.entity_manager.new_entity()?;

        world.entity_manager.add_mesh_collection_for_entity(
            &brain_entity,
            MeshCollectionDescriptor::new(brain_asset.clone(), MeshAcessor::All).with_animation(
                AnimationComponentDescriptor {
                    resource_backing: brain_asset,
                    accessor: AnimationAccessor::All,
                    rigid_animation_mode: AnimationMode::Shared,
                    skinned_animation_mode: AnimationMode::Shared,
                },
            ),
        );
        builder = builder.add_entity(brain_entity);
        let scene_id = builder.create(world)?;

        world
            .add_instances(
                scene_id,
                vec![
                    Spawn {
                        scene_id,
                        entity: buggy_entity,
                        data: Box::new(APosition {
                            position: (cgmath::Matrix4::<f32>::from_scale(0.02)).into(),
                        }),
                    },
                    Spawn {
                        scene_id,
                        entity: brain_entity,
                        data: Box::new(APosition {
                            position: cgmath::Matrix4::<f32>::from_translation(
                                cgmath::Vector3::new(3., 0., 0.),
                            )
                            .into(),
                        }),
                    },
                ],
            )
            .map_err(|e| crate::world::WorldInitError::SceneCreationFailure(e))?;

        world
            .scene_manager
            .set_load_level(scene_id, SceneLoadLevel::GPU)?;

        Ok(())
    }
}

impl Scene {
    pub fn buggy(
        world: &mut crate::world::world::World,
    ) -> Result<Self, crate::world::WorldInitError> {
        let buggy_asset = world.register_asset::<GltfAsset>("buggy")?;
        let buggy_entity = world.entity_manager.new_entity()?;
        world.entity_manager.add_mesh_collection_for_entity(
            &buggy_entity,
            MeshCollectionDescriptor::new(buggy_asset, MeshAcessor::All),
        );

        let mut scene = Scene::new();
        scene.add_entity(buggy_entity);

        let brain_asset = world.register_asset::<GltfAsset>("brain")?;
        let brain_entity = world.entity_manager.new_entity()?;

        world.entity_manager.add_mesh_collection_for_entity(
            &brain_entity,
            MeshCollectionDescriptor::new(brain_asset.clone(), MeshAcessor::All).with_animation(
                AnimationComponentDescriptor {
                    resource_backing: brain_asset,
                    accessor: AnimationAccessor::All,
                    rigid_animation_mode: AnimationMode::Shared,
                    skinned_animation_mode: AnimationMode::Shared,
                },
            ),
        );
        scene.add_entity(brain_entity);
        scene.spawn(vec![
            (
                buggy_entity,
                Box::new(APosition {
                    position: (cgmath::Matrix4::<f32>::from_scale(0.02)).into(),
                }),
            ),
            (
                brain_entity,
                Box::new(APosition {
                    position: cgmath::Matrix4::<f32>::from_translation(cgmath::Vector3::new(
                        3., 0., 0.,
                    ))
                    .into(),
                }),
            ),
        ]);

        Ok(scene)
    }

    pub fn brain(
        world: &mut crate::world::world::World,
    ) -> Result<Self, crate::world::WorldInitError> {
        let brain_asset = world.register_asset::<GltfAsset>("brain")?;
        let brain_entity = world.entity_manager.new_entity()?;

        world.entity_manager.add_mesh_collection_for_entity(
            &brain_entity,
            MeshCollectionDescriptor::new(brain_asset.clone(), MeshAcessor::All).with_animation(
                AnimationComponentDescriptor {
                    resource_backing: brain_asset,
                    accessor: AnimationAccessor::All,
                    rigid_animation_mode: AnimationMode::Shared,
                    skinned_animation_mode: AnimationMode::Shared,
                },
            ),
        );

        let mut scene = Scene::new();
        scene.add_entity(brain_entity);

        scene.spawn(vec![(
            brain_entity,
            Box::new(APosition {
                position: cgmath::Matrix4::<f32>::from_scale(3.).into(),
            }),
        )]);

        Ok(scene)
    }
    pub fn fox_box(
        world: &mut crate::world::world::World,
    ) -> Result<Self, crate::world::WorldInitError> {
        use cgmath::SquareMatrix;

        let box_asset = world.register_asset::<GltfAsset>("box")?;
        let fox_asset = world.register_asset::<GltfAsset>("fox")?;

        let box_entity = world.entity_manager.new_entity()?;
        let fox_entity = world.entity_manager.new_entity()?;

        world.entity_manager.add_mesh_collection_for_entity(
            &box_entity,
            MeshCollectionDescriptor::new(box_asset, MeshAcessor::All),
        ); // mesh
        world.entity_manager.add_mesh_collection_for_entity(
            &fox_entity,
            MeshCollectionDescriptor::new(fox_asset, MeshAcessor::All),
        ); // mesh

        let mut scene = Scene::new();
        scene.add_entity(box_entity);
        scene.add_entity(fox_entity);
        scene.spawn(vec![
            (
                box_entity,
                Box::new(APosition {
                    position: cgmath::Matrix4::<f32>::identity().into(),
                }),
            ),
            (
                fox_entity,
                Box::new(APosition {
                    position: (cgmath::Matrix4::<f32>::from_translation(cgmath::Vector3::new(
                        1.5, 0.0, 0.0,
                    )) * cgmath::Matrix4::<f32>::from_scale(0.05))
                    .into(),
                }),
            ),
        ]);
        Ok(scene)
    }

    pub fn box_animated(
        world: &mut crate::world::world::World,
    ) -> Result<Self, crate::world::WorldInitError> {
        use cgmath::SquareMatrix;

        let box_anim_asset = world.register_asset::<GltfAsset>("box_animated")?;
        let box_anim_entity = world.entity_manager.new_entity()?;
        world.entity_manager.add_mesh_collection_for_entity(
            &box_anim_entity,
            MeshCollectionDescriptor::new(box_anim_asset.clone(), MeshAcessor::All).with_animation(
                AnimationComponentDescriptor {
                    accessor: AnimationAccessor::All,
                    resource_backing: box_anim_asset,
                    rigid_animation_mode: AnimationMode::Shared,
                    skinned_animation_mode: AnimationMode::Shared,
                },
            ),
        );

        let mut scene = Scene::new();
        scene.add_entity(box_anim_entity);

        scene.spawn(vec![(
            box_anim_entity.clone(),
            Box::new(APosition {
                position: cgmath::Matrix4::<f32>::identity().into(),
            }),
        )]);

        Ok(scene)
    }

    pub fn box_animated_shared(
        world: &mut crate::world::world::World,
    ) -> Result<Self, crate::world::WorldInitError> {
        let box_anim_asset = world.register_asset::<GltfAsset>("box_animated")?;

        let entity1 = world.entity_manager.new_entity()?;

        world.entity_manager.add_mesh_collection_for_entity(
            &entity1,
            MeshCollectionDescriptor::new(box_anim_asset.clone(), MeshAcessor::All).with_animation(
                AnimationComponentDescriptor {
                    resource_backing: box_anim_asset.clone(),
                    accessor: AnimationAccessor::All,
                    rigid_animation_mode: AnimationMode::Shared,
                    skinned_animation_mode: AnimationMode::Shared,
                },
            ),
        );

        let mut scene = Scene::new();

        scene.add_entity(entity1);

        scene.spawn(vec![
            (
                entity1,
                Box::new(APosition {
                    position: cgmath::Matrix4::<f32>::identity().into(),
                }),
            ),
            (
                entity1,
                Box::new(APosition {
                    position: cgmath::Matrix4::<f32>::from_translation(cgmath::vec3(3., 0., 0.))
                        .into(),
                }),
            ),
            (
                entity1,
                Box::new(APosition {
                    position: cgmath::Matrix4::<f32>::from_translation(cgmath::vec3(-3., 0., 0.))
                        .into(),
                }),
            ),
        ]);

        Ok(scene)
    }

    pub fn independant_foxes(
        world: &mut crate::world::world::World,
    ) -> Result<Self, crate::world::WorldInitError> {
        let fox_asset = world.register_asset::<GltfAsset>("fox")?;
        let fox_entity = world.entity_manager.new_entity()?;
        let mcc = MeshCollectionDescriptor::new(fox_asset.clone(), MeshAcessor::All)
            .with_animation(AnimationComponentDescriptor {
                resource_backing: fox_asset,
                accessor: AnimationAccessor::All,
                rigid_animation_mode: AnimationMode::Independent,
                skinned_animation_mode: AnimationMode::Independent,
            });

        world
            .entity_manager
            .add_mesh_collection_for_entity(&fox_entity, mcc); // mesh

        let mut scene = Scene::new();
        scene.add_entity(fox_entity);
        scene.spawn(vec![
            (
                EntityHandle(0),
                Box::new(APosition {
                    position: cgmath::Matrix4::<f32>::from_scale(0.05).into(),
                }),
            ),
            (
                EntityHandle(0),
                Box::new(APosition {
                    position: (cgmath::Matrix4::<f32>::from_translation(cgmath::vec3(3., 0., 0.))
                        * cgmath::Matrix4::<f32>::from_scale(0.05))
                    .into(),
                }),
            ),
            (
                EntityHandle(0),
                Box::new(APosition {
                    position: (cgmath::Matrix4::<f32>::from_translation(cgmath::vec3(-3., 0., 0.))
                        * cgmath::Matrix4::<f32>::from_scale(0.05))
                    .into(),
                }),
            ),
        ]);
        Ok(scene)
    }

    pub fn shared_foxes(
        world: &mut crate::world::world::World,
    ) -> Result<Self, crate::world::WorldInitError> {
        let fox_asset = world.register_asset::<GltfAsset>("fox")?;
        let fox_entity = world.entity_manager.new_entity()?;
        let mcc = MeshCollectionDescriptor::new(fox_asset.clone(), MeshAcessor::All)
            .with_animation(AnimationComponentDescriptor {
                resource_backing: fox_asset,
                accessor: AnimationAccessor::All,
                rigid_animation_mode: AnimationMode::Shared,
                skinned_animation_mode: AnimationMode::Shared,
            });

        world
            .entity_manager
            .add_mesh_collection_for_entity(&fox_entity, mcc); // mesh

        let mut scene = Scene::new();
        scene.add_entity(fox_entity);
        scene.spawn(vec![
            (
                EntityHandle(0),
                Box::new(APosition {
                    position: cgmath::Matrix4::<f32>::from_scale(0.05).into(),
                }),
            ),
            (
                EntityHandle(0),
                Box::new(APosition {
                    position: (cgmath::Matrix4::<f32>::from_translation(cgmath::vec3(3., 0., 0.))
                        * cgmath::Matrix4::<f32>::from_scale(0.05))
                    .into(),
                }),
            ),
            (
                EntityHandle(0),
                Box::new(APosition {
                    position: (cgmath::Matrix4::<f32>::from_translation(cgmath::vec3(-3., 0., 0.))
                        * cgmath::Matrix4::<f32>::from_scale(0.05))
                    .into(),
                }),
            ),
        ]);
        Ok(scene)
    }

    #[cfg(test)]
    pub fn box_scene(world: &mut World) -> Result<Self, WorldInitError> {
        use cgmath::SquareMatrix;

        let box_asset = world.register_asset::<GltfAsset>("box")?; // asset

        let box_entity = world.entity_manager.new_entity()?;

        world.entity_manager.add_mesh_collection_for_entity(
            &box_entity,
            MeshCollectionDescriptor {
                mesh_accessor: MeshAcessor::All,
                resource_backing: box_asset.erase(),
                animation: None,
            },
        ); // mesh

        let mut scene = Scene::new();
        scene.add_entity(box_entity);
        scene.spawn(vec![(
            EntityHandle(0),
            Box::new(APosition {
                position: cgmath::Matrix4::<f32>::identity().into(),
            }),
        )]);
        Ok(scene)
    }

    pub fn multi_box_scene(
        world: &mut crate::world::world::World,
    ) -> Result<Self, crate::world::WorldInitError> {
        let box_asset = world.register_asset::<GltfAsset>("box")?; // asset

        let box_entity = world.entity_manager.new_entity()?;

        world.entity_manager.add_mesh_collection_for_entity(
            &box_entity,
            MeshCollectionDescriptor {
                mesh_accessor: MeshAcessor::All,
                resource_backing: box_asset.erase(),
                animation: None,
            },
        ); // mesh

        let mut scene = Scene::new();
        scene.add_entity(box_entity);
        let mut res = Vec::<(EntityHandle, Box<dyn Archetype>)>::new();
        for i in -4..4 {
            for j in -4..4 {
                for k in 0..100 {
                    let x = 2. * i as f32;
                    let y = 2. * j as f32;
                    let z = -2. * k as f32;
                    res.push((
                        EntityHandle(0),
                        Box::new(APosition {
                            position: (cgmath::Matrix4::<f32>::from_translation(
                                cgmath::Vector3::new(x, y, z),
                            ) * cgmath::Matrix4::<f32>::from_scale(0.5))
                            .into(),
                        }),
                    ));
                }
            }
        }
        scene.spawn(res);

        Ok(scene)
    }
    pub fn multi_fox_scene(
        world: &mut crate::world::world::World,
    ) -> Result<Self, crate::world::WorldInitError> {
        let fox_asset = world.register_asset::<GltfAsset>("fox")?; // asset

        let fox_entity = world.entity_manager.new_entity()?;
        let mcc = MeshCollectionDescriptor::new(fox_asset.clone(), MeshAcessor::All)
            .with_animation(AnimationComponentDescriptor {
                resource_backing: fox_asset.clone(),
                accessor: AnimationAccessor::All,
                rigid_animation_mode: AnimationMode::Shared,
                skinned_animation_mode: AnimationMode::Shared,
            });

        world
            .entity_manager
            .add_mesh_collection_for_entity(&fox_entity, mcc); // mesh

        let mut scene = Scene::new();
        scene.add_entity(fox_entity);
        scene.spawn(vec![
            (
                EntityHandle(0),
                Box::new(APosition {
                    position: (cgmath::Matrix4::<f32>::identity()
                        * cgmath::Matrix4::<f32>::from_scale(0.05))
                    .into(),
                }),
            ),
            (
                EntityHandle(0),
                Box::new(APosition {
                    position: (cgmath::Matrix4::<f32>::from_translation(Vector3::new(0., 2., 0.))
                        * cgmath::Matrix4::<f32>::from_scale(0.05))
                    .into(),
                }),
            ),
            (
                EntityHandle(0),
                Box::new(APosition {
                    position: (cgmath::Matrix4::<f32>::from_translation(Vector3::new(2., 0., 0.))
                        * cgmath::Matrix4::<f32>::from_scale(0.05))
                    .into(),
                }),
            ),
            (
                EntityHandle(0),
                Box::new(APosition {
                    position: (cgmath::Matrix4::<f32>::from_translation(Vector3::new(2., 2., 0.))
                        * cgmath::Matrix4::<f32>::from_scale(0.05))
                    .into(),
                }),
            ),
            (
                EntityHandle(0),
                Box::new(APosition {
                    position: (cgmath::Matrix4::<f32>::from_translation(Vector3::new(-2., 2., 0.))
                        * cgmath::Matrix4::<f32>::from_scale(0.05))
                    .into(),
                }),
            ),
        ]);
        Ok(scene)
    }
    pub fn fox_animated_scene(
        world: &mut crate::world::world::World,
    ) -> Result<Self, crate::world::WorldInitError> {
        let fox_asset = world.register_asset::<GltfAsset>("fox")?; // asset

        let fox_entity = world.entity_manager.new_entity()?;

        let mcc = MeshCollectionDescriptor::new(fox_asset.clone(), MeshAcessor::All)
            .with_animation(AnimationComponentDescriptor {
                resource_backing: fox_asset,
                accessor: AnimationAccessor::All,
                rigid_animation_mode: AnimationMode::Shared,
                skinned_animation_mode: AnimationMode::Independent,
            });

        world
            .entity_manager
            .add_mesh_collection_for_entity(&fox_entity, mcc); // mesh

        let mut scene = Scene::new();
        scene.add_entity(fox_entity);
        scene.spawn(vec![(
            EntityHandle(0),
            Box::new(APosition {
                position: cgmath::Matrix4::<f32>::from_scale(0.05).into(),
            }),
        )]);
        Ok(scene)
    }
    #[cfg(test)]
    pub fn fox_scene(world: &mut World) -> Result<Self, WorldInitError> {
        let fox_asset = world.register_asset::<GltfAsset>("fox")?; // asset

        let fox_entity = world.entity_manager.new_entity()?;

        world.entity_manager.add_mesh_collection_for_entity(
            &fox_entity,
            MeshCollectionDescriptor {
                resource_backing: fox_asset.erase(),
                animation: None,
                mesh_accessor: MeshAcessor::All,
            },
        ); // mesh

        let mut scene = Scene::new();
        scene.add_entity(fox_entity);
        scene.spawn(vec![(
            EntityHandle(0),
            Box::new(APosition {
                position: cgmath::Matrix4::<f32>::from_scale(0.05).into(),
            }),
        )]);
        Ok(scene)
    }
}
