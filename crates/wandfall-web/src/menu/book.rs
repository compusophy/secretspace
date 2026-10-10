//! The spellbook: your four slots (pick one), the spells of its kind (put
//! one you know in), the slot's rank, what the spell does and what it
//! beats; on the range (`rules`) every spell at any rank, your level, and
//! the range's rules. One column where it fits; on a short wide screen (a
//! phone held sideways) two, so its buttons stay big enough for a thumb.
//! The panel is as tall as what is in it.

use pixels::{Canvas, Rect, Rgba};
use wandfall::laws::{Kind, MAX_LEVEL, MAX_RANK, SPELLS};
use wandfall::proto::Own;

use super::{button, Act, Spots, BUTTON, DIM, GOLD, INK, LIT, PANEL};
use crate::bar::{icon, pips, rgba, KEYS};
use crate::fx::colour;

/// What the book shows: whose, which slot, the range's rules if there.
struct Book<'a> {
    own: &'a Own,
    sel: usize,
    rules: Option<(bool, bool)>,
    ui: i32,
}

impl Book<'_> {
    fn u(&self) -> f32 {
        self.ui as f32
    }

    /// The spells of the chosen slot's kind.
    fn list(&self) -> Vec<u8> {
        let kind = if self.sel < 2 {
            Kind::Offense
        } else {
            Kind::Utility
        };
        (0..SPELLS.len() as u8)
            .filter(|&k| SPELLS[k as usize].kind == kind)
            .collect()
    }

    /// The four slots' side, `width` across.
    fn side(&self, width: f32) -> f32 {
        ((width - 3.0 * 6.0 * self.u()) / 4.0).min(46.0 * self.u())
    }

    /// What the slot's spell does and what it beats, wrapped to `width`.
    fn about(&self, width: i32) -> Vec<(String, Rgba)> {
        let Some((sp, _)) = self.own.slots[self.sel] else {
            return Vec::new();
        };
        let info = &SPELLS[sp as usize];
        let col = rgba(colour(sp)).mix(INK, 0.4);
        let beats = format!("beats {}", info.beats);
        let beaten = format!("beaten by {}", info.beaten);
        [(info.what, col), (&beats, DIM), (&beaten, DIM)]
            .into_iter()
            .flat_map(|(t, ink)| {
                pixels::wrap(t, width, self.ui)
                    .into_iter()
                    .map(move |l| (l, ink))
            })
            .collect()
    }

    /// Whether the range's two rules go one over the other, `width`
    /// across (side by side, the longer would not read).
    fn stacked(&self, width: f32) -> bool {
        let half = (width - 6.0 * self.u()) / 2.0;
        pixels::text_width(SPAR, self.ui) > half as i32 - 8 * self.ui
    }

    /// How tall each part is, `width` across: the slots and rank; the
    /// list and what the spell does; the tools (or the word on how).
    fn heights(&self, width: f32) -> (f32, f32, f32) {
        let u = self.u();
        let slots = self.side(width) + 8.0 * u + 24.0 * u;
        let rows = self.list().len().div_ceil(2) as f32;
        let about = self.about(width as i32).len() as f32 * 11.0 * u;
        let list = rows * 26.0 * u + 6.0 * u + about + 6.0 * u;
        let tools = if self.rules.is_some() {
            22.0 * u + 18.0 * u + if self.stacked(width) { 22.0 * u } else { 0.0 }
        } else {
            pixels::wrap(HELP, width as i32, self.ui).len() as f32 * 11.0 * u
        };
        (slots, list, tools)
    }
}

const HELP: &str = "run over spell cubes to learn them; pick a slot, then a spell";
const SPAR: &str = "dummies fight back";

/// The spellbook, at `ui` if it fits the screen (in one column, or in
/// two), else smaller.
pub fn book(
    c: &mut Canvas,
    spots: &mut Spots,
    ui: i32,
    own: &Own,
    sel: usize,
    rules: Option<(bool, bool)>,
) {
    let (w, h) = (c.w as f32, c.h as f32);
    // (scale, two columns): the first that fits.
    let fit = (1..=ui).rev().find_map(|k| {
        let b = Book {
            own,
            sel,
            rules,
            ui: k,
        };
        let u = k as f32;
        let one = (360.0 * u).min(w - 8.0 * u);
        let (s, l, t) = b.heights(one - 20.0 * u);
        let pad = 8.0 * u + 24.0 * u + 10.0 * u;
        if pad + s + l + t <= h - 8.0 && 300.0 * u <= w - 8.0 {
            return Some((k, false));
        }
        // Two columns, each wide enough for a spell's name by its icon.
        let two = (600.0 * u).min(w - 8.0 * u);
        let col = (two - 30.0 * u) / 2.0;
        let (s, l, t) = b.heights(col);
        (pad + (s + t).max(l) <= h - 8.0 && col >= 160.0 * u).then_some((k, true))
    });
    let (k, two) = fit.unwrap_or((1, false));
    let b = Book {
        own,
        sel,
        rules,
        ui: k,
    };
    draw(c, spots, &b, two);
}

