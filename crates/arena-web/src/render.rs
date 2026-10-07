//! The picture, pixel by pixel: a dark field, glowing food, striped snakes
//! eased between the server's frames, and the boards around the edge.

use std::collections::VecDeque;

use arena::laws::{food_radius, radius, view_scale};
use arena::mirror::Seen;
use pixels::{text_width, Canvas, Rect, Rgba};

use crate::state::State;

pub const BG: Rgba = Rgba::rgb(7, 10, 18);
pub const INK: Rgba = Rgba::rgb(244, 241, 255);
pub const DIM: Rgba = Rgba(244, 241, 255, 150);
pub const GO: Rgba = Rgba::rgb(87, 227, 137);

/// A lineage of hues: the wire's 0..=255 as degrees.
fn hue(h: u8) -> f32 {
    h as f32 * 360.0 / 256.0
}

/// The point `i` places along a body, between its points.
fn along(body: &VecDeque<(f32, f32)>, i: f32) -> (f32, f32) {
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

/// Where the world is on the canvas: buffer pixels per world unit, and
/// the world point at the canvas's centre.
pub struct View {
    pub w: f32,
    pub h: f32,
    pub k: f32,
    pub cx: f32,
    pub cy: f32,
}

impl View {
    fn at(&self, p: (f32, f32)) -> (f32, f32) {
        (
            (p.0 - self.cx) * self.k + self.w / 2.0,
            (p.1 - self.cy) * self.k + self.h / 2.0,
        )
    }
    fn sees(&self, p: (f32, f32), pad: f32) -> bool {
        p.0 > -pad && p.1 > -pad && p.0 < self.w + pad && p.1 < self.h + pad
    }
}

/// Everything the HUD needs that is not the game's own state.
pub struct Hud {
    /// Text scale.
    pub u: i32,
    /// Where the steering is aimed, for your snake's eyes.
    pub steer: Option<f32>,
    /// The phone's boost button, if there is one, and whether it is held.
    pub boost: Option<(Rect, bool)>,
}

pub fn frame(c: &mut Canvas, st: &mut State, css: (f64, f64), scale: f64, now: f64, hud: &Hud) {
    let alpha = st.alpha(now);
    let m = &st.mirror;
    let you = m.snakes.get(&m.you);
    let (w, h) = (c.w as f32, c.h as f32);

    // Where to look: your head, or near what the server centres on.
    let target = match you {
        Some(s) => along(&s.body, (1.0 - alpha) * s.moved as f32),
        None => m
            .snakes
            .values()
            .min_by(|a, b| {
                let d = |s: &Seen| {
                    let p = s.body.front().copied().unwrap_or_default();
                    (p.0 - m.centre.0).powi(2) + (p.1 - m.centre.1).powi(2)
                };
                d(a).total_cmp(&d(b))
            })
            .map(|s| along(&s.body, (1.0 - alpha) * s.moved as f32))
            .unwrap_or(m.centre),
    };
    let r = you.map_or(18.0, |s| radius(s.mass as f32));
    let want = view_scale(r, css.0 as f32, css.1 as f32) / scale as f32;
    if st.zoom == 0.0 {
        st.zoom = want;
        st.camera = target;
    }
    st.zoom += (want - st.zoom) * 0.05;
    if you.is_some() {
        st.camera = target;
    } else {
        st.camera.0 += (target.0 - st.camera.0) * 0.12;
        st.camera.1 += (target.1 - st.camera.1) * 0.12;
    }
    let v = View {
        w,
        h,
        k: st.zoom,
        cx: st.camera.0,
        cy: st.camera.1,
    };

    ground(c, &v, st.arena);
    food(c, st, &v, now);
    let m = &st.mirror;
    let mut order: Vec<&Seen> = m.snakes.values().collect();
    order.sort_by_key(|s| (s.mass, s.id == m.you));
    for s in order {
        snake(c, &v, s, alpha, s.id == m.you, hud, now);
    }
    bursts(c, st, &v, now);
    board(c, st, hud, now);
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
    let corners = [
        near(0.0, 0.0),
        near(v.w, 0.0),
        near(0.0, v.h),
        near(v.w, v.h),
    ];
    let far = corners.iter().fold(0.0f32, |a, &b| a.max(b));
    if far > rr - 12.0 {
        c.outside_circle(ox, oy, rr, Rgba(40, 0, 10, 185));
        c.ring(ox, oy, rr, 9.0, Rgba(255, 60, 90, 40));
        c.ring(ox, oy, rr, 2.0, Rgba(255, 70, 100, 230));
    }
}

fn food(c: &mut Canvas, st: &State, v: &View, now: f64) {
    for (id, p) in &st.mirror.food {
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
    for g in &st.gulps {
        let Some(s) = st.mirror.snakes.get(&g.by) else {
            continue;
        };
        let head = s.body.front().copied().unwrap_or_default();
        let t = ((now - g.at) / 250.0).clamp(0.0, 1.0) as f32;
        let p = (
            g.pellet.x + (head.0 - g.pellet.x) * t,
            g.pellet.y + (head.1 - g.pellet.y) * t,
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

fn snake(c: &mut Canvas, v: &View, s: &Seen, alpha: f32, mine: bool, hud: &Hud, now: f64) {
    let lag = (1.0 - alpha) * s.moved as f32;
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
    let gap = arena::laws::STEP * v.k;
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
        hud.steer.unwrap_or(s.angle)
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
    if !mine {
        let y = (hy - r) as i32 - 10 * hud.u;
        c.text_centred(hx as i32, y, &s.name, hud.u, Rgba(255, 255, 255, 180));
    }
}

/// A burst: a ring flying out and sparks, where a snake ran into someone.
fn bursts(c: &mut Canvas, st: &State, v: &View, now: f64) {
    for b in &st.bursts {
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

fn board(c: &mut Canvas, st: &State, hud: &Hud, now: f64) {
    let u = hud.u;
    let uf = u as f32;
    let (w, h) = (c.w, c.h);
    let pad = (8.0 * uf) as i32;
    let narrow = (w as f32 / uf) < 420.0;

    // You.
    if st.playing() {
        c.text_shadowed(pad, pad, &st.score().to_string(), 3 * u, INK);
        let rank = match (st.board.rank, narrow) {
            (0, _) => "length".to_string(),
            (r, true) => format!("#{r} of {}", st.board.snakes),
            (r, false) => format!("length   rank {r} of {}", st.board.snakes),
        };
        c.text_shadowed(pad, pad + 25 * u, &rank, u, DIM);
    }
    let people = st.board.people;
    let line = if st.connected {
        format!(
            "{people} {} here",
            if people == 1 { "person" } else { "people" }
        )
    } else {
        "connecting...".to_string()
    };
    let ly = pad + 37 * u;
    c.circle(
        pad as f32 + 2.5 * uf,
        ly as f32 + 3.5 * uf,
        2.5 * uf,
        if st.connected { GO } else { DIM },
    );
    c.text_shadowed(pad + 9 * u, ly, &line, u, Rgba(141, 255, 177, 210));

    // The leaderboard.
    // On a phone it is the top few, narrow, so it never covers the left.
    let (rows, lw, chars, head) = if narrow {
        (3, 78 * u, 6, "TOP")
    } else {
        (10, 118 * u, 11, "LEADERBOARD")
    };
    let lx = w - pad - lw;
    let n = rows.min(st.board.top.len());
    c.round_rect(
        Rect::new(
            (lx - 6 * u) as f32,
            (pad - 5 * u) as f32,
            (lw + 12 * u) as f32,
            (17 + 11 * n as i32) as f32 * uf,
        ),
        5.0 * uf,
        Rgba(10, 14, 26, 150),
    );
    c.text(lx, pad, head, u, Rgba(244, 241, 255, 200));
    let you_name = st
        .mirror
        .snakes
        .get(&st.mirror.you)
        .map(|s| s.name.as_str());
    for (i, l) in st.board.top.iter().take(rows).enumerate() {
        let y = pad + (13 + 11 * i as i32) * u;
        let me = st.board.rank as usize == i + 1 && you_name == Some(l.name.as_str());
        c.circle(
            lx as f32 + 2.5 * uf,
            y as f32 + 3.5 * uf,
            2.5 * uf,
            Rgba::hsl(hue(l.hue), 0.9, 0.6),
        );
        let name: String = l.name.chars().take(chars).collect();
        let ink = if me { INK } else { Rgba(244, 241, 255, 190) };
        c.text(lx + 8 * u, y, &format!("{}.{name}", i + 1), u, ink);
        let sc = l.score.to_string();
        c.text(lx + lw - text_width(&sc, u), y, &sc, u, ink);
    }

    // The feed: who ate whom.
    let feed_y =
        h - pad - (11 * st.feed.len() as i32 + if hud.boost.is_some() { 80 } else { 0 }) * u;
    for (i, (at, line)) in st.feed.iter().enumerate() {
        let a = (1.0 - (now - at - 5000.0) / 2000.0).clamp(0.0, 1.0) as f32;
        c.text_shadowed(
            pad,
            feed_y + 11 * i as i32 * u,
            line,
            u,
            Rgba(255, 210, 150, 220).fade(a),
        );
    }

    // The minimap.
    let mr = if narrow { 30.0 } else { 44.0 } * uf;
    let (mx, my) = (w as f32 - pad as f32 - mr, h as f32 - pad as f32 - mr);
    c.circle(mx, my, mr, Rgba(10, 14, 26, 170));
    c.ring(mx, my, mr, 1.0, Rgba(255, 60, 90, 130));
    for &(x, y, size) in &st.board.dots {
        let (dx, dy) = (mx + x as f32 / 127.0 * mr, my + y as f32 / 127.0 * mr);
        c.circle(
            dx,
            dy,
            (size as f32 / 16.0).clamp(0.8, 2.6) * uf,
            Rgba(255, 255, 255, 140),
        );
    }
    if st.playing() {
        let (cx, cy) = (
            mx + st.camera.0 / st.arena * mr,
            my + st.camera.1 / st.arena * mr,
        );
        c.circle(cx, cy, 2.6 * uf, Rgba::rgb(125, 255, 176));
    }

    // The phone's boost button.
    if let (Some((b, held)), true) = (hud.boost, st.playing()) {
        let (cx, cy, r) = (b.x + b.w / 2.0, b.y + b.h / 2.0, b.w / 2.0);
        c.circle(
            cx,
            cy,
            r,
            if held {
                Rgba(87, 227, 137, 90)
            } else {
                Rgba(255, 255, 255, 25)
            },
        );
        c.ring(cx, cy, r, 2.0, Rgba(255, 255, 255, 110));
        let tw = text_width("BOOST", u);
        c.text(cx as i32 - tw / 2, cy as i32 - 3 * u, "BOOST", u, INK);
    }

    // "you ate noodle!"
    if let Some(t) = &st.toast {
        let age = now - t.at;
        let a = (1.0 - (age - 1400.0) / 800.0).clamp(0.0, 1.0) as f32;
        let lift = (age / 2200.0 * 12.0) as i32 * u;
        let s = if narrow { 2 * u } else { 3 * u };
        c.text_centred(
            w / 2,
            h * 3 / 10 - lift,
            &t.text,
            s,
            Rgba(255, 224, 102, 255).fade(a),
        );
    }
}
