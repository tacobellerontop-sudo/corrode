//! The desktop's WGSL shaders only compile when the GPU path first runs, so a mistake would
//! surface as a crash while watching a stream. Parse and validate them here instead.

fn validate(name: &str, source: &str) {
	let module = naga::front::wgsl::parse_str(source)
		.unwrap_or_else(|error| panic!("{name}: {}", error.emit_to_string(source)));
	naga::valid::Validator::new(
		naga::valid::ValidationFlags::all(),
		naga::valid::Capabilities::empty(),
	)
	.validate(&module)
	.unwrap_or_else(|error| panic!("{name}: {error:?}"));
}

#[test]
fn stream_colour_conversion_shader_is_valid() {
	validate(
		"yuv.wgsl",
		include_str!("../../../apps/desktop/src/yuv.wgsl"),
	);
}
