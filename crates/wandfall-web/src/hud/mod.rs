//! The HUD, drawn in pixels over the picture: the crosshair (and a mark
//! when your bolt lands), your health, the match and the storm (and which
//! way out of it), a map of the island with the storm's circles, the feed,
//! names over heads, and what to do next (you are out, who won). Each
//! block has its place (`Layout`), so none covers another, on a phone as
//! on a desktop.

use std::collections::HashMap;

use pixels::{Canvas, Rect, Rgba};
use render::M4;
use wandfall::laws::MATCH_SIZE;
use wandfall::loot::max_hp;
use wandfall::motion::Body;
use wandfall::proto::{Frame, Own, Seen};

use crate::bar;
use crate::state::{Line, State};

mod card;
mod heads;
mod map;

pub use heads::{in_sight, NAMES};
pub use map::{spot as map_spot, Mini};

const INK: Rgba = Rgba::rgb(250, 246, 236);
const DIM: Rgba = Rgba::rgb(200, 206, 220);
const GOLD: Rgba = Rgba::rgb(255, 214, 128);
const RED: Rgba = Rgba::rgb(240, 80, 80);
const STORM: Rgba = Rgba::rgb(190, 110, 255);
const SHADE: Rgba = Rgba(8, 10, 20, 150);

/// What the HUD shows this frame.
pub struct View<'a> {
    pub st: &'a State,
    pub others: &'a [Seen],
    pub vp: M4,
    /// Where the camera is.
    pub eye: [f32; 3],
    /// Where you are and which way you face, and your own state; your
    /// body as you predict it (exact, and a moment ahead of the room's).
    pub me: Option<([f32; 3], f32)>,
    pub own: Option<&'a Own>,
    pub body: Option<&'a Body>,
    /// When each wizard was last in sight (a name fades out after); none:
    /// every one shows (you are out, watching).
    pub sighted: Option<&'a HashMap<u16, f64>>,
    pub watching: Option<String>,
    pub in_storm: bool,
    pub now: f64,
    pub ui: i32,
    pub perf: Option<String>,
    pub touch: bool,
    /// On the practice range (no match, no storm).
    pub practice: bool,
    /// A wizard under the crosshair, in reach of the Lance.
    pub on_target: bool,
    /// The spellbook is open (it takes the place of the cube underfoot).
    pub book: bool,
    /// The range's lessons are up (on a touch screen they take the band,
    /// and the cube underfoot gives way).
    pub lesson: bool,
    /// The link to the room is down: it is coming back.
    pub lost: bool,
}

/// On a touch screen your health and the rest stack down the top left
/// (the thumbs own the bottom; the menu button is over them): where
/// each row starts, in ui units.
pub mod column {
    pub const X: i32 = 12;
    pub const WIDE: i32 = 120;
    pub const HP: i32 = 28;
    pub const BAR: i32 = 38;
    pub const STAMINA: i32 = 48;
    pub const XP: i32 = 53;
    pub const LEVEL: i32 = 58;
    pub const KO: i32 = 68;
    pub const FEED: i32 = 82;
}

/// Where the HUD's blocks go this frame: the island map (left, top,
/// side), the foot of the top line, and the band a panel over the play
/// takes (the range's lessons, the lobby; on a touch screen the cube
/// underfoot): under the top line, clear of the map and (touch) of your
/// health, over the crosshair.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub map: (i32, i32, i32),
    pub top: i32,
    pub band: Rect,
}

pub fn layout(w: i32, h: i32, ui: i32, touch: bool, top: i32) -> Layout {
    let map = map::spot(w, h, ui);
    let left = if touch {
        (column::X + column::WIDE + 8) * ui
    } else {
        8 * ui
    };
    let right = map.0 - 8 * ui;
    let bottom = h / 2 - 16 * ui;
    let band = if right - left >= 150 * ui {
        let y = top + 4 * ui;
        let tall = (bottom - y).max(30 * ui);
        Rect::new(left as f32, y as f32, (right - left) as f32, tall as f32)
    } else {
        // Too narrow beside them (a phone held upright): under them all.
        let under = (map.1 + map.2).max(if touch { column::FEED * ui } else { top });
        let y = under + 6 * ui;
        let tall = (bottom - y).max(60 * ui);
        Rect::new((8 * ui) as f32, y as f32, (w - 16 * ui) as f32, tall as f32)
    };
    Layout { map, top, band }
}

