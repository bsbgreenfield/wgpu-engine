use std::{collections::HashMap, ops::Range};

use cgmath::SquareMatrix;

use crate::{
    asset_manager::{
        asset_manager::{AssetBuilder, AssetLoadError, AssetResidencyLevel, LoadedAsset},
        gltf_loader::loader::{BinarySource, GltfLoadError, GltfLoader},
        primitive::{GltfValidationError, PrimitiveData},
    },
    util::types::Mat4F32,
    world::components::MeshCollectionComponent,
};

#[derive(Debug)]
pub enum ModelBuilderError {
    NodeNotFound(usize),
    GLTFUndefined,
    GLTFLoadError(GltfLoadError),
    MeshNotFound(usize),
    ValidationError(GltfValidationError),
    IndexRangeError,
}

impl From<GltfValidationError> for ModelBuilderError {
    fn from(value: GltfValidationError) -> Self {
        Self::ValidationError(value)
    }
}
struct ModelData {
    mesh_ids: Vec<usize>,
    local_transforms: Vec<Mat4F32>,
    joint_data: Option<ModelJointData>,
}
impl ModelData {
    fn empty() -> Self {
        Self {
            mesh_ids: vec![],
            local_transforms: vec![],
            joint_data: None,
        }
    }
}

struct ModelJointData {
    joint_ids: Vec<usize>,
    joint_pose_transforms: Mat4F32,
    node_to_joint_id_map: HashMap<usize, usize>,
}

pub struct GltfBuilderRegistered {
    gltf: gltf::Gltf,
    bin_source: BinarySource,
}
impl GltfBuilderRegistered {
    pub fn new(dir_name: &str) -> Result<Self, AssetLoadError> {
        let (gltf, bin) = GltfLoader::load_gltf_from_resource(dir_name)?;
        Ok(Self {
            gltf,
            bin_source: bin,
        })
    }
}

pub struct GltfModelBuilder {
    residency_level: AssetResidencyLevel,
    pub(super) buffer_offsets: Vec<usize>,
    binary_source: BinarySource,
    model_count: usize,
    primitive_data: HashMap<usize, Vec<PrimitiveData>>,
    index_ranges: Vec<Range<usize>>,
    model_data: Vec<ModelData>,
}

impl GltfModelBuilder {
    pub(super) fn new() -> Self {
        Self {
            residency_level: AssetResidencyLevel::Registered,
            binary_source: BinarySource::Undefined,
            buffer_offsets: vec![],
            model_count: 0,
            primitive_data: HashMap::new(),
            model_data: vec![],
            index_ranges: vec![],
        }
    }
    fn get_root_nodes(gltf: &gltf::Gltf) -> Result<Vec<usize>, gltf::Error> {
        let scene = gltf.scenes().next().ok_or(gltf::Error::UnsupportedScheme)?;
        let mesh_node_iter = scene
            .nodes()
            .filter(|n| n.mesh().is_some() || n.children().len() != 0);
        let ids: Vec<usize> = mesh_node_iter.map(|node| node.index()).collect();
        Ok(ids)
    }

    fn get_model_data(
        &self,
        root_node: &gltf::Node,
        base_transform: cgmath::Matrix4<f32>,
        mut model_data: ModelData,
    ) -> Result<ModelData, ModelBuilderError> {
        let cg_trans = cgmath::Matrix4::<f32>::from(root_node.transform().matrix());
        let new_trans = base_transform * cg_trans;
        if let Some(mesh) = root_node.mesh() {
            model_data.mesh_ids.push(mesh.index());
            model_data.local_transforms.push(new_trans.into());
        }
        for child_node in root_node.children() {
            model_data = self.get_model_data(&child_node, base_transform, model_data)?;
        }
        Ok(model_data)
    }
    fn setup_primitive_data(
        &mut self,
        model_data: &ModelData,
        gltf: &gltf::Gltf,
    ) -> Result<bool, ModelBuilderError> {
        let mut primitive_data_buf = Vec::<PrimitiveData>::new();
        for mesh_id in model_data.mesh_ids.iter() {
            let mesh = gltf
                .meshes()
                .find(|m| m.index() == *mesh_id)
                .ok_or(ModelBuilderError::MeshNotFound(*mesh_id))?;

            for primitive in mesh.primitives() {
                let data = self
                    .get_primitive_data(mesh.index(), &primitive)
                    .map_err(|e| ModelBuilderError::ValidationError(e))?;
                primitive_data_buf.push(data);
            }
        }
        self.primitive_data
            .insert(self.model_count, primitive_data_buf);

        Ok(true)
    }

