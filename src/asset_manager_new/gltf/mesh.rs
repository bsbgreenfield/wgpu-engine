use std::{
    any::{TypeId, type_name},
    ops::Range,
};

use gltf::accessor::{DataType, Dimensions};

use crate::{
    asset_manager_new::{GltfValidationError, ModelBuilderError},
    util::types::{ModelVertex, PrimitiveVerticesData},
};

pub(super) struct Primitive {
    pub vertex_type: TypeId,
    pub vertices: Range<u32>,
    pub indices: Option<Range<u32>>,
}

pub(super) struct Mesh {
    pub id: u32,
    pub primitives: Vec<Primitive>,
}

impl Primitive {
    pub(in crate::asset_manager_new) fn new<V: ModelVertex>(
        vertices: Range<u32>,
        indices: Option<Range<u32>>,
    ) -> Self {
        Self {
            vertices,
            indices,
            vertex_type: TypeId::of::<V>(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GLTFDataAccessor {
    buffer_index: u8,
    byte_offset: u32,
    count: u32,
    stride: Option<u8>,
    pub(super) byte_size: u8,
    num_elements: u8,
}

#[derive(Debug)]
pub(super) struct PrimitiveData {
    pub(super) positions: GLTFDataAccessor,
    pub(super) tex_coords: Option<GLTFDataAccessor>,
    pub(super) normals: Option<GLTFDataAccessor>,
    pub(super) joints: Option<GLTFDataAccessor>,
    pub(super) weights: Option<GLTFDataAccessor>,
    pub(super) indices: Option<GLTFDataAccessor>,
}

impl GLTFDataAccessor {
    pub(super) fn from_accessor(acc: &gltf::Accessor) -> Result<Self, GltfValidationError> {
        let view = acc.view().ok_or(GltfValidationError::NoView)?;
        let byte_offset = (view.offset() + acc.offset()) as u32;
        let count = acc.count() as u32;
        let byte_size = match acc.data_type() {
            DataType::U8 => 1,
            DataType::U16 => 2,
            DataType::F32 => 4,
            _ => todo!(),
        };
        let num_elements = match acc.dimensions() {
            Dimensions::Scalar => 1,
            Dimensions::Vec2 => 2,
            Dimensions::Vec3 => 3,
            Dimensions::Vec4 => 4,
            Dimensions::Mat4 => 16,
            _ => todo!(),
        };
        let stride = match view.stride() {
            Some(s) => Some(s as u8),
            None => None,
        };
        let buffer_index = view.buffer().index() as u8;

        Ok(GLTFDataAccessor {
            buffer_index,
            byte_offset,
            count,
            stride,
            byte_size,
            num_elements,
        })
    }
}
impl Primitive {
    pub(super) fn get_primitive_data(
        primitive: &gltf::Primitive,
    ) -> Result<PrimitiveData, GltfValidationError> {
        let position_accessor = GLTFDataAccessor::from_accessor(
            &primitive
                .attributes()
                .find(|a| a.0 == gltf::Semantic::Positions)
                .unwrap()
                .1,
        )?;

        let maybe_normals_accessor = match primitive
            .attributes()
            .find(|a| a.0 == gltf::Semantic::Normals)
        {
            Some(normals) => Some(GLTFDataAccessor::from_accessor(&normals.1)?),
            None => None,
        };
        let maybe_indices_accessor = match primitive.indices() {
            Some(indices) => Some(GLTFDataAccessor::from_accessor(&indices)?),
            None => None,
        };
        let maybe_tex_coords_accessor = match primitive
            .attributes()
            .find(|a| a.0 == gltf::Semantic::TexCoords(0))
        {
            Some(tex_coords) => Some(GLTFDataAccessor::from_accessor(&tex_coords.1)?),
            None => None,
        };
        let maybe_joints0_accessor = match primitive
            .attributes()
            .find(|a| a.0 == gltf::Semantic::Joints(0))
        {
            Some(joints) => Some(GLTFDataAccessor::from_accessor(&joints.1)?),
            None => None,
        };
        let maybe_weights_accessor = match primitive
            .attributes()
            .find(|a| a.0 == gltf::Semantic::Weights(0))
        {
            Some(weights) => Some(GLTFDataAccessor::from_accessor(&weights.1)?),
            None => None,
        };

        Ok(PrimitiveData {
            positions: position_accessor,
            normals: maybe_normals_accessor,
            indices: maybe_indices_accessor,
            tex_coords: maybe_tex_coords_accessor,
            joints: maybe_joints0_accessor,
            weights: maybe_weights_accessor,
        })
    }

    /// get the indices within the binary that contain this primitives index data
    /// TODO: Assert that this is actually an indices accessor
    pub(super) fn get_index_range(
        maybe_accessor: Option<&GLTFDataAccessor>,
        buffer_offsets: &Vec<usize>,
    ) -> Result<Option<Range<usize>>, GltfValidationError> {
        match maybe_accessor {
            Some(accessor) => {
                let length = accessor.byte_size as usize
                    * accessor.num_elements as usize
                    * accessor.count as usize;
                let buffer_offset = buffer_offsets[accessor.buffer_index as usize];
                let offset = accessor.byte_offset as usize + buffer_offset as usize;
                return Ok(Some(Range {
                    start: offset,
                    end: offset + length,
                }));
            }
            None => Ok(None),
        }
    }

    pub(super) fn get_primitive_vertex_data(
        buffer_offsets: &Vec<usize>,
        primitive_data: &PrimitiveData,
        binary_data: &Vec<u8>,
    ) -> Result<PrimitiveVerticesData, ModelBuilderError> {
        let positions =
            copy_binary_data_from_gltf(&primitive_data.positions, buffer_offsets, binary_data)?;

        let mut normals = None;
        let mut tex_coords = None;
        let mut joints = None;
        let mut weights = None;
        if let Some(normals_accessor) = primitive_data.normals {
            normals = Some(copy_binary_data_from_gltf(
                &normals_accessor,
                buffer_offsets,
                binary_data,
            )?);
        }
        if let Some(tex_coords_accessor) = primitive_data.tex_coords {
            tex_coords = Some(copy_binary_data_from_gltf(
                &tex_coords_accessor,
                buffer_offsets,
                binary_data,
            )?);
        }
        if let Some(joints_accessor) = primitive_data.joints {
            joints = Some(copy_binary_data_from_gltf(
                &joints_accessor,
                buffer_offsets,
                binary_data,
            )?);
        }
        if let Some(weights_accessor) = primitive_data.weights {
            weights = Some(copy_binary_data_from_gltf(
                &weights_accessor,
                buffer_offsets,
                binary_data,
            )?);
        }

        Ok(PrimitiveVerticesData {
            positions: positions,
            normal: normals,
            uv: tex_coords,
            joints: joints,
            weights: weights,
            count: primitive_data.positions.count as usize,
        })
    }
}

pub(super) fn copy_and_cast_gltf_binary_data_f32(
    accessor: &GLTFDataAccessor,
    buffer_offsets: &Vec<usize>,
    binary_data: &Vec<u8>,
) -> Result<Vec<f32>, GltfValidationError> {
    assert!(accessor.byte_size == 4);

    let byte_offset =
        accessor.byte_offset as usize + buffer_offsets[accessor.buffer_index as usize];
    let mut copy_dest: Vec<f32> =
        Vec::with_capacity(accessor.num_elements as usize * accessor.count as usize);
    let mut byte_loc = byte_offset;

    let extra_stride = if let Some(stride) = accessor.stride {
        stride as usize - (accessor.byte_size as usize * accessor.num_elements as usize)
    } else {
        0
    };

    for _ in 0..accessor.count as usize {
        for _ in 0..accessor.num_elements as usize {
            let slice: [u8; 4] = binary_data[byte_loc..byte_loc + 4]
                .try_into()
                .map_err(|_| GltfValidationError::UnsupportedScheme)?;
            copy_dest.push(f32::from_le_bytes(slice));
            byte_loc += 4;
        }
        byte_loc += extra_stride;
    }

    assert_eq!(
        copy_dest.len(),
        accessor.num_elements as usize * accessor.count as usize
    );

    Ok(copy_dest)
}
pub(super) fn copy_binary_data_from_gltf(
    accessor: &GLTFDataAccessor,
    buffer_offsets: &Vec<usize>,
    binary_data: &Vec<u8>,
) -> Result<Vec<u8>, GltfValidationError> {
    let byte_offset =
        accessor.byte_offset as usize + buffer_offsets[accessor.buffer_index as usize];

    let mut copy_dest: Vec<u8> = Vec::with_capacity(
        accessor.byte_size as usize * accessor.num_elements as usize * accessor.count as usize,
    );
    let mut byte_loc = byte_offset;
    let extra_stride = if let Some(stride) = accessor.stride {
        stride as usize - (accessor.byte_size as usize * accessor.num_elements as usize)
    } else {
        0
    };

    for _ in 0..accessor.count as usize {
        for _ in 0..accessor.num_elements as usize {
            for _ in 0..accessor.byte_size as usize {
                copy_dest.push(binary_data[byte_loc]);
                byte_loc += 1;
            }
        }

        byte_loc += extra_stride;
        // of the component, then no need to adjust alignment
    }
    assert_eq!(
        copy_dest.len(),
        accessor.byte_size as usize * accessor.num_elements as usize * accessor.count as usize
    );

    Ok(copy_dest)
}
