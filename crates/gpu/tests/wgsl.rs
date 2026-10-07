//! Every WGSL string the GPU layer ships parses and validates (naga, the
//! compiler wgpu uses), so a shader error is a failed test, not a black page.

pub fn valid(name: &str, src: &str) {
    let module = naga::front::wgsl::parse_str(src)
        .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(src)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(src)));
}

#[test]
fn the_layer_shader_is_valid() {
    valid("layer", gpu::layer::WGSL);
}
