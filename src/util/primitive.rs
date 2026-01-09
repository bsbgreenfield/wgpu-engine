use std::ops::Range;

use gltf::accessor::{DataType, Dimensions};

use crate::util::types::ModelVertex;

#[derive(Debug)]
pub enum GltfValidationError {
    NoView,
}
pub struct PrimitiveVerticesData {
    pub positions: Vec<u8>,
    pub normal: Option<Vec<u8>>,
    pub uv: Option<Vec<u8>>,
    pub joints: Option<Vec<u8>>,
    pub weights: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GLTFDataAccessor {
    buffer_index: u8,
    byte_offset: u32,
    count: u32,
    stride: Option<u8>,
    byte_size: u8,
    num_elements: u8,
}

#[derive(Debug)]
pub(super) struct PrimitiveData {
    pub(super) mesh_id: usize,
    pub(super) positions: GLTFDataAccessor,
    pub(super) tex_coords: Option<GLTFDataAccessor>,
    pub(super) normals: Option<GLTFDataAccessor>,
    pub(super) joints: Option<GLTFDataAccessor>,
    pub(super) weights: Option<GLTFDataAccessor>,
    pub(super) indices: Option<GLTFDataAccessor>,
}

impl GLTFDataAccessor {
    fn from_accessor(acc: &gltf::Accessor) -> Result<Self, GltfValidationError> {
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

impl ModelBuilder {
    pub(super) fn get_primitive_data(
        &self,
        mesh_id: usize,
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
            mesh_id: mesh_id,
            positions: position_accessor,
            normals: maybe_normals_accessor,
            indices: maybe_indices_accessor,
            tex_coords: maybe_tex_coords_accessor,
            joints: maybe_joints0_accessor,
            weights: maybe_weights_accessor,
        })
    }

    /// builds out vertex data in a specified model vertex format V
    pub(super) fn get_primitive_vertex_data<V: ModelVertex>(
        &self,
        primitive_data: &PrimitiveData,
        binary_data: &Vec<u8>,
    ) -> Result<Vec<V>, ModelBuilderError> {
        let positions = copy_binary_data_from_gltf(
            &primitive_data.positions,
            &self.buffer_offsets,
            binary_data,
        )?;

        let mut normals = None;
        let mut tex_coords = None;
        let mut joints = None;
        let mut weights = None;
        if let Some(normals_accessor) = primitive_data.normals {
            normals = Some(copy_binary_data_from_gltf(
                &normals_accessor,
                &self.buffer_offsets,
                binary_data,
            )?);
        }
        if let Some(tex_coords_accessor) = primitive_data.tex_coords {
            tex_coords = Some(copy_binary_data_from_gltf(
                &tex_coords_accessor,
                &self.buffer_offsets,
                binary_data,
            )?);
        }
        if let Some(joints_accessor) = primitive_data.joints {
            joints = Some(copy_binary_data_from_gltf(
                &joints_accessor,
                &self.buffer_offsets,
                binary_data,
            )?);
        }
        if let Some(weights_accessor) = primitive_data.weights {
            weights = Some(copy_binary_data_from_gltf(
                &weights_accessor,
                &self.buffer_offsets,
                binary_data,
            )?);
        }

        let pvd: PrimitiveVerticesData = PrimitiveVerticesData {
            positions: positions,
            normal: normals,
            uv: tex_coords,
            joints: joints,
            weights: weights,
        };

        Ok(V::from_primitive_data(&pvd))
    }

    /// get the indices within the binary that contain this primitives index data
    pub(super) fn get_index_range(
        &self,
        maybe_accessor: Option<&GLTFDataAccessor>,
    ) -> Result<Option<Range<usize>>, GltfValidationError> {
        match maybe_accessor {
            Some(accessor) => {
                let length = accessor.byte_size as usize
                    * accessor.num_elements as usize
                    * accessor.count as usize;
                let buffer_offset = self.buffer_offsets[accessor.buffer_index as usize];
                let offset = accessor.byte_offset as usize + buffer_offset as usize;
                return Ok(Some(Range {
                    start: offset,
                    end: offset + length,
                }));
            }
            None => Ok(None),
        }
    }
}
fn copy_binary_data_from_gltf(
    accessor: &GLTFDataAccessor,
    buffer_offsets: &Vec<usize>,
    binary_data: &Vec<u8>,
) -> Result<Vec<u8>, GltfValidationError> {
    let byte_offset =
        accessor.byte_offset as usize + buffer_offsets[accessor.buffer_index as usize];

    let mut copy_dest: Vec<u8> = Vec::with_capacity(
        accessor.byte_size as usize * accessor.num_elements as usize * accessor.count as usize,
    );
    assert_eq!(copy_dest.capacity(), 288);
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
