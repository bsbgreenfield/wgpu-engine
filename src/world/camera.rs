use wgpu::util::{BufferInitDescriptor, DeviceExt};

use crate::util::{pipeline::PipelineUniform, types::Mat4F32};

#[derive(Debug, Clone, Copy)]
struct CameraData {
    fov: f32,
    znear: f32,
    zfar: f32,
    eye_pos: cgmath::Point3<f32>,
    target: cgmath::Point3<f32>,
    up: cgmath::Vector3<f32>,
}

impl CameraData {
    fn new(fov: f32, znear: f32, zfar: f32) -> Self {
        Self {
            fov,
            znear,
            zfar,
            eye_pos: cgmath::Point3 {
                x: 0.0,
                y: 5.0,
                z: 10.0,
            },
            up: cgmath::Vector3::unit_y(),
            target: cgmath::Point3::new(0.0, 0.0, 0.0),
        }
    }
    fn perspective_matrix(&self, aspect_ratio: f32) -> cgmath::Matrix4<f32> {
        let view = cgmath::Matrix4::look_at_rh(self.eye_pos, self.target, self.up);
        let proj = cgmath::perspective(cgmath::Rad(self.fov), aspect_ratio, self.znear, self.zfar);

        proj * view
    }
}

pub struct Camera {
    data: CameraData,
    uniform: Option<CameraUniform>,
}

impl Camera {
    fn new(fov: f32, znear: f32, zfar: f32) -> Self {
        let camera_data = CameraData::new(fov, znear, zfar);
        Self {
            data: camera_data,
            uniform: None,
        }
    }

    pub(super) fn build_camera_uniform(&mut self, aspect_ratio: f32, device: &wgpu::Device) {
        self.uniform = Some(CameraUniform::new(&self.data, aspect_ratio, device));
    }
    pub fn get_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        CameraUniform::get_bind_group_layout(device)
    }

    pub fn get_bind_group(&self) -> &wgpu::BindGroup {
        &self.uniform.as_ref().unwrap().bg
    }
}
#[rustfmt::skip]
const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.5,
    0.0, 0.0, 0.0, 1.0,
);

#[allow(unused)]
struct CameraUniform {
    pub(super) view_proj: [[f32; 4]; 4],
    buffer: wgpu::Buffer,
    pub(super) bg: wgpu::BindGroup,
}

#[allow(unused)]
impl CameraUniform {
    fn update(&mut self, camera_data: &CameraData, aspect_ratio: f32) {
        self.view_proj =
            (OPENGL_TO_WGPU_MATRIX * camera_data.perspective_matrix(aspect_ratio)).into();
    }
    pub(super) fn new(camera: &CameraData, aspect_ratio: f32, device: &wgpu::Device) -> Self {
        let view_proj: Mat4F32 =
            (OPENGL_TO_WGPU_MATRIX * camera.perspective_matrix(aspect_ratio)).into();
        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("camera buffer"),
            usage: wgpu::BufferUsages::UNIFORM,
            contents: bytemuck::cast_slice(&view_proj),
        });
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera bg"),
            layout: &Self::get_bind_group_layout(device),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        Self {
            view_proj: (OPENGL_TO_WGPU_MATRIX * camera.perspective_matrix(aspect_ratio)).into(),
            buffer,
            bg,
        }
    }
}

impl PipelineUniform for CameraUniform {
    fn get_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("Camera bind group layout"),
            });

        camera_bind_group_layout
    }
    fn create_buffer(&self, device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera_buffer"),
            contents: bytemuck::cast_slice(&[self.view_proj]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        })
    }

    fn create_bind_group(
        buffer: &wgpu::Buffer,
        bgl: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> wgpu::BindGroup {
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("camera bind group"),
        });
        camera_bind_group
    }
}

pub(super) fn get_camera_default() -> Camera {
    let camera = Camera::new(std::f32::consts::FRAC_PI_4, 0.1, 100.0);
    camera
}
