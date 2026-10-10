//! The device on a browser canvas, or (hosted: a page run inside another,
//! `kit::host`) on no canvas at all: drawn into a texture and read back as
//! pixels for the host to show.

use crate::readback::{target, Readback};
use crate::{layer, wanted, Caps, Health};
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;

/// A frame being drawn: a view of what it draws into, and an encoder to
/// record into. Hand it back to `Gpu::present`.
pub struct Frame {
    pub view: wgpu::TextureView,
    pub encoder: wgpu::CommandEncoder,
    /// The canvas's texture (none off screen).
    texture: Option<wgpu::SurfaceTexture>,
}

/// Where the picture goes: the page's canvas, or a texture read back.
enum Target {
    Page {
        canvas: HtmlCanvasElement,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
    },
    Off {
        tex: Option<(wgpu::Texture, wgpu::TextureView, (u32, u32))>,
        ring: Readback,
    },
}

/// The colour format off screen: plain RGBA, as a host takes it.
const OFF: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The canvas as a WebGPU screen, with its pixel layer.
pub struct Gpu {
    target: Target,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub caps: Caps,
    /// Whether the device is lost (no frame is given then).
    pub health: Health,
    /// The pixel layer, wiped and drawn each frame, shown over the picture.
    pub hud: pixels::Canvas,
    /// CSS pixels per layer pixel; device pixels per CSS pixel drawn; the
    /// window in CSS pixels; the drawing buffer in device pixels.
    pub scale: f64,
    pub dpr: f64,
    pub css: (f64, f64),
    pub size: (u32, u32),
    layer: layer::Layer,
}

/// The device a page draws with: the fastest adapter, all it can do.
async fn device() -> Result<
    (
        wgpu::Instance,
        wgpu::Adapter,
        wgpu::Device,
        wgpu::Queue,
        Caps,
    ),
    String,
> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::BROWSER_WEBGPU,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        })
        .await
        .map_err(|e| format!("no adapter: {e}"))?;
    let features = adapter.features() & wanted();
    let limits = adapter.limits();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("secretspace"),
            required_features: features,
            required_limits: limits.clone(),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("no device: {e}"))?;
    let info = adapter.get_info();
    let caps = Caps {
        adapter: if info.name.is_empty() {
            "webgpu".to_string()
        } else {
            info.name.clone()
        },
        software: info.device_type == wgpu::DeviceType::Cpu,
        features: device.features(),
        limits,
    };
    Ok((instance, adapter, device, queue, caps))
}

