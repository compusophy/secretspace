//! Battlestation's card: the real desk, drawn by the engine off screen
//! and read back into the card (as Wandfall's match is), as you find it
//! when you sit down: the same hands on the same keys and mouse, and on
//! the monitor the same compusophyOS, mounted here too (its own storage),
//! starting to its welcome. It rests while a game is open. Should it not
//! mount, the desk's own terminal, as the game shows then.

use battlelook::desk::{seated, Desk};
use battlelook::monitor::Screen;
use battlestation::hands::{mouse_for, Hands};
use battlestation::keys;
use battlestation::laws::SCREEN_PX;
use battlestation::term::Term;
use pixels::{text_width, Canvas, Rect, Rgba};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(raw_module = "../battlestation/os/os.js")]
extern "C" {
    #[wasm_bindgen(js_name = default)]
    fn os_init() -> js_sys::Promise;

    type Cartridge;
    #[wasm_bindgen(constructor, catch)]
    fn new(w: f32, h: f32, dpr: f32, base: &str, ns: &str) -> Result<Cartridge, JsValue>;
    #[wasm_bindgen(method)]
    fn frames(this: &Cartridge) -> u32;
    #[wasm_bindgen(method)]
    fn width(this: &Cartridge) -> u32;
    #[wasm_bindgen(method)]
    fn height(this: &Cartridge) -> u32;
    #[wasm_bindgen(method)]
    fn pixels(this: &Cartridge, out: &mut [u8]) -> bool;
    #[wasm_bindgen(method)]
    fn visible(this: &Cartridge, on: bool);
}

/// The OS's desktop (CSS pixels), its files from this page, its storage's
/// namespace: as the game's (`battlestation-web/src/page/os.rs`), its
/// storage apart.
const DESKTOP: (f32, f32) = (960.0, 540.0);
const BASE: &str = "./battlestation/os/";
const NS: &str = "battlestation.card.";
/// Waits before each try to mount (ms), as the game's.
const TRIES: [i32; 4] = [500, 1000, 2500, 5000];

/// The mounted OS: its frames as last copied, and its pixels then.
pub struct Os {
    c: Cartridge,
    seen: u32,
    buf: Vec<u8>,
    resting: bool,
}

async fn sleep(ms: i32) {
    let p = js_sys::Promise::new(&mut |done, _| {
        let _ = kit::window().set_timeout_with_callback_and_timeout_and_arguments_0(&done, ms);
    });
    let _ = wasm_bindgen_futures::JsFuture::from(p).await;
}

/// Start the OS for the card (one mount a page; a failed one leaves
/// nothing behind).
pub async fn mount() -> Option<Os> {
    wasm_bindgen_futures::JsFuture::from(os_init()).await.ok()?;
    for wait in TRIES {
        sleep(wait).await;
        if let Ok(c) = Cartridge::new(DESKTOP.0, DESKTOP.1, 1.0, BASE, NS) {
            return Some(Os {
                c,
                seen: u32::MAX,
                buf: Vec::new(),
                resting: false,
            });
        }
    }
    None
}

const WAIT: Rgba = Rgba::rgb(14, 12, 28);
const SAY: Rgba = Rgba(244, 241, 255, 140);

struct Three {
    off: gpu::Offscreen,
    r: render::Renderer,
    desk: Desk,
    pic: Canvas,
    has: bool,
}

pub struct DeskWatch {
    os: Option<Os>,
    /// Whether the OS is still starting (none yet, none failed).
    booting: bool,
    term: Term,
    hands: Hands,
    screen: Screen,
    heat: Vec<f32>,
    last: Option<f64>,
    three: Option<Three>,
}

impl Default for DeskWatch {
    fn default() -> DeskWatch {
        DeskWatch::new()
    }
}

impl DeskWatch {
    pub fn new() -> DeskWatch {
        let screen = Screen::new();
        let mut hands = Hands::new();
        let (x, z) = mouse_for(screen.cursor, SCREEN_PX);
        hands.put_mouse(x, z);
        DeskWatch {
            os: None,
            booting: true,
            term: Term::new(),
            hands,
            screen,
            heat: vec![0.0; keys::layout().len()],
            last: None,
            three: None,
        }
    }

