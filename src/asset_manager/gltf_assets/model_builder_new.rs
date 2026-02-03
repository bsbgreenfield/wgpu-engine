use std::{any::TypeId, collections::HashMap, marker::PhantomData, ops::Range};

use cgmath::SquareMatrix;

use crate::{
    asset_manager::{
        Asset,
        asset_manager::{AssetBuilder, AssetLoadError, AssetManager, LoadedAsset},
        gltf_assets::{
            gltf_loader::loader::{BinarySource, GltfLoadError, GltfLoader},
            mesh::{Mesh, Primitive},
            primitive::{GltfValidationError, PrimitiveData},
        },
    },
    util::types::{IndexType, Mat4F32, ModelVertex},
};

#[derive(Debug)]
pub enum ModelBuilderError {
    NodeNotFound(usize),
    GLTFUndefined,
    GLTFLoadError(GltfLoadError),
    MeshNotFound(usize),
    ValidationError(GltfValidationError),
    BinarySourceNotFound,
    IndexRangeError,
}

impl From<GltfValidationError> for ModelBuilderError {
    fn from(value: GltfValidationError) -> Self {
        Self::ValidationError(value)
    }
}
pub struct GltfModelBuilderNew<V: ModelVertex, I: IndexType> {
    gltf: gltf::Gltf,
    bin_source: BinarySource,
    loaded_asset: Option<LoadedAsset>,
    v: PhantomData<V>,
    i: PhantomData<I>,
}

struct ModelDataNew {
    id: usize,
    mesh_ids: Vec<usize>,
    local_transforms: Vec<Mat4F32>,
    joint_data: Option<ModelJointDataNew>,
}

impl ModelDataNew {
    fn new(id: usize) -> Self {
        Self {
            id,
            mesh_ids: Vec::new(),
            local_transforms: Vec::new(),
            joint_data: None,
        }
    }
}
struct ModelJointDataNew {
    joint_ids: Vec<usize>,
    joint_pose_transforms: Mat4F32,
    node_to_joint_id_map: HashMap<usize, usize>,
}