    fn with_gltf(
        &mut self,
        gltf: &gltf::Gltf,
        binary_source: BinarySource,
    ) -> Result<&mut Self, ModelBuilderError> {
        self.binary_source = binary_source;

        // set the buffer offets for the current gltf
        let mut buffer_offsets = Vec::<usize>::new();
        let mut last_buffer_size = 0;
        for buffer in gltf.buffers().clone().into_iter() {
            buffer_offsets.push(last_buffer_size);
            last_buffer_size += buffer.length();
        }
        self.buffer_offsets = buffer_offsets;

        // get all root nodes
        for rid in Self::get_root_nodes(gltf).unwrap().iter() {
            let root_node = gltf
                .nodes()
                .find(|root_node| root_node.index() == *rid)
                .ok_or(ModelBuilderError::NodeNotFound(*rid))?;

            let mut model_data = ModelData::empty();
            model_data =
                self.get_model_data(&root_node, cgmath::Matrix4::<f32>::identity(), model_data)?;

            self.setup_primitive_data(&model_data, gltf)?;
            self.model_data.push(model_data);

            self.model_count += 1;
        }

        let mut index_range_vec: Vec<Range<usize>> = Vec::new();
        for (_, data_buf) in self.primitive_data.iter() {
            for data in data_buf.iter() {
                crate::asset_manager::range_splicer::define_index_ranges(
                    &mut index_range_vec,
                    &self
                        .get_index_range(data.indices.as_ref())
                        .map_err(|err| ModelBuilderError::ValidationError(err))?
                        .unwrap_or(Range { start: 0, end: 0 }),
                );
            }
        }
        println!("RANGE_VEC: {:?}", index_range_vec);

        self.index_ranges = index_range_vec;
        Ok(self)
    }
    pub(super) fn create_components(&self) -> Result<Vec<LoadedAsset>, AssetLoadError> {
        let mut loaded_asset = LoadedAsset::new();
        let mesh_collection = MeshCollectionComponent::new(vec![]);
        loaded_asset.add_component(Box::new(mesh_collection));
        Ok(vec![loaded_asset])
    }
}

impl AssetBuilder for GltfBuilderRegistered {
    fn get_components(&self) -> Result<Vec<LoadedAsset>, AssetLoadError> {
        Err(AssetLoadError::AssetNotLoaded)
    }
    fn load_asset(self) -> Result<Box<dyn AssetBuilder>, AssetLoadError> {
        let mut model_builder = GltfModelBuilder::new();
        model_builder.with_gltf(&self.gltf, self.bin_source);
        Ok(Box::new(model_builder))
    }

    fn get_residency_level(&self) -> AssetResidencyLevel {
        AssetResidencyLevel::Registered
    }
}

impl AssetBuilder for GltfModelBuilder {
    fn get_components(&self) -> Result<Vec<LoadedAsset>, AssetLoadError> {
        self.create_components()
    }
    fn load_asset(self) -> Result<Box<dyn AssetBuilder>, AssetLoadError> {
        Ok(Box::new(self))
    }
    fn get_residency_level(&self) -> super::asset_manager::AssetResidencyLevel {
        self.residency_level
    }
}
