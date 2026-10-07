//! The page: pick the backend, then draw every animation frame.

use crate::{shaders, Hooks, SHOT_AFTER};
use gpu::wgpu;
use pixels::Rgba;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use web_sys::WebGl2RenderingContext as GL;

const SKY: [f32; 3] = [0.043, 0.051, 0.102];
const MIN_SHORT: f64 = 352.0;
const MAX_DPR: f64 = 2.0;
/// Radians a second the triangle turns.
const TURN: f64 = 0.8;
const INK: Rgba = Rgba::rgb(244, 238, 222);
const DIM: Rgba = Rgba::rgb(150, 156, 190);

struct OnGpu {
    g: gpu::Gpu,
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    group: wgpu::BindGroup,
}

struct OnGl {
    g: kit::gl::Gl,
    prog: kit::gl::Program,
    why: String,
}

enum Back {
    Gpu(Box<OnGpu>),
    Gl(Box<OnGl>),
}

struct Page {
    back: Back,
    hooks: Hooks,
    frames: u32,
    /// When the first frame was shown (ms since the page opened).
    first: Option<f64>,
    last: f64,
    fps: f64,
}

thread_local! {
    static PAGE: RefCell<Option<Page>> = const { RefCell::new(None) };
}

impl OnGpu {
    fn new(g: gpu::Gpu) -> OnGpu {
        let d = &g.device;
        let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("triangle"),
            source: wgpu::ShaderSource::Wgsl(shaders::TRIANGLE_WGSL.into()),
        });
        let pipeline = d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("triangle"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(g.format().into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        let uniform = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("triangle"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("triangle"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        OnGpu {
            g,
            pipeline,
            uniform,
            group,
        }
    }

    fn draw(&mut self, u: [f32; 4]) -> bool {
        let Some(mut f) = self.g.frame() else {
            return false;
        };
        self.g
            .queue
            .write_buffer(&self.uniform, 0, &gpu::layer::bytes(u));
        {
            let mut pass = f.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("triangle"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &f.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: SKY[0] as f64,
                            g: SKY[1] as f64,
                            b: SKY[2] as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.g.present(f);
        true
    }
}

impl OnGl {
    fn draw(&mut self, u: [f32; 4]) -> bool {
        if self.g.lost() {
            return false;
        }
        self.g.begin(SKY);
        let gl = &self.g.gl;
        self.prog.use_on(gl);
        self.prog.f4(gl, "u", u);
        gl.draw_arrays(GL::TRIANGLES, 0, 3);
        self.g.present();
        true
    }
}

impl Back {
    fn hud(&mut self) -> (&mut pixels::Canvas, i32) {
        match self {
            Back::Gpu(b) => {
                let ui = b.g.ui();
                (&mut b.g.hud, ui)
            }
            Back::Gl(b) => {
                let ui = b.g.ui();
                (&mut b.g.hud, ui)
            }
        }
    }

    fn size(&self) -> (f64, f64) {
        match self {
            Back::Gpu(b) => b.g.css,
            Back::Gl(b) => b.g.css,
        }
    }

    fn fit(&mut self) {
        match self {
            Back::Gpu(b) => b.g.fit(MIN_SHORT, MAX_DPR),
            Back::Gl(b) => b.g.fit(MIN_SHORT, MAX_DPR),
        }
    }

    fn name(&self) -> String {
        match self {
            Back::Gpu(b) => {
                let c = &b.g.caps;
                let soft = if c.software { " (software)" } else { "" };
                format!("webgpu{soft} - {}", c.adapter)
            }
            // The reason's first words (the browser's detail is in `why`).
            Back::Gl(b) => format!("webgl2 - {}", b.why.split(':').next().unwrap_or("")),
        }
    }
}

fn frame(p: &mut Page, now: f64) {
    let dt = now - p.last;
    p.last = now;
    if dt > 0.0 && dt < 1000.0 {
        p.fps += (1000.0 / dt - p.fps) * 0.05;
    }
    let t = p.hooks.t.unwrap_or(now) / 1000.0;
    let (w, h) = p.back.size();
    let u = [(t * TURN) as f32, (w / h.max(1.0)) as f32, 0.0, 0.0];
    let name = p.back.name();
    let first = p.first;
    let (fps, perf, show) = (p.fps, p.hooks.perf, p.hooks.hud);
    let (hud, ui) = p.back.hud();
    hud.wipe();
    if show {
        let x = 8 * ui;
        let mut y = 8 * ui;
        const TITLE: &str = "secretspace engine";
        let k = pixels::fit_scale(TITLE, hud.w - 2 * x, 2 * ui);
        hud.text_shadowed(x, y, TITLE, k, INK);
        y += 10 * k;
        let lines = pixels::wrap(&name, hud.w - 2 * x, ui);
        for l in lines {
            hud.text_shadowed(x, y, &l, ui, DIM);
            y += 10 * ui;
        }
        if let Some(f) = first {
            hud.text_shadowed(x, y, &format!("first frame {f:.0} ms"), ui, DIM);
            y += 10 * ui;
        }
        if perf {
            hud.text_shadowed(x, y, &format!("{fps:.0} fps"), ui, DIM);
        }
    }
    let drawn = match &mut p.back {
        Back::Gpu(b) => b.draw(u),
        Back::Gl(b) => b.draw(u),
    };
    if !drawn {
        return;
    }
    p.frames += 1;
    if p.first.is_none() {
        p.first = Some(now);
    }
    let doc = kit::document();
    if p.hooks.shot && p.frames == SHOT_AFTER {
        doc.set_title("shot ready");
    } else if p.hooks.perf && !p.hooks.shot && p.frames.is_multiple_of(30) {
        let back = if matches!(p.back, Back::Gpu(_)) {
            "webgpu"
        } else {
            "webgl2"
        };
        doc.set_title(&format!(
            "showcase {back} {:.0}fps first {:.0}ms",
            p.fps,
            p.first.unwrap_or(0.0)
        ));
    }
}

fn say(text: &str) {
    if let Some(b) = kit::document().body() {
        b.set_inner_html(&format!(
            "<p style=\"color:#f4eede;font:16px sans-serif;padding:24px\">{text}</p>"
        ));
    }
}

async fn open(hooks: Hooks) -> Option<Back> {
    let why = if !hooks.gpu {
        "asked (?gpu=0)".to_string()
    } else if !gpu::offered() {
        "no webgpu here".to_string()
    } else {
        match gpu::Gpu::new("screen", MIN_SHORT, MAX_DPR).await {
            Ok(g) => return Some(Back::Gpu(Box::new(OnGpu::new(g)))),
            Err(e) => e,
        }
    };
    let mut g = kit::gl::Gl::new("screen")?;
    g.fit(MIN_SHORT, MAX_DPR);
    let prog = kit::gl::Program::new(&g.gl, shaders::TRIANGLE_VS, shaders::TRIANGLE_FS).ok()?;
    Some(Back::Gl(Box::new(OnGl { g, prog, why })))
}

#[wasm_bindgen(start)]
pub fn start() {
    let query = kit::window().location().search().unwrap_or_default();
    let hooks = Hooks::read(&query);
    wasm_bindgen_futures::spawn_local(async move {
        let Some(back) = open(hooks).await else {
            say("the showcase needs a browser with WebGPU or WebGL2.");
            return;
        };
        PAGE.with(|p| {
            *p.borrow_mut() = Some(Page {
                back,
                hooks,
                frames: 0,
                first: None,
                last: kit::now(),
                fps: 60.0,
            })
        });
        kit::frames(|now| {
            PAGE.with(|p| {
                if let Some(p) = p.borrow_mut().as_mut() {
                    frame(p, now);
                }
            })
        });
        kit::on(&kit::window(), "resize", |_| {
            PAGE.with(|p| {
                if let Some(p) = p.borrow_mut().as_mut() {
                    p.back.fit();
                }
            })
        });
    });
}
