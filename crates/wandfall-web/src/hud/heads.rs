//! Over heads: the name, level and health of each wizard near enough to
//! read and in sight (a name never gives away a wizard behind a hill or
//! a wall), and the numbers your hits do, rising off whom they struck.

use pixels::{Canvas, Rect, Rgba};
use render::m4;
use wandfall::laws::{CROUCH_HEIGHT, HEIGHT};
use wandfall::loot::max_hp;
use wandfall::map::Map;
use wandfall::proto::{flag, Seen};

use super::{View, GOLD, INK, RED, SHADE};
use crate::bar;

/// Metres off that a name over a head still shows (it fades from
/// `NAMES_FADE` on), and how long it fades once out of sight (ms).
pub const NAMES: f32 = 45.0;
const NAMES_FADE: f32 = 30.0;
const OUT_OF_SIGHT: f64 = 400.0;
const AMBER: Rgba = Rgba::rgb(240, 200, 90);

/// Whether the wizard `s` can be seen from `eye`: its head or its middle
/// not behind the ground, a deck or anything standing.
pub fn in_sight(map: &Map, eye: [f32; 3], s: &Seen) -> bool {
    let tall = if s.flags & flag::CROUCH != 0 {
        CROUCH_HEIGHT
    } else {
        HEIGHT
    };
    [tall * 0.9, tall * 0.5]
        .iter()
        .any(|&up| map.strikes(eye, [s.p[0], s.p[1] + up, s.p[2]]).is_none())
}

/// Where a point in the world is on the layer, if in front of the eye.
fn on_screen(v: &View, c: &Canvas, p: [f32; 3]) -> Option<(i32, i32)> {
    let (x, y, ww) = m4::project(&v.vp, p);
    (ww > 0.1).then(|| {
        (
            ((x / ww * 0.5 + 0.5) * c.w as f32) as i32,
            ((0.5 - y / ww * 0.5) * c.h as f32) as i32,
        )
    })
}

/// The names over heads: nearest first, and one that would cover a
/// nearer one's left out.
pub(super) fn names(c: &mut Canvas, v: &View) {
    let (w, h, ui) = (c.w, c.h, v.ui);
    let eye = v.eye;
    let mut near: Vec<_> = v
        .others
        .iter()
        .filter(|s| s.flags & flag::ALIVE != 0 && s.id != v.st.you)
        .filter_map(|s| {
            let d = (s.p[0] - eye[0]).hypot(s.p[2] - eye[2]);
            let seen = v.sighted.map_or(1.0, |m| {
                m.get(&s.id)
                    .map_or(0.0, |&t| 1.0 - ((v.now - t) / OUT_OF_SIGHT) as f32)
            });
            // Out and watching, every name shows, however far.
            let far = if v.me.is_some() {
                1.0 - ((d - NAMES_FADE) / (NAMES - NAMES_FADE)).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let fade = seen.min(1.0) * far;
            (fade > 0.0).then_some((d, fade, s))
        })
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut placed: Vec<Rect> = Vec::new();
    for (_, fade, s) in near {
        let Some((sx, sy)) = on_screen(v, c, [s.p[0], s.p[1] + 2.5, s.p[2]]) else {
            continue;
        };
        if sx < 0 || sx > w || sy < 0 || sy > h {
            continue;
        }
        // The level on a little badge before the name; the health under
        // both.
        let name = v.st.name(s.id);
        let level = format!("{}", s.level);
        let (gap, chip) = (4 * ui, pixels::text_width(&level, ui) + 4 * ui);
        let tw = chip + gap + pixels::text_width(&name, ui);
        let bar = tw.max(24 * ui);
        // Kept whole on the screen, at its edge if need be.
        let x = (sx - bar / 2).clamp(4 * ui, (w - 4 * ui - bar).max(4 * ui));
        let r = Rect::new(
            x as f32,
            (sy - 12 * ui) as f32,
            bar as f32,
            (16 * ui) as f32,
        );
        if placed.iter().any(|p| p.overlaps(&r)) {
            continue;
        }
        placed.push(r);
        let tx = x + (bar - tw) / 2;
        let ty = sy - 10 * ui;
        let badge = Rect::new(
            tx as f32,
            (ty - 2 * ui) as f32,
            chip as f32,
            (11 * ui) as f32,
        );
        c.round_rect(badge, 2.0 * ui as f32, SHADE.fade(fade));
        c.round_rect_line(badge, 2.0 * ui as f32, 1.0, GOLD.fade(0.5 * fade));
        c.text(tx + 2 * ui, ty, &level, ui, GOLD.fade(fade));
        c.text_shadowed(tx + chip + gap, ty, &name, ui, INK.fade(0.9 * fade));
        // The health: framed, so it reads as a bar, not a line under
        // the name; amber under two thirds, red under one.
        let full = max_hp(s.level);
        let frame = Rect::new(x as f32, sy as f32, bar as f32, (3 * ui) as f32);
        c.round_rect(frame.grow(1.0), ui as f32, SHADE.fade(fade));
        let k = (s.hp as i32 * bar / full).clamp(0, bar);
        let col = if (s.hp as i32) * 3 < full {
            RED
        } else if (s.hp as i32) * 3 < full * 2 {
            AMBER
        } else {
            INK
        };
        c.fill_rect(x, sy, k, 3 * ui, col.fade(0.95 * fade));
    }
}

/// The numbers your hits do, rising off whom they struck.
pub(super) fn numbers(c: &mut Canvas, v: &View) {
    let ui = v.ui;
    for (k, &(at, to, amount, what)) in v.st.numbers.iter().enumerate() {
        let Some(s) = v.others.iter().find(|s| s.id == to) else {
            continue;
        };
        let age = (v.now - at) as f32 / 900.0;
        let side = if k % 2 == 0 { 0.4 } else { -0.4 };
        let up = [s.p[0] + side, s.p[1] + 2.2 + age * 1.2, s.p[2]];
        let Some((sx, sy)) = on_screen(v, c, up) else {
            continue;
        };
        let big = if amount >= 25 { 3 } else { 2 };
        let col = bar::rgba(crate::fx::colour(what)).mix(INK, 0.25);
        let n = format!("{amount}");
        let fade = 1.0 - age * age;
        c.text_centred(sx + ui, sy + ui, &n, big * ui, SHADE.fade(fade));
        c.text_centred(sx, sy, &n, big * ui, col.fade(fade));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_never_shows_through_the_spire() {
        use wandfall::laws::{EYE, PLATEAU_TOP};
        let map = Map::new(7);
        let at = |x: f32, z: f32| Seen {
            p: [x, PLATEAU_TOP, z],
            flags: flag::ALIVE,
            ..Seen::default()
        };
        let eye = [-12.0, PLATEAU_TOP + EYE, 0.0];
        assert!(!in_sight(&map, eye, &at(12.0, 0.0)), "the tower between");
        assert!(
            in_sight(&map, eye, &at(-12.0, 5.0)),
            "beside you on the plaza"
        );
        // Nor through the ground: one down the far side of the plateau.
        let low = [-50.0, map.height(-50.0, 0.0), 0.0];
        let under = Seen {
            p: low,
            ..at(0.0, 0.0)
        };
        assert!(!in_sight(&map, [-8.0, PLATEAU_TOP + 0.4, 0.0], &under));
    }
}
