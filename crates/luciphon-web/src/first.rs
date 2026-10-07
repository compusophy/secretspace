//! The first screen: the name (one name everywhere), PLAY, the five
//! gestures in a line each, your recovery words to write down, and a
//! field to restore a soul from its words. And once playing, the ghost-
//! thumb hints, one gesture at a time until you have done each.

use lucilook::palette::{DARK, DIM, GOLD, INK};
use pixels::{fit_scale, text_width, wrap, Canvas, Rect, Rgba};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Panel {
    #[default]
    Title,
    Words,
    Restore,
}

/// Where the screen's parts were drawn, for presses and the fields.
#[derive(Clone, Copy, Debug, Default)]
pub struct Spots {
    pub name: Rect,
    pub play: Rect,
    pub words: Rect,
    pub restore: Rect,
    pub back: Rect,
    pub field: Rect,
    pub go: Rect,
    pub home: Rect,
}

pub struct Look<'a> {
    pub u: i32,
    pub panel: Panel,
    pub name: &'a str,
    pub editing: bool,
    pub taken: bool,
    pub words: &'a str,
    pub restore: &'a str,
    pub restore_bad: bool,
    pub connected: bool,
    pub people: u16,
    pub awake: u16,
    pub touch: bool,
}

fn button(c: &mut Canvas, r: Rect, label: &str, s: i32, lit: bool) {
    c.round_rect(r, 5.0, if lit { GOLD } else { Rgba(255, 255, 255, 22) });
    let ink = if lit { Rgba::rgb(40, 26, 8) } else { INK };
    let w = text_width(label, s);
    c.text(
        (r.x + r.w / 2.0) as i32 - w / 2,
        (r.y + r.h / 2.0) as i32 - 4 * s + s / 2,
        label,
        s,
        ink,
    );
}

fn field(c: &mut Canvas, r: Rect, text: &str, hint: &str, editing: bool, s: i32, now: f64) {
    c.round_rect(r, 5.0, Rgba(255, 255, 255, 16));
    c.round_rect_line(
        r,
        5.0,
        1.0,
        if editing {
            GOLD
        } else {
            Rgba(255, 255, 255, 50)
        },
    );
    let (shown, ink) = if text.is_empty() {
        (hint, Rgba(244, 238, 222, 80))
    } else {
        (text, INK)
    };
    let s = fit_scale(shown, (r.w - 10.0) as i32, s);
    let w = text_width(shown, s);
    let (x, y) = (
        (r.x + r.w / 2.0) as i32 - w / 2,
        (r.y + r.h / 2.0) as i32 - 4 * s + s / 2,
    );
    c.text(x, y, shown, s, ink);
    if editing && (now / 500.0) as i64 % 2 == 0 {
        let cx = if text.is_empty() {
            (r.x + r.w / 2.0) as i32
        } else {
            x + w + s
        };
        c.fill_rect(cx, y - s / 2, s.max(1), 8 * s, GOLD);
    }
}

fn lines(c: &mut Canvas, y: f32, s: &str, u: i32, ink: Rgba) -> f32 {
    let mut y = y;
    for line in wrap(s, c.w - 24 * u, u) {
        c.text_centred(c.w / 2, y as i32, &line, u, ink);
        y += 10.0 * u as f32;
    }
    y
}

