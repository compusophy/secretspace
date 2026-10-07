//! How wyrm looks, pixel by pixel: a dark field with a faint grid and a
//! glowing edge, pulsing food, striped snakes eased along their paths
//! between the server's frames, and bursts where they die. The game's page
//! and the hub's live preview both draw with this, so they look the same.

use std::collections::VecDeque;

use pixels::{Canvas, Rgba};
use wyrm::laws::{food_radius, radius};
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

/// The point `i` places along a body, between its points.
pub fn along(body: &VecDeque<(f32, f32)>, i: f32) -> (f32, f32) {
    let n = body.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let i = i.max(0.0);
    let k = (i.floor() as usize).min(n - 1);
    let t = i - k as f32;
    let a = body[k];
    let b = body[(k + 1).min(n - 1)];
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

/// Where a snake's head is drawn now, `alpha` of the way to the next frame.
pub fn head(s: &Seen, alpha: f32) -> (f32, f32) {
    along(&s.body, (1.0 - alpha) * s.moved as f32)
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
        snake(c, v, s, sc, s.id == m.you, now);
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
    for (id, p) in &sc.mirror.food {
        let (x, y) = v.at((p.x, p.y));
        if !v.sees((x, y), 12.0) {
            continue;
        }
        let pulse = 1.0 + 0.18 * ((now / 380.0) as f32 + (*id % 97) as f32).sin();
        let r = (food_radius(p.value) * v.k * pulse).max(1.0);
        c.glow(x, y, r * 2.6, Rgba::hsl(hue(p.hue), 1.0, 0.6).fade(0.55));
        c.circle(x, y, r, Rgba::hsl(hue(p.hue), 1.0, 0.68));
    }
    // Food on its way into a mouth.
    for g in sc.gulps {
        let Some(s) = sc.mirror.snakes.get(&g.by) else {
            continue;
        };
        let to = s.body.front().copied().unwrap_or_default();
        let t = ((now - g.at) / 250.0).clamp(0.0, 1.0) as f32;
        let p = (
            g.pellet.x + (to.0 - g.pellet.x) * t,
            g.pellet.y + (to.1 - g.pellet.y) * t,
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

fn snake(c: &mut Canvas, v: &View, s: &Seen, sc: &Scene, mine: bool, now: f64) {
    let lag = (1.0 - sc.alpha) * s.moved as f32;
    let n = s.body.len();
    if n == 0 {
        return;
    }
    let r = radius(s.mass as f32) * v.k;
    let pts: Vec<(f32, f32)> = (0..n)
        .map(|k| v.at(along(&s.body, k as f32 + lag)))
        .collect();
    if !pts.iter().step_by(4).any(|&p| v.sees(p, r + 40.0)) {
        return;
    }
    // Close points on a fat snake overlap anyway: draw fewer.
    let gap = wyrm::laws::STEP * v.k;
    let every = ((r * 0.6) / gap.max(0.1)).floor().max(1.0) as usize;
    let see = |p: (f32, f32)| v.sees(p, r + 2.0);
    let a = if s.ghost {
        0.35 + 0.2 * ((now / 120.0) as f32).sin()
    } else {
        1.0
    };
    let h = hue(s.hue);

    if s.boosting {
        let flicker = 0.35 + 0.15 * ((now / 60.0) as f32).sin();
        for &p in pts.iter().rev().step_by(every * 3) {
            if see(p) {
                c.glow(p.0, p.1, r * 2.6, Rgba::hsl(h, 1.0, 0.65).fade(flicker * a));
            }
        }
    }
    // Outline under everything, then the body from the tail up, in bands
    // that stay put on the body as it moves.
    let outline = Rgba::hsl(h, 0.7, 0.16).fade(a);
    for (k, &p) in pts.iter().enumerate().rev() {
        if (k % every == 0 || k == 0) && see(p) {
            c.circle(p.0, p.1, r + 1.2, outline);
        }
    }
    for (k, &p) in pts.iter().enumerate().rev() {
        if (k % every == 0 || k == 0) && see(p) {
            let band = (s.seq.wrapping_sub(k as u32) / 5).is_multiple_of(2);
            let l = if band { 0.6 } else { 0.5 };
            c.circle(p.0, p.1, r, Rgba::hsl(h, 0.88, l).fade(a));
        }
    }

    // Eyes, looking where it heads (yours: where you point).
    let (hx, hy) = pts[0];
    let look = if mine {
        sc.steer.unwrap_or(s.angle)
    } else {
        s.angle
    };
    for side in [-1.0f32, 1.0] {
        let e = s.angle + side * 0.75;
        let (ex, ey) = (hx + e.cos() * r * 0.55, hy + e.sin() * r * 0.55);
        c.circle(
            ex,
            ey,
            (r * 0.38).max(1.2),
            Rgba::rgb(255, 255, 255).fade(a),
        );
        c.circle(
            ex + look.cos() * r * 0.15,
            ey + look.sin() * r * 0.15,
            (r * 0.2).max(0.7),
            Rgba::rgb(11, 13, 20).fade(a),
        );
    }

    // Names over other snakes.
    if let (Some(u), false) = (sc.names, mine) {
        let y = (hy - r) as i32 - 10 * u;
        c.text_centred(hx as i32, y, &s.name, u, Rgba(255, 255, 255, 180));
    }
}

/// A burst: a ring flying out and sparks, where a snake ran into someone.
fn bursts(c: &mut Canvas, v: &View, bursts: &[Burst], now: f64) {
    for b in bursts {
        let t = ((now - b.at) / 700.0).clamp(0.0, 1.0) as f32;
        let (x, y) = v.at((b.x, b.y));
        let r = b.r * v.k;
        let fade = 1.0 - t;
        let h = hue(b.hue);
        c.ring(
            x,
            y,
            r * (1.0 + 5.0 * t),
            2.0 * fade + 1.0,
            Rgba::hsl(h, 1.0, 0.7).fade(0.8 * fade),
        );
        for k in 0..12 {
            let a = k as f32 / 12.0 * std::f32::consts::TAU + (b.at % 1.0) as f32;
            let d = r * (1.0 + 7.0 * t);
            c.circle(
                x + a.cos() * d,
                y + a.sin() * d,
                (r * 0.25 * fade).max(1.0),
                Rgba::hsl(h, 1.0, 0.75).fade(fade),
            );
        }
    }
}
