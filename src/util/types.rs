use std::{
    fmt::Debug,
    ops::{Deref, Index},
};

use bytemuck::{AnyBitPattern, NoUninit};
use wgpu::naga::front;

pub type Mat4F32 = [[f32; 4]; 4];

pub struct PrimitiveVerticesData {
    pub positions: Vec<u8>,
    pub normal: Option<Vec<u8>>,
    pub uv: Option<Vec<u8>>,
    pub joints: Option<Vec<u8>>,
    pub weights: Option<Vec<u8>>,
    pub count: usize,
}
pub trait ModelVertex: Debug + bytemuck::Pod {
    fn from_primitive_data(p: &PrimitiveVerticesData) -> Vec<Self>;
    fn normalize_f32_to_u8(input: Vec<f32>) -> Vec<u8> {
        input
            .into_iter()
            .map(|x| {
                assert!(x <= 1.0 && x >= 0.0);
                let scaled = (x * 255.0).round();
                scaled.clamp(0.0, 255.0) as u8
            })
            .collect()
    }
    fn desc() -> wgpu::VertexBufferLayout<'static>;
}
pub trait IndexType: AnyBitPattern + bytemuck::NoUninit + Debug {
    const GLTF_INDEX_TYPE: gltf::accessor::DataType;
    const BYTE_SIZE: usize;
}

impl IndexType for u16 {
    const GLTF_INDEX_TYPE: gltf::accessor::DataType = gltf::accessor::DataType::U16;
    const BYTE_SIZE: usize = 2;
}

#[repr(C)]
#[derive(bytemuck::Pod, Clone, Copy, bytemuck::Zeroable, Debug)]
pub struct LocalTransform(Mat4F32);

impl From<Mat4F32> for LocalTransform {
    fn from(value: Mat4F32) -> Self {
        Self(value)
    }
}

pub fn mat4_from_cgmath(value: cgmath::Matrix4<f32>) -> Mat4F32 {
    let x: [f32; 4] = value.x.into();
    let y: [f32; 4] = value.y.into();
    let z: [f32; 4] = value.z.into();
    let w: [f32; 4] = value.w.into();
    [x, y, z, w]
}

// ************************* PNUJ *************************
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PNUJWVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub joints: [u8; 4],
    pub weights: [u8; 4],
}

const PNUJW_ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
    0 => Float32x3,
    1 => Float32x3,
    2 => Float32x2,
    3 => Uint8x4,
    4 => Unorm8x4,
];

impl ModelVertex for PNUJWVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &PNUJW_ATTRIBUTES,
        }
    }
    fn from_primitive_data(p: &PrimitiveVerticesData) -> Vec<Self> {
        let position_f32: &[f32] = bytemuck::cast_slice(&p.positions);
        let normals_f32: Option<Vec<f32>> = match &p.normal {
            Some(normals) => Some(bytemuck::cast_slice(normals).to_vec()),
            None => None,
        };
        let tex_coords_f32: Option<Vec<f32>> = match &p.uv {
            Some(tex_coords) => Some(bytemuck::cast_slice(tex_coords).to_vec()),
            None => None,
        };
        let joints_u16: Option<Vec<u16>> = match &p.joints {
            Some(joints) => Some(bytemuck::cast_slice(&joints).to_vec()),
            None => None,
        };
        let weights_f32: Option<Vec<f32>> = match &p.weights {
            Some(weights) => Some(bytemuck::cast_slice(&weights).to_vec()),
            None => None,
        };
        let weights_normalized = if let Some(w) = weights_f32 {
            Some(Self::normalize_f32_to_u8(w.to_vec()))
        } else {
            None
        };
        let vertex_vec: Vec<Self> = (0..(position_f32.len() / 3))
            .map(|i| {
                let normal = match &normals_f32 {
                    Some(n) => n[i * 3..i * 3 + 3].try_into().unwrap(),
                    None => [0.0, 0.0, 0.0],
                };
                let tex = match &tex_coords_f32 {
                    Some(t) => t[i * 2..i * 2 + 2].try_into().unwrap(),
                    None => [0.0, 0.0],
                };
                let joints = match &joints_u16 {
                    Some(j) => j[i * 4..i * 4 + 4].try_into().unwrap(),
                    None => [0, 0, 0, 0],
                };
                let weights = match &weights_normalized {
                    Some(w) => &w[i * 4..i * 4 + 4],
                    None => match &joints_u16 {
                        Some(_) => &[0, 0, 0, 0],
                        None => &[1, 1, 1, 1],
                    },
                };
                if joints != [0, 0, 0, 0] {
                    let mut sum: u32 = 0;
                    for x in weights.iter() {
                        sum += *x as u32;
                    }
                    assert!(sum <= 256 && sum >= 254);
                }

                return PNUJWVertex {
                    position: position_f32[i * 3..i * 3 + 3].try_into().unwrap(),
                    normal: normal,
                    uv: tex,
                    joints: [
                        joints[0] as u8,
                        joints[1] as u8,
                        joints[2] as u8,
                        joints[3] as u8,
                    ],
                    weights: weights.try_into().unwrap(),
                };
            })
            .collect();

        vertex_vec
    }
}

// ************************* PNUJ *************************
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PNUVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

const PNU_ATTRIBUTES: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
    0 => Float32x3,
    1 => Float32x3,
    2 => Float32x2,
];

impl ModelVertex for PNUVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &PNU_ATTRIBUTES,
        }
    }
    fn from_primitive_data(p: &PrimitiveVerticesData) -> Vec<Self> {
        let position_f32: &[f32] = bytemuck::cast_slice(&p.positions);
        let normals_f32: Option<Vec<f32>> = match &p.normal {
            Some(normals) => Some(bytemuck::cast_slice(normals).to_vec()),
            None => None,
        };
        let tex_coords_f32: Option<Vec<f32>> = match &p.uv {
            Some(tex_coords) => Some(bytemuck::cast_slice(tex_coords).to_vec()),
            None => None,
        };
        let vertex_vec: Vec<Self> = (0..(position_f32.len() / 3))
            .map(|i| {
                let normal = match &normals_f32 {
                    Some(n) => n[i * 3..i * 3 + 3].try_into().unwrap(),
                    None => [0.0, 0.0, 0.0],
                };
                let tex = match &tex_coords_f32 {
                    Some(t) => t[i * 2..i * 2 + 2].try_into().unwrap(),
                    None => [0.0, 0.0],
                };

                return PNUVertex {
                    position: position_f32[i * 3..i * 3 + 3].try_into().unwrap(),
                    normal: normal,
                    uv: tex,
                };
            })
            .collect();

        vertex_vec
    }
}

const ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
             5 => Float32x4,
             6 => Float32x4,
             7 => Float32x4,
             8 => Float32x4,

];
pub trait InstanceData {
    fn desc() -> wgpu::VertexBufferLayout<'static>;
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlobalTransform {
    pub transform: Mat4F32,
}
impl GlobalTransform {
    pub fn new(data: Mat4F32) -> Self {
        Self { transform: data }
    }
}

impl Deref for GlobalTransform {
    type Target = Mat4F32;
    fn deref(&self) -> &Self::Target {
        &self.transform
    }
}

impl From<cgmath::Matrix4<f32>> for GlobalTransform {
    fn from(value: cgmath::Matrix4<f32>) -> Self {
        Self {
            transform: value.into(),
        }
    }
}

impl InstanceData for GlobalTransform {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: (std::mem::size_of::<Mat4F32>() as wgpu::BufferAddress),
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &ATTRIBUTES,
        }
    }
}
