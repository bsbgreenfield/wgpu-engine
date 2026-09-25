use std::collections::HashMap;
use std::range::Range;
use std::sync::Arc;

use crate::animation::{
    AnimationChannels, AnimationSampler, AnimationTransformType, AnimationTransforms,
    InterpolationType,
};
use crate::asset_manager::gltf_asset::mesh::{
    PrimitiveRange, copy_and_cast_gltf_binary_data_f32, copy_and_cast_gltf_binary_data_mat4f32,
};
use crate::asset_manager::gltf_asset::util::collect_mesh_ids;
use crate::asset_manager::gltf_asset::{
    AssetSources, GltfAnimation, GltfAsset, GltfLoadError, GltfMaterial, GltfTexture,
    NodeTransforms, NodeType, PBRMetallicRoughness, loader,
};
use crate::asset_manager::texture::{decode_embedded, decode_embedded_parallel};
use crate::asset_manager::{
    Asset, AssetHandle, BinaryData, GltfValidationError, ModelBuilderError, texture,
};
use crate::util::types::{
    AssetIndices, Mat4F32, ModelVertex, PrimitiveVerticesData, VIndex16, VIndex32,
};
use crate::{
    asset_manager::{
        gltf_asset::{
            GltfNode, Mesh,
            mesh::{GLTFDataAccessor, Primitive, PrimitiveData},
        },
        range_splicer,
    },
    util::types::{PNUJWVertex, PNUVertex},
};

impl GltfNode {
    fn new(node: &gltf::Node, skins: &Vec<Vec<usize>>) -> Self {
        let node_id = node.index();
        let children: Vec<GltfNode> = node.children().map(|c| GltfNode::new(&c, skins)).collect();
        Self {
            node_type: match node.mesh() {
                Some(m) => NodeType::Mesh(m.index()),
                None => match skin_index(node, skins) {
                    Some(joint_id) => NodeType::Joint(joint_id),
                    None => NodeType::Node,
                },
            },
            skin_idx: node.skin().map(|s| s.index()),
            node_id,
            children,
            transform_components: gltf_mat_to_transforms(node.transform().decomposed()),
        }
    }
}

fn skin_index(node: &gltf::Node, skins: &Vec<Vec<usize>>) -> Option<(u32, u32)> {
    for (skin_idx, skin) in skins.iter().enumerate() {
        if let Some(joint_idx) = skin.iter().position(|joint_idx| *joint_idx == node.index()) {
            return Some((skin_idx as u32, joint_idx as u32));
        } else {
            continue;
        };
    }
    None
}

fn gltf_mat_to_transforms(transforms: ([f32; 3], [f32; 4], [f32; 3])) -> [NodeTransforms; 3] {
    return [
        NodeTransforms::Translation(transforms.0.into()),
        NodeTransforms::Rotation(transforms.1.into()),
        NodeTransforms::Scale(transforms.2.into()),
    ];
}

fn node_subtree_has_channels(node: &GltfNode, channels: &AnimationChannels) -> bool {
    if channels.contains_key(&node.node_id) {
        return true;
    }
    node.children
        .iter()
        .any(|child| node_subtree_has_channels(child, channels))
}

fn get_animations(
    gltf: &gltf::Gltf,
    buffer_offsets: &Vec<usize>,
    binary_data: &Vec<u8>,
    node_tree: &[Arc<GltfNode>],
) -> Result<Vec<Arc<GltfAnimation>>, GltfValidationError> {
    let mut animations = Vec::<Arc<GltfAnimation>>::with_capacity(gltf.animations().count());
    for animation in gltf.animations() {
        let mut samplers: Vec<AnimationSampler> = Vec::with_capacity(animation.samplers().count());
        let mut channels = AnimationChannels::new();
        for sampler in animation.samplers() {
            let times_bytes = copy_and_cast_gltf_binary_data_f32(
                &GLTFDataAccessor::from_accessor(&sampler.input())?,
                buffer_offsets,
                binary_data,
            )?;
            let transforms_bytes = copy_and_cast_gltf_binary_data_f32(
                &GLTFDataAccessor::from_accessor(&sampler.output())?,
                buffer_offsets,
                binary_data,
            )?;

            let relevant_channels = animation
                .channels()
                .filter(|c| c.sampler().index() == sampler.index());

            for relevant_channel in relevant_channels {
                let ty =
                    AnimationTransformType::from_gltf_prop(&relevant_channel.target().property());
                channels
                    .entry(relevant_channel.target().node().index())
                    .or_insert_with(Vec::new)
                    .push((sampler.index(), ty));
            }

            samplers.push(AnimationSampler::new(
                InterpolationType::from(sampler.interpolation()),
                times_bytes,
                AnimationTransforms(transforms_bytes),
            ));
        }
        let root_nodes = node_tree
            .iter()
            .filter(|n| node_subtree_has_channels(n, &channels))
            .cloned()
            .collect();

        animations.push(Arc::new(GltfAnimation {
            samplers,
            channels,
            root_nodes,
        }));
    }
    Ok(animations)
}

