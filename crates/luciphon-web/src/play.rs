//! The hold's HUD: what you carry, the Heart's wheel and Build's ring of
//! pieces, the keys, the Underlight's two lights to return to, and the
//! dreaming screen.

use lucilook::palette::{self, GOLD, INK, RIM};
use luciphon::proto::Own;
use pixels::{text_width, Canvas, Rect, Rgba};

pub const SLOTS: [&str; 8] = [
    "build", "wave", "kindle", "cheer", "rekindle", "sit", "recall", "bow",
];
pub const PIECES: [&str; 6] = ["hearth", "wall", "door", "thorns", "lantern", "planter"];

/// What you carry: glim, wood, stone, Sunwheat, a coloured mark and a
/// number each, in a column from (x, y).
pub fn bag(c: &mut Canvas, x: i32, y: i32, o: &Own, u: i32) {
    let rows = [
        (o.glim, GOLD),
        (o.wood, Rgba::rgb(170, 120, 80)),
        (o.stone, Rgba::rgb(160, 160, 176)),
        (o.wheat, Rgba::rgb(240, 200, 100)),
    ];
    let mut yy = y;
    for (n, col) in rows {
        if n == 0 && col != GOLD {
            continue;
        }
        c.fill_rect(x, yy + u, 4 * u, 4 * u, col);
        c.text_shadowed(x + 6 * u, yy, &n.to_string(), u, INK);
        yy += 9 * u;
    }
    if o.kindle {
        c.text_shadowed(x, yy + 2 * u, "kindling", u, Rgba(255, 210, 122, 230));
    }
}

/// The Heart's wheel about (cx, cy): eight slots (or Build's six pieces),
/// each a spot to tap; with their keys on a desktop.
pub fn wheel(
    c: &mut Canvas,
    cx: f32,
    cy: f32,
    picking: bool,
    keys: bool,
    u: i32,
) -> Vec<(Rect, u8)> {
    let names: &[&str] = if picking { &PIECES } else { &SLOTS };
    let uf = u as f32;
    let r = 44.0 * uf;
    c.circle(cx, cy, r + 22.0 * uf, Rgba(11, 13, 26, 190));
    c.ring(cx, cy, r + 22.0 * uf, 1.0, Rgba(255, 210, 122, 90));
    let mut spots = Vec::new();
    for (k, name) in names.iter().enumerate() {
        let a = (k as f32 * 360.0 / names.len() as f32).to_radians();
        let (x, y) = (cx + a.sin() * r, cy - a.cos() * r);
        let label = if keys {
            format!("{} {name}", k + 1)
        } else {
            name.to_string()
        };
        let w = text_width(&label, u);
        let spot = Rect::new(
            x - w as f32 / 2.0 - 4.0 * uf,
            y - 8.0 * uf,
            w as f32 + 8.0 * uf,
            16.0 * uf,
        );
        c.round_rect(spot, 4.0, Rgba(255, 255, 255, 20));
        c.text_shadowed(
            x as i32 - w / 2,
            y as i32 - 4 * u,
            &label,
            u,
            Rgba(244, 238, 222, 220),
        );
        spots.push((spot, k as u8));
    }
    let mid = if picking { "build" } else { "heart" };
    c.text_centred(cx as i32, cy as i32 - 4 * u, mid, u, GOLD);
    spots
}

/// What the keys do, for a desktop: shown until hidden.
pub fn keys(c: &mut Canvas, building: bool, u: i32) {
    let lines: &[&str] = if building {
        &["1-6 pick a piece", "click: place   hold: remove", "b: done"]
    } else {
        &[
            "wasd move   shift sprint   space jump   q dash",
            "click wand   hold great beam   right throw",
            "tab gear   e heart   b build   k kindle",
            "r rekindle   t recall",
            "/ hide this",
        ]
    };
    let mut y = c.h - (lines.len() as i32 * 9 + 4) * u;
    for l in lines {
        c.text_shadowed(6 * u, y, l, u, Rgba(244, 238, 222, 150));
        y += 9 * u;
    }
}

/// The Underlight's two lights: return at the Luciphon, or at your hearth.
pub fn choice(c: &mut Canvas, has_hearth: bool, home: bool, u: i32) -> (Rect, Rect) {
    let (w, h) = (c.w as f32, c.h as f32);
    let uf = u as f32;
    let bw = (w - 30.0 * uf) / 2.0;
    let luci = Rect::new(10.0 * uf, h - 70.0 * uf, bw, 26.0 * uf);
    let hearth = Rect::new(20.0 * uf + bw, h - 70.0 * uf, bw, 26.0 * uf);
    for (r, label, on, there) in [
        (luci, "luciphon", !home || !has_hearth, true),
        (hearth, "hearth", home && has_hearth, has_hearth),
    ] {
        if !there {
            continue;
        }
        c.round_rect(
            r,
            5.0,
            if on {
                Rgba(255, 210, 122, 230)
            } else {
                Rgba(20, 18, 34, 200)
            },
        );
        let ink = if on { Rgba::rgb(40, 26, 8) } else { INK };
        let tw = text_width(label, u);
        c.text(
            (r.x + r.w / 2.0) as i32 - tw / 2,
            (r.y + r.h / 2.0) as i32 - 4 * u,
            label,
            u,
            ink,
        );
    }
    let line = "return at";
    c.text_centred(
        c.w / 2,
        (h - 84.0 * uf) as i32,
        line,
        u,
        Rgba(244, 238, 222, 200),
    );
    (luci, hearth)
}

/// Asleep: the world goes on; a tap wakes you where you lay.
pub fn dreaming(c: &mut Canvas, u: i32, now: f64) {
    c.fill_rect(0, 0, c.w, c.h, Rgba(11, 13, 26, 150));
    let k = (((now / 900.0).sin() + 1.0) * 40.0) as u8;
    let line = "dreaming";
    let s = pixels::fit_scale(line, c.w - 20, 3 * u);
    c.text_centred(c.w / 2, c.h / 3, line, s, Rgba(127, 224, 255, 160 + k));
    c.text_centred(
        c.w / 2,
        c.h / 3 + 10 * s + 8,
        "tap to wake where you lay",
        u,
        palette::DIM,
    );
    let _ = RIM;
}
