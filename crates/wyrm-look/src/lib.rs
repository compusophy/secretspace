//! How wyrm looks, pixel by pixel: a dark field with a faint grid and a
//! glowing edge, pulsing food, glossy striped snakes eased along their
//! paths between the server's frames, and bursts where they die. The
//! game's page and the hub's live preview both draw with this, so they
//! look the same; `live` keeps what they are sent.

pub mod live;
mod snake;

pub use live::{ease, Live, BURST_MS, CAMERA_MS, GULP_MS, ZOOM_MS};

use pixels::{Canvas, Rgba};
use wyrm::laws::food_radius;
use wyrm::mirror::{Mirror, Pellet, Seen};

pub const BG: Rgba = Rgba::rgb(7, 10, 18);

/// Food flying into the mouth that ate it.
pub struct Gulp {
    pub pellet: Pellet,
    pub by: u16,
    pub at: f64,
}

/// A snake bursting: a ring and sparks where its head was.
pub struct Burst {
    pub x: f32,
    pub y: f32,
    pub hue: u8,
    pub r: f32,
    pub at: f64,
}

/// A hue off the wire (0..=255) as degrees.
pub fn hue(h: u8) -> f32 {
    h as f32 * 360.0 / 256.0
}

/// Where a wave `period_ms` long is at `now`, in radians. Worked out in
/// f64 first, so a page left open for days still moves smoothly.
pub fn phase(now: f64, period_ms: f64) -> f32 {
    ((now / period_ms) % std::f64::consts::TAU) as f32
}

/// How a snake is drawn now, `alpha` of the way to the next frame: how
/// many points behind its newest head it starts, and how many points long
/// it is.
pub fn drawn(s: &Seen, alpha: f32) -> (f32, f32) {
    let k = 1.0 - alpha.clamp(0.0, 1.0);
    (s.lag * k, (s.body.len() as f32 - s.short * k).max(1.0))
}

/// The point `i` places along a snake, between its points: along its body
/// and on into the trail just cut off it.
pub fn along(s: &Seen, i: f32) -> (f32, f32) {
    let (nb, n) = (s.body.len(), s.body.len() + s.trail.len());
    if n == 0 {
        return (0.0, 0.0);
    }
    let at = |k: usize| {
        let k = k.min(n - 1);
        if k < nb {
            s.body[k]
        } else {
            s.trail[k - nb]
        }
    };
    let i = i.max(0.0);
    let k = i.floor() as usize;
    let t = i - k as f32;
    let (a, b) = (at(k), at(k + 1));
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

/// Where a snake's head is drawn now, `alpha` of the way to the next frame.
pub fn head(s: &Seen, alpha: f32) -> (f32, f32) {
    along(s, drawn(s, alpha).0)
}

/// Where a watcher's camera should be: the head of the snake nearest the
/// point the server centres on.
pub fn watched(m: &Mirror, alpha: f32) -> (f32, f32) {
    m.snakes
        .values()
        .min_by(|a, b| {
            let d = |s: &Seen| {
                let p = s.body.front().copied().unwrap_or_default();
                (p.0 - m.centre.0).powi(2) + (p.1 - m.centre.1).powi(2)
            };
            d(a).total_cmp(&d(b))
        })
        .map(|s| head(s, alpha))
        .unwrap_or(m.centre)
}

/// Where the world is on the canvas: buffer pixels per world unit, and the
/// world point at the canvas's centre.
pub struct View {
    pub w: f32,
    pub h: f32,
    pub k: f32,
    pub cx: f32,
    pub cy: f32,
}

impl View {
    pub fn at(&self, p: (f32, f32)) -> (f32, f32) {
        (
            (p.0 - self.cx) * self.k + self.w / 2.0,
            (p.1 - self.cy) * self.k + self.h / 2.0,
        )
    }
    pub fn sees(&self, p: (f32, f32), pad: f32) -> bool {
        p.0 > -pad && p.1 > -pad && p.0 < self.w + pad && p.1 < self.h + pad
    }
}

/// Everything to draw, and how.
pub struct Scene<'a> {
    pub mirror: &'a Mirror,
    /// How far between the last frame and the next: 0..=1.
    pub alpha: f32,
    pub arena: f32,
    pub gulps: &'a [Gulp],
    pub bursts: &'a [Burst],
    /// Where the player points, for their snake's eyes.
    pub steer: Option<f32>,
    /// Text scale for names over snakes; None to leave them off.
    pub names: Option<i32>,
}

/// The whole world, as the scene has it.
pub fn world(c: &mut Canvas, v: &View, sc: &Scene, now: f64) {
    ground(c, v, sc.arena);
    food(c, v, sc, now);
    let m = sc.mirror;
    let mut order: Vec<&Seen> = m.snakes.values().collect();
    order.sort_by_key(|s| (s.mass, s.id == m.you));
    for s in order {
        snake::draw(c, v, s, sc, s.id == m.you, now);
    }
    bursts(c, v, sc.bursts, now);
}

