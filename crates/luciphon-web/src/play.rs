//! The hold's HUD (v0.2): what you carry beside the Heart, the wheel and
//! Build's ring of pieces, the build ghost and its channel, the rings of
//! nodes near you (strike on the ring for double), the Underlight's two
//! lights to return to, and the dreaming screen.

use lucilook::palette::{self, GOLD, INK, RIM};
use lucilook::View;
use luciphon::proto::Own;
use luciphon::tiles::{obj, Tiles};
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

/// The wheel over a Heart press at (cx, cy): eight slots (or Build's six
/// pieces), the one under the pointer lit.
pub fn wheel(c: &mut Canvas, cx: f32, cy: f32, lit: Option<u8>, picking: bool, u: i32) {
    let names: &[&str] = if picking { &PIECES } else { &SLOTS };
    let r = 30.0 * u as f32;
    c.circle(cx, cy, r + 12.0 * u as f32, Rgba(11, 13, 26, 170));
    c.ring(cx, cy, r + 12.0 * u as f32, 1.0, Rgba(255, 210, 122, 90));
    for (k, name) in names.iter().enumerate() {
        let a = (k as f32 * 45.0).to_radians();
        let (x, y) = (cx + a.sin() * r, cy - a.cos() * r);
        let on = lit == Some(k as u8);
        let col = if on { GOLD } else { Rgba(244, 238, 222, 190) };
        if on {
            c.glow_add(x as i32, y as i32, 10 * u, Rgba(255, 210, 122, 120));
        }
        let w = text_width(name, u);
        c.text_shadowed(x as i32 - w / 2, y as i32 - 4 * u, name, u, col);
    }
}

/// The build ghost: the tile in front of you, and the channel closing.
pub fn ghost(
    c: &mut Canvas,
    v: &View,
    (x, y, facing): (f32, f32, u16),
    o: &Own,
    tiles: &Tiles,
    l: &luciphon::laws::Laws,
) {
    let (ux, uy) = engine::fixed::unit(facing);
    let (gx, gy) = ((x + ux.to_f32()).floor(), (y + uy.to_f32()).floor());
    let (sx, sy) = v.to_screen(c, gx, gy);
    let t = tiles.get(gx as i32, gy as i32);
    let mine = t.land_of(o.me.body.claim) || o.build == obj::HEARTH;
    let ok = !t.void() && !t.solid() && mine;
    let col = if ok {
        Rgba(255, 210, 122, 200)
    } else {
        Rgba(255, 110, 90, 200)
    };
    let s = 16.0;
    if o.build == obj::HEARTH {
        c.round_rect_line(
            Rect::new(sx - 2.0 * s, sy - 2.0 * s, 5.0 * s, 5.0 * s),
            2.0,
            1.0,
            col,
        );
    }
    c.round_rect_line(Rect::new(sx, sy, s, s), 2.0, 1.5, col);
    if o.channel > 0 {
        let k = 1.0 - o.channel as f32 / l.build_channel.max(1) as f32;
        c.ring(sx + 8.0, sy + 8.0, 4.0 + 8.0 * k, 2.0, GOLD);
    }
    let name = PIECES
        .iter()
        .zip(luciphon::build::PIECES)
        .find(|p| p.1 == o.build)
        .map_or("build", |p| p.0);
    let w = text_width(name, 1);
    c.text_shadowed(sx as i32 + 8 - w / 2, sy as i32 - 10, name, 1, col);
}

/// The rings of nodes near (x, y): each rings every 30 ticks on its own
/// phase, so a strike landing on the ring gives double.
pub fn rings(c: &mut Canvas, v: &View, tiles: &Tiles, at: (f32, f32), seed: u64, tick: f64) {
    let (cx, cy) = (at.0.floor() as i32, at.1.floor() as i32);
    for ty in cy - 3..=cy + 3 {
        for tx in cx - 3..=cx + 3 {
            let o = tiles.get(tx, ty).obj;
            if !obj::node(o) {
                continue;
            }
            let Some(idx) = Tiles::index(tx, ty) else {
                continue;
            };
            let ph = luciphon::gather::phase(seed, idx as u16) as f64;
            // 0 at the ring, rising to 1 just before the next.
            let k = ((tick + ph) % 30.0) / 30.0;
            let (sx, sy) = v.to_screen(c, tx as f32 + 0.5, ty as f32 + 0.6);
            let near = (1.0 - k).min(k) < 0.08;
            let r = 4.0 + 10.0 * (1.0 - k as f32);
            let a = if near { 230 } else { (40.0 + 80.0 * k) as u8 };
            c.ring(
                sx,
                sy - 4.0,
                r,
                if near { 2.0 } else { 1.0 },
                Rgba(127, 224, 255, a),
            );
        }
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
