//! The screen's WGSL parses and validates (naga, as wgpu compiles it).

#[test]
fn the_glass_shader_is_valid() {
    let src = battlestation_web::glass::WGSL;
    let module =
        naga::front::wgsl::parse_str(src).unwrap_or_else(|e| panic!("{}", e.emit_to_string(src)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("{}", e.emit_to_string(src)));
}
