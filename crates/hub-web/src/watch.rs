//! The live preview on wyrm's card: the hub watches the real arena (as a
//! watcher, which cannot play and is not counted as one of the people
//! there), keeps its own copy of what it is sent, and draws it with the
//! game's own look, following whichever snake the server shows.

use look::{Burst, Gulp, Scene, View};
use pixels::{Canvas, Rect, Rgba};
use wyrm::mirror::Mirror;
use wyrm::proto::{unq, Down};

pub struct Watch {
    pub socket: Option<kit::Socket>,
    pub up: bool,
    pub retry_at: f64,
    pub retries: u32,
    mirror: Mirror,
    arena: f32,
    frame_at: f64,
    gap: f64,
    gulps: Vec<Gulp>,
    bursts: Vec<Burst>,
    camera: Option<(f32, f32)>,
    buf: Canvas,
}

impl Watch {
    pub fn new() -> Watch {
        Watch {
            socket: None,
            up: false,
            retry_at: 0.0,
            retries: 0,
            mirror: Mirror::default(),
            arena: wyrm::laws::ARENA,
            frame_at: 0.0,
            gap: 1000.0 / wyrm::laws::TICK_HZ as f64,
            gulps: Vec::new(),
            bursts: Vec::new(),
            camera: None,
            buf: Canvas::new(1, 1),
        }
    }

    pub fn receive(&mut self, now: f64, bytes: &[u8]) {
        match Down::decode(bytes) {
            Some(Down::Hello { arena, .. }) => {
                self.arena = arena as f32;
                self.mirror = Mirror::default();
            }
            Some(Down::Frame(f)) => {
                if self.frame_at > 0.0 {
                    let g = (now - self.frame_at).clamp(10.0, 250.0);
                    self.gap += (g - self.gap) * 0.1;
                }
                self.frame_at = now;
                for &(x, y, hue, r) in &f.bursts {
                    self.bursts.push(Burst {
                        x: unq(x),
                        y: unq(y),
                        hue,
                        r: r as f32,
                        at: now,
                    });
                }
                for (pellet, by) in self.mirror.apply(&f) {
                    if by != 0 {
                        self.gulps.push(Gulp {
                            pellet,
                            by,
                            at: now,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    pub fn closed(&mut self, now: f64) {
        self.up = false;
        self.socket = None;
        self.retries += 1;
        self.retry_at = now + 1000.0 * 2f64.powi(self.retries.min(5) as i32);
        self.mirror = Mirror::default();
        self.camera = None;
    }

    /// Draw what is happening now into `at`, at `k` pixels per world unit.
    pub fn draw(&mut self, into: &mut Canvas, at: Rect, k: f32, corner: f32, u: i32, now: f64) {
        self.gulps.retain(|g| now - g.at < 250.0);
        self.bursts.retain(|b| now - b.at < 700.0);
        self.buf.resize(at.w as i32, at.h as i32);
        let alpha = ((now - self.frame_at) / self.gap).clamp(0.0, 1.0) as f32;
        let live = !self.mirror.snakes.is_empty();
        if live {
            let target = look::watched(&self.mirror, alpha);
            let cam = self.camera.get_or_insert(target);
            cam.0 += (target.0 - cam.0) * 0.12;
            cam.1 += (target.1 - cam.1) * 0.12;
            let v = View {
                w: self.buf.w as f32,
                h: self.buf.h as f32,
                k,
                cx: cam.0,
                cy: cam.1,
            };
            let scene = Scene {
                mirror: &self.mirror,
                alpha,
                arena: self.arena,
                gulps: &self.gulps,
                bursts: &self.bursts,
                steer: None,
                names: Some(u),
            };
            look::world(&mut self.buf, &v, &scene, now);
        } else {
            self.buf.clear(look::BG);
            let msg = if self.up { "..." } else { "connecting..." };
            let (w, h) = (self.buf.w, self.buf.h);
            self.buf
                .text_centred(w / 2, h / 2 - 4 * u, msg, u, Rgba(244, 241, 255, 120));
        }
        // LIVE, with a beating dot.
        if live {
            let p = (4 * u) as f32;
            let beat = 0.55 + 0.45 * ((now / 400.0) as f32).sin().abs();
            self.buf.round_rect(
                Rect::new(p, p, (31 * u) as f32, (11 * u) as f32),
                3.0 * u as f32,
                Rgba(7, 10, 18, 170),
            );
            self.buf.circle(
                p + 5.5 * u as f32,
                p + 5.5 * u as f32,
                2.5 * u as f32,
                Rgba(255, 70, 90, 255).fade(beat),
            );
            self.buf.text(
                (p as i32) + 10 * u,
                (p as i32) + 2 * u,
                "LIVE",
                u,
                Rgba(244, 241, 255, 230),
            );
        }
        into.blit(&self.buf, at.x as i32, at.y as i32, corner);
    }
}
