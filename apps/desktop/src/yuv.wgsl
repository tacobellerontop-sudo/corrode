// I420 → RGBA for watched streams (apps/desktop/src/video_gpu.rs). Limited-range BT.601,
// the WebRTC default, matching the CPU conversion in discord-voice's video_receive.rs.
struct Out {
	@builtin(position) position: vec4<f32>,
	@location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) index: u32) -> Out {
	// One triangle that covers the whole target.
	let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
	var out: Out;
	out.position = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
	out.uv = uv;
	return out;
}

@group(0) @binding(0) var luma: texture_2d<f32>;
@group(0) @binding(1) var blue: texture_2d<f32>;
@group(0) @binding(2) var red: texture_2d<f32>;
@group(0) @binding(3) var linear_sampler: sampler;

@fragment
fn fs(in: Out) -> @location(0) vec4<f32> {
	let y = 1.164 * (textureSample(luma, linear_sampler, in.uv).r - 16.0 / 255.0);
	let u = textureSample(blue, linear_sampler, in.uv).r - 128.0 / 255.0;
	let v = textureSample(red, linear_sampler, in.uv).r - 128.0 / 255.0;
	let rgb = vec3<f32>(y + 1.596 * v, y - 0.392 * u - 0.813 * v, y + 2.017 * u);
	return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
