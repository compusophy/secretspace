//! How your match went, when you are out or it is won: your place (or
//! victory), what took you, your knockouts and level, and what comes next.

use pixels::{Canvas, Rect, Rgba};
use wandfall::laws::MATCH_SIZE;
use wandfall::proto::Frame;

use super::{View, DIM, GOLD, INK};
use crate::bar;

/// The card, its top no higher than `top` (under the top line).
pub(super) fn draw(c: &mut Canvas, v: &View, f: &Frame, won: bool, top: i32) {
    let (w, h, ui) = (c.w, c.h, v.ui);
    let u = ui as f32;
    let (cw, ch) = ((250 * ui).min(w - 16 * ui), 104 * ui);
    let b = Rect::new(
        ((w - cw) / 2) as f32,
        (h / 2 - ch + 10 * ui).max(top) as f32,
        cw as f32,
        ch as f32,
    );
    c.round_rect(b, 8.0 * u, Rgba(8, 10, 22, 215));
    c.round_rect_line(b, 8.0 * u, u, if won { GOLD } else { DIM.fade(0.5) });
    let cx = w / 2;
    let mut y = b.y as i32 + 10 * ui;
    let (by, place) = v.st.out.unwrap_or((0, 1));
    let of = f.entrants.max(MATCH_SIZE as u8);
    let title = if won {
        "victory!".to_string()
    } else {
        format!("#{place} of {of}")
    };
    c.text_centred(cx, y, &title, 3 * ui, if won { GOLD } else { INK });
    y += 28 * ui;
    if won {
        c.text_centred(cx, y, "the last wizard standing", ui, INK);
    } else if by == 0 {
        c.text_centred(cx, y, "the storm took you", ui, INK);
    } else {
        // Knocked out by whom, with what.
        let line = format!("knocked out by {}", v.st.name(by));
        let tw = pixels::text_width(&line, ui) + 14 * ui;
        let x = cx - tw / 2;
        c.text_shadowed(x, y, &line, ui, INK);
        let at = (x + tw - 10 * ui) as f32;
        bar::icon(
            c,
            v.st.out_with,
            Rect::new(at, (y - 2 * ui) as f32, 10.0 * u, 10.0 * u),
        );
    }
    y += 16 * ui;
    if let Some(o) = &v.st.last_own {
        let stats = format!("knockouts {}     level {}", o.kills, o.level);
        c.text_centred(cx, y, &stats, ui, GOLD);
    }
    y += 16 * ui;
    let next = if f.phase == 2 {
        format!("the next match in {}", f.secs)
    } else {
        "the next match starts when this one ends".to_string()
    };
    // Wrapped, not shrunk: on a phone it is two lines at the size of the
    // rest.
    for line in pixels::wrap(&next, cw - 12 * ui, ui) {
        c.text_centred(cx, y, &line, ui, DIM);
        y += 10 * ui;
    }
}
