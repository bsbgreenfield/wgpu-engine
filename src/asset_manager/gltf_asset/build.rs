use std::{collections::HashMap, ops::Range};

use cgmath::SquareMatrix;

use crate::{
    asset_manager::{
        AssetLoadError, Mesh, ModelBuilderError, ModelData, Primitive,
        gltf_asset::{
            BinarySource, GltfAsset, GltfLoadError, GltfLoadResult, GltfMeshData,
            primitive::PrimitiveData,
        },
    },
    util::types::{LocalTransform, ModelVertex, PNUJWVertex, PNUVertex, VIndex, mat4_from_cgmath},
};

impl GltfAsset {
    fn get_buffer_offsets(gltf: &gltf::Gltf) -> Vec<usize> {
        let mut buffer_offsets = Vec::<usize>::new();
        let mut last_buffer_size = 0;
        for buffer in gltf.buffers() {
            buffer_offsets.push(last_buffer_size);
            last_buffer_size += buffer.length();
        }
        buffer_offsets
    }
    fn get_root_nodes(gltf: &gltf::Gltf) -> Result<Vec<usize>, GltfLoadError> {
        let scene = gltf.scenes().next().ok_or(gltf::Error::UnsupportedScheme)?;
        let mesh_node_iter = scene
            .nodes()
            .filter(|n| n.mesh().is_some() || n.children().len() != 0);
        let ids: Vec<usize> = mesh_node_iter.map(|node| node.index()).collect();
        Ok(ids)
    }

    /// For each root node, build
    fn get_model_data(gltf: &gltf::Gltf) -> Result<Vec<ModelData>, GltfLoadError> {
        let mut model_data_vec = Vec::<ModelData>::new();
        let mesh_count = gltf.meshes().len();
        let root_nodes = Self::get_root_nodes(gltf)?;
        for (idx, rid) in root_nodes.iter().enumerate() {
            let root_node = gltf
                .nodes()
                .find(|root_node| root_node.index() == *rid)
                .ok_or(ModelBuilderError::NodeNotFound(*rid))?;
            let mut model_data = ModelData::new(idx, mesh_count);
            model_data =
                Self::process_root_node(&root_node, cgmath::Matrix4::identity(), model_data)?;
            model_data_vec.push(model_data);
        }
        Ok(model_data_vec)
    }
    fn process_root_node(
        root_node: &gltf::Node,
        base_transform: cgmath::Matrix4<f32>,
        mut model_data: ModelData,
    ) -> Result<ModelData, ModelBuilderError> {
        let cg_trans = cgmath::Matrix4::<f32>::from(root_node.transform().matrix());
        let new_trans = base_transform * cg_trans;
        if let Some(mesh) = root_node.mesh() {
            model_data.mesh_ids.push(mesh.index());
            model_data
                .local_transforms
                .push(mat4_from_cgmath(new_trans).into());
        }
        for child_node in root_node.children() {
            model_data = Self::process_root_node(&child_node, base_transform, model_data)?;
        }

        Ok(model_data)
    }
    fn get_primitive_data_map(
        gltf: &gltf::Gltf,
        model_data_vec: &Vec<ModelData>,
    ) -> Result<Vec<Vec<PrimitiveData>>, ModelBuilderError> {
        let mut all_mesh_primitives = Vec::new();
        for model_data in model_data_vec.iter() {
            let mut mesh_primitives = Vec::<PrimitiveData>::new();
            for mesh_id in model_data.mesh_ids.iter() {
                let mesh = gltf
                    .meshes()
                    .find(|m| m.index() == *mesh_id)
                    .ok_or(ModelBuilderError::MeshNotFound(*mesh_id))?;

                for primitive in mesh.primitives() {
                    let data = Primitive::get_primitive_data(mesh.index(), &primitive)
                        .map_err(|e| ModelBuilderError::ValidationError(e))?;

                    mesh_primitives.push(data);
                }
            }
            all_mesh_primitives.push(mesh_primitives);
        }

        Ok(all_mesh_primitives)
    }
    fn get_index_range_vec(
        primitive_data: &Vec<Vec<PrimitiveData>>,
        buffer_offsets: &Vec<usize>,
    ) -> Result<Vec<Range<usize>>, ModelBuilderError> {
        let mut index_range_vec: Vec<Range<usize>> = Vec::new();
        for data_buf in primitive_data.iter() {
            for data in data_buf.iter() {
                let maybe_index_ranges =
                    &Primitive::get_index_range(data.indices.as_ref(), buffer_offsets)
                        .map_err(|err| ModelBuilderError::ValidationError(err))?;
                if let Some(index_ranges) = maybe_index_ranges {
                    crate::asset_manager::range_splicer::define_index_ranges(
                        &mut index_range_vec,
                        index_ranges,
                    );
                }
            }
        }

        Ok(index_range_vec)
    }

    fn get_relative_indices(
        index_ranges: &Vec<Range<usize>>,
        primitive_index_range: &Range<usize>,
    ) -> Result<Range<usize>, ModelBuilderError> {
        let mut offset = 0;
        for range in index_ranges.iter() {
            if !range.contains(&primitive_index_range.start) {
                offset += range.len();
                continue;
            }
            let relative_primitive_index_offset =
                offset + primitive_index_range.start - range.start;

            return Ok(Range {
                start: relative_primitive_index_offset,
                end: relative_primitive_index_offset + primitive_index_range.len(),
            });
        }

        Err(ModelBuilderError::IndexRangeError)
    }

