use crate::{
    asset_manager::{
        asset_manager::{AssetBuilder, AssetLoadError, AssetResidencyLevel, LoadedAsset, MeshPool},
        gltf_assets::{
            gltf_loader::loader::{BinarySource, GltfLoadError, GltfLoader},
            mesh::{Mesh, Primitive},
            primitive::{GltfValidationError, PrimitiveData},
        },
    },
    util::types::{IndexType, Mat4F32, ModelVertex},
    world::components::MeshCollectionComponent,
};
use cgmath::SquareMatrix;
use std::{any::Any, collections::HashMap, ops::Range, rc::Rc};

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
    pub(in crate::asset_manager) buffer_offsets: Vec<usize>,
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

    fn get_relative_indices(
        &self,
        primitive_index_range: &Range<usize>,
    ) -> Result<Range<usize>, ModelBuilderError> {
        let mut offset = 0;
        for range in self.index_ranges.iter() {
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

    pub fn build_all_models<V: ModelVertex, I: IndexType>(
        &self,
        mesh_pool: &mut MeshPool<V, I>,
    ) -> Result<(), ModelBuilderError> {
        let vertex_stride = size_of::<V>();
        let index_stride = size_of::<I>();

        let binary_data = GltfLoader::load_binary_data_from_source(&self.binary_source)
            .map_err(|_| ModelBuilderError::BinarySourceNotFound)?;

        let mut mesh_collections = Vec::<MeshCollectionComponent>::new();

        let mut meshes = Vec::<Mesh>::new();
        for ((model_id, primitive_data), model_data) in
            self.primitive_data.iter().zip(self.model_data.iter())
        {
            let mut primitives = Vec::<Primitive>::new();
            let mut mesh_id = primitive_data
                .first()
                .expect("there are no primtives!")
                .mesh_id;
            for data in primitive_data.iter() {
                assert!(data.mesh_id == mesh_id);
                mesh_id = data.mesh_id;
                let model_vertices: Vec<V> = self.get_primitive_vertex_data(data, &binary_data)?;
                let primitive_index_range = self.get_index_range(data.indices.as_ref()).unwrap();
                let relative_index_range = self.get_relative_indices(
                    &primitive_index_range.unwrap_or(Range { start: 0, end: 0 }),
                )?;

                let index_len = mesh_pool.cpu.indices.len().clone();
                let vertex_len = mesh_pool.cpu.vertices.len().clone();
                let index_range = Range {
                    start: (index_len + (relative_index_range.start / index_stride)) as u32,
                    end: (index_len + (relative_index_range.end / index_stride)) as u32,
                };

                let vertex_range = Range {
                    start: (vertex_len * vertex_stride) as u32,
                    end: ((vertex_len + model_vertices.len()) * vertex_stride) as u32,
                };
                primitives.push(Primitive::new(vertex_range, index_range));
                mesh_pool.push_vertices(model_vertices);
            }
            meshes.push(Mesh {
                primitives,
                id: mesh_id as u32,
            });
        }
        mesh_pool.push_indices(&self.index_ranges, &binary_data);
        todo!()
    }

    pub(super) fn create_components(&self) -> Result<LoadedAsset, AssetLoadError> {
        let mut loaded_asset = LoadedAsset::new();
        // TODO: extract mesh collection components
        let mesh_collection: Vec<Rc<dyn Any>> =
            vec![Rc::new(MeshCollectionComponent::new(vec![], vec![]))];
        loaded_asset.add_component(mesh_collection);
        Ok(loaded_asset)
    }
}

impl AssetBuilder for GltfBuilderRegistered {
    fn get_components(&self) -> Result<LoadedAsset, AssetLoadError> {
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
    fn get_components(&self) -> Result<LoadedAsset, AssetLoadError> {
        self.create_components()
    }
    fn load_asset(self) -> Result<Box<dyn AssetBuilder>, AssetLoadError> {
        Ok(Box::new(self))
    }
    fn get_residency_level(&self) -> AssetResidencyLevel {
        self.residency_level
    }
}
