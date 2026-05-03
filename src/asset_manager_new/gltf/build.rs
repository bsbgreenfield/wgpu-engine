use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use gltf::animation;

use crate::animation::animation::{
    AnimationChannel, AnimationSampler, AnimationTransforms, InterpolationType,
};
use crate::asset_manager_new::gltf::mesh::copy_binary_data_from_gltf;
use crate::asset_manager_new::gltf::{
    GltfAnimation, GltfAttributeType, get_root_node_from_child_id,
};
use crate::asset_manager_new::{GltfValidationError, LoadedAsset, ModelBuilderError};
use crate::util::types::{ModelVertex, VIndex};
use crate::{
    asset_manager_new::{
        LoadableAsset,
        gltf::{
            BinarySource, GltfAsset, GltfNode, LoadedGltfAsset, Mesh,
            mesh::{GLTFDataAccessor, Primitive, PrimitiveData},
        },
        range_splicer,
    },
    util::types::{PNUJWVertex, PNUVertex},
};

impl GltfNode {
    fn new(node: &gltf::Node) -> Self {
        let node_id = node.index();
        let mesh_id = node.mesh().map(|m| m.index());
        let children: Vec<GltfNode> = node.children().map(|c| GltfNode::new(&c)).collect();
        Self {
            mesh_id,
            node_id,
            children,
            transform: node.transform().matrix(),
        }
    }
}

fn get_animations(
    gltf: &gltf::Gltf,
    buffer_offsets: &Vec<usize>,
    binary_data: &Vec<u8>,
    node_tree: &Arc<Vec<GltfNode>>,
) -> Result<Vec<GltfAnimation>, GltfValidationError> {
    let mut animations = Vec::<GltfAnimation>::with_capacity(gltf.animations().count());
    for animation in gltf.animations() {
        let mut root_node_ids = Vec::<usize>::new();
        let mut gltf_channels = animation.channels();
        let mut samplers: Vec<AnimationSampler> = Vec::with_capacity(animation.samplers().count());
        let mut channels: Vec<AnimationChannel> = Vec::with_capacity(animation.channels().count());
        let mut sampler_idx = 0;
        for sampler in animation.samplers() {
            let times_bytes = copy_binary_data_from_gltf(
                &GLTFDataAccessor::from_accessor(&sampler.input())?,
                buffer_offsets,
                binary_data,
            )?;
            let transforms_bytes = copy_binary_data_from_gltf(
                &GLTFDataAccessor::from_accessor(&sampler.output())?,
                buffer_offsets,
                binary_data,
            )?;

            let relevent_channel = &gltf_channels
                .find(|x| x.sampler().index() == sampler_idx)
                .expect("should be a sampler");

            // this is annoying because we have to recurse for every sampler
            // but its only at upload time, and its needed to select the correct tree root to
            // recurse per frame at animation time.
            if let Some(idx) =
                get_root_node_from_child_id(&node_tree, relevent_channel.target().node().index())
            {
                root_node_ids.push(idx);
            } else {
                return Err(GltfValidationError::UnsupportedScheme); // if we cant find the node
                // target, then we can't build
                // the animation
            }

            samplers.push(AnimationSampler::new(
                InterpolationType::from(sampler.interpolation()),
                bytemuck::cast_vec(times_bytes),
                AnimationTransforms::from_bytes(
                    GltfAttributeType::from_animation_channel(
                        &gltf_channels
                            .find(|x| x.sampler().index() == sampler_idx)
                            .expect("should be a sampler"),
                    ),
                    transforms_bytes,
                ),
            ));
            sampler_idx += 1;
        }

        for channel in animation.channels() {
            channels.push(AnimationChannel::from_gltf_channel(&channel));
        }
        animations.push(GltfAnimation {
            samplers,
            channels,
            root_nodes: root_node_ids,
        });
    }
    Ok(animations)
}

fn get_buffer_offsets(gltf: &gltf::Gltf) -> Vec<usize> {
    let mut buffer_offsets = Vec::<usize>::new();
    let mut last_buffer_size = 0;
    for buffer in gltf.buffers() {
        buffer_offsets.push(last_buffer_size);
        last_buffer_size += buffer.length();
    }
    buffer_offsets
}
fn build_node_trees(gltf: &gltf::Gltf) -> Result<Vec<GltfNode>, ModelBuilderError> {
    let scene = gltf
        .scenes()
        .next()
        .ok_or(gltf::Error::UnsupportedScheme)
        .map_err(|_| {
            return ModelBuilderError::ValidationError(GltfValidationError::UnsupportedScheme);
        })?;

    Ok(scene
        .nodes()
        .map(|root_node| GltfNode::new(&root_node))
        .collect())
}

fn get_primitive_data_map(
    gltf: &gltf::Gltf,
) -> Result<HashMap<usize, Vec<PrimitiveData>>, ModelBuilderError> {
    let mut mesh_id_to_prim_data = HashMap::<usize, Vec<PrimitiveData>>::new();
    for mesh in gltf.meshes() {
        let mut prim_data_list: Vec<PrimitiveData> = Vec::with_capacity(mesh.primitives().len());
        for primitive in mesh.primitives() {
            prim_data_list.push(
                Primitive::get_primitive_data(&primitive)
                    .map_err(|e| ModelBuilderError::ValidationError(e))?,
            );
        }
        mesh_id_to_prim_data.insert(mesh.index(), prim_data_list);
    }
    Ok(mesh_id_to_prim_data)
}

