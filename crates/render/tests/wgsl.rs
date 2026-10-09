//! The engine's WGSL parses and validates (naga, as wgpu compiles it).

fn valid(name: &str, src: &str, entries: &[&str]) {
    let module = naga::front::wgsl::parse_str(src)
        .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(src)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(src)));
    for entry in entries {
        assert!(
            module.entry_points.iter().any(|e| e.name == *entry),
            "{name}: {entry}"
        );
    }
}

#[test]
fn the_scene_shaders_are_valid() {
    for msaa in [false, true] {
        valid(
            "scene",
            &render::shaders::scene(msaa),
            &[
                "world_vs",
                "world_fs",
                "sky_vs",
                "sky_fs",
                "spark_vs",
                "spark_fs",
                "spark_soft_fs",
                "grass_vs",
                "grass_fs",
            ],
        );
    }
}

#[test]
fn the_shadow_and_post_shaders_are_valid() {
    valid("shadow", &render::shaders::shadow(), &["shadow_vs"]);
    valid(
        "post",
        &render::shaders::post(),
        &["post_vs", "down_fs", "up_fs", "finish_fs"],
    );
    for msaa in [false, true] {
        valid(
            "ao",
            &render::shaders::ao::ao(msaa),
            &["ao_vs", "ao_fs", "blur_fs", "apply_fs"],
        );
        valid(
            "shafts",
            &render::shaders::shafts::shafts(msaa),
            &["shafts_vs", "shafts_fs"],
        );
    }
}

#[test]
fn globals_match_the_struct() {
    // A mat4 is 64 bytes, a vec4 16: what draw.rs writes.
    let src = render::shaders::common::COMMON;
    let body = &src[src.find("struct Globals {").unwrap()..];
    let body = &body[..body.find("};").unwrap()];
    let vec4s = body.matches("vec4<f32>,").count() as u64;
    let mats = body.matches("mat4x4<f32>").count() as u64;
    assert_eq!(mats, 2, "vp, and the cascades' array of three");
    assert_eq!(render::shaders::GLOBALS, 64 + 3 * 64 + vec4s * 16);
}
