//! Battlestation's card: the real desk, drawn by the engine off screen
//! and read back into the card (as Wandfall's match is), doing what the
//! game does before anyone sits down: the same demo typing on the same
//! keys with the same hands, its terminal on the monitor, from the chair.

use battlelook::desk::{seated, Desk};
use battlelook::monitor::Screen;
use battlestation::demo::{Demo, Ev};
use battlestation::hands::{mouse_for, Hands};
use battlestation::keys;
use battlestation::laws::{CURSOR_SPEED, SCREEN_PX};
use battlestation::term::{Facts, Term};
use pixels::{text_width, Canvas, Rect, Rgba};

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
    demo: Demo,
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
        DeskWatch {
            demo: Demo::new(0xde5c),
            term: Term::new(),
            hands: Hands::new(),
            screen: Screen::new(),
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

    /// The demo on by `dt` seconds: the keys, the hands, the terminal.
    fn step(&mut self, now: f64, dt: f32) {
        let facts = Facts {
            gpu: "",
            when: "",
            up: now / 1000.0,
            screen: SCREEN_PX,
        };
        for ev in self.demo.step(dt) {
            match ev {
                Ev::Key { code, key, down } => {
                    self.hands.key(code, down);
                    if down {
                        if let Some(i) = keys::layout().iter().position(|k| k.code == code) {
                            self.heat[i] = 1.0;
                        }
                        self.term.key(&key, false, &facts);
                    }
                }
                Ev::Move { dx, dy } => {
                    self.screen.nudge(dx * CURSOR_SPEED, dy * CURSOR_SPEED);
                    self.hands.stir(dx.hypot(dy));
                    let (x, z) = mouse_for(self.screen.cursor, SCREEN_PX);
                    self.hands.put_mouse(x, z);
                }
                Ev::Button { b, down } => self.hands.button(b, down),
            }
        }
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
            self.step_and_draw(now, dt, t, b);
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

    fn step_and_draw(&mut self, now: f64, dt: f32, t: f32, b: Rect) {
        self.step(now, dt);
        self.screen.draw(&self.term, &clock(), t, dt);
        let Some(th) = self.three.as_mut() else {
            return;
        };
        let size = (b.w.max(1.0) as u32, b.h.max(1.0) as u32);
        if let Some((view, mut enc)) = th.off.begin(size) {
            let cam = seated(size.0 as f32 / size.1 as f32);
            th.desk.draw(
                &mut th.r,
                &th.off.queue,
                (&mut enc, &view, size),
                cam,
                (&self.hands, &self.heat),
                (&self.screen, None),
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
