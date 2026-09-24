struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) normal: vec3<f32>,
  	@location(2) tex_coords: vec2<f32>,
}

struct VertexOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(2) tex_coords: vec2<f32>,
}

struct DrawPushConstants {
    lt_idx: u32,
	material_offset: u32,
}


struct InstanceRecord {
	lt_base: u32,
	joint_base: u32,
	pad_1: u32,
	pad_2: u32,
}

struct Material {
	base_color_factors: vec4<f32>,
	roughness: f32,
	metallic: f32,
	tex_mod: u32,
	pad_2: u32,
}

struct CameraUniform {
	transform: mat4x4<f32>,
}

var<immediate> pc: DrawPushConstants;

@group(0) @binding(0)
var<uniform> camera_uniform: CameraUniform;

@group(1) @binding(0)
var<storage, read> local_mesh_transforms: array<mat4x4<f32>>;


@group(2) @binding(0)
var<storage, read> instance_records: array<InstanceRecord>;
@group(2) @binding(1)
var<storage, read> instance_offsets: array<u32>;
@group(2) @binding(2)
var<storage, read> global_transforms: array<mat4x4<f32>>;

@group(3) @binding(0)
var t_default: texture_2d_array<f32>;
@group(3) @binding(1)
var t_64: texture_2d_array<f32>;
@group(3) @binding(2)
var t_128: texture_2d_array<f32>;
@group(3) @binding(3)
var t_256: texture_2d_array<f32>;
@group(3) @binding(4)
var t_1024: texture_2d_array<f32>;
@group(3) @binding(5)
var t_2048: texture_2d_array<f32>;
@group(3) @binding(6)
var s_diffuse: sampler;
@group(3) @binding(7)
var<storage, read> materials: array<Material>;


fn sample_diffuse(tex_modifier: u32, uv: vec2<f32>) -> vec4<f32> {
      let bucket: u32 = tex_modifier >> 16u;
      let layer: i32 = i32(tex_modifier & 0xFFFFu);
      switch bucket {
              case 1u: { return textureSampleLevel(t_64,   s_diffuse, uv, layer, 0.0); }
              case 2u: { return textureSampleLevel(t_128,  s_diffuse, uv, layer, 0.0); }
              case 3u: { return textureSampleLevel(t_256,  s_diffuse, uv, layer, 0.0); }
              case 4u: { return textureSampleLevel(t_1024, s_diffuse, uv, layer, 0.0); }
              case 5u: { return textureSampleLevel(t_2048, s_diffuse, uv, layer, 0.0); }
              default: { return textureSampleLevel(t_default, s_diffuse, uv, 0, 0.0); }
      }
}

@vertex
fn vs_main(obj: VertexInput, @builtin(instance_index) inst_idx: u32) -> VertexOutput {
	let record_idx: u32 = instance_offsets[inst_idx];
	let record: InstanceRecord = instance_records[record_idx];
	let global_t_matrix: mat4x4<f32> = global_transforms[inst_idx];
    var out: VertexOutput;
    out.clip_position = camera_uniform.transform * global_t_matrix * local_mesh_transforms[record.lt_base + pc.lt_idx] * vec4<f32>(obj.position, 1.0);
	out.tex_coords = obj.tex_coords;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	let material = materials[pc.material_offset];
	return sample_diffuse(material.tex_mod, in.tex_coords) * material.base_color_factors;
}
