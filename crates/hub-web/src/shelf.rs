//! Where everything goes on the front page, from the screen's size alone:
//! the name and its line, the cards (side by side as many as fit, one
//! under another on a phone held upright) and the footer. On a short
//! screen (a phone on its side) the name is smaller and the cards shorter
//! and narrower, two to a row; on a tall one the lot sits a little above
//! the middle rather than at the top with the floor left empty.

use pixels::{fit_scale, text_width, Rect};

pub const TITLE: &str = "SECRETSPACE";
/// The line under the name.
pub const TAGLINE: &str = "tiny games, everyone in them";
/// How it is made, along the bottom: whole where it fits, else shorter
/// (on most phones: a 412-px Android's buffer is 361 pixels wide).
pub const MADE: [&str; 2] = [
    "all rust - no javascript written",
    "all rust - no js written",
];

/// The first of `lines` that fits in `room` pixels at text scale `u`, at
/// `u`; else the last, as big as it fits.
pub fn fitted<'a>(lines: &[&'a str], room: i32, u: i32) -> (&'a str, i32) {
    let last = lines.last().copied().unwrap_or("");
    match lines.iter().find(|l| text_width(l, u) <= room) {
        Some(l) => (l, u),
        None => (last, fit_scale(last, room, u)),
    }
}

/// The room a line across a `w`-pixel page has at text scale `u`: all but
/// a margin each side.
pub fn across(w: f32, u: i32) -> i32 {
    (w - 8.0 * u as f32) as i32
}

/// The page laid out, before it is scrolled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shelf {
    /// The name's text scale, and its top.
    pub title_scale: i32,
    pub title_y: f32,
    /// The top of the line under the name.
    pub tag_y: f32,
    /// The first card's top left, each card's size and its picture's
    /// height, the gap between cards, and how many to a row.
    pub x0: f32,
    pub top: f32,
    pub card: (f32, f32),
    pub preview_h: f32,
    pub gap: f32,
    pub cols: usize,
    /// The footer's height (it stays put; the rest scrolls under it).
    pub foot_h: f32,
    /// How far down what scrolls goes.
    pub bottom: f32,
}

/// The page laid out on a `w` by `ht` screen at text scale `u`, for `n`
/// cards.
pub fn shelf(w: f32, ht: f32, u: i32, n: usize) -> Shelf {
    let uf = u as f32;
    let narrow = w / uf < 420.0;
    let short = ht / uf < 300.0;
    let foot_h = 34.0 * uf;
    // The name: as wide as the screen allows, and no taller than a
    // seventieth of its height a font pixel.
    let most = (5 * u).min((ht / 70.0) as i32).max(u);
    let title_scale = fit_scale(TITLE, (w - 16.0 * uf) as i32, most);
    let above = if short { 10.0 } else { 18.0 } * uf;
    let to_tag = 8.0 * title_scale as f32 + 6.0 * uf;
    let to_cards = to_tag + if short { 16.0 } else { 22.0 } * uf;

    let gap = 12.0 * uf;
    let preview_h = if narrow || short { 64.0 } else { 104.0 } * uf;
    let ch = preview_h + 84.0 * uf;
    let room = w - 24.0 * uf;
    let (cw, cols) = if narrow {
        (room.min(360.0 * uf), 1)
    } else {
        // A short screen's cards may be a little narrower, to sit two to
        // a row rather than one.
        let least = if short { 180.0 } else { 196.0 } * uf;
        let cols = (((room + gap) / (least + gap)).floor() as usize).clamp(1, n.max(1));
        let fill = (room - (cols - 1) as f32 * gap) / cols as f32;
        let cw = if cols == 1 {
            room.min(360.0 * uf)
        } else {
            fill.min(196.0 * uf)
        };
        (cw, cols)
    };
    let rows = n.div_ceil(cols).max(1);
    let grid_h = rows as f32 * ch + (rows - 1) as f32 * gap;
    let block = to_cards + grid_h + 8.0 * uf;
    // Room to spare: the block a little above the middle of it.
    let spare = ht - foot_h - above - block;
    let title_y = above + if spare > 0.0 { spare * 0.4 } else { 0.0 };
    let grid_w = cols as f32 * cw + (cols - 1) as f32 * gap;
    Shelf {
        title_scale,
        title_y,
        tag_y: title_y + to_tag,
        x0: (w - grid_w) / 2.0,
        top: title_y + to_cards,
        card: (cw, ch),
        preview_h,
        gap,
        cols,
        foot_h,
        bottom: title_y + block,
    }
}

