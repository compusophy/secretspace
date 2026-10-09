//! The engine's GPU layer: a WebGPU device on a canvas (through wgpu), what
//! it can do (`Caps`), a frame to draw into, and the pixel layer (`hud`, a
//! `pixels::Canvas` sized as `kit::gl` sizes it) composited sharp over the
//! picture before it is shown. Generic: no game knowledge. The adapter is
//! asked for before the canvas is touched, so a browser without WebGPU
//! leaves the canvas free for the WebGL2 fallback. Off the web only the
//! shaders and the tables build (the tests check every WGSL string).

pub mod layer;
mod offscreen;

pub use offscreen::Offscreen;

pub use wgpu;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::{offered, Frame, Gpu};

/// Off the web, no browser offers WebGPU.
#[cfg(not(target_arch = "wasm32"))]
pub fn offered() -> bool {
    false
}

/// What the device can do, as the engine needs to know it.
#[derive(Clone, Debug)]
pub struct Caps {
    /// The adapter as the browser describes it (often only "webgpu": the
    /// browser hides the hardware).
    pub adapter: String,
    /// A software adapter (no GPU): draw the least.
    pub software: bool,
    pub features: wgpu::Features,
    pub limits: wgpu::Limits,
}

impl Caps {
    /// Whether an optional feature came with the device.
    pub fn has(&self, f: wgpu::Features) -> bool {
        self.features.contains(f)
    }
}

/// Optional features asked for whenever the adapter offers them.
pub fn wanted() -> wgpu::Features {
    wgpu::Features::TIMESTAMP_QUERY
        | wgpu::Features::FLOAT32_FILTERABLE
        | wgpu::Features::RG11B10UFLOAT_RENDERABLE
        | wgpu::Features::SHADER_F16
        | wgpu::Features::INDIRECT_FIRST_INSTANCE
}