/// Which way `from` is from `at`, facing `yaw`, as the screen has it
/// (radians: 0 ahead, up the screen; round to the right, clockwise);
/// none if it is right here.
fn bearing(at: [f32; 3], yaw: f32, from: [f32; 3]) -> Option<f32> {
    let (s, c) = yaw.sin_cos();
    let (dx, dz) = (from[0] - at[0], from[2] - at[2]);
    // Ahead is (c, s) on the ground; to the right, (-s, c).
    let (ahead, right) = (dx * c + dz * s, -dx * s + dz * c);
    (ahead.hypot(right) >= 0.5).then(|| right.atan2(ahead))
}

/// A wedge of a ring about `o`, `r` out, centred at `mid` radians
/// (0 up, clockwise), pointing outward.
fn arc(c: &mut Canvas, o: (f32, f32), r: f32, mid: f32, ui: i32, col: Rgba) {
    let u = ui as f32;
    let at = |a: f32, d: f32| (o.0 + a.sin() * d, o.1 - a.cos() * d);
    let (inner, outer) = (r, r + 5.0 * u);
    let n = 8;
    for k in 0..n {
        let a0 = mid - 0.38 + 0.76 * k as f32 / n as f32;
        let a1 = mid - 0.38 + 0.76 * (k + 1) as f32 / n as f32;
        c.poly(
            &[at(a0, inner), at(a0, outer), at(a1, outer), at(a1, inner)],
            col,
        );
    }
    // A point at its middle, outward.
    c.poly(
        &[
            at(mid - 0.09, outer),
            at(mid, outer + 7.0 * u),
            at(mid + 0.09, outer),
        ],
        col,
    );
}

fn clock(secs: u16) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// What the top line says: the range, the lobby, the match and its storm,
/// or who won.
fn top_line(v: &View, f: &Frame) -> String {
    match f.phase {
        _ if v.practice => {
            if v.touch {
                "practice range - menu: spellbook".to_string()
            } else {
                "practice range - B spellbook - Esc menu".to_string()
            }
        }
        0 if f.secs > 0 => format!("the match begins in {}", f.secs),
        0 => "waiting for wizards".to_string(),
        1 => {
            let storm = if f.storm.1 < 0.5 {
                "the storm has closed".to_string()
            } else if f.shrinking {
                format!("the storm closes - {}", clock(f.secs))
            } else {
                format!("the storm moves in {}", clock(f.secs))
            };
            format!(
                "{} of {} alive   {}",
                f.alive,
                f.entrants.max(MATCH_SIZE as u8),
                storm
            )
        }
        _ => format!("{} wins!", v.st.name(f.winner)),
    }
}

/// The crosshair, outlined so it holds on pale stone and spell light (red
/// on a wizard in the Lance's reach), and a mark when your bolt lands.
fn crosshair(c: &mut Canvas, v: &View) {
    let (cx, cy, ui) = (c.w / 2, c.h / 2, v.ui);
    let (g, l) = (3 * ui, 4 * ui);
    let col = if v.on_target { RED } else { INK.fade(0.9) };
    let u = ui as f32;
    let ticks = [(1, 0), (-1, 0), (0, 1), (0, -1)].map(|(dx, dy)| {
        (
            (cx + dx * g) as f32,
            (cy + dy * g) as f32,
            (cx + dx * (g + l)) as f32,
            (cy + dy * (g + l)) as f32,
        )
    });
    let (cxf, cyf) = (cx as f32, cy as f32);
    for (x0, y0, x1, y1) in ticks {
        c.line(x0, y0, x1, y1, u + 2.0, SHADE.fade(0.9));
    }
    c.circle(cxf, cyf, 0.5 * u + 1.2, SHADE.fade(0.9));
    for (x0, y0, x1, y1) in ticks {
        c.line(x0, y0, x1, y1, u, col);
    }
    c.circle(cxf, cyf, 0.5 * u, col);
    if v.now - v.st.hit_at < 220.0 {
        let k = (6 * ui) as f32;
        for (a, b) in [(-k, -k), (-k, k)] {
            c.line(cxf + a, cyf + b, cxf - a, cyf - b, u * 1.5, GOLD);
        }
    }
}

