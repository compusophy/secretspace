//! The engine's GPU layer: a WebGPU device on a canvas (through wgpu), what
//! it can do (`Caps`), what became of it (`Health`), a frame to draw into,
//! and the pixel layer (`hud`, a `pixels::Canvas` sized as `kit::gl` sizes
//! it) composited sharp over the picture before it is shown. Generic: no
//! game knowledge. The adapter is asked for before the canvas is touched,
//! so a browser without WebGPU leaves the canvas free for the WebGL2
//! fallback. A page run inside another (`kit::host`) draws off screen
//! instead (`Gpu::hosted`), each picture read back for its host (`read`).
//! Off the web only the shaders and the tables build (the tests check
//! every WGSL string).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

pub mod layer;
mod offscreen;
mod readback;

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

/// What became of a device: whether it was lost (the browser's GPU
/// process restarted, or a tab put away had it taken back: every call on
/// it does nothing from then on, so a page that does not ask freezes), and
/// how many errors no one caught (the first is logged).
#[derive(Clone, Debug, Default)]
pub struct Health {
    lost: Arc<AtomicBool>,
    errors: Arc<AtomicU32>,
}

impl Health {
    /// Watch `device` from now on.
    pub fn watch(device: &wgpu::Device) -> Health {
        let h = Health::default();
        let lost = h.lost.clone();
        device.set_device_lost_callback(move |reason, why| {
            // Destroyed is the page letting it go, not a loss.
            if reason != wgpu::DeviceLostReason::Destroyed {
                say(&format!("WebGPU device lost: {why}"));
                lost.store(true, Ordering::Relaxed);
            }
        });
        let errors = h.errors.clone();
        device.on_uncaptured_error(Arc::new(move |e| {
            if errors.fetch_add(1, Ordering::Relaxed) == 0 {
                say(&format!("WebGPU: {e}"));
            }
        }));
        h
    }

    /// Whether the device is gone (make a new one, or reload the page).
    pub fn lost(&self) -> bool {
        self.lost.load(Ordering::Relaxed)
    }

    /// How many errors no one caught.
    pub fn errors(&self) -> u32 {
        self.errors.load(Ordering::Relaxed)
    }
}

/// Into the browser's console (or, off the web, standard error).
fn say(s: &str) {
    #[cfg(target_arch = "wasm32")]
    web_sys::console::error_1(&s.into());
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("{s}");
}

/// Optional features asked for whenever the adapter offers them (only
/// what the engine uses): drawing into the small float format (bloom's
/// chain at half the bytes).
pub fn wanted() -> wgpu::Features {
    wgpu::Features::RG11B10UFLOAT_RENDERABLE
}
