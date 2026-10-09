//! The engine's WGSL, as Rust strings (rule 1): the scene's shaders (the
//! shared part, then the world, sky, sparks and grass), the sun's shadow
//! map, the ambient occlusion, and the post-processing. Each module is validated by naga in a
//! test, so a shader error is a failed test, not a black page.

pub mod ao;
pub mod common;
pub mod post;
pub mod world;

/// Bytes of `Globals`: four mat4s and eighteen vec4s.
pub const GLOBALS: u64 = 4 * 64 + 18 * 16;

/// The scene's module: the shared part, then its shaders, the engine's
/// numbers put in.
pub fn scene() -> String {
    format!("{}{}", common::COMMON, world::WORLD)
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
