//! The device on a browser canvas.

use crate::{layer, wanted, Caps, Health};
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;

/// A frame being drawn: the surface's texture, a view of it, and an
/// encoder to record into. Hand it back to `Gpu::present`.
pub struct Frame {
    pub view: wgpu::TextureView,
    pub encoder: wgpu::CommandEncoder,
    texture: wgpu::SurfaceTexture,
}

/// The canvas as a WebGPU screen, with its pixel layer.
pub struct Gpu {
    canvas: HtmlCanvasElement,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
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

impl Gpu {
    /// WebGPU on the canvas with this id, or why not (the canvas is then
    /// untouched).
    pub async fn new(id: &str, min_short: f64, max_dpr: f64) -> Result<Gpu, String> {
        let canvas: HtmlCanvasElement = kit::document()
            .get_element_by_id(id)
            .and_then(|e| e.dyn_into().ok())
            .ok_or_else(|| format!("no canvas #{id}"))?;
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
            canvas,
            health: Health::watch(&device),
            device,
            queue,
            surface,
            config,
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

    /// Match the window (as `kit::gl::Gl::fit` does).
    pub fn fit(&mut self, min_short: f64, max_dpr: f64) {
        let f = kit::gl::measure(min_short, max_dpr);
        (self.scale, self.dpr, self.css, self.size) = (f.scale, f.dpr, f.css, f.size);
        self.canvas.set_width(f.size.0);
        self.canvas.set_height(f.size.1);
        kit::place(&self.canvas, f.css.0, f.css.1);
        self.hud.resize(f.hud.0, f.hud.1);
        self.config.width = f.size.0;
        self.config.height = f.size.1;
        self.surface.configure(&self.device, &self.config);
    }

    /// The surface's colour format (what a pass drawing to it must use).
    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
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

    pub fn canvas(&self) -> &HtmlCanvasElement {
        &self.canvas
    }

    /// Start a frame, or None if the surface cannot give one now (it is
    /// configured again for the next) or the device is lost (`health`).
    pub fn frame(&mut self) -> Option<Frame> {
        if self.health.lost() {
            return None;
        }
        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return None;
            }
            _ => return None,
        };
        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
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

    /// The pixel layer over everything drawn, then the frame shown.
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
        self.queue.submit([f.encoder.finish()]);
        self.queue.present(f.texture);
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
