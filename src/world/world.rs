use super::scene::Scene;
use crate::{
    asset_manager::asset_manager::{AssetHandle, AssetLoadError, AssetManager},
    world::{
        camera::Camera,
        components::ExtractComponents,
        entity_manager::{EntityHandle, EntityManager},
    },
};

pub struct World {
    camera: Camera,
    scene: Scene,
    entity_manager: EntityManager,
}

impl World {
    pub fn new(aspect_ratio: f32, device: &wgpu::Device) -> Self {
        let mut camera = crate::world::camera::get_camera_default();
        camera.build_camera_uniform(aspect_ratio, device);
        Self {
            camera,
            scene: Scene::new(),
            entity_manager: EntityManager::new(),
        }
    }

    pub fn add_resource_backed_entity<C: ExtractComponents>(
        asset_manager: &mut AssetManager,
        asset_handle: AssetHandle,
    ) -> Result<EntityHandle, AssetLoadError> {
        let components: C::Output = C::extract_from(asset_manager, &asset_handle)?;
        todo!()
    }
}
