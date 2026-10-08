//! The spell bar at the foot of the screen: your two spells to hurt with
//! (Q, E) and two to live by (R, F), each a tile in its spell's colour
//! with its glyph, its rank and its cooldown sweeping away; your level
//! and XP; what lies underfoot and how to take it; and a cheer when you
//! level. The icons are drawn here, shape by shape, so every size is
//! sharp: a flame, a spear of light, a snowflake, a bolt, a step, a
//! shield, a cross, the wind.

use pixels::{Canvas, Rect, Rgba};
use render::V3;
use wandfall::laws::{spell, MAX_LEVEL, MAX_RANK, SCROLL_REACH, SPELLS, TICK_HZ, XP_PER_LEVEL};
use wandfall::loot::{cooldown, slots_of};
use wandfall::proto::Own;

use crate::fx::colour;
use crate::state::State;

pub const KEYS: [&str; 4] = ["Q", "E", "R", "F"];
const INK: Rgba = Rgba::rgb(250, 246, 236);
const DIM: Rgba = Rgba::rgb(200, 206, 220);
const GOLD: Rgba = Rgba::rgb(255, 214, 128);
const SHADE: Rgba = Rgba(8, 10, 20, 170);

pub fn rgba(c: V3) -> Rgba {
    Rgba::rgb(
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
    )
}

/// A line through points, `w` wide.
fn stroke(c: &mut Canvas, pts: &[(f32, f32)], w: f32, col: Rgba) {
    for p in pts.windows(2) {
        c.line(p[0].0, p[0].1, p[1].0, p[1].1, w, col);
    }
}

/// A four-pointed glint at (x, y), `r` from its middle to a point.
fn glint(c: &mut Canvas, x: f32, y: f32, r: f32, col: Rgba) {
    let t = r * 0.22;
    c.poly(&[(x - r, y), (x, y - t), (x + r, y), (x, y + t)], col);
    c.poly(&[(x, y - r), (x + t, y), (x, y + r), (x - t, y)], col);
}

