//! The engine's WGSL, as Rust strings (rule 1): the scene's shaders (the
//! shared part, then the world, sky, sparks and grass, then decals), the sun's shadow
//! map, the ambient occlusion, the sun's shafts, and the post-processing.
//! Each module is validated by naga in a test, so a shader error is a
//! failed test, not a black page.

pub mod ao;
pub mod common;
pub mod decal;
pub mod post;
pub mod shafts;
pub mod world;

/// Bytes of `Globals`: four mat4s and eighteen vec4s.
pub const GLOBALS: u64 = 4 * 64 + 18 * 16;

/// A number in 0..1 that shifts from pixel to pixel with little pattern
/// (interleaved gradient noise, after Jimenez): what the passes that
/// march or dither turn their start by, so a blur or the eye evens it out.
pub const IGN: &str = r#"
fn ign(v: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(0.06711056 * v.x + 0.00583715 * v.y));
}
"#;

/// The scene's depth as a shader reads it: many-sampled or not.
pub fn depth_type(msaa: bool) -> &'static str {
    if msaa {
        "texture_depth_multisampled_2d"
    } else {
        "texture_depth_2d"
    }
}

/// The scene's module: the shared part, then its shaders, the engine's
/// numbers put in; for a depth that is many-sampled (`msaa`) or not.
/// The scene's shaders: `msaa` (the depth multisampled or not), the
/// sea's reflections marching `ssr` steps (out to 120 m).
pub fn scene(msaa: bool, ssr: u32) -> String {
    let steps = ssr.max(2);
    let grow = (120.0f32 / 0.6).powf(1.0 / (steps - 1) as f32);
    format!("{}{}{}", common::COMMON, world::WORLD, decal::DECAL)
        .replace("DEPTH_TYPE", depth_type(msaa))
        .replace("NEAR_PLANE", &format!("{:.4}", crate::laws::NEAR))
        .replace("SPARK_MAX_PX", &format!("{:.1}", crate::laws::SPARK_MAX_PX))
        .replace("PUFF_MAX", &format!("{:.3}", crate::laws::PUFF_MAX))
        .replace("SPARK_NEAR", &format!("{:.3}", crate::laws::SPARK_NEAR))
        .replace("GRASS_THIN", &format!("{:.3}", crate::laws::GRASS_THIN))
        .replace("SSR_STEPS", &format!("{steps}"))
        .replace("SSR_GROW", &format!("{grow:.5}"))
}

pub fn shadow() -> String {
    post::SHADOW.to_string()
}

pub fn post() -> String {
    format!("{}{IGN}", post::POST)
}