fn get_skins(gltf: &gltf::Gltf) -> Vec<Vec<usize>> {
    gltf.skins()
        .map(|skin| skin.joints().map(|joint| joint.index()).collect())
        .collect()
}
fn build_node_trees(
    gltf: &gltf::Gltf,
    skins: &Vec<Vec<usize>>,
) -> Result<Vec<Arc<GltfNode>>, ModelBuilderError> {
    let scene = gltf
        .scenes()
        .next()
        .ok_or(gltf::Error::UnsupportedScheme)
        .map_err(|_| {
            return ModelBuilderError::ValidationError(GltfValidationError::UnsupportedScheme);
        })?;

    Ok(scene
        .nodes()
        .map(|root_node| Arc::new(GltfNode::new(&root_node, skins)))
        .collect())
}

/// return a Vec<Vec<PrimitiveData>> that is sorted into DFS order
fn get_primitive_data_map(
    gltf: &gltf::Gltf,
    node_tree: &[Arc<GltfNode>],
    buffer_offsets: &Vec<usize>,
) -> Result<Vec<(usize, Vec<PrimitiveData>)>, ModelBuilderError> {
    let mut mesh_id_to_prim_data = HashMap::<usize, Vec<PrimitiveData>>::new();
    for mesh in gltf.meshes() {
        let mut prim_data_list: Vec<PrimitiveData> = Vec::with_capacity(mesh.primitives().len());
        for primitive in mesh.primitives() {
            prim_data_list.push(
                Primitive::get_primitive_data(&primitive, buffer_offsets)
                    .map_err(|e| ModelBuilderError::ValidationError(e))?,
            );
        }
        mesh_id_to_prim_data.insert(mesh.index(), prim_data_list);
    }
    let mut dfs_mesh_ids: Vec<usize> = Vec::new();
    for node in node_tree {
        collect_mesh_ids(node, &mut dfs_mesh_ids);
    }
    Ok(dfs_mesh_ids
        .into_iter()
        .filter_map(|id| mesh_id_to_prim_data.remove(&id).map(|data| (id, data)))
        .collect())
}

fn get_index_range_vec(
    primitive_data: &Vec<(usize, Vec<PrimitiveData>)>,
) -> Result<(Vec<Range<usize>>, Vec<Range<usize>>), ModelBuilderError> {
    let mut index16_range_vec: Vec<Range<usize>> = Vec::new();
    let mut index32_range_vec: Vec<Range<usize>> = Vec::new();
    for (_mesh_id, mesh_primitives) in primitive_data.iter() {
        for prim_data in mesh_primitives.iter() {
            match &prim_data.indices {
                Some(PrimitiveRange::U32(r)) => {
                    range_splicer::define_index_ranges(&mut index32_range_vec, r)
                }
                Some(PrimitiveRange::U16(r)) => {
                    range_splicer::define_index_ranges(&mut index16_range_vec, r)
                }
                None => {}
            }
        }
    }

    Ok((index16_range_vec, index32_range_vec))
}
fn get_relative_indices(
    index_ranges: &[Range<usize>],
    primitive_index_range: &Range<usize>,
) -> Result<Range<usize>, ModelBuilderError> {
    let mut offset = 0;
    for range in index_ranges.iter() {
        if !range.contains(&primitive_index_range.start) {
            offset += range.end - range.start;
            continue;
        }
        let relative_primitive_index_offset = offset + primitive_index_range.start - range.start;

        return Ok(Range {
            start: relative_primitive_index_offset,
            end: relative_primitive_index_offset
                + (primitive_index_range.end - primitive_index_range.start),
        });
    }

    Err(ModelBuilderError::IndexRangeError)
}