/// A spell's glyph in the box `b`, in `col` (`back`: the tile behind it,
/// for the cut-outs).
pub fn glyph(c: &mut Canvas, sp: u8, b: Rect, col: Rgba, back: Rgba) {
    let s = b.w;
    let at = |u: f32, v: f32| (b.x + u * s, b.y + v * s);
    let pts = |list: &[(f32, f32)]| list.iter().map(|&(u, v)| at(u, v)).collect::<Vec<_>>();
    match sp {
        spell::FIREBALL => {
            c.poly(
                &pts(&[
                    (0.5, 0.08),
                    (0.63, 0.3),
                    (0.76, 0.45),
                    (0.8, 0.62),
                    (0.73, 0.78),
                    (0.6, 0.88),
                    (0.5, 0.9),
                    (0.4, 0.88),
                    (0.27, 0.78),
                    (0.2, 0.62),
                    (0.26, 0.44),
                    (0.36, 0.52),
                    (0.37, 0.35),
                ]),
                col,
            );
            c.poly(
                &pts(&[
                    (0.52, 0.44),
                    (0.63, 0.6),
                    (0.63, 0.72),
                    (0.52, 0.8),
                    (0.41, 0.73),
                    (0.42, 0.6),
                ]),
                back,
            );
        }
        spell::LANCE => {
            c.poly(
                &pts(&[(0.88, 0.12), (0.7, 0.4), (0.12, 0.88), (0.6, 0.3)]),
                col,
            );
            let (x, y) = at(0.76, 0.24);
            glint(c, x, y, s * 0.2, col);
        }
        spell::FROST => {
            let (cx, cy) = at(0.5, 0.5);
            let w = s * 0.075;
            for k in 0..6 {
                let a = k as f32 * std::f32::consts::PI / 3.0 + std::f32::consts::FRAC_PI_2;
                let (dx, dy) = (a.cos(), a.sin());
                let end = (cx + dx * s * 0.38, cy + dy * s * 0.38);
                c.line(cx, cy, end.0, end.1, w, col);
                let m = (cx + dx * s * 0.24, cy + dy * s * 0.24);
                for side in [-1.0f32, 1.0] {
                    let b = a + side * 0.8;
                    let tip = (m.0 + b.cos() * s * 0.12, m.1 + b.sin() * s * 0.12);
                    c.line(m.0, m.1, tip.0, tip.1, w * 0.85, col);
                }
            }
            c.circle(cx, cy, s * 0.07, col);
        }
        spell::LIGHTNING => c.poly(
            &pts(&[
                (0.6, 0.06),
                (0.25, 0.54),
                (0.47, 0.54),
                (0.37, 0.94),
                (0.76, 0.42),
                (0.54, 0.42),
                (0.67, 0.06),
            ]),
            col,
        ),
        spell::BLINK => {
            let (ox, oy) = at(0.3, 0.68);
            c.ring(ox, oy, s * 0.13, s * 0.06, col.fade(0.55));
            for (k, u) in [0.36f32, 0.5, 0.64].into_iter().enumerate() {
                let (x, y) = at(0.3 + 0.38 * u * 1.0, 0.68 - 0.32 * u);
                c.circle(
                    x,
                    y,
                    s * (0.03 + 0.012 * k as f32),
                    col.fade(0.6 + 0.13 * k as f32),
                );
            }
            let (dx, dy) = at(0.66, 0.38);
            c.circle(dx, dy, s * 0.15, col);
            let (gx, gy) = at(0.84, 0.17);
            glint(c, gx, gy, s * 0.12, col);
        }
        spell::WARD => {
            let shield = [
                (0.5, 0.08),
                (0.82, 0.2),
                (0.79, 0.52),
                (0.5, 0.92),
                (0.21, 0.52),
                (0.18, 0.2),
            ];
            c.poly(&pts(&shield), col);
            let inset: Vec<(f32, f32)> = shield
                .iter()
                .map(|&(u, v)| (0.5 + (u - 0.5) * 0.62, 0.47 + (v - 0.47) * 0.62))
                .collect();
            c.poly(&pts(&inset), back);
            let inner: Vec<(f32, f32)> = shield
                .iter()
                .map(|&(u, v)| (0.5 + (u - 0.5) * 0.3, 0.46 + (v - 0.46) * 0.3))
                .collect();
            c.poly(&pts(&inner), col);
        }
        spell::MEND => {
            let w = s * 0.21;
            let (a, bb) = (at(0.5, 0.2), at(0.5, 0.8));
            c.line(a.0, a.1, bb.0, bb.1, w, col);
            let (a, bb) = (at(0.2, 0.5), at(0.8, 0.5));
            c.line(a.0, a.1, bb.0, bb.1, w, col);
            let (gx, gy) = at(0.82, 0.18);
            glint(c, gx, gy, s * 0.12, col);
        }
        spell::GUST => {
            let w = s * 0.075;
            for (y, x0, x1, r) in [
                (0.32, 0.1, 0.6, 0.12),
                (0.54, 0.2, 0.74, 0.1),
                (0.76, 0.12, 0.48, 0.08),
            ] {
                let mut line = vec![at(x0, y)];
                // Along, then a curl up and back over itself.
                let (cx, cy) = (x1, y - r);
                for k in 0..=12 {
                    let u = k as f32 / 12.0;
                    let a = std::f32::consts::FRAC_PI_2 - u * 1.5 * std::f32::consts::PI;
                    let rr = r * (1.0 - 0.4 * u);
                    line.push(at(cx + a.cos() * rr, cy + a.sin() * rr));
                }
                stroke(c, &line, w, col);
            }
        }
        _ => {}
    }
}

/// A spell's icon: a tile in its colour, its glyph upon it.
pub fn icon(c: &mut Canvas, sp: u8, b: Rect) {
    let base = rgba(colour(sp));
    let dark = Rgba::rgb(14, 16, 26).mix(base, 0.32);
    let r = b.w * 0.2;
    c.round_rect(b, r, dark);
    c.glow(
        b.x + b.w / 2.0,
        b.y + b.h * 0.45,
        b.w * 0.5,
        base.fade(0.55),
    );
    c.round_rect(
        Rect::new(b.x + b.w * 0.08, b.y + b.h * 0.06, b.w * 0.84, b.h * 0.36),
        r * 0.7,
        Rgba(255, 255, 255, 14),
    );
    c.round_rect_line(
        b.grow(-0.5),
        r,
        (b.w / 26.0).max(1.0),
        base.mix(INK, 0.35).fade(0.9),
    );
    let g = b.grow(-b.w * 0.16);
    let off = (b.w / 28.0).max(1.0);
    let back = dark.mix(base, 0.25);
    glyph(
        c,
        sp,
        Rect::new(g.x + off, g.y + off, g.w, g.h),
        Rgba(0, 0, 0, 110),
        Rgba(0, 0, 0, 0),
    );
    glyph(c, sp, g, INK.mix(base, 0.12), back);
}