    /// A device to draw the desk with (WebGPU, off screen).
    pub fn give(&mut self, off: gpu::Offscreen) {
        let q = render::Quality::pick("q=medium", off.caps.software, false);
        let q = render::Quality {
            grass_spacing: 0.0,
            grass_reach: 0.0,
            ..q
        };
        let mut r = render::Renderer::new(&off.device, &off.queue, gpu::Offscreen::FORMAT, q);
        let desk = Desk::new(&mut r, &off.device, gpu::Offscreen::FORMAT, 2);
        self.three = Some(Three {
            off,
            r,
            desk,
            pic: Canvas::new(1, 1),
            has: false,
        });
    }

    /// The OS mounted (or not: the desk's own terminal then).
    pub fn mounted(&mut self, os: Option<Os>) {
        self.os = os;
        self.booting = false;
    }

    /// While a game is open the card is out of view: the OS rests.
    pub fn rest(&mut self, rest: bool) {
        if let Some(os) = self.os.as_mut().filter(|o| o.resting != rest) {
            os.c.visible(!rest);
            os.resting = rest;
        }
    }

    /// The hands on by `dt` seconds, the keys' light fading.
    fn step(&mut self, dt: f32) {
        self.hands.step(dt);
        for h in &mut self.heat {
            *h *= (-dt / 0.35).exp();
        }
    }

    /// Draw the card's picture into `b` (text scale `u`).
    pub fn draw(&mut self, c: &mut Canvas, b: Rect, u: i32, now: f64) {
        let dt = self
            .last
            .map_or(0.0, |l| ((now - l) / 1000.0) as f32)
            .clamp(0.0, 0.1);
        self.last = Some(now);
        let t = (now / 1000.0) as f32;
        let uf = u as f32;
        if self.three.is_some() {
            self.step_and_draw(dt, t, b);
            if let Some(th) = self.three.as_ref().filter(|th| th.has) {
                c.blit(&th.pic, b.x as i32, b.y as i32, 6.0 * uf);
                return;
            }
        }
        c.round_rect(b, 6.0 * uf, WAIT);
        let say = if gpu::offered() {
            "pulling up a chair..."
        } else {
            "the desk needs WebGPU"
        };
        let w = text_width(say, u);
        c.text(
            (b.x + (b.w - w as f32) / 2.0) as i32,
            (b.y + b.h / 2.0 - 4.0 * uf) as i32,
            say,
            u,
            SAY,
        );
    }

    fn step_and_draw(&mut self, dt: f32, t: f32, b: Rect) {
        self.step(dt);
        if self.booting {
            self.screen.boot("compusophyOS is starting...", t);
        } else if self.os.is_none() {
            self.screen.draw(&self.term, &clock(), t, dt);
        }
        let Some(th) = self.three.as_mut() else {
            return;
        };
        // The OS's newest frame onto the glass, if one has come.
        let mut arrow = None;
        if let Some(os) = self.os.as_mut() {
            let (w, h) = (os.c.width(), os.c.height());
            if os.c.frames() != os.seen && w > 0 && h > 0 && w <= 8192 && h <= 8192 {
                os.buf.resize((w * h * 4) as usize, 0);
                if os.c.pixels(&mut os.buf) {
                    th.desk
                        .glass
                        .show_os(&th.off.device, &th.off.queue, (w, h), &os.buf);
                    os.seen = os.c.frames();
                }
            }
            let k = w as f32 / SCREEN_PX.0 as f32;
            arrow = Some((self.screen.cursor.0 * k, self.screen.cursor.1 * k));
        }
        let size = (b.w.max(1.0) as u32, b.h.max(1.0) as u32);
        if let Some((view, mut enc)) = th.off.begin(size) {
            let cam = seated(size.0 as f32 / size.1 as f32);
            th.desk.draw(
                &mut th.r,
                &th.off.queue,
                (&mut enc, &view, size),
                cam,
                (&self.hands, &self.heat),
                (&self.screen, arrow),
                t,
            );
            th.off.end(enc);
        }
        if th.off.read(&mut th.pic) {
            for a in th.pic.data.iter_mut().skip(3).step_by(4) {
                *a = 255;
            }
            th.has = true;
        }
    }
}

/// The time, for the monitor's bar.
fn clock() -> String {
    let d = js_sys::Date::new_0();
    format!("{:02}:{:02}", d.get_hours(), d.get_minutes())
}