fn find_relative_index_range(
    indices_16: &[Range<usize>],
    indices_32: &[Range<usize>],
    maybe_primitive_range: Option<&PrimitiveRange>,
) -> Result<Option<Range<u32>>, ModelBuilderError> {
    let Some(prim_range) = maybe_primitive_range else {
        return Ok(None);
    };

    let (index_ranges, base) = match prim_range {
        PrimitiveRange::U16(_) => (indices_16, 0),
        PrimitiveRange::U32(_) => (indices_32, element_count(indices_16, 2)),
    };

    let relative = get_relative_indices(index_ranges, prim_range)?;
    let stride = prim_range.byte_size();
    Ok(Some(Range {
        start: (base + relative.start / stride) as u32,
        end: (base + relative.end / stride) as u32,
    }))
}

fn element_count(index_ranges: &[Range<usize>], byte_size: usize) -> usize {
    index_ranges
        .iter()
        .map(|r| (r.end - r.start) / byte_size)
        .sum()
}

fn set_index_data(
    promote: bool,
    indices_16: &[Range<usize>],
    indices_32: &[Range<usize>],
    bin: &Vec<u8>,
) -> Option<AssetIndices> {
    if indices_16.is_empty() && indices_32.is_empty() {
        return None;
    }

    if !promote {
        debug_assert!(indices_32.is_empty());
        let mut index_vec: Vec<VIndex16> =
            Vec::with_capacity(element_count(indices_16, size_of::<u16>()));
        for range in indices_16.iter() {
            index_vec.extend(
                bin[range.start..range.end]
                    .chunks_exact(size_of::<u16>())
                    .map(|c| VIndex16::from(u16::from_le_bytes([c[0], c[1]]))),
            );
        }
        return Some(AssetIndices::U16(index_vec.into()));
    };
    let mut index_vec: Vec<VIndex32> = Vec::with_capacity(
        element_count(indices_16, size_of::<u16>()) + element_count(indices_32, size_of::<u32>()),
    );
    for range in indices_16.iter() {
        index_vec.extend(
            bin[range.start..range.end]
                .chunks_exact(size_of::<u16>())
                .map(|c| VIndex32::from(u16::from_le_bytes([c[0], c[1]]))),
        );
    }
    for range in indices_32.iter() {
        index_vec.extend(
            bin[range.start..range.end]
                .chunks_exact(size_of::<u32>())
                .map(|c| VIndex32::from(u32::from_le_bytes([c[0], c[1], c[2], c[3]]))),
        );
    }
    Some(AssetIndices::U32(index_vec.into()))
}

fn get_ibms(
    gltf: &gltf::Gltf,
    binary_data: &Vec<u8>,
    buffer_offsets: &Vec<usize>,
) -> Result<Vec<Vec<Mat4F32>>, ModelBuilderError> {
    let mut ibms = Vec::new();
    for skin in gltf.skins() {
        let acc =
            GLTFDataAccessor::from_accessor(&skin.inverse_bind_matrices().expect("must have ibm"))?;
        let skin_ibms = copy_and_cast_gltf_binary_data_mat4f32(&acc, buffer_offsets, binary_data)?;
        ibms.push(skin_ibms);
    }

    Ok(ibms)
}

fn get_materials(
    gltf: &gltf::Gltf,
    bin: &BinaryData,
    external_textures: &[Option<AssetHandle>],
) -> Result<Arc<[GltfMaterial]>, ModelBuilderError> {
    let mut materials: Vec<GltfMaterial> = Vec::new();
    let mut embedded_texture_indices = Vec::with_capacity(gltf.materials().len());

    for material in gltf.materials() {
        let pbr_data = material.pbr_metallic_roughness();
        // push the material with an empty texture for now
        materials.push(GltfMaterial {
            pbr_metallic_roughness: PBRMetallicRoughness {
                roughness: pbr_data.roughness_factor(),
                metallicness: pbr_data.metallic_factor(),
                base_color_factor: pbr_data.base_color_factor(),
                texture_idx: pbr_data.base_color_texture().map(|t| t.texture().index()),
                texture: None,
            },
        });

        if let Some(texture_info) = pbr_data.base_color_texture() {
            let tex_index = texture_info.texture().index();
            if matches!(
                texture_info.texture().source().source(),
                gltf::image::Source::View { .. }
            ) {
                embedded_texture_indices.push(tex_index);
            }
        }

        // returns a hash map containing every embedded texture's image data
        let decoded = decode_embedded_parallel(gltf, bin, &embedded_texture_indices)?;
        for material in materials.iter_mut() {
            if let Some(tex_idx) = material.pbr_metallic_roughness.texture_idx {
                // if there is a texture, either its in the decoded map
                // or its external, and the idx of the asset handle should be at tex.index
                match decoded.get(&tex_idx) {
                    Some(embedded_image) => {
                        material.pbr_metallic_roughness.texture =
                            Some(GltfTexture::Embedded(embedded_image.clone()))
                    }
                    None => {
                        material.pbr_metallic_roughness.texture = Some(GltfTexture::External(
                            external_textures[tex_idx].expect("should to be an asset handle here"),
                        ))
                    }
                }
            }
        }
    }

    Ok(materials.into())
}

