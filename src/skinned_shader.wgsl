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

struct InstanceInput {
	@location(5) gtm_0: vec4<f32>,
	@location(6) gtm_1: vec4<f32>,
	@location(7) gtm_2: vec4<f32>,
	@location(8) gtm_3: vec4<f32>,
}

struct DrawPushConstants {
    mesh_index: u32,
}


struct CameraUniform {
	transform: mat4x4<f32>,
}

var<immediate> pc: DrawPushConstants;

@group(0) @binding(0)
var<uniform> camera_uniform: CameraUniform;

@group(1) @binding(0)
var<storage, read> local_mesh_transforms: array<mat4x4<f32>>;


@vertex
fn vs_main(obj: VertexInput, instance: InstanceInput) -> VertexOutput {
	let global_t_matrix = mat4x4<f32>(
		instance.gtm_0, 
		instance.gtm_1, 
		instance.gtm_2, 
		instance.gtm_3,
	);
    var out: VertexOutput;
    out.clip_position = camera_uniform.transform * global_t_matrix * local_mesh_transforms[pc.mesh_index] * vec4<f32>(obj.position, 1.0);
	out.tex_coords = obj.tex_coords;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	let colors = vec4<f32>(0.8, 0.3, 0.1, 1.0);
	return colors;
}
