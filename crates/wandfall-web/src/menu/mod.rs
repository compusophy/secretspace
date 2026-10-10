//! The menus, drawn in pixels: the title (`title`: your name, play
//! online, or the practice range), the settings over the shared menu,
//! the spellbook (`book`; on the range its tools: any spell at any rank
//! in any slot, your level, no cooldowns, sparring) and the online
//! lobby's panel. Each button is a spot the page hit-tests.

use pixels::{Canvas, Rect, Rgba};

use crate::settings::{Settings, PICTURES};

mod book;
mod title;
pub use book::book;
pub use title::{title, Title};

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
    /// Leave the match (or the range) for the title.
    Leave,
    /// The settings panel, over the shared menu, and back from it.
    Settings,
    CloseSettings,
    Book,
    CloseBook,
    Slot(usize),
    Spell(u8),
    Rank(u8),
    /// Your level a step up or down, or to the most there is.
    Level(i8),
    MaxLevel,
    NoCooldowns,
    Sparring,
    /// The range's lessons from the start.
    Lessons,
    /// Settings: the view's turning speed and the sound's loudness a
    /// step up or down; a picture.
    Look(i8),
    Volume(i8),
    Picture(u8),
    /// A panel's own ground: a click there does nothing (off every panel,
    /// a click goes back to the game).
    Stay,
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

/// The keys, and the moves they make, for the menu's help.
const KEYS_HELP: [&str; 16] = [
    "WASD move",
    "shift sprint",
    "space jump",
    "space in the air: jump again",
    "space at a ledge: climb",
    "space by a wall: jump off it",
    "jump as you land: hop on",
    "C or ctrl crouch",
    "crouch at a sprint: slide",
    "click cast",
    "right click aim",
    "Q E R F spells",
    "F at a rock or a tree: tether",
    "B spellbook",
    "M sound",
    "Esc menu",
];

/// The keys, in lines no wider than `width`, each holding whole ones (one
/// wider than a line is wrapped).
pub fn keys_help(width: i32, ui: i32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for k in KEYS_HELP {
        match lines.last_mut() {
            Some(l) if pixels::text_width(&format!("{l} - {k}"), ui) <= width => {
                l.push_str(" - ");
                l.push_str(k);
            }
            _ => lines.extend(pixels::wrap(k, width, ui)),
        }
    }
    lines
}

/// The settings, over the shared menu (its "settings"): how fast the view
/// turns, how loud, the picture; and back to the menu.
pub fn settings_panel(c: &mut Canvas, spots: &mut Spots, ui: i32, set: &Settings) {
    let (w, h) = (c.w, c.h);
    let u = ui as f32;
    let bw = (220 * ui).min(w - 32 * ui) as f32;
    let tall = (12.0 + 26.0 + SETTINGS + 10.0 + 22.0 + 12.0) * u;
    let panel = Rect::new(
        (w as f32 - bw) / 2.0 - 14.0 * u,
        ((h as f32 - tall) / 2.0).max(6.0 * u),
        bw + 28.0 * u,
        tall,
    );
    c.fill_rect(0, 0, w, h, Rgba(4, 6, 14, 150));
    c.round_rect(panel, 7.0 * u, PANEL);
    spots.0.push((panel, Act::Stay));
    let x = (w as f32 - bw) / 2.0;
    let mut y = panel.y + 12.0 * u;
    c.text_centred(w / 2, y as i32, "settings", 2 * ui, GOLD);
    y += 26.0 * u;
    settings(c, spots, (x, y, bw), ui, set);
    y += (SETTINGS + 10.0) * u;
    button(
        c,
        spots,
        Rect::new(x, y, bw, 22.0 * u),
        "back",
        false,
        Act::CloseSettings,
        ui,
    );
}

/// How tall the settings are (in ui units).
const SETTINGS: f32 = 3.0 * 24.0 + 4.0;

