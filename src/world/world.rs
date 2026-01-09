use crate::world::camera::Camera;

pub struct World {
    camera: Camera,
}

impl World {
    pub fn new(aspect_ratio: f32, device: &wgpu::Device) -> Self {
        let mut camera = crate::world::camera::get_camera_default();
        camera.build_camera_uniform(aspect_ratio, device);
        Self { camera }
    }
}