fn get_index_range_vec(
    primitive_data: &HashMap<usize, Vec<PrimitiveData>>,
    buffer_offsets: &Vec<usize>,
) -> Result<Vec<Range<usize>>, ModelBuilderError> {
    let mut index_range_vec: Vec<Range<usize>> = Vec::new();
    for mesh_prim_data in primitive_data.iter() {
        for prim_data in mesh_prim_data.1.iter() {
            let maybe_index_ranges =
                &Primitive::get_index_range(prim_data.indices.as_ref(), buffer_offsets)
                    .map_err(|e| ModelBuilderError::ValidationError(e))?;
            if let Some(index_ranges) = maybe_index_ranges {
                range_splicer::define_index_ranges(&mut index_range_vec, index_ranges);
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
        let relative_primitive_index_offset = offset + primitive_index_range.start - range.start;

        return Ok(Range {
            start: relative_primitive_index_offset,
            end: relative_primitive_index_offset + primitive_index_range.len(),
        });
    }

    Err(ModelBuilderError::IndexRangeError)
}

fn find_relative_index_range(
    index_ranges: &Vec<Range<usize>>,
    indices_accessor: Option<GLTFDataAccessor>,
    buffer_offsets: &Vec<usize>,
) -> Result<Option<Range<u32>>, ModelBuilderError> {
    if !index_ranges.is_empty() {
        let maybe_primitive_index_range =
            Primitive::get_index_range(indices_accessor.as_ref(), buffer_offsets)?;
        // range of this primitives indices within the final GPU index buffer allocation
        let maybe_relative_index_range = maybe_primitive_index_range.map(|primitive_index_range| {
            get_relative_indices(index_ranges, &primitive_index_range).unwrap()
        });

        return Ok(
            maybe_relative_index_range.map(|relative_index_range| Range {
                start: (relative_index_range.start / size_of::<u16>()) as u32,
                end: (relative_index_range.end / size_of::<u16>()) as u32,
            }),
        );
    } else {
        return Ok(None);
    }
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
    binary_data: &Vec<u8>,
    index_ranges: &Vec<Range<usize>>,
    buffer_offsets: &Vec<usize>,
    primitive_data: &HashMap<usize, Vec<PrimitiveData>>,
) -> Result<
    (
        Vec<PNUJWVertex>,
        Vec<PNUVertex>,
        Option<Vec<VIndex>>,
        Vec<Mesh>,
    ),
    ModelBuilderError,
> {
    let mut pnujw_vertices: Vec<PNUJWVertex> = Vec::new();
    let mut pnu_vertices: Vec<PNUVertex> = Vec::new();

    let mut meshes = Vec::<Mesh>::new();
    for (mesh_id, mesh_primitive_data) in primitive_data.iter() {
        let mut primitives = Vec::with_capacity(mesh_primitive_data.len());
        for primitive_data in mesh_primitive_data.iter() {
            if primitive_data.indices.is_some() {
                assert_eq!(primitive_data.indices.as_ref().unwrap().byte_size, 2);
            }
            // binary data per vertex attribute
            let primitive_vertex_data =
                Primitive::get_primitive_vertex_data(buffer_offsets, primitive_data, &binary_data)?;

            let maybe_index_range =
                find_relative_index_range(index_ranges, primitive_data.indices, buffer_offsets)?;

            let is_jointed = primitive_data.joints.is_some().clone();

            let current_primitive = if is_jointed {
                let vertex_range = Range {
                    start: pnujw_vertices.len() as u32,
                    end: (pnujw_vertices.len() + primitive_vertex_data.count) as u32,
                };
                Primitive::new::<PNUJWVertex>(vertex_range, maybe_index_range)
            } else {
                let vertex_range = Range {
                    start: pnu_vertices.len() as u32,
                    end: (pnu_vertices.len() + primitive_vertex_data.count) as u32,
                };
                Primitive::new::<PNUVertex>(vertex_range, maybe_index_range)
            };

            primitives.push(current_primitive);

            if is_jointed {
                pnujw_vertices.extend(PNUJWVertex::from_primitive_data(&primitive_vertex_data));
            } else {
                pnu_vertices.extend(PNUVertex::from_primitive_data(&primitive_vertex_data));
            }
        }
        meshes.push(Mesh {
            id: *mesh_id as u32,
            primitives,
        });
    }
    let maybe_index_data = set_index_data(&index_ranges, &binary_data);
    Ok((pnujw_vertices, pnu_vertices, maybe_index_data, meshes))
}

impl LoadableAsset for GltfAsset {
    fn load(&self) -> Result<Box<dyn LoadedAsset>, ModelBuilderError> {
        let buffer_offsets = get_buffer_offsets(&self.gltf);
        let node_tree = Arc::new(build_node_trees(&self.gltf)?);
        let binary_data = super::loader::load_binary_data_from_source(&self.bin)
            .map_err(|_| ModelBuilderError::BinarySourceNotFound)?;
        let animations = get_animations(&self.gltf, &buffer_offsets, &binary_data, &node_tree)?;
        let primitive_data = get_primitive_data_map(&self.gltf)?;
        let index_range_vec = get_index_range_vec(&primitive_data, &buffer_offsets)?;
        let (pnujw, pnu, indices, meshes) = build_all_models(
            &binary_data,
            &index_range_vec,
            &buffer_offsets,
            &primitive_data,
        )?;
        Ok(Box::new(LoadedGltfAsset {
            pnujw_vertices: pnujw,
            pnu_vertices: pnu,
            node_tree,
            meshes,
            indices,
            animations,
        }))
    }
}
