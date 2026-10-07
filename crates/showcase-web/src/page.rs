//! The page: pick the backend, then draw every animation frame.

use crate::scene::{self, Scene};
use crate::{shaders, Hooks, SHOT_AFTER};
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
    r: render::Renderer,
    scene: Scene,
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
        let mut r = render::Renderer::new(&g.device, &g.queue, g.format());
        let scene = Scene::new(&mut r);
        OnGpu { g, r, scene }
    }

    fn draw(&mut self, t: f32, cam: Option<[f32; 5]>) -> bool {
        let Some(mut f) = self.g.frame() else {
            return false;
        };
        let (w, h) = self.g.css;
        let cam = camera(t, cam, (w / h.max(1.0)) as f32);
        let (items, lights, sparks) = self.scene.frame(t, &cam);
        let frame = render::Frame {
            cam,
            look: Scene::look(),
            time: t,
            items: &items,
            lights: &lights,
            sparks: &sparks,
            view_fov: 0.9,
        };
        self.r.draw(&mut f.encoder, &f.view, self.g.size, &frame);
        self.g.present(f);
        true
    }
}

/// The camera: `?cam=x,z,h,yaw,pitch` (metres, degrees) holds it there;
/// otherwise it circles the shrine.
fn camera(t: f32, at: Option<[f32; 5]>, aspect: f32) -> render::Camera {
    let (eye, yaw, pitch) = match at {
        Some([x, z, h, yaw, pitch]) => ([x, h, z], yaw.to_radians(), pitch.to_radians()),
        None => {
            let a = t * 0.06 + 0.8;
            let (x, z) = (a.cos() * 30.0, a.sin() * 30.0);
            let eye = [x, scene::height(x, z) + 3.2, z];
            let to = [-x, 4.0 - eye[1], -z];
            let flat = (to[0] * to[0] + to[2] * to[2]).sqrt();
            (eye, to[2].atan2(to[0]), to[1].atan2(flat) * 0.6)
        }
    };
    render::Camera {
        eye,
        yaw,
        pitch,
        fov: 1.15,
        aspect,
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

    /// What the engine drew last frame.
    fn stats(&self) -> String {
        match self {
            Back::Gpu(b) => {
                let s = b.r.stats;
                format!(
                    "draws {} inst {} tris {}k lights {}",
                    s.draws,
                    s.instances,
                    s.triangles / 1000,
                    s.lights
                )
            }
            Back::Gl(_) => String::new(),
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
    let stats = p.back.stats();
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
            hud.text_shadowed(x, y, &format!("{fps:.0} fps {stats}"), ui, DIM);
        }
    }
    let drawn = match &mut p.back {
        Back::Gpu(b) => b.draw(t as f32, p.hooks.cam),
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
            "showcase {back} {:.0}fps first {:.0}ms {}",
            p.fps,
            p.first.unwrap_or(0.0),
            p.back.stats()
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
