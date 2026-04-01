use std::{any::TypeId, collections::HashMap, ops::Range};

use cgmath::SquareMatrix;

use crate::{
    asset_manager::{
        Asset, AssetHandle, AssetLoadError, LoadedAsset,
        asset_manager::AssetResidency,
        gltf_assets::{
            GltfAsset, GltfLoadResult, ModelBuilderError,
            gltf_loader::{
                GltfLoadError,
                loader::{BinarySource, GltfLoader},
            },
            mesh::{Mesh, Primitive},
            primitive::PrimitiveData,
        },
    },
    util::types::{
        IndexType, LocalTransform, Mat4F32, ModelVertex, PNUJWVertex, PNUVertex, mat4_from_cgmath,
    },
};

#[allow(unused)]
struct ModelData {
    id: usize,
    mesh_ids: Vec<usize>,
    joint_data: Option<ModelJointData>,
}

impl ModelData {
    fn new(id: usize) -> Self {
        Self {
            id,
            mesh_ids: Vec::new(),
            joint_data: None,
        }
    }
}
impl LoadedAsset {
    pub fn mesh_ids_and_prim_ranges_of<V: ModelVertex>(&self) -> (Vec<u32>, Vec<Range<u32>>) {
        let mut mesh_ids = Vec::<u32>::new();
        let mut primitive_ranges = Vec::<Range<u32>>::new();
        for mesh_data in self.gltf_mesh_data.mesh_data.iter() {
            // find all meshes which contain primitives of the correct type
            let filtered_meshes = mesh_data.meshes.iter().filter(|m| {
                m.primitives
                    .iter()
                    .any(|p| p.vertex_type == TypeId::of::<V>())
            });
            for filtered_mesh in filtered_meshes {
                for candidate_primitive in filtered_mesh.primitives.iter() {
                    if candidate_primitive.vertex_type == TypeId::of::<V>() {
                        mesh_ids.push(filtered_mesh.id);
                        // ADD PRIMITIVE COUNT FOR MODEL IF NECESSARY HERE
                        primitive_ranges.push(candidate_primitive.vertices.clone());
                    }
                }
            }
        }
        (mesh_ids, primitive_ranges)
    }
}
#[allow(unused)]
struct ModelJointData {
    joint_ids: Vec<usize>,
    joint_pose_transforms: Mat4F32,
    node_to_joint_id_map: HashMap<usize, usize>,
}

