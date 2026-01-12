use std::{collections::HashMap, ops::Range};

use crate::{
    asset_manager::{
        asset_manager::{AssetBuilder, AssetLoadError, AssetType},
        gltf_loader::loader::{BinarySource, GltfLoadError, GltfLoader},
    },
    util::{primitive::PrimitiveData, types::Mat4F32},
};
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
pub(super) struct GltfModelBuilder {
    buffer_offsets: Vec<usize>,
    binary_source: BinarySource,
    model_count: usize,
    primitive_data: HashMap<usize, PrimitiveData>,
    index_ranges: Vec<Range<usize>>,
    model_data: Vec<ModelData>,
}

impl GltfModelBuilder {
    pub(super) fn new() -> Self {
        Self {
            binary_source: BinarySource::Undefined,
            buffer_offsets: vec![],
            model_count: 0,
            primitive_data: HashMap::new(),
            model_data: vec![],
            index_ranges: vec![],
        }
    }
}

impl AssetBuilder for GltfModelBuilder {
    fn with_asset(&mut self, dir_name: &str) -> Result<(), AssetLoadError> {
        let (gltf, binary_source) = GltfLoader::load_gltf_from_resource(dir_name)?;
        todo!()
    }
}