/// A slot's rank, as pips under it.
pub fn pips(c: &mut Canvas, cx: f32, y: f32, rank: u8, ui: i32) {
    let u = ui as f32;
    let gap = 5.0 * u;
    let x0 = cx - gap * (MAX_RANK as f32 - 1.0) / 2.0;
    for r in 0..MAX_RANK {
        let x = x0 + r as f32 * gap;
        let on = r < rank;
        let k = 1.6 * u;
        let d = [(x, y - k), (x + k, y), (x, y + k), (x - k, y)];
        c.poly(&d, if on { GOLD } else { DIM.fade(0.3) });
    }
}

/// Where the bar goes: its left, its top, a tile's size, and how wide
/// it is in all.
pub struct Layout {
    pub x: i32,
    pub y: i32,
    pub s: i32,
    pub total: i32,
}

pub fn layout(w: i32, h: i32, ui: i32) -> Layout {
    let s = 32 * ui;
    let total = 4 * s + 2 * 6 * ui + 16 * ui;
    Layout {
        x: (w - total) / 2,
        y: h - s - 26 * ui,
        s,
        total,
    }
}

/// The bar, your level, and what is underfoot.
/// `slots`: draw the four slots (a touch screen has buttons instead).
pub fn draw(c: &mut Canvas, own: &Own, st: &State, now: f64, ui: i32, slots: bool) {
    let (w, h) = (c.w, c.h);
    let u = ui as f32;
    let Layout { x: x0, y, s, total } = layout(w, h, ui);
    let (gap, split) = (6 * ui, 16 * ui);
    for (k, key) in KEYS.iter().enumerate().filter(|_| slots) {
        let x = x0 + k as i32 * (s + gap) + if k >= 2 { split - gap } else { 0 };
        let b = Rect::new(x as f32, y as f32, s as f32, s as f32);
        let r = s as f32 * 0.2;
        match own.slots[k] {
            Some((sp, rank)) => {
                icon(c, sp, b);
                let cd = own.cds[k] as u32;
                if cd > 0 {
                    let full = cooldown(sp, rank).max(1);
                    let left = (cd as f32 / full as f32).min(1.0);
                    c.sweep(b, r, 1.0 - left, 1.0, Rgba(4, 6, 12, 175));
                    let secs = format!("{}", cd.div_ceil(TICK_HZ));
                    let ty = y + s / 2 - 5 * ui;
                    c.text_centred(x + s / 2 + ui, ty + ui, &secs, 2 * ui, Rgba(0, 0, 0, 160));
                    c.text_centred(x + s / 2, ty, &secs, 2 * ui, INK);
                } else {
                    // Ready again: a flash about it.
                    let age = (now - st.ready[k]) as f32;
                    if age < 350.0 {
                        let f = 1.0 - age / 350.0;
                        let col = rgba(colour(sp)).mix(INK, 0.5).fade(f);
                        c.round_rect_line(b.grow(age / 350.0 * 5.0 * u), r, 2.0 * u, col);
                    }
                }
                pips(
                    c,
                    x as f32 + s as f32 / 2.0,
                    (y + s) as f32 + 5.0 * u,
                    rank,
                    ui,
                );
            }
            None => {
                c.round_rect(b, r, SHADE);
                c.round_rect_line(b.grow(-2.0 * u), r, u, DIM.fade(0.25));
            }
        }
        let kb = Rect::new(b.x - 3.0 * u, b.y - 3.0 * u, 10.0 * u, 10.0 * u);
        c.round_rect(kb, 2.5 * u, Rgba(8, 10, 20, 210));
        c.text_centred(x - 3 * ui + 5 * ui, y - ui, key, ui, INK);
    }
    // Your level, and XP toward the next: under the bar (on a touch
    // screen the level is in the health line, and XP under its bar).
    let (xx, xy, xw) = if slots {
        let (lx, ly) = (x0 - 24 * ui, y + s / 2);
        c.circle(lx as f32, ly as f32, 13.0 * u, SHADE);
        c.ring(lx as f32, ly as f32, 13.0 * u, u, GOLD);
        c.text_centred(lx, ly - 3 * ui, &format!("{}", own.level), ui, GOLD);
        (x0, y + s + 10 * ui, total)
    } else {
        (12 * ui, 50 * ui, 120 * ui)
    };
    let xp = if own.level >= MAX_LEVEL {
        1.0
    } else {
        own.xp as f32 / XP_PER_LEVEL as f32
    };
    c.fill_rect(xx, xy, xw, 2 * ui, SHADE);
    c.fill_rect(xx, xy, (xw as f32 * xp) as i32, 2 * ui, GOLD);
    // What lies underfoot: over the health bar, or (touch) at the top.
    let bottom = if slots { y - 22 * ui } else { 108 * ui };
    underfoot(c, own, st, bottom, ui);
    // A level gained.
    let age = now - st.levelled.0;
    if age < 1800.0 && st.levelled.1 > 1 {
        let f = (1.0 - (age - 1200.0).max(0.0) / 600.0) as f32;
        let t = format!("level {}!", st.levelled.1);
        c.text_centred(w / 2, h / 3, &t, 3 * ui, GOLD.fade(f));
    }
}

