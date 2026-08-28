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
        scene::{Scene, SceneLoadLevel, builder::SceneBuilder, scene::Spawn},
    },
};

impl Scene {
    pub fn buggy(
        world: &mut crate::world::world::World,
    ) -> Result<(), crate::world::WorldInitError> {
        let buggy_asset = world.register_asset::<GltfAsset>("buggy")?;
        let buggy_entity = world.entity_manager.new_entity()?;
        world.entity_manager.add_mesh_collection_for_entity(
            &buggy_entity,
            MeshCollectionDescriptor::new(buggy_asset, MeshAcessor::All),
        );

        let mut builder = SceneBuilder::new();
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
                        entity: buggy_entity,
                        data: Box::new(APosition {
                            position: (cgmath::Matrix4::<f32>::from_scale(0.02)).into(),
                        }),
                    },
                    Spawn {
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
            .set_load_level(scene_id, SceneLoadLevel::GPU, &world.asset_manager)?;

        Ok(())
    }

    pub fn box_scene(
        world: &mut crate::world::world::World,
    ) -> Result<(), crate::world::WorldInitError> {
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
        //
        let builder = SceneBuilder::new();

        let scene_id = builder.add_entity(box_entity).create(world)?;

        world.add_instances(
            scene_id,
            vec![Spawn {
                entity: box_entity,
                data: Box::new(APosition {
                    position: cgmath::Matrix4::<f32>::identity().into(),
                }),
            }],
        )?;

        world
            .scene_manager
            .set_load_level(scene_id, SceneLoadLevel::GPU, &world.asset_manager)?;

        Ok(())
    }

    pub fn multi_box_scene(
        world: &mut crate::world::world::World,
    ) -> Result<(), crate::world::WorldInitError> {
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

        let builder = SceneBuilder::new();
        let id = builder.add_entity(box_entity).create(world)?;
        let mut res = Vec::<Spawn<dyn Archetype>>::new();
        for i in -4..4 {
            for j in -4..4 {
                for k in 0..100 {
                    let x = 2. * i as f32;
                    let y = 2. * j as f32;
                    let z = -2. * k as f32;
                    res.push(Spawn {
                        entity: EntityHandle(0),
                        data: Box::new(APosition {
                            position: (cgmath::Matrix4::<f32>::from_translation(
                                cgmath::Vector3::new(x, y, z),
                            ) * cgmath::Matrix4::<f32>::from_scale(0.5))
                            .into(),
                        }),
                    })
                }
            }
        }
        world.add_instances(id, res)?;
        world
            .scene_manager
            .set_load_level(id, SceneLoadLevel::GPU, &world.asset_manager)?;

        Ok(())
    }
    #[cfg(test)]
    pub fn fox_scene(world: &mut World) -> Result<(), WorldInitError> {
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

        let builder = SceneBuilder::new();
        let id = builder.add_entity(fox_entity).create(world)?;
        world.add_instances(
            id,
            vec![Spawn {
                entity: EntityHandle(0),
                data: Box::new(APosition {
                    position: cgmath::Matrix4::<f32>::from_scale(0.05).into(),
                }),
            }],
        )?;

        world
            .scene_manager
            .set_load_level(id, SceneLoadLevel::GPU, &world.asset_manager)?;
        Ok(())
    }

    pub fn fox_animated_scene(
        world: &mut crate::world::world::World,
    ) -> Result<(), crate::world::WorldInitError> {
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
        let builder = SceneBuilder::new();
        let id = builder.add_entity(fox_entity).create(world)?;

        world.add_instances(
            id,
            vec![
                (
                    EntityHandle(0),
                    Box::new(APosition {
                        position: cgmath::Matrix4::<f32>::from_scale(0.05).into(),
                    }),
                )
                    .into(),
            ],
        )?;

        world
            .scene_manager
            .set_load_level(id, SceneLoadLevel::GPU, &world.asset_manager)?;
        Ok(())
    }

    pub fn fox_box(
        world: &mut crate::world::world::World,
    ) -> Result<(), crate::world::WorldInitError> {
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
        let builder = SceneBuilder::new();
        let id = builder
            .add_entity(box_entity)
            .add_entity(fox_entity)
            .create(world)?;

        world.add_instances(
            id,
            vec![
                (
                    box_entity,
                    Box::new(APosition {
                        position: cgmath::Matrix4::<f32>::identity().into(),
                    }),
                )
                    .into(),
                (
                    fox_entity,
                    Box::new(APosition {
                        position: (cgmath::Matrix4::<f32>::from_translation(cgmath::Vector3::new(
                            1.5, 0.0, 0.0,
                        )) * cgmath::Matrix4::<f32>::from_scale(0.05))
                        .into(),
                    }),
                )
                    .into(),
            ],
        )?;
        world
            .scene_manager
            .set_load_level(id, SceneLoadLevel::GPU, &world.asset_manager)?;
        Ok(())
    }

    pub fn box_animated(
        world: &mut crate::world::world::World,
    ) -> Result<(), crate::world::WorldInitError> {
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

        let id = SceneBuilder::new()
            .add_entity(box_anim_entity)
            .create(world)?;

        world.add_instances(
            id,
            vec![
                (
                    box_anim_entity.clone(),
                    Box::new(APosition {
                        position: cgmath::Matrix4::<f32>::identity().into(),
                    }),
                )
                    .into(),
            ],
        )?;

        world
            .scene_manager
            .set_load_level(id, SceneLoadLevel::GPU, &world.asset_manager)?;

        Ok(())
    }

    pub fn independant_foxes(
        world: &mut crate::world::world::World,
    ) -> Result<(), crate::world::WorldInitError> {
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

        let id = SceneBuilder::new().add_entity(fox_entity).create(world)?;
        world.add_instances(
            id,
            vec![
                (
                    EntityHandle(0),
                    Box::new(APosition {
                        position: cgmath::Matrix4::<f32>::from_scale(0.05).into(),
                    }),
                )
                    .into(),
                (
                    EntityHandle(0),
                    Box::new(APosition {
                        position: (cgmath::Matrix4::<f32>::from_translation(cgmath::vec3(
                            3., 0., 0.,
                        )) * cgmath::Matrix4::<f32>::from_scale(0.05))
                        .into(),
                    }),
                )
                    .into(),
                (
                    EntityHandle(0),
                    Box::new(APosition {
                        position: (cgmath::Matrix4::<f32>::from_translation(cgmath::vec3(
                            -3., 0., 0.,
                        )) * cgmath::Matrix4::<f32>::from_scale(0.05))
                        .into(),
                    }),
                )
                    .into(),
            ],
        )?;

        world
            .scene_manager
            .set_load_level(id, SceneLoadLevel::GPU, &world.asset_manager)?;
        Ok(())
    }
}