fn ground(c: &mut Canvas, v: &View, arena: f32) {
    c.clear(BG);
    // A faint grid, so motion reads even on empty ground.
    let step = 64.0;
    let line = Rgba(120, 150, 255, 16);
    let x0 = v.cx - v.w / 2.0 / v.k;
    let y0 = v.cy - v.h / 2.0 / v.k;
    let mut x = (x0 / step).floor() * step;
    while (x - v.cx) * v.k + v.w / 2.0 < v.w {
        let sx = v.at((x, 0.0)).0.round() as i32;
        c.vline(sx, 0, c.h, line);
        x += step;
    }
    let mut y = (y0 / step).floor() * step;
    while (y - v.cy) * v.k + v.h / 2.0 < v.h {
        let sy = v.at((0.0, y)).1.round() as i32;
        c.hline(0, c.w, sy, line);
        y += step;
    }
    // Beyond the edge is death: darken it, and draw the edge glowing.
    let (ox, oy) = v.at((0.0, 0.0));
    let rr = arena * v.k;
    let near = |px: f32, py: f32| ((px - ox).powi(2) + (py - oy).powi(2)).sqrt();
    let far = [
        near(0.0, 0.0),
        near(v.w, 0.0),
        near(0.0, v.h),
        near(v.w, v.h),
    ]
    .iter()
    .fold(0.0f32, |a, &b| a.max(b));
    if far > rr - 12.0 {
        c.outside_circle(ox, oy, rr, Rgba(40, 0, 10, 185));
        c.ring(ox, oy, rr, 9.0, Rgba(255, 60, 90, 40));
        c.ring(ox, oy, rr, 2.0, Rgba(255, 70, 100, 230));
    }
}

fn food(c: &mut Canvas, v: &View, sc: &Scene, now: f64) {
    let beat = phase(now, 380.0);
    for (id, p) in &sc.mirror.food {
        let (x, y) = v.at((p.x, p.y));
        if !v.sees((x, y), 12.0) {
            continue;
        }
        let pulse = 1.0 + 0.18 * (beat + (*id % 97) as f32).sin();
        let r = (food_radius(p.value) * v.k * pulse).max(1.0);
        c.glow(x, y, r * 2.6, Rgba::hsl(hue(p.hue), 1.0, 0.6).fade(0.55));
        c.circle(x, y, r, Rgba::hsl(hue(p.hue), 1.0, 0.68));
    }
    // Food on its way into a mouth, faster as it nears.
    for g in sc.gulps {
        let Some(s) = sc.mirror.snakes.get(&g.by) else {
            continue;
        };
        let to = head(s, sc.alpha);
        let t = ((now - g.at) / GULP_MS).clamp(0.0, 1.0) as f32;
        let tt = t * t;
        let p = (
            g.pellet.x + (to.0 - g.pellet.x) * tt,
            g.pellet.y + (to.1 - g.pellet.y) * tt,
        );
        let (x, y) = v.at(p);
        let r = food_radius(g.pellet.value) * v.k * (1.0 - t);
        c.circle(
            x,
            y,
            r.max(0.5),
            Rgba::hsl(hue(g.pellet.hue), 1.0, 0.72).fade(1.0 - t * 0.5),
        );
    }
}

/// A number from 0 to 1 made from a few others, the same every time.
fn hash(a: u32, b: u32, k: u32) -> f32 {
    let mut h = a
        .wrapping_mul(0x9e37_79b1)
        .wrapping_add(b.wrapping_mul(0x85eb_ca77))
        .wrapping_add(k.wrapping_mul(0xc2b2_ae3d));
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h & 0xffff) as f32 / 65535.0
}

/// A burst: a flash of its colour, a ring flying out, and sparks streaking
/// away, each its own size and speed, slowing as they go.
fn bursts(c: &mut Canvas, v: &View, bursts: &[Burst], now: f64) {
    const SPARKS: u32 = 18;
    let hot = Rgba::rgb(255, 250, 235);
    for b in bursts {
        let t = ((now - b.at) / BURST_MS).clamp(0.0, 1.0) as f32;
        let (x, y) = v.at((b.x, b.y));
        if !v.sees((x, y), b.r * v.k * 9.0) {
            continue;
        }
        let r = b.r * v.k;
        let fade = 1.0 - t;
        let out = 1.0 - fade * fade * fade;
        let h = hue(b.hue);
        let colour = Rgba::hsl(h, 1.0, 0.66);
        if t < 0.35 {
            let f = 1.0 - t / 0.35;
            c.glow(x, y, r * (3.0 + 3.0 * t), colour.fade(0.7 * f));
            c.glow(x, y, r * 1.6, hot.fade(0.9 * f * f));
        }
        c.ring(
            x,
            y,
            r * (1.0 + 5.0 * out),
            3.5 * fade * fade + 1.0,
            colour.mix(hot, 0.4 * fade).fade(0.85 * fade),
        );
        let (sx, sy) = (b.x.to_bits(), b.y.to_bits());
        let ink = hot.mix(colour, (t * 2.5).min(1.0)).fade(fade);
        for k in 0..SPARKS {
            let a = (k as f32 + hash(sx, sy, k)) / SPARKS as f32 * std::f32::consts::TAU;
            let speed = 0.3 + 0.7 * hash(sy, sx, k + 99);
            let size = r * (0.08 + 0.1 * hash(sx ^ sy, k, 7)) * (0.4 + 0.6 * fade);
            // A streak from where it was a moment ago to where it is.
            let d1 = r * (1.0 + 7.0 * speed * out);
            let d0 = (d1 - r * (0.5 + 2.5 * speed) * fade).max(r * 0.8);
            let (ca, sa) = (a.cos(), a.sin());
            c.line(
                x + ca * d0,
                y + sa * d0,
                x + ca * d1,
                y + sa * d1,
                (2.0 * size).max(1.0),
                ink,
            );
        }
    }
}
