//! The spell bar at the foot of the screen: your two offensive spells (Q,
//! E) and two utility (R, F), each an icon in its colour with its rank
//! and its cooldown; your level and XP; what lies underfoot and how to
//! take it; and a cheer when you level.

use pixels::{Canvas, Rect, Rgba};
use render::V3;
use wandfall::laws::{spell, MAX_LEVEL, SCROLL_REACH, SPELLS, TICK_HZ, XP_PER_LEVEL};
use wandfall::loot::{cooldown, slots_of};
use wandfall::proto::{Loot, Own};

use crate::fx::colour;

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

/// A spell's icon in a box `s` across at (x, y).
pub fn icon(c: &mut Canvas, sp: u8, x: f32, y: f32, s: f32, col: Rgba) {
    let w = (s / 10.0).max(1.0);
    let (cx, cy) = (x + s / 2.0, y + s / 2.0);
    let at = |u: f32, v: f32| (x + u * s, y + v * s);
    let line = |c: &mut Canvas, a: (f32, f32), b: (f32, f32)| c.line(a.0, a.1, b.0, b.1, w, col);
    match sp {
        spell::LANCE => {
            line(c, at(0.15, 0.85), at(0.85, 0.15));
            c.circle(x + 0.82 * s, y + 0.18 * s, w * 1.6, col);
        }
        spell::COMET => {
            c.circle(x + 0.65 * s, y + 0.35 * s, s * 0.18, col);
            for k in 0..3 {
                let o = k as f32 * 0.1;
                line(c, at(0.15 + o, 0.65 + o), at(0.5 + o, 0.35 + o));
            }
        }
        spell::CHAIN => {
            let pts = [
                at(0.1, 0.2),
                at(0.4, 0.45),
                at(0.3, 0.6),
                at(0.65, 0.75),
                at(0.55, 0.85),
                at(0.9, 0.9),
            ];
            for p in pts.windows(2) {
                line(c, p[0], p[1]);
            }
        }
        spell::STARFALL => {
            line(c, at(0.5, 0.1), at(0.5, 0.5));
            line(c, at(0.3, 0.3), at(0.7, 0.3));
            line(c, at(0.36, 0.16), at(0.64, 0.44));
            line(c, at(0.64, 0.16), at(0.36, 0.44));
            c.ring(cx, y + 0.78 * s, s * 0.18, w, col);
        }
        spell::ROOT => {
            for u in [0.3, 0.5, 0.7] {
                line(c, at(u, 0.9), at(u - 0.08, 0.6));
                line(c, at(u - 0.08, 0.6), at(u + 0.05, 0.25));
            }
        }
        spell::BLINK => {
            line(c, at(0.15, 0.5), at(0.85, 0.5));
            line(c, at(0.85, 0.5), at(0.6, 0.3));
            line(c, at(0.85, 0.5), at(0.6, 0.7));
            c.circle(x + 0.15 * s, cy, w * 1.4, col);
        }
        spell::WARD => {
            c.ring(cx, cy, s * 0.32, w * 1.2, col);
            c.ring(cx, cy, s * 0.18, w, col.fade(0.7));
        }
        spell::MEND => {
            line(c, at(0.5, 0.2), at(0.5, 0.8));
            line(c, at(0.2, 0.5), at(0.8, 0.5));
        }
        spell::GUST => {
            for (k, r) in [0.15, 0.27, 0.39].into_iter().enumerate() {
                c.ring(x + 0.25 * s, cy, s * r, w, col.fade(1.0 - k as f32 * 0.25));
            }
        }
        spell::HASTE => {
            for o in [0.0, 0.3] {
                line(c, at(0.2 + o, 0.25), at(0.45 + o, 0.5));
                line(c, at(0.45 + o, 0.5), at(0.2 + o, 0.75));
            }
        }
        _ => {}
    }
}

