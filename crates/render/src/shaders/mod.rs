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

/// Bytes of `Globals`: five mat4s and twenty-one vec4s.
pub const GLOBALS: u64 = 5 * 64 + 21 * 16;

/// `Globals`' fields in their order (what `globals.rs` writes; the tests
/// hold both the WGSL and the writer to it).
pub const FIELDS: [&str; 23] = [
    "vp", "shadow", "eye", "fwd", "right", "up", "sun_dir", "sun", "sky", "low", "zenith",
    "horizon", "deep", "grid", "view", "splits", "bias", "offset", "island", "terrain", "wind",
    "water", "grass",
];

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
    // How much a layer's bias grows a metre of its normal offset, a unit
    // of tilt: its soft edge's reach (in texels) over the bias.
    let laws = (
        crate::laws::SHADOW_SOFT,
        crate::laws::SHADOW_NORMAL,
        crate::laws::SHADOW_BIAS,
    );
    let slope = laws.0 / (laws.1 * laws.2);
    format!("{}{IGN}{}{}", common::COMMON, world::WORLD, decal::DECAL)
        .replace("DEPTH_TYPE", depth_type(msaa))
        .replace("NEAR_PLANE", &format!("{:.4}", crate::laws::NEAR))
        .replace("SPARK_MAX_PX", &format!("{:.1}", crate::laws::SPARK_MAX_PX))
        .replace("PUFF_MAX", &format!("{:.3}", crate::laws::PUFF_MAX))
        .replace("SPARK_NEAR", &format!("{:.3}", crate::laws::SPARK_NEAR))
        .replace("GRASS_THIN", &format!("{:.3}", crate::laws::GRASS_THIN))
        .replace("SHADOW_SOFT", &format!("{:.3}", crate::laws::SHADOW_SOFT))
        .replace("SHADOW_BAND", &format!("{:.3}", crate::laws::SHADOW_BAND))
        .replace("SHADOW_TILT", &format!("{:.3}", crate::laws::SHADOW_TILT))
        .replace("SHADOW_SLOPE", &format!("{slope:.4}"))
        .replace("SSR_ON", if ssr > 0 { "true" } else { "false" })
        .replace("SSR_STEPS", &format!("{steps}"))
        .replace("SOFT_FADE", &format!("{:.3}", crate::laws::SOFT_FADE))
        .replace("SPARK_MIN_PX", &format!("{:.3}", crate::laws::SPARK_MIN_PX))
        .replace(
            "SPARK_FAR_FLOOR",
            &format!("{:.3}", crate::laws::SPARK_FAR_FLOOR),
        )
        .replace("SEA_DEEP", &format!("{:.3}", crate::laws::SEA_DEEP))
        .replace("GRASS_ROOT", &format!("{:.3}", crate::laws::GRASS_ROOT))
        .replace("LEAF_BUMP", &format!("{:.3}", crate::laws::LEAF_BUMP))
        .replace("LEAF_GRAIN", &format!("{:.3}", crate::laws::LEAF_GRAIN))
        .replace("SSR_GROW", &format!("{grow:.5}"))
}

pub fn shadow() -> String {
    post::SHADOW.to_string()
}

pub fn post() -> String {
    format!("{}{IGN}", post::POST)
        .replace("TONE_MAP", crate::laws::TONE_MAP)
        .replace("BLOOM_KARIS", &format!("{:.3}", crate::laws::BLOOM_KARIS))
        .replace("BLOOM_WHITE", &format!("{:.3}", crate::laws::BLOOM_WHITE))
        .replace("BLOOM_KNEE", &format!("{:.3}", crate::laws::BLOOM_KNEE))
}
