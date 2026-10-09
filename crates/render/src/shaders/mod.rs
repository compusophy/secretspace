//! The engine's WGSL, as Rust strings (rule 1): the scene's shaders (the
//! shared part, then the world, sky, sparks and grass), the sun's shadow
//! map, the ambient occlusion, the sun's shafts, and the post-processing.
//! Each module is validated by naga in a test, so a shader error is a
//! failed test, not a black page.

pub mod ao;
pub mod common;
pub mod post;
pub mod shafts;
pub mod world;

/// Bytes of `Globals`: four mat4s and eighteen vec4s.
pub const GLOBALS: u64 = 4 * 64 + 18 * 16;

/// The scene's module: the shared part, then its shaders, the engine's
/// numbers put in; for a depth that is many-sampled (`msaa`) or not.
pub fn scene(msaa: bool) -> String {
    let depth = if msaa {
        "texture_depth_multisampled_2d"
    } else {
        "texture_depth_2d"
    };
    format!("{}{}", common::COMMON, world::WORLD)
        .replace("DEPTH_TYPE", depth)
        .replace("NEAR_PLANE", &format!("{:.4}", crate::laws::NEAR))
        .replace("SPARK_MAX_PX", &format!("{:.1}", crate::laws::SPARK_MAX_PX))
        .replace("PUFF_MAX", &format!("{:.3}", crate::laws::PUFF_MAX))
        .replace("SPARK_NEAR", &format!("{:.3}", crate::laws::SPARK_NEAR))
}

pub fn shadow() -> String {
    post::SHADOW.to_string()
}

pub fn post() -> String {
    post::POST.to_string()
}