trait GltfBuilder {
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
    fn get_model_data(
        gltf: &gltf::Gltf,
    ) -> Result<(Vec<ModelData>, Vec<LocalTransform>), GltfLoadError> {
        let mut model_data_vec = Vec::<ModelData>::new();
        let mesh_count = gltf.meshes().len();
        let mut local_transforms = Vec::<LocalTransform>::with_capacity(mesh_count);
        let root_nodes = Self::get_root_nodes(gltf)?;
        for (idx, rid) in root_nodes.iter().enumerate() {
            let root_node = gltf
                .nodes()
                .find(|root_node| root_node.index() == *rid)
                .ok_or(ModelBuilderError::NodeNotFound(*rid))?;
            let mut model_data = ModelData::new(idx);
            model_data = Self::process_root_node(
                &root_node,
                cgmath::Matrix4::identity(),
                &mut local_transforms,
                model_data,
            )?;
            model_data_vec.push(model_data);
        }
        Ok((model_data_vec, local_transforms))
    }
    fn process_root_node(
        root_node: &gltf::Node,
        base_transform: cgmath::Matrix4<f32>,
        local_transforms: &mut Vec<LocalTransform>,
        mut model_data: ModelData,
    ) -> Result<ModelData, ModelBuilderError> {
        let cg_trans = cgmath::Matrix4::<f32>::from(root_node.transform().matrix());
        let new_trans = base_transform * cg_trans;
        if let Some(mesh) = root_node.mesh() {
            model_data.mesh_ids.push(mesh.index());
            local_transforms.insert(mesh.index(), mat4_from_cgmath(new_trans).into());
        }
        for child_node in root_node.children() {
            model_data =
                Self::process_root_node(&child_node, base_transform, local_transforms, model_data)?;
        }

        Ok(model_data)
    }
    fn get_primitive_data_map(
        gltf: &gltf::Gltf,
        model_data_vec: &Vec<ModelData>,
    ) -> Result<HashMap<usize, Vec<PrimitiveData>>, ModelBuilderError> {
        let mut primtive_map = HashMap::new();
        for model_data in model_data_vec.iter() {
            let mut primitive_data_buf = Vec::<PrimitiveData>::new();
            for mesh_id in model_data.mesh_ids.iter() {
                let mesh = gltf
                    .meshes()
                    .find(|m| m.index() == *mesh_id)
                    .ok_or(ModelBuilderError::MeshNotFound(*mesh_id))?;

                for primitive in mesh.primitives() {
                    let data = Primitive::get_primitive_data(mesh.index(), &primitive)
                        .map_err(|e| ModelBuilderError::ValidationError(e))?;
                    primitive_data_buf.push(data);
                }
            }
            primtive_map.insert(model_data.id, primitive_data_buf);
        }

        Ok(primtive_map)
    }
    fn get_index_range_vec(
        primitive_data: &HashMap<usize, Vec<PrimitiveData>>,
        buffer_offsets: &Vec<usize>,
    ) -> Result<Vec<Range<usize>>, ModelBuilderError> {
        let mut index_range_vec: Vec<Range<usize>> = Vec::new();
        for (_, data_buf) in primitive_data.iter() {
            for data in data_buf.iter() {
                crate::asset_manager::range_splicer::define_index_ranges(
                    &mut index_range_vec,
                    &Primitive::get_index_range(data.indices.as_ref(), buffer_offsets)
                        .map_err(|err| ModelBuilderError::ValidationError(err))?
                        .unwrap_or(Range { start: 0, end: 0 }),
                );
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

    fn set_index_data<I: IndexType>(
        index_ranges: &Vec<Range<usize>>,
        index_data: &mut Vec<I>,
        bin: &Vec<u8>,
    ) {
        let mut index_vec: Vec<I> = Vec::new();
        for range in index_ranges.iter() {
            let indices_bytes: &[u8] = &bin[range.start..range.end];
            let indices: &[I] = bytemuck::cast_slice::<u8, I>(indices_bytes);
            index_vec.extend(indices.to_vec());
        }
        index_data.extend(index_vec);
    }

    fn build_all_models(
        bin_source: &BinarySource,
        index_ranges: &Vec<Range<usize>>,
        buffer_offsets: &Vec<usize>,
        local_transforms: Vec<LocalTransform>,
        model_data_vec: &Vec<ModelData>,
        primitive_data_map: &HashMap<usize, Vec<PrimitiveData>>,
    ) -> Result<GltfLoadResult, ModelBuilderError> {
        let binary_data = GltfLoader::load_binary_data_from_source(bin_source)
            .map_err(|_| ModelBuilderError::BinarySourceNotFound)?;

        let mut pnujw_vertices: Vec<PNUJWVertex> = Vec::new();
        let mut pnu_vertices: Vec<PNUVertex> = Vec::new();
        let mut mesh_data = Vec::<GltfMeshData>::new();
        for ((_, model_primitive_data), _) in primitive_data_map.iter().zip(model_data_vec.iter()) {
            let mut meshes = Vec::<Mesh>::new();
            for primitive_data in model_primitive_data.iter() {
                // TODO: either coerce all indices to u16 OR handle diff index types
                assert_eq!(primitive_data.indices.as_ref().unwrap().byte_size, 2);

                // binary data per vertex attribute
                let primitive_vertex_data = Primitive::get_primitive_vertex_data(
                    buffer_offsets,
                    primitive_data,
                    &binary_data,
                )?;

                // range within the blob in which the indices for this primitive are located
                let primitive_index_range =
                    Primitive::get_index_range(primitive_data.indices.as_ref(), buffer_offsets)?;

                // range of this primitives indices within the final GPU index buffer
                let relative_index_range = Self::get_relative_indices(
                    index_ranges,
                    &primitive_index_range.unwrap_or(Range { start: 0, end: 0 }),
                )?;

                let index_range = Range {
                    start: (relative_index_range.start / size_of::<u16>()) as u32,
                    end: (relative_index_range.end / size_of::<u16>()) as u32,
                };

                let is_jointed = primitive_data.joints.is_some().clone();

                let mut vertex_range = Range::<u32>::default();
                let mut current_primitive: Option<Primitive> = None;
                if is_jointed {
                    vertex_range.start = (pnujw_vertices.len() * size_of::<PNUJWVertex>()) as u32;
                    vertex_range.end = ((pnujw_vertices.len() + primitive_vertex_data.count)
                        * size_of::<PNUJWVertex>()) as u32;
                    let _ = current_primitive
                        .insert(Primitive::new::<PNUJWVertex>(vertex_range, index_range));
                } else {
                    vertex_range.start = (pnujw_vertices.len() * size_of::<PNUVertex>()) as u32;
                    vertex_range.end = ((pnujw_vertices.len() + primitive_vertex_data.count)
                        * size_of::<PNUVertex>()) as u32;
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

            mesh_data.push(GltfMeshData { meshes });
        }

        let mut index_data = Vec::<u16>::new();
        Self::set_index_data(&index_ranges, &mut index_data, &binary_data);
        Ok(GltfLoadResult {
            pnujw_vertices,
            pnu_vertices,
            local_transforms,
            indices: index_data,
            mesh_data,
        })
    }

    fn load_gltf(
        gltf: &gltf::Gltf,
        bin_source: &BinarySource,
    ) -> Result<GltfLoadResult, AssetLoadError> {
        let buffer_offsets = Self::get_buffer_offsets(gltf);
        let (model_data_vec, local_transforms) = Self::get_model_data(gltf)?;
        let primitive_data = Self::get_primitive_data_map(gltf, &model_data_vec)?;
        let index_range_vec = Self::get_index_range_vec(&primitive_data, &buffer_offsets)?;
        let load_result = Self::build_all_models(
            bin_source,
            &index_range_vec,
            &buffer_offsets,
            local_transforms,
            &model_data_vec,
            &primitive_data,
        )?;

        Ok(load_result)
    }
}
impl GltfBuilder for GltfAsset {}

impl Asset for GltfAsset {
    fn get_residency_level(&self) -> &AssetResidency {
        &self.res_level
    }
    fn set_residency_level(&mut self, level: AssetResidency) {
        self.res_level = level;
    }
    fn new(dir_name: &str) -> Result<Self, AssetLoadError>
    where
        Self: Sized,
    {
        let (gltf, bin) = GltfLoader::load_gltf_from_resource(dir_name)?;
        Ok(Self {
            gltf,
            bin,
            res_level: AssetResidency::Registered,
        })
    }
    fn load_asset(&self, handle: AssetHandle) -> Result<LoadedAsset, AssetLoadError> {
        let a = Self::load_gltf(&self.gltf, &self.bin).unwrap();
        Ok(LoadedAsset {
            gltf_mesh_data: a,
            handle,
        })
    }
}

#[derive(Debug)]
pub struct GltfMeshData {
    meshes: Vec<Mesh>,
}
