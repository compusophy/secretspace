//! The feel, drawn only: sparks where blows land, streaks behind the
//! knocked-back, skid marks, slingshot sparks (blue, then gold), the ring
//! of a perfect release, a shake and a 50 ms hit-stop on heavy hits. None
//! of it touches prediction.

use pixels::{Canvas, Rgba};

use crate::palette::{GOLD, RIM};

#[derive(Clone, Copy, Debug)]
pub struct Spark {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub born: f64,
    pub life: f64,
    pub c: Rgba,
}

#[derive(Clone, Copy, Debug)]
pub struct Ring {
    pub x: f32,
    pub y: f32,
    pub born: f64,
    pub life: f64,
    pub r: f32,
    pub c: Rgba,
}

#[derive(Clone, Copy, Debug)]
pub struct Mark {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub born: f64,
}

/// Everything in flight. Positions are in world tiles.
#[derive(Default)]
pub struct Effects {
    pub sparks: Vec<Spark>,
    pub rings: Vec<Ring>,
    pub marks: Vec<Mark>,
    /// Until when the picture shakes, and holds still.
    pub shake_until: f64,
    pub stop_until: f64,
    seed: u32,
}

impl Effects {
    fn rand(&mut self) -> f32 {
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    /// A burst of `n` sparks at (x, y).
    pub fn burst(&mut self, x: f32, y: f32, n: usize, speed: f32, c: Rgba, now: f64) {
        for _ in 0..n {
            let a = self.rand() * std::f32::consts::TAU;
            let s = speed * (0.4 + self.rand() * 0.8);
            let life = 220.0 + self.rand() as f64 * 260.0;
            self.sparks.push(Spark {
                x,
                y,
                vx: a.cos() * s,
                vy: a.sin() * s,
                born: now,
                life,
                c,
            });
        }
    }

    pub fn ring(&mut self, x: f32, y: f32, r: f32, c: Rgba, now: f64) {
        self.rings.push(Ring {
            x,
            y,
            born: now,
            life: 380.0,
            r,
            c,
        });
    }

    /// A blow landed: sparks, and on a big one a shake and a hit-stop.
    pub fn hit(&mut self, x: f32, y: f32, big: bool, now: f64) {
        self.burst(
            x,
            y,
            if big { 18 } else { 9 },
            if big { 7.0 } else { 4.5 },
            GOLD,
            now,
        );
        if big {
            self.shake_until = now + 160.0;
            self.stop_until = now + 50.0;
            self.ring(x, y, 1.4, GOLD, now);
        }
    }

    pub fn sling(&mut self, x: f32, y: f32, gold: bool, now: f64) {
        let c = if gold { GOLD } else { RIM };
        self.burst(x, y, 12, 5.0, c, now);
        self.ring(x, y, 0.9, c, now);
    }

    pub fn skid(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, now: f64) {
        self.marks.push(Mark {
            x0,
            y0,
            x1,
            y1,
            born: now,
        });
        if self.marks.len() > 160 {
            self.marks.remove(0);
        }
    }

    /// The shake offset now, in pixels.
    pub fn shake(&mut self, now: f64) -> (i32, i32) {
        if now >= self.shake_until {
            return (0, 0);
        }
        let k = ((self.shake_until - now) / 160.0) as f32;
        let (a, b) = (self.rand() - 0.5, self.rand() - 0.5);
        ((a * 4.0 * k).round() as i32, (b * 4.0 * k).round() as i32)
    }

    /// Skid marks, under everything.
    pub fn draw_under(&mut self, c: &mut Canvas, to: &dyn Fn(f32, f32) -> (f32, f32), now: f64) {
        self.marks.retain(|m| now - m.born < 2_500.0);
        for m in &self.marks {
            let a = (1.0 - (now - m.born) / 2_500.0).clamp(0.0, 1.0) as f32;
            let (x0, y0) = to(m.x0, m.y0);
            let (x1, y1) = to(m.x1, m.y1);
            c.line(x0, y0, x1, y1, 1.5, Rgba(10, 10, 20, (a * 90.0) as u8));
        }
    }

    /// Sparks and rings, over everything.
    pub fn draw_over(&mut self, c: &mut Canvas, to: &dyn Fn(f32, f32) -> (f32, f32), now: f64) {
        self.sparks.retain(|s| now - s.born < s.life);
        for s in &self.sparks {
            let t = ((now - s.born) / 1000.0) as f32;
            let (x, y) = to(s.x + s.vx * t * (1.0 - t), s.y + s.vy * t * (1.0 - t));
            let k = (1.0 - (now - s.born) / s.life).clamp(0.0, 1.0);
            c.add(x as i32, y as i32, s.c, (k * 255.0) as u32);
            c.add(x as i32 + 1, y as i32, s.c, (k * 120.0) as u32);
        }
        self.rings.retain(|r| now - r.born < r.life);
        for r in &self.rings {
            let k = ((now - r.born) / r.life) as f32;
            let (x, y) = to(r.x, r.y);
            let rad = r.r * 16.0 * (0.4 + k);
            c.ring(x, y, rad, 1.5, r.c.fade(1.0 - k));
        }
    }
}
