use super::scene::Scene;
use crate::{
    asset_manager::{
        asset_manager::{Asset, AssetHandle, AssetLoadError, AssetManager},
        gltf_assets::gltf_model::GltfAsset,
    },
    util::types::Mat4F32,
    world::{
        camera::Camera,
        components::{MeshCollectionComponent, ResourceBacking},
        entity_manager::{EntityHandle, EntityManager, EntityManagerError},
        scene::SceneLoadLevel,
    },
};

enum WorldInitError {
    AssetFailure(AssetLoadError),
    EntityFailure(EntityManagerError),
}

impl From<AssetLoadError> for WorldInitError {
    fn from(value: AssetLoadError) -> Self {
        Self::AssetFailure(value)
    }
}
impl From<EntityManagerError> for WorldInitError {
    fn from(value: EntityManagerError) -> Self {
        Self::EntityFailure(value)
    }
}

pub struct World {
    camera: Camera,
    scene: Scene,
    asset_manager: AssetManager,
    entity_manager: EntityManager,
}

impl World {
    pub fn new(aspect_ratio: f32, device: &wgpu::Device) -> Result<Self, WorldInitError> {
        let mut camera = crate::world::camera::get_camera_default();
        camera.build_camera_uniform(aspect_ratio, device);

        let mut asset_manager = AssetManager::new();
        let mut entity_manager = EntityManager::new();

        let box_asset = asset_manager.register_asset::<GltfAsset>("box")?;

        let mesh = MeshCollectionComponent::new(ResourceBacking::new(box_asset, 0));

        let box_entity = entity_manager.new_entity()?;
        entity_manager.add_mesh_collection_for_entity(box_entity, mesh);
        let mut scene = Scene::new();
        scene.add_entity(box_entity);
        scene.set_load_level(SceneLoadLevel::GPU);

        Ok(Self {
            camera,
            scene,
            asset_manager,
            entity_manager,
        })
    }

    pub fn include_asset<A: Asset + 'static>(
        &mut self,
        dir_name: &str,
    ) -> Result<AssetHandle, AssetLoadError> {
        self.asset_manager.register_asset::<A>(dir_name)
    }

    // pub fn add_resource_backed_entity<C: ExtractComponents>(
    //     asset_manager: &mut AssetManager,
    //     asset_handle: AssetHandle,
    // ) -> Result<EntityHandle, AssetLoadError> {
    //     let components: C::Output = C::extract_from(asset_manager, &asset_handle)?;
    //     todo!()
    // }
}

pub struct EntityBuilder<'m> {
    entity_manager: &'m mut EntityManager,
    asset_manger: &'m mut AssetManager,
}

impl<'m> EntityBuilder<'m> {
    pub fn create_physical_entity(
        &mut self,
        mesh: MeshCollectionComponent,
        physical_position: Mat4F32,
    ) -> Result<EntityHandle, EntityManagerError> {
        let entity = self.entity_manager.new_entity()?;
        self.entity_manager
            .add_mesh_collection_for_entity(entity, mesh);
        Ok(entity)
    }
}