impl Shelf {
    /// How far into a card its words start, and how wide they may run (to
    /// a little short of its right edge), at text scale `u`.
    pub fn words(&self, u: i32) -> (f32, i32) {
        let uf = u as f32;
        (9.0 * uf, (self.card.0 - 11.0 * uf) as i32)
    }

    /// Where card `slot` goes, before scrolling.
    pub fn at(&self, slot: usize) -> Rect {
        let (col, row) = (slot % self.cols, slot / self.cols);
        Rect::new(
            self.x0 + col as f32 * (self.card.0 + self.gap),
            self.top + row as f32 * (self.card.1 + self.gap),
            self.card.0,
            self.card.1,
        )
    }

    /// The most the page scrolls on a screen `ht` high.
    pub fn max_scroll(&self, ht: f32) -> f32 {
        (self.bottom + self.foot_h - ht).max(0.0)
    }

    /// The scroll that shows card `slot` whole (or its top, if it cannot
    /// be), from `scroll`, on a screen `ht` high.
    pub fn show(&self, slot: usize, scroll: f32, ht: f32) -> f32 {
        let b = self.at(slot);
        let margin = self.gap;
        let floor = ht - self.foot_h - margin;
        let s = if b.y + b.h - scroll > floor {
            b.y + b.h - floor
        } else {
            scroll
        };
        let s = if b.y - s < margin { b.y - margin } else { s };
        s.clamp(0.0, self.max_scroll(ht))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: usize = 3;

    #[test]
    fn a_wide_screen_puts_the_cards_side_by_side_and_centres_the_lot() {
        // 1920x1080 (960x540 buffer pixels) and 1280x800 (640x400).
        for (w, ht) in [(960.0, 540.0), (640.0, 400.0)] {
            let s = shelf(w, ht, 1, N);
            assert_eq!(s.cols, 3);
            assert_eq!(s.max_scroll(ht), 0.0);
            // As much room under the cards as there is over the name, or
            // more: not a shelf hugging the top over an empty floor.
            let under = ht - s.foot_h - s.bottom;
            assert!(s.title_y >= 18.0 && under >= s.title_y * 0.9, "{w}: {s:?}");
            assert!(under <= s.title_y * 2.0, "{w}: {s:?}");
        }
    }

    #[test]
    fn a_phone_on_its_side_gets_a_smaller_name_and_two_cards_a_row() {
        // 844x390 at text scale 2.
        let s = shelf(844.0, 390.0, 2, N);
        assert!(8 * s.title_scale <= 40, "{s:?}");
        assert_eq!(s.cols, 2);
        // The first row's pictures show above the footer.
        assert!(s.top + s.preview_h < 390.0 - s.foot_h, "{s:?}");
        // Every card fits across.
        assert!(s.at(1).x + s.at(1).w <= 844.0 - 12.0);
    }

    #[test]
    fn a_phone_held_upright_stacks_the_cards_and_scrolls() {
        let s = shelf(390.0, 844.0, 2, N);
        assert_eq!(s.cols, 1);
        assert!(s.card.0 <= 390.0 - 48.0);
        assert!(s.max_scroll(844.0) > 0.0);
        // Showing the last card scrolls to it; the first needs none.
        assert_eq!(s.show(0, 0.0, 844.0), 0.0);
        let to = s.show(N - 1, 0.0, 844.0);
        let b = s.at(N - 1);
        assert!(b.y + b.h - to <= 844.0 - s.foot_h);
    }
}