/// The bar, your level, and what is underfoot.
/// `slots`: draw the four slots (a touch screen has buttons instead).
pub fn draw(
    c: &mut Canvas,
    own: &Own,
    loot: &Loot,
    now: f64,
    levelled: (f64, u8),
    ui: i32,
    slots: bool,
) {
    let (w, h) = (c.w, c.h);
    let s = 28 * ui;
    let gap = 5 * ui;
    let split = 12 * ui;
    let total = 4 * s + 2 * gap + split;
    let x0 = (w - total) / 2;
    let y = h - s - 26 * ui;
    for (k, key) in KEYS.iter().enumerate().filter(|_| slots) {
        let x = x0 + k as i32 * (s + gap) + if k >= 2 { split - gap } else { 0 };
        let b = Rect::new(x as f32, y as f32, s as f32, s as f32);
        c.round_rect(b, 4.0 * ui as f32, SHADE);
        match own.slots[k] {
            Some((sp, rank)) => {
                let col = rgba(colour(sp));
                icon(
                    c,
                    sp,
                    (x + 4 * ui) as f32,
                    (y + 3 * ui) as f32,
                    (s - 8 * ui) as f32,
                    col,
                );
                for r in 0..rank as i32 {
                    c.fill_rect(
                        x + 3 * ui + r * 4 * ui,
                        y + s - 4 * ui,
                        3 * ui,
                        2 * ui,
                        GOLD,
                    );
                }
                let cd = own.cds[k] as u32;
                if cd > 0 {
                    let full = cooldown(sp, rank).max(1);
                    let k2 = (cd * s as u32 / full) as i32;
                    c.fill_rect(x, y, s, k2.min(s), Rgba(0, 0, 0, 150));
                    let secs = format!("{}", cd.div_ceil(TICK_HZ));
                    c.text_centred(x + s / 2, y + s / 2 - 3 * ui, &secs, ui, INK);
                }
            }
            None => {
                c.round_rect_line(
                    b.grow(-2.0 * ui as f32),
                    3.0 * ui as f32,
                    ui as f32,
                    DIM.fade(0.25),
                );
            }
        }
        c.text_centred(x + s / 2, y + s + 3 * ui, key, ui, DIM);
    }
    // Your level, and XP toward the next (on a touch screen the level is
    // in the health line, and XP under the health bar).
    let (x0, y, total) = if slots {
        let (lx, ly) = (x0 - 22 * ui, y + s / 2);
        c.circle(lx as f32, ly as f32, 13.0 * ui as f32, SHADE);
        c.ring(lx as f32, ly as f32, 13.0 * ui as f32, ui as f32, GOLD);
        c.text_centred(lx, ly - 3 * ui, &format!("{}", own.level), ui, GOLD);
        (x0, y, total)
    } else {
        (12 * ui, 58 * ui, 120 * ui)
    };
    let xp = if own.level >= MAX_LEVEL {
        1.0
    } else {
        own.xp as f32 / XP_PER_LEVEL as f32
    };
    c.fill_rect(x0, y - 6 * ui, total, 2 * ui, SHADE);
    c.fill_rect(x0, y - 6 * ui, (total as f32 * xp) as i32, 2 * ui, GOLD);
    // What lies underfoot.
    let me = own.body.p;
    let near = loot
        .scrolls
        .iter()
        .map(|s| (s, (s.3[0] - me[0]).powi(2) + (s.3[2] - me[2]).powi(2)))
        .filter(|(_, d2)| *d2 < 16.0)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((&(_, sp, rank, _), d2)) = near {
        let sp_info = &SPELLS[sp as usize % SPELLS.len()];
        let col = rgba(colour(sp));
        let line = format!("{} - rank {}: {}", sp_info.name, rank, sp_info.what);
        let k = pixels::fit_scale(&line, w - 20 * ui, ui);
        c.text_centred(w / 2, y - 30 * ui, &line, k, col);
        let [a, b] = slots_of(sp);
        let have = own.slots.iter().flatten().any(|s| s.0 == sp);
        let free = own.slots[a].is_none() || own.slots[b].is_none();
        let hint = if have {
            "walk over it: your spell ranks up".to_string()
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
                format!("hold G to swap your {old} for it")
            } else {
                format!("stand on it and hold G to swap your {old}")
            }
        };
        c.text_centred(w / 2, y - 19 * ui, &hint, ui, INK);
    }
    // A level gained.
    let age = now - levelled.0;
    if age < 1800.0 && levelled.1 > 1 {
        let f = (1.0 - (age - 1200.0).max(0.0) / 600.0) as f32;
        let t = format!("level {}!", levelled.1);
        c.text_centred(w / 2, h / 3, &t, 3 * ui, GOLD.fade(f));
    }
}