    fn set_index_data(index_ranges: &Vec<Range<usize>>, bin: &Vec<u8>) -> Option<Vec<VIndex>> {
        if index_ranges.is_empty() {
            return None;
        } else {
            let mut index_vec: Vec<VIndex> = Vec::new();
            for range in index_ranges.iter() {
                let indices_bytes: &[u8] = &bin[range.start..range.end];
                let indices: &[VIndex] = bytemuck::cast_slice::<u8, VIndex>(indices_bytes);
                index_vec.extend(indices.to_vec());
            }
            Some(index_vec)
        }
    }

    fn build_all_models(
        bin_source: &BinarySource,
        index_ranges: &Vec<Range<usize>>,
        buffer_offsets: &Vec<usize>,
        model_data_vec: Vec<ModelData>,
        all_mesh_primitives: &Vec<Vec<PrimitiveData>>,
    ) -> Result<GltfLoadResult, ModelBuilderError> {
        let binary_data = Self::load_binary_data_from_source(bin_source)
            .map_err(|_| ModelBuilderError::BinarySourceNotFound)?;

        let mut pnujw_vertices: Vec<PNUJWVertex> = Vec::new();
        let mut pnu_vertices: Vec<PNUVertex> = Vec::new();
        let mut mesh_data = Vec::<GltfMeshData>::new();

        // model_primitive_data is a flat list of all primitives for a model, each tagged with the
        // mesh id to which they belong. Model data contains node info for the model, like local transforms
        for (model_primitive_data, model_data) in all_mesh_primitives.iter().zip(model_data_vec) {
            let local_transform_map = ModelData::get_local_transform_map(
                model_data.mesh_ids,
                model_data.local_transforms,
            );
            let mut meshes = Vec::<Mesh>::new();
            for primitive_data in model_primitive_data.iter() {
                // TODO: either coerce all indices to u16 OR handle diff index types
                if primitive_data.indices.is_some() {
                    assert_eq!(primitive_data.indices.as_ref().unwrap().byte_size, 2);
                }

                // binary data per vertex attribute
                let primitive_vertex_data = Primitive::get_primitive_vertex_data(
                    buffer_offsets,
                    primitive_data,
                    &binary_data,
                )?;

                let index_range: Option<Range<u32>> = if !index_ranges.is_empty() {
                    // range within the blob in which the indices for this primitive are located
                    let maybe_primitive_index_range = Primitive::get_index_range(
                        primitive_data.indices.as_ref(),
                        buffer_offsets,
                    )?;

                    // range of this primitives indices within the final GPU index buffer allocation
                    let maybe_relative_index_range =
                        maybe_primitive_index_range.map(|primitive_index_range| {
                            Self::get_relative_indices(index_ranges, &primitive_index_range)
                                .unwrap()
                        });

                    maybe_relative_index_range.map(|relative_index_range| Range {
                        start: (relative_index_range.start / size_of::<u16>()) as u32,
                        end: (relative_index_range.end / size_of::<u16>()) as u32,
                    })
                } else {
                    None
                };

                let is_jointed = primitive_data.joints.is_some().clone();

                let mut vertex_range = Range::<u32>::default();
                let mut current_primitive: Option<Primitive> = None;
                if is_jointed {
                    vertex_range.start = pnujw_vertices.len() as u32;
                    vertex_range.end = (pnujw_vertices.len() + primitive_vertex_data.count) as u32;
                    let _ = current_primitive
                        .insert(Primitive::new::<PNUJWVertex>(vertex_range, index_range));
                } else {
                    vertex_range.start = pnu_vertices.len() as u32;
                    vertex_range.end = (pnu_vertices.len() + primitive_vertex_data.count) as u32;
                    let _ = current_primitive
                        .insert(Primitive::new::<PNUVertex>(vertex_range, index_range));
                }

                if let Some(current_mesh) = meshes
                    .iter_mut()
                    .find(|mesh| mesh.id == primitive_data.mesh_id as u32)
                {
                    current_mesh.primitives.push(current_primitive.unwrap());
                } else {
                    meshes.push(Mesh {
                        id: primitive_data.mesh_id as u32,
                        primitives: vec![current_primitive.unwrap()],
                    });
                }

                // write vertex data into the proper data vec
                if is_jointed {
                    pnujw_vertices.extend(PNUJWVertex::from_primitive_data(&primitive_vertex_data));
                } else {
                    pnu_vertices.extend(PNUVertex::from_primitive_data(&primitive_vertex_data));
                }
            }

            mesh_data.push(GltfMeshData {
                meshes,
                local_transforms: local_transform_map,
            });
        }

        let maybe_index_data = Self::set_index_data(&index_ranges, &binary_data);
        Ok(GltfLoadResult {
            pnujw_vertices,
            pnu_vertices,
            indices: maybe_index_data,
            mesh_data,
        })
    }

    pub(super) fn build_gltf(
        gltf: &gltf::Gltf,
        bin_source: &BinarySource,
    ) -> Result<GltfLoadResult, AssetLoadError> {
        let buffer_offsets = Self::get_buffer_offsets(gltf);
        let model_data_vec = Self::get_model_data(gltf)?;
        let primitive_data = Self::get_primitive_data_map(gltf, &model_data_vec)?;
        let index_range_vec = Self::get_index_range_vec(&primitive_data, &buffer_offsets)?;
        let load_result = Self::build_all_models(
            bin_source,
            &index_range_vec,
            &buffer_offsets,
            model_data_vec,
            &primitive_data,
        )?;

        Ok(load_result)
    }
}
