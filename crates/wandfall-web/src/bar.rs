//! The spell bar at the foot of the screen: your two spells to hurt with
//! (Q, E) and two to live by (R, F), each a tile in its spell's colour
//! with its glyph, its rank and its cooldown sweeping away; your level
//! and XP; the spell cube underfoot and what running over it does; and a cheer when you
//! level. The icons are drawn here, shape by shape, so every size is
//! sharp: a flame, a spear of light, a snowflake, a bolt, a step, a
//! shield, a cross, the wind.

use pixels::{Canvas, Rect, Rgba};
use wandfall::laws::{MAX_LEVEL, MAX_RANK, SPELLS, TICK_HZ, XP_PER_LEVEL};
use wandfall::loot::cooldown;
use wandfall::proto::Own;

use crate::fx::colour;
pub use crate::icon::{glyph, icon, rgba};
use crate::state::State;

pub const KEYS: [&str; 4] = ["Q", "E", "R", "F"];
const INK: Rgba = Rgba::rgb(250, 246, 236);
const DIM: Rgba = Rgba::rgb(200, 206, 220);
const GOLD: Rgba = Rgba::rgb(255, 214, 128);
const SHADE: Rgba = Rgba(8, 10, 20, 170);

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

/// Where the bar goes (at the foot of the screen, to the right, clear of
/// your wizard in the middle): its left, its top, a tile's size, and how
/// wide it is in all.
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
        x: w - total - 14 * ui,
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
    // What lies underfoot: over the bar, or (touch) at the top.
    let at = if slots {
        (Some(x0 + total), y - 10 * ui)
    } else {
        (None, 108 * ui)
    };
    underfoot(c, own, st, at, ui);
    // A level gained.
    learned(c, st, now, ui);
    let age = now - st.levelled.0;
    if age < 1800.0 && st.levelled.1 > 1 {
        let f = (1.0 - (age - 1200.0).max(0.0) / 600.0) as f32;
        let t = format!("level {}!", st.levelled.1);
        c.text_centred(w / 2, h / 3, &t, 3 * ui, GOLD.fade(f));
    }
}

/// A spell just learned or ranked up, over the middle of the screen; a
/// reminder of the spellbook when it is not in a slot.
fn learned(c: &mut Canvas, st: &State, now: f64, ui: i32) {
    let (at, sp, rank, slotted) = st.learned;
    let age = now - at;
    if rank == 0 || age > 2600.0 {
        return;
    }
    let f = (1.0 - ((age - 1800.0) / 800.0).max(0.0)) as f32;
    let col = rgba(colour(sp)).mix(INK, 0.25).fade(f);
    let name = SPELLS[sp as usize % SPELLS.len()].name;
    let t = if rank == 1 {
        format!("learned {name}")
    } else {
        format!("{name} {}", "I".repeat(rank as usize))
    };
    let (w, y) = (c.w, c.h * 2 / 3 - 30 * ui);
    let k = pixels::fit_scale(&t, w - 16 * ui, 2 * ui);
    c.text_centred(w / 2, y, &t, k, col);
    if !slotted {
        c.text_centred(
            w / 2,
            y + 18 * ui,
            "B: the spellbook, to use it",
            ui,
            DIM.fade(f),
        );
    }
}

/// What lies underfoot: its icon, what it is, and what running over it
/// does, on a card whose foot is at `y` (its right edge at `right`, or
/// across the middle).
fn underfoot(c: &mut Canvas, own: &Own, st: &State, (right, y): (Option<i32>, i32), ui: i32) {
    let me = own.body.p;
    let near = st
        .loot
        .scrolls
        .iter()
        .map(|s| (s, (s.3[0] - me[0]).powi(2) + (s.3[2] - me[2]).powi(2)))
        .filter(|(_, d2)| *d2 < 16.0)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let Some((&(_, sp, rank, _), _)) = near else {
        return;
    };
    let w = c.w;
    let info = &SPELLS[sp as usize % SPELLS.len()];
    let col = rgba(colour(sp));
    let known = own.book[sp as usize % SPELLS.len()];
    let hint = if known == 0 {
        "run over it to learn it".to_string()
    } else if known >= MAX_RANK {
        "yours is at its highest rank".to_string()
    } else {
        format!(
            "run over it: yours ranks up to {}",
            "I".repeat(known as usize + 1)
        )
    };
    let u = ui as f32;
    let title = format!("{}  {}", info.name, "I".repeat(rank as usize));
    let line = info.what;
    let tw = pixels::text_width(&title, 2 * ui)
        .max(pixels::text_width(line, ui))
        .max(pixels::text_width(&hint, ui));
    let cw = (tw + 46 * ui).min(w - 8 * ui);
    let ch = 38 * ui;
    let x = right.map_or((w - cw) / 2, |r| (r - cw).max(4 * ui));
    let card = Rect::new(x as f32, (y - ch) as f32, cw as f32, ch as f32);
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