/// The HUD over the picture (`mini`: the island map at its size); where
/// its blocks went, for what is drawn over it.
pub fn draw(c: &mut Canvas, mini: &Canvas, v: &View) -> Layout {
    let ui = v.ui;
    let (w, h) = (c.w, c.h);
    let cx = w / 2;
    let Some(f) = v.st.frame.as_ref() else {
        c.text_centred(cx, h / 2, "finding the island...", 2 * ui, INK);
        return layout(w, h, ui, v.touch, 8 * ui);
    };
    heads::names(c, v);
    heads::numbers(c, v);
    if v.me.is_some() {
        crosshair(c, v);
    }
    // Hurt, or in the storm: the edges of the screen say so.
    let hurt = (1.0 - (v.now - v.st.hurt_at) / 400.0).clamp(0.0, 1.0) as f32;
    if hurt > 0.0 {
        let r = (w.min(h) as f32) * 0.62;
        c.outside_circle(cx as f32, h as f32 / 2.0, r, RED.fade(0.35 * hurt));
    }
    // And which way it came from: a red arc on a ring about the middle.
    let ring = (w.min(h) as f32) * 0.2;
    if let Some((at, yaw)) = v.me {
        for &(when, from) in &v.st.hurt_from {
            let f = (1.0 - (v.now - when) / 1500.0) as f32;
            if let Some(mid) = bearing(at, yaw, from).filter(|_| f > 0.0) {
                let col = RED.fade(0.85 * f);
                arc(c, (cx as f32, h as f32 / 2.0), ring, mid, ui, col);
            }
        }
    }
    if v.in_storm {
        let r = (w.min(h) as f32) * 0.55;
        c.outside_circle(cx as f32, h as f32 / 2.0, r, STORM.fade(0.4));
        // Which way is safe: the shortest way back, to the circle's
        // middle, and how far its edge is.
        let (o, r) = f.storm;
        let (text, mid) = match v.me {
            Some((at, yaw)) => {
                let out = ((at[0] - o[0]).hypot(at[2] - o[1]) - r).max(0.0).ceil();
                let mid = bearing(at, yaw, [o[0], 0.0, o[1]]);
                (format!("in the storm: {out:.0} m to the circle"), mid)
            }
            None => ("in the storm".to_string(), None),
        };
        if let Some(mid) = mid {
            let col = STORM
                .mix(INK, 0.35)
                .fade(0.5 + 0.4 * ((v.now / 160.0).sin() as f32).abs());
            arc(c, (cx as f32, h as f32 / 2.0), ring, mid, ui, col);
        }
        let k = pixels::fit_scale(&text, w - 16 * ui, 2 * ui);
        c.text_centred(cx, h / 2 + 30 * ui, &text, k, INK);
    }
    // Top: the match; on a phone at the size of the rest, wrapped if need
    // be (the menu button is in the corner, the map in the other).
    let top = top_line(v, f);
    let (lines, k) = if v.touch {
        (pixels::wrap(&top, w - 20 * ui, ui), ui)
    } else {
        let k = pixels::fit_scale(&top, w - 20 * ui, 2 * ui);
        (vec![top], k)
    };
    for (n, l) in lines.iter().enumerate() {
        let y = 8 * ui + n as i32 * 9 * k;
        c.text_shadowed(cx - pixels::text_width(l, k) / 2, y, l, k, INK);
    }
    let lay = layout(w, h, ui, v.touch, 8 * ui + lines.len() as i32 * 9 * k);
    if let Some(o) = v.own.filter(|_| v.me.is_some()) {
        health(c, v, o, &lay);
    }
    // The island, the storm's circles (in a match), and you.
    let storm = (f.phase == 1 && !v.practice).then_some(f);
    map::draw(c, mini, lay.map, storm, v.me, ui);
    feed(c, v, &lay);
    // What to do now: the card when you are out or the match is won.
    let mid = h / 2 - 40 * ui;
    let won = f.phase == 2 && f.winner == v.st.you && f.winner != 0;
    if !v.practice && (won || v.st.out.is_some_and(|_| f.phase >= 1)) {
        card::draw(c, v, f, won, lay.top + 6 * ui);
    } else if v.me.is_none() && v.practice {
        let t = "knocked out - you stand again in a moment";
        c.text_centred(cx, mid, t, pixels::fit_scale(t, w - 16 * ui, ui), INK);
    } else if v.me.is_none() && f.phase == 1 {
        c.text_centred(cx, mid, "a match is on: you join the next one", ui, INK);
    }
    if v.lost {
        let t = "the island is coming back...";
        c.text_centred(cx, h / 2 - 12 * ui, t, 2 * ui, INK);
    }
    if let Some(name) = &v.watching {
        c.text_centred(cx, h - 30 * ui, &format!("watching {name}"), ui, DIM);
    }
    if let Some(p) = &v.perf {
        let tw = pixels::text_width(p, ui);
        c.text_shadowed(w - tw - 6 * ui, h - 12 * ui, p, ui, DIM);
    }
    lay
}

