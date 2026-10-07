//! The engine's WGSL parses and validates (naga, as wgpu compiles it).

#[test]
fn the_engine_shaders_are_valid() {
    let src = render::shaders::wgsl();
    let module =
        naga::front::wgsl::parse_str(&src).unwrap_or_else(|e| panic!("{}", e.emit_to_string(&src)));
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&src)));
    let _ = info;
    for entry in [
        "world_vs", "world_fs", "sky_vs", "sky_fs", "spark_vs", "spark_fs",
    ] {
        assert!(
            module.entry_points.iter().any(|e| e.name == entry),
            "{entry}"
        );
    }
}

#[test]
fn globals_match_the_struct() {
    // A mat4 and a vec4 a line after it: what draw.rs writes.
    let src = render::shaders::WGSL;
    let body = &src[src.find("struct Globals {").unwrap()..];
    let body = &body[..body.find("};").unwrap()];
    let vec4s = body.matches("vec4<f32>,").count() as u64;
    assert_eq!(render::shaders::GLOBALS, 64 + vec4s * 16);
}
