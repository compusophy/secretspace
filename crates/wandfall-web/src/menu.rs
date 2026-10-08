//! The menus, drawn in pixels: the title (play online, or the practice
//! range), the pause menu, the spellbook (the practice range's tools:
//! any spell at any rank in any slot, your level, no cooldowns, sparring)
//! and the online lobby's panel. Each button is a spot the page hit-tests.

use pixels::{Canvas, Rect, Rgba};
use wandfall::laws::{Kind, MAX_LEVEL, MAX_RANK, SPELLS};
use wandfall::proto::Own;

use crate::bar::{icon, rgba, KEYS};
use crate::fx::colour;

const INK: Rgba = Rgba::rgb(250, 246, 236);
const DIM: Rgba = Rgba::rgb(190, 196, 214);
const GOLD: Rgba = Rgba::rgb(255, 214, 128);
const PANEL: Rgba = Rgba(10, 12, 26, 215);
const BUTTON: Rgba = Rgba(34, 38, 66, 235);
const LIT: Rgba = Rgba(70, 64, 40, 240);

/// What a spot does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    Online,
    Practice,
    Resume,
    Leave,
    Book,
    CloseBook,
    Slot(usize),
    Spell(u8),
    Rank(u8),
    Level(i8),
    NoCooldowns,
    Sparring,
}

/// The buttons on screen now, in layer pixels.
#[derive(Default)]
pub struct Spots(pub Vec<(Rect, Act)>);

impl Spots {
    pub fn hit(&self, x: f32, y: f32) -> Option<Act> {
        self.0
            .iter()
            .rev()
            .find(|(r, _)| r.contains(x, y))
            .map(|s| s.1)
    }
}

fn button(c: &mut Canvas, spots: &mut Spots, b: Rect, text: &str, lit: bool, act: Act, ui: i32) {
    c.round_rect(b, 4.0 * ui as f32, if lit { LIT } else { BUTTON });
    let col = if lit { GOLD } else { INK };
    c.round_rect_line(b, 4.0 * ui as f32, 1.0, col.fade(0.5));
    // Big buttons, big words; small ones, small.
    let most = if b.h >= 26.0 * ui as f32 { 2 * ui } else { ui };
    let k = pixels::fit_scale(text, b.w as i32 - 8 * ui, most).max(1);
    c.text_centred(
        (b.x + b.w / 2.0) as i32,
        (b.y + b.h / 2.0) as i32 - 3 * k,
        text,
        k,
        col,
    );
    spots.0.push((b, act));
}

/// The title: the game's name, and how to play it.
pub fn title(c: &mut Canvas, spots: &mut Spots, ui: i32, note: &str) {
    let (w, h) = (c.w, c.h);
    c.fill_rect(0, 0, w, h, Rgba(6, 8, 18, 110));
    let name = "WANDFALL";
    let k = pixels::fit_scale(name, w - 20 * ui, 7 * ui);
    let top = h / 4;
    c.text_shadowed((w - pixels::text_width(name, k)) / 2, top, name, k, GOLD);
    let sub = "a wand battle royale";
    c.text_centred(w / 2, top + 9 * k, sub, 2 * ui, INK);
    let bw = (220 * ui).min(w - 24 * ui) as f32;
    let x = (w as f32 - bw) / 2.0;
    let y = (top + 9 * k + 30 * ui) as f32;
    let bh = 30.0 * ui as f32;
    button(
        c,
        spots,
        Rect::new(x, y, bw, bh),
        "play online",
        true,
        Act::Online,
        ui,
    );
    button(
        c,
        spots,
        Rect::new(x, y + bh + 10.0 * ui as f32, bw, bh),
        "practice range",
        false,
        Act::Practice,
        ui,
    );
    let mut ty = y as i32 + 2 * bh as i32 + 30 * ui;
    for line in [
        note,
        "drop on the island, find spells in chests,",
        "outlast the storm: last wizard standing wins",
    ] {
        let k = pixels::fit_scale(line, w - 16 * ui, ui);
        c.text_centred(w / 2, ty, line, k, DIM);
        ty += 11 * ui;
    }
}

