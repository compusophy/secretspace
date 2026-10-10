//! The title: the game's name, your name, play online or the practice
//! range, a word on how to play (or why the island is not there just
//! now), what is new, and the spells. What does not fit a short screen is
//! let go, the spells and the news first; the name takes what room is
//! left.

use pixels::{Canvas, Rect};
use wandfall::laws::spell;

use super::{button, Act, Spots, DIM, GOLD, INK};
use crate::bar::icon;

/// What is new, under the buttons.
const NEWS: &str = "new: the basalt causeway, the Tether, wall jumps, weather";
const HOW: &str =
    "drop on the island, grab spell cubes, outlast the storm: last wizard standing wins";

/// What the title shows of the page: the name being written (and whether
/// it is: a caret), the name asked for being someone else's, a word in
/// place of how to play, and a line at its foot.
pub struct Title<'a> {
    pub name: &'a str,
    pub typing: bool,
    pub taken: bool,
    pub notice: Option<&'a str>,
    pub note: &'a str,
    pub now: f64,
}

/// `s` wrapped no wider than `width`, its lines as even as they can be
/// (no word left alone on the last).
fn even(s: &str, width: i32, k: i32) -> Vec<String> {
    let n = pixels::wrap(s, width, k).len();
    let mut w = width;
    while w > 0 && pixels::wrap(s, w - 6 * k, k).len() == n {
        w -= 6 * k;
    }
    pixels::wrap(s, w, k)
}

/// The title; where your name's box is (the page lays the field there).
pub fn title(c: &mut Canvas, spots: &mut Spots, ui: i32, t: &Title) -> Rect {
    let (w, h) = (c.w, c.h);
    c.fill_rect(0, 0, w, h, pixels::Rgba(6, 8, 18, 110));
    let u = ui as f32;
    let name = "WANDFALL";
    let sub = "a wand battle royale";
    let wide = (w - 16 * ui).min(300 * ui);
    let words = even(t.notice.unwrap_or(HOW), wide, ui);
    let news = even(NEWS, wide, ui);
    // The spells' icons, and their labels' room beside them.
    let s = (22 * ui).min((w - 24 * ui) / 10);
    let rows = 6 * ui + 2 * (s + s / 4);
    // Under the name, always: the subtitle, your name and the buttons (and
    // a notice); then, as room allows, the word on how to play, the
    // spells, the news, the foot line. The name at least `least` tall,
    // or they give way to it.
    let bh = if h >= 240 * ui { 30 * ui } else { 24 * ui };
    let ks = pixels::fit_scale(sub, w - 16 * ui, 2 * ui);
    let (least, room) = (36 * ui, h - 12 * ui);
    let mut used = 9 * ks + 10 * ui + 18 * ui + 8 * ui + 2 * bh + 10 * ui + 12 * ui;
    let mut fits = |tall: i32, must: bool| {
        let ok = must || used + tall + least <= room;
        if ok {
            used += tall;
        }
        ok
    };
    let how = fits(words.len() as i32 * 11 * ui, t.notice.is_some());
    let spells = fits(rows, false);
    let show_news = t.notice.is_none() && fits(4 * ui + news.len() as i32 * 11 * ui, false);
    let foot = fits(16 * ui, false);
    let k = pixels::fit_scale(name, w - 20 * ui, 7 * ui)
        .min((room - used) / 9)
        .max(ui);
    // Never a subtitle bigger than half the name.
    let ks = ks.min((k / 2).max(ui));
    let mut y = ((h - 9 * k - used) / 2).max(6 * ui);
    c.text_shadowed((w - pixels::text_width(name, k)) / 2, y, name, k, GOLD);
    y += 9 * k;
    c.text_centred(w / 2, y, sub, ks, INK);
    y += 9 * ks + 10 * ui;
    // Your name: a box the page's field lies over (a tap writes in it).
    let bw = (220 * ui).min(w - 24 * ui) as f32;
    let x = (w as f32 - bw) / 2.0;
    let field = Rect::new(x, y as f32, bw, 18.0 * u);
    c.round_rect(field, 4.0 * u, pixels::Rgba(0, 0, 0, 120));
    c.round_rect_line(
        field,
        4.0 * u,
        1.0,
        if t.typing {
            GOLD.fade(0.7)
        } else {
            DIM.fade(0.5)
        },
    );
    let ty = (field.y + field.h / 2.0) as i32 - 3 * ui;
    if t.name.is_empty() && !t.typing {
        c.text_centred(w / 2, ty, "your name (tap to write it)", ui, DIM);
    } else {
        let blink = t.typing && ((t.now / 500.0) as u64).is_multiple_of(2);
        let shown = format!("{}{}", t.name, if blink { "_" } else { " " });
        c.text_centred(w / 2, ty, &shown, ui, INK);
    }
    y += 18 * ui + 8 * ui;
    let at = |y: i32| Rect::new(x, y as f32, bw, bh as f32);
    button(c, spots, at(y), "play online", true, Act::Online, ui);
    y += bh + 10 * ui;
    button(c, spots, at(y), "practice range", false, Act::Practice, ui);
    y += bh + 12 * ui;
    // The name asked for is someone else's: said in place of how to play.
    let taken = ["that name is someone else's: pick another".to_string()];
    let (lines, ink) = match (t.taken, t.notice) {
        (true, _) => (&taken[..], GOLD),
        (_, Some(_)) => (&words[..], GOLD),
        _ => (&words[..], DIM),
    };
    if how || t.taken {
        for line in lines {
            let k = pixels::fit_scale(line, w - 16 * ui, ui);
            c.text_centred(w / 2, y, line, k, ink);
            y += 11 * ui;
        }
    }
    if show_news {
        y += 4 * ui;
        for line in &news {
            c.text_centred(w / 2, y, line, ui, GOLD.fade(0.9));
            y += 11 * ui;
        }
    }
    if spells {
        spell_rows(c, ui, s, y + 6 * ui);
    }
    if foot {
        let k = pixels::fit_scale(t.note, w - 16 * ui, ui);
        c.text_centred(w / 2, h - 14 * ui, t.note, k, DIM.fade(0.7));
    }
    field
}

