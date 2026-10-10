//! Where a frame goes: the canvas, through WebGPU, as ever; or (with
//! `?capture=1`) drawn off any screen, read back, the pixel layer laid
//! over it, and put on the canvas as an image: what a headless browser
//! can photograph (it cannot see a WebGPU canvas), a frame or two late.

use render::wgpu;
use wasm_bindgen::{Clamped, JsCast};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData};

pub(super) struct Capture {
    off: gpu::Offscreen,
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    pic: pixels::Canvas,
    hud: pixels::Canvas,
    css: (f64, f64),
    scale: f64,
}

pub(super) enum Out {
    Gpu(Box<gpu::Gpu>),
    Capture(Box<Capture>),
}

/// A frame being drawn: its target, and the encoder recording into it.
pub(super) enum Target {
    Gpu(gpu::Frame),
    Off(wgpu::TextureView, wgpu::CommandEncoder),
}

impl Target {
    pub(super) fn parts(&mut self) -> (&wgpu::TextureView, &mut wgpu::CommandEncoder) {
        match self {
            Target::Gpu(f) => (&f.view, &mut f.encoder),
            Target::Off(v, e) => (v, e),
        }
    }
}

impl Out {
    pub(super) async fn open(
        id: &str,
        capture: bool,
        min_short: f64,
        max_dpr: f64,
    ) -> Result<Out, String> {
        if !capture {
            return gpu::Gpu::new(id, min_short, max_dpr)
                .await
                .map(|g| Out::Gpu(Box::new(g)));
        }
        let off = gpu::Offscreen::new().await?;
        let canvas: HtmlCanvasElement = kit::document()
            .get_element_by_id(id)
            .and_then(|e| e.dyn_into().ok())
            .ok_or("no canvas")?;
        let ctx = canvas
            .get_context("2d")
            .ok()
            .flatten()
            .and_then(|c| c.dyn_into::<CanvasRenderingContext2d>().ok())
            .ok_or("no 2d context")?;
        let mut c = Capture {
            off,
            canvas,
            ctx,
            pic: pixels::Canvas::new(1, 1),
            hud: pixels::Canvas::new(1, 1),
            css: (1.0, 1.0),
            scale: 1.0,
        };
        c.fit(min_short);
        Ok(Out::Capture(Box::new(c)))
    }

    pub(super) fn device(&self) -> &wgpu::Device {
        match self {
            Out::Gpu(g) => &g.device,
            Out::Capture(c) => &c.off.device,
        }
    }

    pub(super) fn queue(&self) -> &wgpu::Queue {
        match self {
            Out::Gpu(g) => &g.queue,
            Out::Capture(c) => &c.off.queue,
        }
    }

    pub(super) fn format(&self) -> wgpu::TextureFormat {
        match self {
            Out::Gpu(g) => g.format(),
            Out::Capture(_) => gpu::Offscreen::FORMAT,
        }
    }

    pub(super) fn caps(&self) -> &gpu::Caps {
        match self {
            Out::Gpu(g) => &g.caps,
            Out::Capture(c) => &c.off.caps,
        }
    }

    /// The page's canvas (none when drawn for a host).
    pub(super) fn canvas(&self) -> Option<&HtmlCanvasElement> {
        match self {
            Out::Gpu(g) => g.canvas(),
            Out::Capture(c) => Some(&c.canvas),
        }
    }

    /// The drawing's size in device pixels.
    pub(super) fn size(&self) -> (u32, u32) {
        match self {
            Out::Gpu(g) => g.size,
            Out::Capture(c) => (c.css.0 as u32, c.css.1 as u32),
        }
    }

    pub(super) fn css(&self) -> (f64, f64) {
        match self {
            Out::Gpu(g) => g.css,
            Out::Capture(c) => c.css,
        }
    }

    /// CSS pixels per layer pixel.
    pub(super) fn scale(&self) -> f64 {
        match self {
            Out::Gpu(g) => g.scale,
            Out::Capture(c) => c.scale,
        }
    }

    pub(super) fn ui(&self) -> i32 {
        if self.scale() >= 2.0 {
            1
        } else {
            2
        }
    }

    pub(super) fn to_px(&self, x: f64, y: f64) -> (f32, f32) {
        let s = self.scale();
        ((x / s) as f32, (y / s) as f32)
    }

    pub(super) fn hud(&mut self) -> &mut pixels::Canvas {
        match self {
            Out::Gpu(g) => &mut g.hud,
            Out::Capture(c) => &mut c.hud,
        }
    }

    pub(super) fn fit(&mut self, min_short: f64, max_dpr: f64) {
        match self {
            Out::Gpu(g) => g.fit(min_short, max_dpr),
            Out::Capture(c) => c.fit(min_short),
        }
    }

    pub(super) fn begin(&mut self) -> Option<Target> {
        match self {
            Out::Gpu(g) => g.frame().map(Target::Gpu),
            Out::Capture(c) => {
                let size = (c.css.0 as u32, c.css.1 as u32);
                c.off.begin(size).map(|(v, e)| Target::Off(v, e))
            }
        }
    }

    /// No frame to draw into now (every read-back buffer busy): show the
    /// newest picture that has come back, which frees its buffer.
    pub(super) fn idle(&mut self) {
        if let Out::Capture(c) = self {
            if c.off.read(&mut c.pic) {
                c.show();
            }
        }
    }

    pub(super) fn finish(&mut self, t: Target) {
        match (self, t) {
            (Out::Gpu(g), Target::Gpu(f)) => g.present(f),
            (Out::Capture(c), Target::Off(_, e)) => {
                c.off.end(e);
                if c.off.read(&mut c.pic) {
                    c.show();
                }
            }
            _ => {}
        }
    }
}

impl Capture {
    fn fit(&mut self, min_short: f64) {
        let f = kit::gl::measure(min_short, 1.0);
        self.css = (f.css.0.round().max(1.0), f.css.1.round().max(1.0));
        self.scale = f.scale;
        self.hud.resize(f.hud.0, f.hud.1);
        self.canvas.set_width(self.css.0 as u32);
        self.canvas.set_height(self.css.1 as u32);
    }

    /// The picture read back, the layer over it (each layer pixel a
    /// square of `scale`), onto the canvas.
    fn show(&mut self) {
        let (w, h) = (self.pic.w, self.pic.h);
        let k = self.scale.max(1.0) as i32;
        for y in 0..h {
            let hy = y / k;
            if hy >= self.hud.h {
                break;
            }
            for x in 0..w {
                let hx = x / k;
                if hx >= self.hud.w {
                    break;
                }
                let s = ((hy * self.hud.w + hx) * 4) as usize;
                let a = self.hud.data[s + 3] as u32;
                if a == 0 {
                    continue;
                }
                let d = ((y * w + x) * 4) as usize;
                for c in 0..3 {
                    let over =
                        self.hud.data[s + c] as u32 + self.pic.data[d + c] as u32 * (255 - a) / 255;
                    self.pic.data[d + c] = over.min(255) as u8;
                }
            }
        }
        for px in self.pic.data.chunks_exact_mut(4) {
            px[3] = 255;
        }
        if let Ok(img) =
            ImageData::new_with_u8_clamped_array_and_sh(Clamped(&self.pic.data), w as u32, h as u32)
        {
            let _ = self.ctx.put_image_data(&img, 0.0, 0.0);
        }
    }
}