fn build_all_models(
    binary_data: &Vec<u8>,
    index_ranges_16: &Vec<Range<usize>>,
    index_ranges_32: &Vec<Range<usize>>,
    buffer_offsets: &Vec<usize>,
    primitive_data: &Vec<(usize, Vec<PrimitiveData>)>,
    promote_indices: bool,
) -> Result<
    (
        Vec<PNUJWVertex>,
        Vec<PNUVertex>,
        Option<AssetIndices>,
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
            // binary data per vertex attribute
            let primitive_vertex_data: PrimitiveVerticesData =
                Primitive::get_primitive_vertex_data(buffer_offsets, primitive_data, &binary_data)?;

            let maybe_index_range = find_relative_index_range(
                index_ranges_16,
                index_ranges_32,
                primitive_data.indices.as_ref(),
            )?;

            let is_jointed = primitive_data.joints.is_some().clone();

            let current_primitive = if is_jointed {
                let vertex_range = Range {
                    start: pnujw_vertices.len() as u32,
                    end: (pnujw_vertices.len() + primitive_vertex_data.count) as u32,
                };
                Primitive::new::<PNUJWVertex>(
                    vertex_range,
                    maybe_index_range,
                    primitive_data.material_idx,
                )
            } else {
                let vertex_range = Range {
                    start: pnu_vertices.len() as u32,
                    end: (pnu_vertices.len() + primitive_vertex_data.count) as u32,
                };
                Primitive::new::<PNUVertex>(
                    vertex_range,
                    maybe_index_range,
                    primitive_data.material_idx,
                )
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
    let maybe_index_data = set_index_data(
        promote_indices,
        &index_ranges_16,
        index_ranges_32,
        &binary_data,
    );
    Ok((pnujw_vertices, pnu_vertices, maybe_index_data, meshes))
}
impl GltfAsset {
    pub fn load_binary_data(
        gltf: &gltf::Gltf,
        sources: &AssetSources,
    ) -> Result<BinaryData, GltfLoadError> {
        loader::load_binary_data_from_source(gltf, sources)
    }
    pub fn load(
        gltf: &gltf::Gltf,
        bin: &BinaryData,
        textures: &[Option<AssetHandle>],
    ) -> Result<Box<dyn Asset>, ModelBuilderError> {
        let binary_data = &bin.data;
        let buffer_offsets = &bin.buffer_offsets;
        let skins = get_skins(gltf);
        let node_tree = build_node_trees(gltf, &skins)?;

        let material_palette = get_materials(gltf, bin, textures)?;
        let ibms = get_ibms(&gltf, binary_data, buffer_offsets)?;
        let primitive_data = get_primitive_data_map(&gltf, &node_tree, buffer_offsets)?;
        let promote_indices: bool = primitive_data.iter().any(|(_, prims)| {
            prims
                .iter()
                .any(|p| p.indices.as_ref().is_some_and(|r| r.byte_size() == 4))
        });

        let (index_ranges_16, index_ranges_32) = get_index_range_vec(&primitive_data)?;
        let (pnujw, pnu, indices, meshes) = build_all_models(
            binary_data,
            &index_ranges_16,
            &index_ranges_32,
            buffer_offsets,
            &primitive_data,
            promote_indices,
        )?;
        let animations: Vec<Arc<GltfAnimation>> =
            get_animations(&gltf, buffer_offsets, binary_data, &node_tree)?;
        Ok(Box::new(GltfAsset {
            material_palette,
            pnujw_vertices: Arc::from_iter(pnujw),
            pnu_vertices: Arc::from_iter(pnu),
            node_tree,
            meshes,
            indices,
            animations,
            skins,
            ibms,
        }))
    }
}
