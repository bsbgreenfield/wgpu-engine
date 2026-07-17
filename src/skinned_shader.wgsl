struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) normal: vec3<f32>,
  	@location(2) tex_coords: vec2<f32>,
	@location(3) joints: vec4<u32>,
  	@location(4) weights: vec4<f32>,

}

struct VertexOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(2) tex_coords: vec2<f32>,
}


struct DrawPushConstants {
    lt_idx: u32,
	joint_offset: u32,
}


struct CameraUniform {
	transform: mat4x4<f32>,
}

struct InstanceRecord {
	lt_base: u32,
	joint_base: u32,
	pad_1: u32,
	pad_2: u32,
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
var<storage, read> joint_transforms: array<mat4x4<f32>>;
@group(3) @binding(1)
var<storage, read> ibm_transforms: array<mat4x4<f32>>;

fn apply_bone_transform(joint_base: u32, joints: vec4<u32>, weights: vec4<f32>, position: vec3<f32>)  -> vec4<f32> {
	let joint0 = joint_transforms[joint_base + joints[0]] * ibm_transforms[joint_base + joints[0]];
	let joint1 = joint_transforms[joint_base + joints[1]] * ibm_transforms[joint_base + joints[1]];
	let joint2 = joint_transforms[joint_base + joints[2]] * ibm_transforms[joint_base + joints[2]];
	let joint3 = joint_transforms[joint_base + joints[3]] * ibm_transforms[joint_base + joints[3]];
	let skin_mat: mat4x4<f32> = 
	 							joint0 * weights[0] +
	 							joint1 * weights[1] +
	 							joint2 * weights[2] +
	 							joint3 * weights[3];
	let result: vec4<f32> = skin_mat * vec4<f32>(position, 1.0);
	return result;
}

@vertex
fn vs_main(obj: VertexInput, @builtin(instance_index) inst_idx: u32 ) -> VertexOutput {
	let record_idx: u32 = instance_offsets[inst_idx];
	let record: InstanceRecord = instance_records[record_idx];
	let global_t_matrix: mat4x4<f32> = global_transforms[inst_idx];
    var out: VertexOutput;
	let joint_base: u32 = record.joint_base + pc.joint_offset;
	let new_position: vec4<f32> = apply_bone_transform(joint_base, obj.joints, obj.weights, obj.position);
    out.clip_position = camera_uniform.transform * global_t_matrix * local_mesh_transforms[record.lt_base + pc.lt_idx] * new_position;
	out.tex_coords = obj.tex_coords;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	let colors = vec4<f32>(0.8, 0.3, 0.1, 1.0);
	return colors;
}