fn draw(c: &mut Canvas, spots: &mut Spots, b: &Book, two: bool) {
    let (w, h) = (c.w as f32, c.h as f32);
    let (u, ui) = (b.u(), b.ui);
    let pw = if two {
        (600.0 * u).min(w - 8.0 * u)
    } else {
        (360.0 * u).min(w - 8.0 * u)
    };
    let col = if two {
        (pw - 30.0 * u) / 2.0
    } else {
        pw - 20.0 * u
    };
    let (s, l, t) = b.heights(col);
    let body = if two { (s + t).max(l) } else { s + l + t };
    let ph = (8.0 * u + 24.0 * u + body + 10.0 * u).min(h - 8.0);
    let panel = Rect::new((w - pw) / 2.0, ((h - ph) / 2.0).max(4.0), pw, ph);
    c.round_rect(panel, 6.0 * u, PANEL);
    spots.0.push((panel, Act::Stay));
    let x0 = panel.x + 10.0 * u;
    let y = panel.y + 8.0 * u;
    c.text_shadowed(x0 as i32, y as i32, "spellbook", 2 * ui, GOLD);
    let close = Rect::new(panel.x + pw - 70.0 * u, y - 2.0 * u, 60.0 * u, 18.0 * u);
    button(c, spots, close, "close", false, Act::CloseBook, ui);
    let y = y + 24.0 * u;
    let y_left = slots(c, spots, b, (x0, y, col));
    if two {
        // Left: the slots, the rank, the tools; right: the spells.
        tools(c, spots, b, (x0, y_left, col));
        spells(c, spots, b, (x0 + col + 10.0 * u, y, col));
    } else {
        let y = spells(c, spots, b, (x0, y_left, col));
        tools(c, spots, b, (x0, y, col));
    }
}

/// The four slots (pick one), and the slot's rank (on the range, any);
/// where it ends.
fn slots(c: &mut Canvas, spots: &mut Spots, b: &Book, (x0, mut y, width): (f32, f32, f32)) -> f32 {
    let (u, ui) = (b.u(), b.ui);
    let s = b.side(width);
    for (k, key) in KEYS.iter().enumerate() {
        let r = Rect::new(x0 + k as f32 * (s + 6.0 * u), y, s, s);
        c.round_rect(r, 4.0 * u, if k == b.sel { LIT } else { BUTTON });
        if let Some((sp, rank)) = b.own.slots[k] {
            icon(c, sp, r.grow(-4.0 * u));
            pips(c, r.x + s / 2.0, r.y + s - 3.0 * u, rank, ui);
        }
        let ink = if k == b.sel { GOLD } else { DIM };
        c.text_shadowed(r.x as i32 + 2 * ui, r.y as i32 + 2 * ui, key, ui, ink);
        spots.0.push((r, Act::Slot(k)));
    }
    y += s + 8.0 * u;
    let rank = b.own.slots[b.sel].map_or(1, |s| s.1);
    c.text_shadowed(x0 as i32, y as i32 + 4 * ui, "rank", ui, DIM);
    if b.rules.is_some() {
        for r in 1..=MAX_RANK {
            let at = Rect::new(
                x0 + 40.0 * u + (r - 1) as f32 * 24.0 * u,
                y,
                20.0 * u,
                16.0 * u,
            );
            button(c, spots, at, &format!("{r}"), r == rank, Act::Rank(r), ui);
        }
    } else if b.own.slots[b.sel].is_some() {
        let t = "I".repeat(rank as usize);
        c.text_shadowed((x0 + 40.0 * u) as i32, y as i32 + 4 * ui, &t, ui, GOLD);
    }
    y + 24.0 * u
}

