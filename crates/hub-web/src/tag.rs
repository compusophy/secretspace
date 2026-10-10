//! The tags in a card's corner: how its game stands, on a dark pill over
//! the live picture, after a beating red dot while it is live. One look
//! for every card, and never wider than the card: a tag that would run
//! off it is cut short.

use pixels::{text_width, Canvas, Rect, Rgba};

const LIVE: Rgba = Rgba::rgb(255, 70, 80);
const BACK: Rgba = Rgba(7, 10, 18, 170);
const INK: Rgba = Rgba(244, 241, 255, 230);

/// `text` as it fits in `room` pixels at scale `u`: whole, or cut and
/// ending in '.'.
pub fn fit(text: &str, room: i32, u: i32) -> String {
    if text_width(text, u) <= room {
        return text.to_string();
    }
    let most = ((room / u.max(1) + 1) / pixels::font::CELL_W).max(1) as usize;
    let mut cut: String = text.chars().take(most - 1).collect();
    cut.truncate(cut.trim_end().len());
    cut.push('.');
    cut
}

/// The room a tag's text has on a card `w` pixels wide.
pub fn room(w: i32, live: bool, u: i32) -> i32 {
    w - (14 + if live { 8 } else { 0 }) * u
}

/// A tag at the top left of `buf`, `row` tags down (0 the first); with a
/// beating dot when `live`.
pub fn tag(buf: &mut Canvas, row: i32, text: &str, live: bool, u: i32, now: f64) {
    let uf = u as f32;
    let (x, y) = (4.0 * uf, (4 + 13 * row) as f32 * uf);
    let dot = if live { 8.0 * uf } else { 0.0 };
    let text = fit(text, room(buf.w, live, u), u);
    let tw = text_width(&text, u) as f32;
    buf.round_rect(
        Rect::new(x, y, tw + dot + 6.0 * uf, 11.0 * uf),
        3.0 * uf,
        BACK,
    );
    if live {
        let beat = 0.55 + 0.45 * ((now / 400.0) as f32).sin().abs();
        buf.circle(x + 5.5 * uf, y + 5.5 * uf, 2.5 * uf, LIVE.fade(beat));
    }
    buf.text(
        (x + 3.0 * uf + dot) as i32,
        (y + 2.0 * uf) as i32,
        &text,
        u,
        INK,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_never_runs_off_its_card() {
        // The narrowest cards' pictures: a desktop's (ui 1) and a 320-px
        // phone's (ui 2).
        for (w, u) in [(182, 1), (244, 2), (314, 2)] {
            for live in [false, true] {
                let r = room(w, live, u);
                for text in [
                    "next match in 30s",
                    "champion abcdefghijklmn",
                    "LIVE  16 of 16 left",
                    "abcdefghijklmn won",
                ] {
                    let t = fit(text, r, u);
                    assert!(text_width(&t, u) <= r, "{w} {u}: {t}");
                }
            }
        }
        assert_eq!(fit("next match in 9s", 182 - 14, 1), "next match in 9s");
        assert_eq!(fit("champion abcdefghijklmn", 100, 2), "champio.");
    }

    #[test]
    fn what_a_card_says_fits_whole_on_a_phone() {
        // A 390-px phone's card picture (ui 2), and a desktop's (ui 1).
        for (w, u) in [(314, 2), (182, 1)] {
            for text in ["champion abcdefghijklmn", "next match in 30s"] {
                assert_eq!(fit(text, room(w, false, u), u), text, "{w}");
            }
            assert_eq!(
                fit("LIVE  16 of 16 left", room(w, true, u), u),
                "LIVE  16 of 16 left"
            );
        }
    }
}