/// What lies underfoot: its icon, what it is, and how to take it, on a
/// card whose foot is at `y`.
fn underfoot(c: &mut Canvas, own: &Own, st: &State, y: i32, ui: i32) {
    let me = own.body.p;
    let near = st
        .loot
        .scrolls
        .iter()
        .map(|s| (s, (s.3[0] - me[0]).powi(2) + (s.3[2] - me[2]).powi(2)))
        .filter(|(_, d2)| *d2 < 16.0)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let Some((&(_, sp, rank, _), d2)) = near else {
        return;
    };
    let w = c.w;
    let info = &SPELLS[sp as usize % SPELLS.len()];
    let col = rgba(colour(sp));
    let [a, b] = slots_of(sp);
    let have = own.slots.iter().flatten().find(|s| s.0 == sp);
    let free = own.slots[a].is_none() || own.slots[b].is_none();
    let hint = if let Some(h) = have {
        if h.1 >= MAX_RANK {
            "yours is at its highest rank".to_string()
        } else {
            "walk over it: yours ranks up".to_string()
        }
    } else if free {
        "walk over it to take it".to_string()
    } else {
        let weak = if own.slots[a].map_or(0, |s| s.1) <= own.slots[b].map_or(0, |s| s.1) {
            a
        } else {
            b
        };
        let old = own.slots[weak].map_or("", |s| SPELLS[s.0 as usize % SPELLS.len()].name);
        if d2 < SCROLL_REACH * SCROLL_REACH {
            format!("hold G to swap your {old} ({}) for it", KEYS[weak])
        } else {
            format!("stand on it and hold G to swap your {old}")
        }
    };
    let u = ui as f32;
    let title = format!("{}  {}", info.name, "I".repeat(rank as usize));
    let line = info.what;
    let tw = pixels::text_width(&title, 2 * ui)
        .max(pixels::text_width(line, ui))
        .max(pixels::text_width(&hint, ui));
    let cw = (tw + 46 * ui).min(w - 8 * ui);
    let ch = 38 * ui;
    let card = Rect::new(((w - cw) / 2) as f32, (y - ch) as f32, cw as f32, ch as f32);
    c.round_rect(card, 6.0 * u, Rgba(8, 10, 20, 200));
    c.round_rect_line(card, 6.0 * u, u, col.fade(0.6));
    icon(
        c,
        sp,
        Rect::new(card.x + 5.0 * u, card.y + 5.0 * u, 28.0 * u, 28.0 * u),
    );
    let tx = (card.x + 39.0 * u) as i32;
    let ty = card.y as i32;
    let k = pixels::fit_scale(&title, cw - 44 * ui, 2 * ui);
    c.text_shadowed(tx, ty + 5 * ui, &title, k, col.mix(INK, 0.3));
    c.text_shadowed(
        tx,
        ty + 19 * ui,
        line,
        pixels::fit_scale(line, cw - 44 * ui, ui),
        DIM,
    );
    c.text_shadowed(
        tx,
        ty + 28 * ui,
        &hint,
        pixels::fit_scale(&hint, cw - 44 * ui, ui),
        INK,
    );
}