impl Gpu {
    /// WebGPU on the canvas with this id, or why not (the canvas is then
    /// untouched).
    pub async fn new(id: &str, min_short: f64, max_dpr: f64) -> Result<Gpu, String> {
        let canvas: HtmlCanvasElement = kit::document()
            .get_element_by_id(id)
            .and_then(|e| e.dyn_into().ok())
            .ok_or_else(|| format!("no canvas #{id}"))?;
        let (instance, adapter, device, queue, caps) = device().await?;
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|e| format!("no surface: {e}"))?;
        let mut config = surface
            .get_default_config(&adapter, 1, 1)
            .ok_or("the surface and the adapter do not agree")?;
        // Plain (not sRGB) colour, as the WebGL2 path writes it.
        config.format = config.format.remove_srgb_suffix();
        config.alpha_mode = wgpu::CompositeAlphaMode::Opaque;
        let layer = layer::Layer::new(&device, config.format);
        let mut g = Gpu {
            target: Target::Page {
                canvas,
                surface,
                config,
            },
            health: Health::watch(&device),
            device,
            queue,
            caps,
            hud: pixels::Canvas::new(1, 1),
            scale: 1.0,
            dpr: 1.0,
            css: (1.0, 1.0),
            size: (1, 1),
            layer,
        };
        g.fit(min_short, max_dpr);
        Ok(g)
    }

    /// The same device as `new`, drawn off any screen for a host (sized as
    /// `kit::host` says): no canvas, no surface; each picture read back
    /// with `read`, plain RGBA.
    pub async fn hosted(min_short: f64, max_dpr: f64) -> Result<Gpu, String> {
        let (_, _, device, queue, caps) = device().await?;
        let layer = layer::Layer::new(&device, OFF);
        let mut g = Gpu {
            target: Target::Off {
                tex: None,
                ring: Readback::new(),
            },
            health: Health::watch(&device),
            device,
            queue,
            caps,
            hud: pixels::Canvas::new(1, 1),
            scale: 1.0,
            dpr: 1.0,
            css: (1.0, 1.0),
            size: (1, 1),
            layer,
        };
        g.fit(min_short, max_dpr);
        Ok(g)
    }

    /// Match the window (as `kit::gl::Gl::fit` does); hosted, the host's
    /// surface.
    pub fn fit(&mut self, min_short: f64, max_dpr: f64) {
        let f = kit::gl::measure(min_short, max_dpr);
        (self.scale, self.dpr, self.css, self.size) = (f.scale, f.dpr, f.css, f.size);
        match &mut self.target {
            Target::Page {
                canvas,
                surface,
                config,
            } => {
                canvas.set_width(f.size.0);
                canvas.set_height(f.size.1);
                kit::place(canvas, f.css.0, f.css.1);
                self.hud.resize(f.hud.0, f.hud.1);
                config.width = f.size.0;
                config.height = f.size.1;
                surface.configure(&self.device, config);
            }
            Target::Off { .. } => {
                // No bigger than the device can draw.
                let most = self.caps.limits.max_texture_dimension_2d.max(1);
                self.size = (f.size.0.min(most), f.size.1.min(most));
                self.hud.resize(f.hud.0, f.hud.1);
            }
        }
    }

    /// The surface's colour format (what a pass drawing to it must use).
    pub fn format(&self) -> wgpu::TextureFormat {
        match &self.target {
            Target::Page { config, .. } => config.format,
            Target::Off { .. } => OFF,
        }
    }

    /// The text scale that reads the same size on any screen.
    pub fn ui(&self) -> i32 {
        if self.scale >= 2.0 {
            1
        } else {
            2
        }
    }

    /// A CSS point as a layer point.
    pub fn to_px(&self, x: f64, y: f64) -> (f32, f32) {
        ((x / self.scale) as f32, (y / self.scale) as f32)
    }

    /// The page's canvas (none when hosted).
    pub fn canvas(&self) -> Option<&HtmlCanvasElement> {
        match &self.target {
            Target::Page { canvas, .. } => Some(canvas),
            Target::Off { .. } => None,
        }
    }

    /// Start a frame, or None if the surface cannot give one now (it is
    /// configured again for the next) or the device is lost (`health`).
    /// Hosted, also None while the host is not looking (`kit::host::drawing`)
    /// or every read-back buffer is busy.
    pub fn frame(&mut self) -> Option<Frame> {
        if self.health.lost() {
            return None;
        }
        let (view, texture) = match &mut self.target {
            Target::Page {
                surface, config, ..
            } => {
                let texture = match surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                        surface.configure(&self.device, config);
                        return None;
                    }
                    _ => return None,
                };
                let view = texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                (view, Some(texture))
            }
            Target::Off { tex, ring } => {
                if !kit::host::drawing(kit::now()) {
                    return None;
                }
                let size = self.size;
                if tex.as_ref().map(|t| t.2) != Some(size) {
                    *tex = Some(target(&self.device, size, OFF));
                }
                if !ring.take(&self.device, size) {
                    return None;
                }
                (tex.as_ref()?.1.clone(), None)
            }
        };
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        Some(Frame {
            view,
            encoder,
            texture,
        })
    }

    /// The pixel layer over everything drawn, then the frame shown (hosted:
    /// copied out, to be `read`).
    pub fn present(&mut self, mut f: Frame) {
        let k = (self.scale * self.dpr) as f32;
        self.layer.draw(
            &self.device,
            &self.queue,
            &mut f.encoder,
            &f.view,
            &self.hud,
            k,
        );
        match &mut self.target {
            Target::Page { .. } => {
                self.queue.submit([f.encoder.finish()]);
                if let Some(t) = f.texture {
                    self.queue.present(t);
                }
            }
            Target::Off { tex, ring } => {
                if let Some((t, _, size)) = tex.as_ref() {
                    ring.end(&self.queue, f.encoder, t, *size);
                }
            }
        }
    }

    /// Hosted: the newest picture read back, into `into` (sized to it; RGBA,
    /// alpha 255, rows packed). False if none has come since the last, or
    /// on a page's own canvas.
    pub fn read(&mut self, into: &mut pixels::Canvas) -> bool {
        match &mut self.target {
            Target::Off { ring, .. } => ring.read(into, true),
            Target::Page { .. } => false,
        }
    }

    /// Let the device go now (the web's `Drop` does not).
    pub fn destroy(&self) {
        self.device.destroy();
    }
}

/// Whether this browser offers WebGPU at all (an adapter may still fail).
pub fn offered() -> bool {
    reflect_has(&kit::window().navigator(), "gpu")
}

#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = Reflect, js_name = has)]
    fn reflect_has(target: &wasm_bindgen::JsValue, key: &str) -> bool;
}