/// Your health (and a ward's shield after it), your stamina while any is
/// spent, your level and knockouts, and the spell bar.
fn health(c: &mut Canvas, v: &View, o: &Own, lay: &Layout) {
    let (w, h, ui) = (c.w, c.h, v.ui);
    // Warded: the screen's edge glows (you cannot see your own bubble
    // from inside it).
    if o.shield > 0 {
        let ward = bar::rgba(crate::fx::colour(wandfall::laws::spell::WARD));
        for k in 0..4 {
            let inset = (k * 3 * ui) as f32;
            let a = 0.32 - k as f32 * 0.07;
            let r = Rect::new(inset, inset, w as f32 - 2.0 * inset, h as f32 - 2.0 * inset);
            c.round_rect_line(r, 10.0 * ui as f32, 3.0 * ui as f32, ward.fade(a));
        }
    }
    let full = max_hp(o.level);
    // At the foot of the screen, left (the spells are right, your wizard
    // between); on a touch screen the thumbs own the bottom, and health
    // goes top left.
    let (x, y, bw, bh) = if v.touch {
        (column::X * ui, column::BAR * ui, column::WIDE * ui, 8 * ui)
    } else {
        let l = bar::layout(w, h, ui);
        (14 * ui, l.y + l.s - 11 * ui, (150 * ui).min(w / 3), 11 * ui)
    };
    let u = ui as f32;
    c.round_rect(
        Rect::new(x as f32, y as f32, bw as f32, bh as f32),
        2.0 * u,
        SHADE,
    );
    let k = (o.hp as i32 * bw / full).min(bw);
    let col = if (o.hp as i32) * 3 < full {
        RED
    } else {
        Rgba::rgb(110, 220, 120)
    };
    c.round_rect(
        Rect::new(x as f32, y as f32, k as f32, bh as f32),
        2.0 * u,
        col,
    );
    if o.shield > 0 {
        let sw = (o.shield as i32 * bw / full).min(bw - k);
        let ward = bar::rgba(crate::fx::colour(wandfall::laws::spell::WARD)).mix(INK, 0.3);
        c.round_rect(
            Rect::new((x + k) as f32, y as f32, sw as f32, bh as f32),
            2.0 * u,
            ward,
        );
    }
    // Stamina, over the health (under it on a touch screen) while any is
    // spent: red while winded. As you predict it, so a jump's cost shows
    // the moment you jump.
    let b = v.body.unwrap_or(&o.body);
    if b.spent > 0 || b.sprint {
        let left = 1.0 - b.spent as f32 / wandfall::laws::STAMINA as f32;
        let sy = if v.touch {
            column::STAMINA * ui
        } else {
            y - 6 * ui
        };
        let col = if b.winded {
            RED
        } else {
            Rgba::rgb(240, 200, 90)
        };
        c.round_rect(
            Rect::new(x as f32, sy as f32, bw as f32, 3.0 * u),
            1.5 * u,
            SHADE,
        );
        c.round_rect(
            Rect::new(x as f32, sy as f32, bw as f32 * left.max(0.0), 3.0 * u),
            1.5 * u,
            col,
        );
    }
    let hp = format!("{} / {}", o.hp, full);
    let ko = format!("knocked out {}", o.kills);
    if v.touch {
        // Stacked at the top left: health, XP, level and knockouts.
        c.text_shadowed(x, column::HP * ui, &hp, ui, INK);
        let level = format!("level {}", o.level);
        c.text_shadowed(x, column::LEVEL * ui, &level, ui, INK);
        c.text_shadowed(x, column::KO * ui, &ko, ui, GOLD);
    } else {
        c.text_centred(x + bw / 2, y + 2 * ui, &hp, ui, INK);
        let (_, my, s) = lay.map;
        let kx = w - 10 * ui - pixels::text_width(&ko, ui);
        c.text_shadowed(kx, my + s + 6 * ui, &ko, ui, GOLD);
    }
    if b.chill > 0 {
        let cold = bar::rgba(crate::fx::colour(wandfall::laws::spell::FROST));
        c.text_centred(w / 2, h / 2 + 18 * ui, "chilled", ui, cold);
    }
    // The bar (a touch screen has buttons instead), and what lies
    // underfoot (not with the book open; on a touch screen in the band,
    // unless the lessons have it).
    let under = (!v.book && !(v.touch && v.lesson)).then(|| bar::Under {
        feet: v.me.map_or(o.body.p, |m| m.0),
        band: v.touch.then_some(lay.band),
    });
    bar::draw(c, o, v.st, v.now, ui, (!v.touch, under));
}