pub trait GltfBuilder {
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
    fn get_model_data<V: ModelVertex, I: IndexType>(
        gltf: &gltf::Gltf,
    ) -> Result<Vec<ModelDataNew>, GltfLoadError> {
        let mut model_data_vec = Vec::<ModelDataNew>::new();
        let root_nodes = Self::get_root_nodes(gltf)?;
        for (idx, rid) in root_nodes.iter().enumerate() {
            let root_node = gltf
                .nodes()
                .find(|root_node| root_node.index() == *rid)
                .ok_or(ModelBuilderError::NodeNotFound(*rid))?;
            let mut model_data = ModelDataNew::new(idx);
            model_data =
                Self::process_root_node(&root_node, cgmath::Matrix4::identity(), model_data)?;
            model_data_vec.push(model_data);
        }
        Ok(model_data_vec)
    }
    fn process_root_node(
        root_node: &gltf::Node,
        base_transform: cgmath::Matrix4<f32>,
        mut model_data: ModelDataNew,
    ) -> Result<ModelDataNew, ModelBuilderError> {
        let cg_trans = cgmath::Matrix4::<f32>::from(root_node.transform().matrix());
        let new_trans = base_transform * cg_trans;
        if let Some(mesh) = root_node.mesh() {
            model_data.mesh_ids.push(mesh.index());
            model_data.local_transforms.push(new_trans.into());
        }
        for child_node in root_node.children() {
            model_data = Self::process_root_node(&child_node, base_transform, model_data)?;
        }

        Ok(model_data)
    }
    fn get_primitive_data_map<V: ModelVertex, I: IndexType>(
        gltf: &gltf::Gltf,
        model_data_vec: &Vec<ModelDataNew>,
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
                    let data =
                        GltfModelBuilderNew::<V, I>::get_primitive_data(mesh.index(), &primitive)
                            .map_err(|e| ModelBuilderError::ValidationError(e))?;
                    primitive_data_buf.push(data);
                }
            }
            primtive_map.insert(model_data.id, primitive_data_buf);
        }

        Ok(primtive_map)
    }
    fn get_index_range_vec<V: ModelVertex, I: IndexType>(
        primitive_data: &HashMap<usize, Vec<PrimitiveData>>,
        buffer_offsets: &Vec<usize>,
    ) -> Result<Vec<Range<usize>>, ModelBuilderError> {
        let mut index_range_vec: Vec<Range<usize>> = Vec::new();
        for (_, data_buf) in primitive_data.iter() {
            for data in data_buf.iter() {
                crate::asset_manager::range_splicer::define_index_ranges(
                    &mut index_range_vec,
                    &GltfModelBuilderNew::<V, I>::get_index_range(
                        data.indices.as_ref(),
                        buffer_offsets,
                    )
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
    fn build_all_models<V: ModelVertex, I: IndexType>(
        bin_source: &BinarySource,
        index_ranges: &Vec<Range<usize>>,
        buffer_offsets: &Vec<usize>,
        model_data_vec: &Vec<ModelDataNew>,
        primitive_data_map: &HashMap<usize, Vec<PrimitiveData>>,
        index_data_offset: usize,
        vertex_data_offset: usize,
    ) -> Result<((Vec<u8>, Vec<u8>), LoadedAsset), ModelBuilderError> {
        let vertex_stride = size_of::<V>();
        let index_stride = size_of::<I>();

        let binary_data = GltfLoader::load_binary_data_from_source(bin_source)
            .map_err(|_| ModelBuilderError::BinarySourceNotFound)?;

        let mut mesh_collections_data = Vec::<MeshCollectionAssetData>::new();

        for ((_, primitive_data), model_data) in
            primitive_data_map.iter().zip(model_data_vec.iter())
        {
            let mut meshes = Vec::<Mesh>::new();
            for data in primitive_data.iter() {
                let model_vertices = GltfModelBuilderNew::<V, I>::get_primitive_vertex_data(
                    buffer_offsets,
                    data,
                    &binary_data,
                )?;
                let primitive_index_range = GltfModelBuilderNew::<V, I>::get_index_range(
                    data.indices.as_ref(),
                    buffer_offsets,
                )?;
                let relative_index_range = Self::get_relative_indices(
                    index_ranges,
                    &primitive_index_range.unwrap_or(Range { start: 0, end: 0 }),
                )?;

                let index_range = Range {
                    start: (index_data_offset + (relative_index_range.start / index_stride)) as u32,
                    end: (index_data_offset + (relative_index_range.end / index_stride)) as u32,
                };

                let vertex_range = Range {
                    start: (vertex_data_offset * vertex_stride) as u32,
                    end: ((vertex_data_offset + model_vertices.len()) * vertex_stride) as u32,
                };
                let current_primitive = Primitive::new(vertex_range, index_range);
                if let Some(current_mesh) = meshes
                    .iter_mut()
                    .find(|mesh| mesh.id == data.mesh_id as u32)
                {
                    current_mesh.primitives.push(current_primitive);
                } else {
                    meshes.push(Mesh {
                        id: data.mesh_id as u32,
                        primitives: vec![current_primitive],
                    });
                }
            }

            mesh_collections_data.push(MeshCollectionAssetData::new(
                model_data.local_transforms.clone(),
                meshes,
            ));
        }
        let mut loaded_asset = LoadedAsset::new();
        loaded_asset.add_mesh_collections(mesh_collections_data);
        todo!()
    }

    fn load_gltf<V: ModelVertex, I: IndexType>(
        gltf: &gltf::Gltf,
        bin_source: &BinarySource,
        index_data_offset: usize,
        vertex_data_offset: usize,
    ) -> Result<(), AssetLoadError> {
        let buffer_offsets = Self::get_buffer_offsets(gltf);
        let model_data_vec = Self::get_model_data::<V, I>(gltf)?;
        let primitive_data = Self::get_primitive_data_map::<V, I>(gltf, &model_data_vec)?;
        let index_range_vec = Self::get_index_range_vec::<V, I>(&primitive_data, &buffer_offsets)?;
        Self::build_all_models::<V, I>(
            bin_source,
            &index_range_vec,
            &buffer_offsets,
            &model_data_vec,
            &primitive_data,
            index_data_offset,
            vertex_data_offset,
        );

        todo!()
    }
}

#[derive(Debug)]
pub struct MeshCollectionAssetData {
    local_transforms: Vec<Mat4F32>,
    meshes: Vec<Mesh>,
}

impl MeshCollectionAssetData {
    fn new(local_transforms: Vec<Mat4F32>, meshes: Vec<Mesh>) -> Self {
        Self {
            local_transforms,
            meshes,
        }
    }
}