/// The settings, a row each: how fast the view turns, how loud, the
/// picture.
fn settings(
    c: &mut Canvas,
    spots: &mut Spots,
    (x, y, bw): (f32, f32, f32),
    ui: i32,
    set: &Settings,
) {
    let u = ui as f32;
    let h = 18.0 * u;
    let label = |c: &mut Canvas, y: f32, t: &str| {
        c.text_shadowed(x as i32, (y + h / 2.0) as i32 - 3 * ui, t, ui, DIM);
    };
    let step = |c: &mut Canvas, spots: &mut Spots, y: f32, value: &str, acts: (Act, Act)| {
        let w = 22.0 * u;
        button(
            c,
            spots,
            Rect::new(x + bw - 92.0 * u, y, w, h),
            "-",
            false,
            acts.0,
            ui,
        );
        c.text_centred(
            (x + bw - 46.0 * u) as i32,
            (y + h / 2.0) as i32 - 3 * ui,
            value,
            ui,
            INK,
        );
        button(
            c,
            spots,
            Rect::new(x + bw - w, y, w, h),
            "+",
            false,
            acts.1,
            ui,
        );
    };
    label(c, y, "look speed");
    step(
        c,
        spots,
        y,
        &format!("{:.2}x", set.look),
        (Act::Look(-1), Act::Look(1)),
    );
    let y = y + 24.0 * u;
    label(c, y, "sound");
    let loud = format!("{:.0}%", set.volume * 100.0);
    step(c, spots, y, &loud, (Act::Volume(-1), Act::Volume(1)));
    let y = y + 24.0 * u;
    label(c, y, "picture");
    let x0 = x + 56.0 * u;
    let gap = 4.0 * u;
    let bwk = (bw - 56.0 * u - 3.0 * gap) / 4.0;
    for (k, name) in PICTURES.iter().enumerate() {
        let b = Rect::new(x0 + k as f32 * (bwk + gap), y, bwk, h);
        button(
            c,
            spots,
            b,
            name,
            set.picture == k as u8,
            Act::Picture(k as u8),
            ui,
        );
    }
}

/// The online lobby: who is here, waiting for the match, in the band the
/// HUD keeps for it; the hall of wizards under them, down the left (on a
/// touch screen at `hall_at`, under your health).
pub fn lobby(
    c: &mut Canvas,
    ui: i32,
    names: &[String],
    hall: &[(String, u32, u32)],
    band: Rect,
    hall_at: Option<(i32, i32)>,
) {
    let (cx, bw) = ((band.x + band.w / 2.0) as i32, band.w as i32);
    let mut y = band.y as i32 + 2 * ui;
    let head = format!("{} in the lobby - warm up, nothing hurts here", names.len());
    for l in pixels::wrap(&head, bw, ui) {
        c.text_centred(cx, y, &l, ui, DIM);
        y += 10 * ui;
    }
    y += ui;
    let line = names
        .iter()
        .take(12)
        .cloned()
        .collect::<Vec<_>>()
        .join("   ");
    for l in pixels::wrap(&line, bw, ui).into_iter().take(3) {
        c.text_centred(cx, y, &l, ui, INK);
        y += 10 * ui;
    }
    // The hall of wizards, down the left.
    if hall.is_empty() {
        return;
    }
    let rows = hall.len().min(5) as i32;
    let (px, py, pw) = match hall_at {
        Some((x, y)) => (x, y, 140 * ui),
        None => (8 * ui, y + 8 * ui, (164 * ui).min(c.w / 2 - 12 * ui)),
    };
    let panel = Rect::new(
        px as f32,
        py as f32,
        pw as f32,
        (18 * ui + rows * 10 * ui) as f32,
    );
    c.round_rect(panel, 4.0 * ui as f32, PANEL);
    let (x, mut y) = (panel.x as i32 + 6 * ui, panel.y as i32 + 5 * ui);
    // Two columns at the right: matches won, wizards knocked out.
    let right = panel.x as i32 + pw - 6 * ui;
    let col = |c: &mut Canvas, edge: i32, y: i32, t: &str, ink: Rgba| {
        c.text_shadowed(edge - pixels::text_width(t, ui), y, t, ui, ink);
    };
    c.text_shadowed(x, y, "hall of wizards", ui, GOLD);
    col(c, right - 28 * ui, y, "won", DIM);
    col(c, right, y, "ko", DIM);
    y += 12 * ui;
    let room = pw - 12 * ui - 10 * ui - 28 * ui - 30 * ui;
    for (n, (name, wins, outs)) in hall.iter().take(5).enumerate() {
        let mut shown = name.clone();
        while pixels::text_width(&shown, ui) > room && shown.pop().is_some() {}
        c.text_shadowed(x, y, &format!("{}", n + 1), ui, DIM);
        c.text_shadowed(x + 10 * ui, y, &shown, ui, INK);
        col(c, right - 28 * ui, y, &wins.to_string(), INK);
        col(c, right, y, &outs.to_string(), INK);
        y += 10 * ui;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keys_help_keeps_inside_its_panel() {
        for (width, ui) in [(210, 1), (238, 1), (420, 2), (160, 1)] {
            let lines = keys_help(width, ui);
            for l in &lines {
                assert!(pixels::text_width(l, ui) <= width, "{width}: {l}");
            }
            // Every key is in it, whole where it fits a line.
            let all = lines.join(" - ");
            for k in KEYS_HELP {
                assert!(all.contains(k) || pixels::text_width(k, ui) > width, "{k}");
            }
        }
    }
}
