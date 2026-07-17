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
}


struct InstanceRecord {
	lt_base: u32,
	joint_base: u32,
	pad_1: u32,
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
	let colors = vec4<f32>(0.8, 0.3, 0.1, 1.0);
	return colors;
}