/// The spells, a row of what you hurt with and one of what you live by,
/// each with its label to its left, label and row laid out as one (the
/// labels above the rows if the screen is too narrow for both).
fn spell_rows(c: &mut Canvas, ui: i32, s: i32, mut y: i32) {
    let w = c.w;
    let gap = s / 4;
    let list = [
        ("to hurt", &spell::OFFENSE[..]),
        ("to live", &spell::UTILITY[..]),
    ];
    let label = list
        .iter()
        .map(|l| pixels::text_width(l.0, ui))
        .max()
        .unwrap_or(0);
    let most = list.iter().map(|l| l.1.len()).max().unwrap_or(0) as i32;
    let row_w = most * s + (most - 1) * gap;
    let beside = label + 8 * ui + row_w <= w - 16 * ui;
    let x0 = if beside {
        (w - (label + 8 * ui + row_w)) / 2 + label + 8 * ui
    } else {
        (w - row_w) / 2
    };
    for (name, row) in list {
        if beside {
            let lx = x0 - 8 * ui - pixels::text_width(name, ui);
            c.text_shadowed(lx, y + s / 2 - 3 * ui, name, ui, DIM);
        } else {
            c.text_centred(w / 2, y, name, ui, DIM);
            y += 10 * ui;
        }
        for (k, &sp) in row.iter().enumerate() {
            let b = Rect::new(
                (x0 + k as i32 * (s + gap)) as f32,
                y as f32,
                s as f32,
                s as f32,
            );
            icon(c, sp, b);
        }
        y += s + gap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blurb_wraps_evenly() {
        let lines = even(HOW, 300, 1);
        let plain = pixels::wrap(HOW, 300, 1);
        assert_eq!(lines.len(), plain.len());
        let widths: Vec<i32> = lines.iter().map(|l| pixels::text_width(l, 1)).collect();
        let (lo, hi) = (widths.iter().min().unwrap(), widths.iter().max().unwrap());
        assert!(hi - lo < 120, "{lines:?}");
        assert!(
            lines.last().unwrap().contains(' '),
            "no word alone: {lines:?}"
        );
    }

    #[test]
    fn a_short_screen_keeps_the_name_and_the_buttons() {
        for (w, h, ui) in [(844, 390, 2), (740, 360, 2), (960, 540, 1), (390, 844, 2)] {
            let mut c = Canvas::new(w, h);
            let t = Title {
                name: "",
                typing: false,
                taken: false,
                notice: Some("the island is being updated: try again in a minute"),
                note: "a note",
                now: 0.0,
            };
            let mut spots = Spots::default();
            let field = title(&mut c, &mut spots, ui, &t);
            // The name's box and both buttons on the screen, the box
            // under the game's name (its gold at the top).
            assert!(field.y > 40.0 && field.y + field.h < h as f32, "{w}x{h}");
            assert_eq!(spots.0.len(), 2);
            for (r, _) in &spots.0 {
                assert!(r.y + r.h <= h as f32, "{w}x{h}: {r:?}");
            }
            let gold = (0..field.y as i32).any(|y| {
                (0..w).any(|x| c.data[((y * w + x) * 4) as usize..][..3] == [255, 214, 128])
            });
            assert!(gold, "{w}x{h}: the name");
        }
    }
}
