use std::{collections::HashMap, ops::Range};

use cgmath::SquareMatrix;

use crate::{
    asset_manager::{
        asset_manager::{AssetBuilder, AssetLoadError, LoadedAsset, MeshPool},
        gltf_assets::{
            gltf_loader::loader::{BinarySource, GltfLoadError, GltfLoader},
            model_builder::{GltfModelBuilder, ModelBuilderError},
            primitive::PrimitiveData,
        },
    },
    util::types::{IndexType, Mat4F32, ModelVertex},
};

pub struct GltfModelBuilderNew {
    gltf: gltf::Gltf,
    bin_source: BinarySource,
    loaded_asset: Option<LoadedAsset>,
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

impl GltfModelBuilderNew {
    pub fn new(dir_name: &str) -> Result<Self, AssetLoadError> {
        let (gltf, bin) = GltfLoader::load_gltf_from_resource(dir_name)?;
        Ok(Self {
            gltf: gltf,
            bin_source: bin,
            loaded_asset: None,
        })
    }

    fn get_buffer_offsets(&self) -> Vec<usize> {
        let mut buffer_offsets = Vec::<usize>::new();
        let mut last_buffer_size = 0;
        for buffer in self.gltf.buffers() {
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

    fn get_model_data(&self) -> Result<Vec<ModelDataNew>, GltfLoadError> {
        let mut model_data_vec = Vec::<ModelDataNew>::new();
        let root_nodes = Self::get_root_nodes(&self.gltf)?;
        for (idx, rid) in root_nodes.iter().enumerate() {
            let root_node = self
                .gltf
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

    fn get_primitive_data_map(
        &self,
        model_data_vec: &Vec<ModelDataNew>,
    ) -> Result<HashMap<usize, Vec<PrimitiveData>>, ModelBuilderError> {
        let mut primtive_map = HashMap::new();
        for model_data in model_data_vec.iter() {
            let mut primitive_data_buf = Vec::<PrimitiveData>::new();
            for mesh_id in model_data.mesh_ids.iter() {
                let mesh = self
                    .gltf
                    .meshes()
                    .find(|m| m.index() == *mesh_id)
                    .ok_or(ModelBuilderError::MeshNotFound(*mesh_id))?;

                for primitive in mesh.primitives() {
                    let data = GltfModelBuilderNew::get_primitive_data(mesh.index(), &primitive)
                        .map_err(|e| ModelBuilderError::ValidationError(e))?;
                    primitive_data_buf.push(data);
                }
            }
            primtive_map.insert(model_data.id, primitive_data_buf);
        }

        Ok(primtive_map)
    }

    fn get_index_range_vec(
        &self,
        primitive_data: &HashMap<usize, Vec<PrimitiveData>>,
        buffer_offsets: &Vec<usize>,
    ) -> Result<Vec<Range<usize>>, ModelBuilderError> {
        let mut index_range_vec: Vec<Range<usize>> = Vec::new();
        for (_, data_buf) in primitive_data.iter() {
            for data in data_buf.iter() {
                crate::asset_manager::range_splicer::define_index_ranges(
                    &mut index_range_vec,
                    &self
                        .get_index_range(data.indices.as_ref(), buffer_offsets)
                        .map_err(|err| ModelBuilderError::ValidationError(err))?
                        .unwrap_or(Range { start: 0, end: 0 }),
                );
            }
        }

        Ok(index_range_vec)
    }
}

impl AssetBuilder for GltfModelBuilderNew {
    fn load_asset<V: ModelVertex, I: IndexType>(
        &mut self,
        mesh_pool: &mut MeshPool<V, I>,
    ) -> Result<(), crate::asset_manager::asset_manager::AssetLoadError> {
        let buffer_offsets = self.get_buffer_offsets();
        let model_data_vec = self.get_model_data()?;
        let primitive_data = self.get_primitive_data_map(&model_data_vec)?;
        let index_range_vec = self.get_index_range_vec(&primitive_data, &buffer_offsets);
        todo!("Port build all models!!!")
    }

    fn get_residency_level(&self) -> crate::asset_manager::asset_manager::AssetResidencyLevel {
        todo!()
    }
}
