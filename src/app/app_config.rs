use std::{error::Error, sync::Arc, time::Duration};

use winit::{dpi::PhysicalSize, window::Window};

pub struct AppConfig<'a> {
    pub target_fps: Duration,
    pub size: PhysicalSize<u32>,
    pub surface: Option<wgpu::Surface<'a>>,
    pub surface_config: Option<wgpu::SurfaceConfiguration>,
    pub queue: wgpu::Queue,
    pub device: wgpu::Device,
}

impl<'a> AppConfig<'a> {
    pub fn get_aspect_ratio(&self) -> f32 {
        if let Some(config) = &self.surface_config {
            (config.width / config.height) as f32
        } else {
            1.0 // HEADLESS
        }
    }
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            if let Some(config) = &mut self.surface_config {
                config.width = new_size.width;
                config.height = new_size.height;
                self.surface
                    .as_ref()
                    .unwrap()
                    .configure(&self.device, &config);
            }
        }
    }

    pub async fn new_headless() -> Self {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: None,
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .expect("failed to make adapter");
        let mut limits = wgpu::Limits::default();
        limits.max_immediate_size = 8;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("HEADLESS DEVICE"),
                required_features: wgpu::Features::IMMEDIATES,
                required_limits: limits,
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::default(),
            })
            .await
            .expect("failed to create device");

        Self {
            target_fps: Duration::new(1 / 100, 0),
            size: PhysicalSize::default(),
            surface: None,
            surface_config: None,
            queue,
            device,
        }
    }

    pub(super) async fn new(
        window: Arc<Window>,
        handle: winit::event_loop::OwnedDisplayHandle,
    ) -> Result<Self, Box<dyn Error>> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            backend_options: wgpu::BackendOptions::default(),
            flags: wgpu::InstanceFlags::empty(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds {
                for_resource_creation: None,
                for_device_loss: None,
            },
            display: Some(Box::new(handle)),
        });
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await?;

        let mut limits = wgpu::Limits::default();
        limits.max_immediate_size = 8;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::IMMEDIATES,
                required_limits: limits,
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::default(),
            })
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::default(),
        };
        Ok(AppConfig {
            target_fps: Duration::new(1 / 100, 0),
            size,
            surface: Some(surface),
            device,
            queue,
            surface_config: Some(config),
        })
    }
}