pub fn draw(c: &mut Canvas, l: &Look, now: f64) -> Spots {
    let (u, uf) = (l.u, l.u as f32);
    let w = c.w as f32;
    c.fill_rect(0, 0, c.w, c.h, Rgba(11, 13, 26, 170));
    let mut s = Spots::default();
    let panel_w = (w - 32.0 * uf).min(240.0 * uf);
    let x = (w - panel_w) / 2.0;
    // Back to every game.
    s.home = Rect::new(
        8.0 * uf,
        8.0 * uf,
        text_width("< ALL GAMES", u) as f32 + 12.0 * uf,
        16.0 * uf,
    );
    c.round_rect(s.home, 5.0, Rgba(255, 255, 255, 18));
    c.text(
        s.home.x as i32 + 6 * u,
        s.home.y as i32 + 4 * u,
        "< ALL GAMES",
        u,
        DIM,
    );

    let mut y = (c.h as f32 * 0.16).max(30.0 * uf);
    let title = "LUCIPHON";
    let ts = fit_scale(title, (w - 24.0 * uf) as i32, 5 * u);
    let glow = (180.0 + (now / 600.0).sin() * 60.0) as u8;
    c.glow_add(
        c.w / 2,
        y as i32 + 4 * ts,
        (text_width(title, ts) / 2).max(10),
        Rgba(255, 210, 122, glow / 3),
    );
    c.text_centred(c.w / 2, y as i32, title, ts, GOLD);
    y += 10.0 * ts as f32 + 6.0 * uf;
    y = lines(
        c,
        y,
        "carry your light out, knock them off the edge",
        u,
        DIM,
    );
    y += 10.0 * uf;

    match l.panel {
        Panel::Title => {
            s.name = Rect::new(x, y, panel_w, 24.0 * uf);
            field(c, s.name, l.name, "your name", l.editing, 2 * u, now);
            y += s.name.h + 6.0 * uf;
            if l.taken {
                y = lines(
                    c,
                    y,
                    "that name is taken - pick another",
                    u,
                    Rgba::rgb(255, 140, 110),
                );
                y += 2.0 * uf;
            }
            s.play = Rect::new(x, y, panel_w, 28.0 * uf);
            button(c, s.play, "PLAY", 2 * u, true);
            y += s.play.h + 12.0 * uf;
            let how = if l.touch {
                [
                    "drag: run",
                    "tap: strike",
                    "flick: dash",
                    "hold: heavy, or drag to aim a throw",
                ]
            } else {
                [
                    "drag: run",
                    "click: strike",
                    "flick: dash",
                    "hold: heavy, or drag to aim a throw",
                ]
            };
            for h in how {
                y = lines(c, y, h, u, Rgba(244, 238, 222, 190));
            }
            y = lines(
                c,
                y,
                "the dimmer you are, the further you fly",
                u,
                Rgba(255, 210, 122, 170),
            );
            y += 8.0 * uf;
            let half = (panel_w - 6.0 * uf) / 2.0;
            s.words = Rect::new(x, y, half, 18.0 * uf);
            s.restore = Rect::new(x + half + 6.0 * uf, y, half, 18.0 * uf);
            button(c, s.words, "MY WORDS", u, false);
            button(c, s.restore, "RESTORE", u, false);
            y += 26.0 * uf;
            let line = if l.connected {
                format!("{} here, {} awake on the island", l.people, l.awake)
            } else {
                "reaching the island...".into()
            };
            lines(c, y, &line, u, Rgba(255, 210, 122, 200));
        }
        Panel::Words => {
            y = lines(
                c,
                y,
                "write these down: they are you, on any device",
                u,
                INK,
            );
            y += 6.0 * uf;
            for line in wrap(l.words, (panel_w as i32).max(60), 2 * u) {
                c.text_centred(c.w / 2, y as i32, &line, 2 * u, GOLD);
                y += 20.0 * uf;
            }
            y += 8.0 * uf;
            s.back = Rect::new(x, y, panel_w, 22.0 * uf);
            button(c, s.back, "BACK", 2 * u, false);
        }
        Panel::Restore => {
            y = lines(c, y, "type the eleven words of a soul", u, INK);
            y += 6.0 * uf;
            s.field = Rect::new(x, y, panel_w, 24.0 * uf);
            field(c, s.field, l.restore, "eleven words", true, u, now);
            y += s.field.h + 6.0 * uf;
            if l.restore_bad {
                y = lines(
                    c,
                    y,
                    "those words are not a soul",
                    u,
                    Rgba::rgb(255, 140, 110),
                );
            }
            s.go = Rect::new(x, y, panel_w, 24.0 * uf);
            button(c, s.go, "BECOME THIS SOUL", u, true);
            y += s.go.h + 6.0 * uf;
            s.back = Rect::new(x, y, panel_w, 20.0 * uf);
            button(c, s.back, "BACK", u, false);
        }
    }
    let _ = DARK;
    s
}

/// A ghost thumb showing the next gesture you have not yet made, near
/// where your thumb would be: drag, tap, flick, hold.
pub fn hint(c: &mut Canvas, done: u8, u: i32, now: f64) {
    let (text, kind) = match () {
        _ if done & 1 == 0 => ("drag to run", 0),
        _ if done & 2 == 0 => ("tap to strike", 1),
        _ if done & 4 == 0 => ("flick to dash", 2),
        _ if done & 8 == 0 => ("hold to charge", 3),
        _ => return,
    };
    let (cx, cy) = (c.w / 2, c.h * 3 / 4);
    let t = (now % 1600.0) / 1600.0;
    let (dx, dy, a) = match kind {
        0 => ((t * 40.0) as i32, 0, 200),
        1 => (0, 0, if t < 0.3 { 255 } else { 90 }),
        2 => ((t * t * 90.0) as i32, -(t * t * 30.0) as i32, 200),
        _ => (0, 0, (120.0 + 135.0 * t) as i32),
    };
    c.glow_add(
        cx - 20 * u + dx * u,
        cy + dy * u,
        6 * u,
        Rgba(255, 240, 210, a.min(255) as u8),
    );
    if kind == 3 {
        c.ring(
            (cx - 20 * u) as f32,
            cy as f32,
            (16.0 - 10.0 * t as f32) * u as f32,
            1.5,
            Rgba(255, 210, 122, 200),
        );
    }
    let w = text_width(text, u);
    c.text_shadowed(
        cx - w / 2 + 10 * u,
        cy + 14 * u,
        text,
        u,
        Rgba(244, 238, 222, 210),
    );
}