/// The pause menu.
pub fn pause(c: &mut Canvas, spots: &mut Spots, ui: i32, practice: bool, touch: bool) {
    let (w, h) = (c.w, c.h);
    let bw = (200 * ui).min(w - 24 * ui) as f32;
    let bh = 28.0 * ui as f32;
    let n = if practice { 3.0 } else { 2.0 };
    let panel = Rect::new(
        (w as f32 - bw) / 2.0 - 12.0 * ui as f32,
        h as f32 / 2.0 - 70.0 * ui as f32,
        bw + 24.0 * ui as f32,
        44.0 * ui as f32 + n * (bh + 8.0 * ui as f32) + 24.0 * ui as f32,
    );
    c.round_rect(panel, 6.0 * ui as f32, PANEL);
    let x = (w as f32 - bw) / 2.0;
    let mut y = panel.y + 10.0 * ui as f32;
    let title = if practice {
        "practice range"
    } else {
        "wandfall"
    };
    c.text_centred(w / 2, y as i32, title, 2 * ui, GOLD);
    y += 24.0 * ui as f32;
    let resume = if touch { "back to it" } else { "click to play" };
    button(
        c,
        spots,
        Rect::new(x, y, bw, bh),
        resume,
        true,
        Act::Resume,
        ui,
    );
    y += bh + 8.0 * ui as f32;
    if practice {
        button(
            c,
            spots,
            Rect::new(x, y, bw, bh),
            "spellbook (B)",
            false,
            Act::Book,
            ui,
        );
        y += bh + 8.0 * ui as f32;
    }
    button(
        c,
        spots,
        Rect::new(x, y, bw, bh),
        "leave to the title",
        false,
        Act::Leave,
        ui,
    );
    y += bh + 10.0 * ui as f32;
    if !touch {
        let help =
            "WASD move - space jump - click cast - right click aim - Q E R F spells - G take";
        let k = pixels::fit_scale(help, w - 16 * ui, ui);
        c.text_centred(w / 2, y as i32, help, k, DIM);
    }
}