/// The feed: under the island map, or (on a touch screen, whose right
/// side is the thumb's) under your health.
fn feed(c: &mut Canvas, v: &View, lay: &Layout) {
    let (w, ui) = (c.w, v.ui);
    let (_, my, s) = lay.map;
    let mut y = if v.touch {
        column::FEED * ui
    } else {
        my + s + 20 * ui
    };
    for (at, line) in &v.st.feed {
        let age = v.now - at;
        if age > 8000.0 {
            continue;
        }
        let fade = (1.0 - (age - 6000.0).max(0.0) / 2000.0) as f32;
        match line {
            Line::Text(t) => {
                let tw = pixels::text_width(t, ui);
                let x = if v.touch {
                    column::X * ui
                } else {
                    w - tw - 10 * ui
                };
                c.text_shadowed(x, y, t, ui, DIM.fade(fade));
            }
            // A knockout: who, the icon of what did it, whom.
            &Line::Out { by, with, who } => {
                let (a, b) = (v.st.name(by), v.st.name(who));
                let (isz, gap) = (10 * ui, 4 * ui);
                let wa = pixels::text_width(&a, ui);
                let tw = wa + 2 * gap + isz + pixels::text_width(&b, ui);
                let x = if v.touch {
                    column::X * ui
                } else {
                    w - tw - 10 * ui
                };
                let you = v.st.you;
                let ca = if by == you { GOLD } else { INK };
                let cb = if who == you { RED } else { DIM };
                c.text_shadowed(x, y, &a, ui, ca.fade(fade));
                let tile = Rect::new(
                    (x + wa + gap) as f32,
                    (y - 2 * ui) as f32,
                    isz as f32,
                    isz as f32,
                );
                bar::icon(c, with, tile);
                c.text_shadowed(x + wa + 2 * gap + isz, y, &b, ui, cb.fade(fade));
            }
        }
        y += 12 * ui;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hit_shows_from_the_way_it_came() {
        let at = [0.0; 3];
        let ahead = bearing(at, 0.0, [10.0, 0.0, 0.0]).unwrap();
        assert!(ahead.abs() < 1e-4);
        let right = bearing(at, 0.0, [0.0, 0.0, 10.0]).unwrap();
        assert!((right - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        let behind = bearing(at, 0.0, [-10.0, 0.0, 0.0]).unwrap();
        assert!((behind.abs() - std::f32::consts::PI).abs() < 1e-4);
        // Turned to face it, it is ahead.
        let turned = bearing(at, std::f32::consts::FRAC_PI_2, [0.0, 0.0, 10.0]).unwrap();
        assert!(turned.abs() < 1e-4);
        assert!(bearing(at, 0.0, [0.1, 3.0, 0.0]).is_none());
    }

    /// No two of the HUD's blocks on top of each other, on the screens
    /// people play on: the band under the top line, clear of the map, of
    /// the touch column, and of the crosshair.
    #[test]
    fn the_band_covers_nothing_else() {
        for (w, h, ui, touch) in [
            (844, 390, 2, true),
            (740, 360, 2, true),
            (512, 384, 1, true),
            (960, 540, 1, false),
            (683, 384, 2, false),
            (960, 540, 2, false),
        ] {
            let top = 8 * ui + 9 * ui;
            let l = layout(w, h, ui, touch, top);
            let b = l.band;
            let (mx, my, s) = l.map;
            let map = Rect::new(mx as f32, my as f32, s as f32, s as f32);
            assert!(!b.overlaps(&map), "{w}x{h}: the map");
            assert!(b.y >= top as f32, "{w}x{h}: the top line");
            assert!(
                b.y + b.h <= (h / 2 - 7 * ui) as f32,
                "{w}x{h}: the crosshair"
            );
            if touch {
                let col = (column::X + column::WIDE) * ui;
                assert!(b.x >= col as f32, "{w}x{h}: your health");
            }
            assert!(b.w >= 150.0 * ui as f32, "{w}x{h}: room in it");
        }
    }
}