/// Every spell of the slot's kind (put one you know in), and what the
/// slot's does; where it ends.
fn spells(c: &mut Canvas, spots: &mut Spots, b: &Book, (x0, mut y, width): (f32, f32, f32)) -> f32 {
    let (u, ui) = (b.u(), b.ui);
    let list = b.list();
    let cols = 2;
    let cw = (width - 6.0 * u) / cols as f32;
    let rh = 22.0 * u;
    for (n, &sp) in list.iter().enumerate() {
        let r = Rect::new(
            x0 + (n % cols) as f32 * (cw + 6.0 * u),
            y + (n / cols) as f32 * (rh + 4.0 * u),
            cw,
            rh,
        );
        let have = b.own.slots[b.sel].is_some_and(|s| s.0 == sp);
        let known = b.own.book[sp as usize];
        c.round_rect(r, 3.0 * u, if have { LIT } else { BUTTON });
        let tile = Rect::new(r.x + 3.0 * u, r.y + 3.0 * u, rh - 6.0 * u, rh - 6.0 * u);
        icon(c, sp, tile);
        let name = SPELLS[sp as usize].name;
        let (tx, ty) = (
            (r.x + rh + 2.0 * u) as i32,
            (r.y + rh / 2.0) as i32 - 3 * ui,
        );
        if known == 0 {
            // Not found yet: dimmed, and not to be chosen.
            c.round_rect(r, 3.0 * u, Rgba(6, 8, 16, 170));
            c.text_shadowed(tx, ty, name, ui, DIM.fade(0.5));
            continue;
        }
        c.text_shadowed(tx, ty, name, ui, if have { GOLD } else { INK });
        if b.rules.is_none() {
            pips(c, r.x + r.w - 12.0 * u, r.y + rh / 2.0, known, ui);
        }
        spots.0.push((r, Act::Spell(sp)));
    }
    y += list.len().div_ceil(cols) as f32 * (rh + 4.0 * u) + 6.0 * u;
    for (line, ink) in b.about(width as i32) {
        c.text_shadowed(x0 as i32, y as i32, &line, ui, ink);
        y += 11.0 * u;
    }
    y + 6.0 * u
}

/// The range's tools (your level, and its rules), or a word on how the
/// book works.
fn tools(c: &mut Canvas, spots: &mut Spots, b: &Book, (x0, mut y, width): (f32, f32, f32)) {
    let (u, ui) = (b.u(), b.ui);
    let Some(rules) = b.rules else {
        for line in pixels::wrap(HELP, width as i32, ui) {
            c.text_shadowed(x0 as i32, y as i32, &line, ui, DIM);
            y += 11.0 * u;
        }
        return;
    };
    let level = format!("level {}", b.own.level);
    c.text_shadowed(x0 as i32, y as i32 + 4 * ui, &level, ui, INK);
    let at = |x: f32, wide: f32| Rect::new(x0 + x * u, y, wide * u, 16.0 * u);
    button(c, spots, at(60.0, 20.0), "-", false, Act::Level(-1), ui);
    button(c, spots, at(84.0, 20.0), "+", false, Act::Level(1), ui);
    let top = b.own.level >= MAX_LEVEL;
    button(c, spots, at(108.0, 30.0), "max", top, Act::MaxLevel, ui);
    y += 22.0 * u;
    // The rules side by side, or one over the other where their words
    // would not fit at the size of the rest.
    let stack = b.stacked(width);
    let tw = if stack {
        width
    } else {
        (width - 6.0 * u) / 2.0
    };
    let rule = Rect::new(x0, y, tw, 18.0 * u);
    button(
        c,
        spots,
        rule,
        "no cooldowns",
        rules.0,
        Act::NoCooldowns,
        ui,
    );
    let spar = if stack {
        Rect::new(x0, y + 22.0 * u, tw, 18.0 * u)
    } else {
        Rect::new(x0 + tw + 6.0 * u, y, tw, 18.0 * u)
    };
    button(c, spots, spar, SPAR, rules.1, Act::Sparring, ui);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn own() -> Own {
        let mut book = [0; SPELLS.len()];
        (book[0], book[1]) = (1, 2);
        Own {
            slots: [Some((0, 1)), Some((1, 2)), Some((4, 1)), None],
            book,
            ..Own::default()
        }
    }

    /// A phone held sideways gets the book at its full size, in two
    /// columns, every button on the screen; the panel ends where its
    /// content does.
    #[test]
    fn the_book_fits_a_phone_at_full_size() {
        for (w, h) in [(844, 390), (740, 360)] {
            let mut c = Canvas::new(w, h);
            let mut spots = Spots::default();
            book(&mut c, &mut spots, 2, &own(), 0, Some((false, true)));
            let panel = spots.0[0].0;
            for (r, a) in &spots.0 {
                assert!(r.y >= 0.0 && r.y + r.h <= h as f32, "{w}x{h}: {a:?} {r:?}");
                assert!(r.x >= 0.0 && r.x + r.w <= w as f32, "{w}x{h}: {a:?} {r:?}");
            }
            // Rank buttons a thumb can press: 20 x 16 at ui 2.
            let rank = spots.0.iter().find(|s| s.1 == Act::Rank(1)).unwrap().0;
            assert_eq!((rank.w, rank.h), (40.0, 32.0), "{w}x{h}");
            // And every rule's words at that size.
            let spar = spots.0.iter().find(|s| s.1 == Act::Sparring).unwrap().0;
            assert!(spar.w as i32 >= pixels::text_width(SPAR, 2) + 16, "{w}x{h}");
            // As tall as what is in it, not a fixed 290 (which no phone
            // had room for).
            assert!(
                panel.h < 290.0 * 2.0 && panel.w > panel.h,
                "{w}x{h}: {panel:?}"
            );
        }
    }
}