/// The spellbook: your four slots (pick one), every spell (put it in),
/// the slot's rank, your level, and the range's rules.
pub fn book(
    c: &mut Canvas,
    spots: &mut Spots,
    ui: i32,
    own: &Own,
    sel: usize,
    rules: (bool, bool),
) {
    let (w, h) = (c.w, c.h);
    let u = ui as f32;
    let pw = (360.0 * u).min(w as f32 - 12.0 * u);
    let ph = (300.0 * u).min(h as f32 - 12.0 * u);
    let panel = Rect::new((w as f32 - pw) / 2.0, (h as f32 - ph) / 2.0, pw, ph);
    c.round_rect(panel, 6.0 * u, PANEL);
    let x0 = panel.x + 10.0 * u;
    let mut y = panel.y + 8.0 * u;
    c.text_shadowed(x0 as i32, y as i32, "spellbook", 2 * ui, GOLD);
    button(
        c,
        spots,
        Rect::new(panel.x + pw - 70.0 * u, y - 2.0 * u, 60.0 * u, 18.0 * u),
        "close",
        false,
        Act::CloseBook,
        ui,
    );
    y += 24.0 * u;
    // The four slots.
    let s = ((pw - 20.0 * u - 3.0 * 6.0 * u) / 4.0).min(46.0 * u);
    for (k, key) in KEYS.iter().enumerate() {
        let b = Rect::new(x0 + k as f32 * (s + 6.0 * u), y, s, s);
        c.round_rect(b, 4.0 * u, if k == sel { LIT } else { BUTTON });
        if let Some((sp, rank)) = own.slots[k] {
            icon(
                c,
                sp,
                b.x + 5.0 * u,
                b.y + 4.0 * u,
                s - 10.0 * u,
                rgba(colour(sp)),
            );
            for r in 0..rank as i32 {
                c.fill_rect(
                    b.x as i32 + 3 * ui + r * 4 * ui,
                    (b.y + s) as i32 - 4 * ui,
                    3 * ui,
                    2 * ui,
                    GOLD,
                );
            }
        }
        c.text_shadowed(
            b.x as i32 + 2 * ui,
            b.y as i32 + 2 * ui,
            key,
            ui,
            if k == sel { GOLD } else { DIM },
        );
        spots.0.push((b, Act::Slot(k)));
    }
    y += s + 8.0 * u;
    // The slot's rank.
    let rank = own.slots[sel].map_or(1, |s| s.1);
    c.text_shadowed(x0 as i32, y as i32 + 4 * ui, "rank", ui, DIM);
    for r in 1..=MAX_RANK {
        let b = Rect::new(
            x0 + 40.0 * u + (r - 1) as f32 * 24.0 * u,
            y,
            20.0 * u,
            16.0 * u,
        );
        button(c, spots, b, &format!("{r}"), r == rank, Act::Rank(r), ui);
    }
    y += 24.0 * u;
    // Every spell of the slot's kind.
    let kind = if sel < 2 {
        Kind::Offense
    } else {
        Kind::Utility
    };
    let list: Vec<u8> = (0..SPELLS.len() as u8)
        .filter(|&k| SPELLS[k as usize].kind == kind)
        .collect();
    let cols = 2;
    let cw = (pw - 20.0 * u - 6.0 * u) / cols as f32;
    let rh = 22.0 * u;
    for (n, &sp) in list.iter().enumerate() {
        let b = Rect::new(
            x0 + (n % cols) as f32 * (cw + 6.0 * u),
            y + (n / cols) as f32 * (rh + 4.0 * u),
            cw,
            rh,
        );
        let have = own.slots[sel].is_some_and(|s| s.0 == sp);
        c.round_rect(b, 3.0 * u, if have { LIT } else { BUTTON });
        icon(
            c,
            sp,
            b.x + 3.0 * u,
            b.y + 3.0 * u,
            rh - 6.0 * u,
            rgba(colour(sp)),
        );
        c.text_shadowed(
            (b.x + rh + 2.0 * u) as i32,
            (b.y + rh / 2.0) as i32 - 3 * ui,
            SPELLS[sp as usize].name,
            ui,
            if have { GOLD } else { INK },
        );
        spots.0.push((b, Act::Spell(sp)));
    }
    y += ((list.len() + 1) / cols) as f32 * (rh + 4.0 * u) + 6.0 * u;
    if let Some((sp, _)) = own.slots[sel] {
        let what = SPELLS[sp as usize].what;
        let k = pixels::fit_scale(what, pw as i32 - 20 * ui, ui);
        c.text_shadowed(x0 as i32, y as i32, what, k, DIM);
    }
    y += 14.0 * u;
    // Your level, and the rules.
    c.text_shadowed(
        x0 as i32,
        y as i32 + 4 * ui,
        &format!("level {}", own.level),
        ui,
        INK,
    );
    button(
        c,
        spots,
        Rect::new(x0 + 60.0 * u, y, 20.0 * u, 16.0 * u),
        "-",
        false,
        Act::Level(-1),
        ui,
    );
    button(
        c,
        spots,
        Rect::new(x0 + 84.0 * u, y, 20.0 * u, 16.0 * u),
        "+",
        false,
        Act::Level(1),
        ui,
    );
    button(
        c,
        spots,
        Rect::new(x0 + 108.0 * u, y, 30.0 * u, 16.0 * u),
        "max",
        own.level >= MAX_LEVEL,
        Act::Level(20),
        ui,
    );
    y += 22.0 * u;
    let tw = (pw - 26.0 * u) / 2.0;
    button(
        c,
        spots,
        Rect::new(x0, y, tw, 18.0 * u),
        "no cooldowns",
        rules.0,
        Act::NoCooldowns,
        ui,
    );
    button(
        c,
        spots,
        Rect::new(x0 + tw + 6.0 * u, y, tw, 18.0 * u),
        "dummies fight back",
        rules.1,
        Act::Sparring,
        ui,
    );
}

/// The online lobby: who is here, waiting for the match.
pub fn lobby(c: &mut Canvas, ui: i32, names: &[String]) {
    let w = c.w;
    let mut y = 30 * ui;
    let head = format!("{} in the lobby - warm up, nothing hurts here", names.len());
    let k = pixels::fit_scale(&head, w - 20 * ui, ui);
    c.text_centred(w / 2, y, &head, k, DIM);
    y += 11 * ui;
    let line = names
        .iter()
        .take(12)
        .cloned()
        .collect::<Vec<_>>()
        .join("   ");
    for l in pixels::wrap(&line, w * 2 / 3, ui) {
        c.text_centred(w / 2, y, &l, ui, INK);
        y += 10 * ui;
    }
}
