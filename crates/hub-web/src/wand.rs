//! Wandfall's card: the island seen from above as its map shows it, the
//! storm's violet ring closing and opening again, and wizards duelling,
//! their bolts in the colours of the spells. Drawn, not watched.

use pixels::{Canvas, Rect, Rgba};

/// The spells' colours (as the game draws them).
const SPELLS: [(u8, u8, u8); 8] = [
    (255, 120, 40),
    (255, 215, 90),
    (120, 225, 255),
    (165, 115, 255),
    (255, 100, 200),
    (80, 140, 255),
    (120, 235, 110),
    (215, 235, 245),
];

fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9e37_79b9) ^ 0x85eb_ca6b;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2c1b_3c6d);
    x ^= x >> 12;
    (x & 0xffff) as f32 / 65535.0
}

pub fn draw(c: &mut Canvas, b: Rect, t: f32, u: f32) {
    c.round_rect(b, 6.0 * u, Rgba::rgb(14, 30, 52));
    let (cx, cy) = (b.x + b.w / 2.0, b.y + b.h / 2.0);
    let r = b.h.min(b.w) * 0.42;
    c.glow(cx, cy, r * 1.6, Rgba::rgb(40, 90, 130).fade(0.5));
    // The island: sand, then grass, a hill or two.
    c.circle(cx, cy, r + 2.0 * u, Rgba::rgb(190, 172, 120));
    c.circle(cx, cy, r, Rgba::rgb(70, 124, 64));
    c.circle(cx - r * 0.3, cy - r * 0.2, r * 0.35, Rgba::rgb(96, 140, 76));
    c.circle(
        cx + r * 0.35,
        cy + r * 0.25,
        r * 0.25,
        Rgba::rgb(110, 146, 84),
    );
    // The storm: closing over twelve seconds, then again.
    let k = (t / 12.0).fract();
    let sr = r * (1.25 - 0.95 * k);
    c.ring(cx, cy, sr, 2.0 * u, Rgba::rgb(190, 110, 255));
    c.ring(cx, cy, sr + 3.0 * u, 3.0 * u, Rgba(190, 110, 255, 60));
    // Wizards wandering inside the storm.
    let n = 6;
    let at = |i: u32| {
        let a = hash(i) * std::f32::consts::TAU + t * (0.2 + 0.2 * hash(i + 9));
        let d = sr.min(r) * (0.25 + 0.6 * hash(i + 3));
        (cx + a.cos() * d, cy + a.sin() * d * 0.8)
    };
    for i in 0..n {
        let (x, y) = at(i);
        c.circle(x, y, 2.2 * u, Rgba::rgb(250, 246, 236));
    }
    // Bolts: one duel at a time, a spell's colour each.
    let beat = (t / 0.9) as u32;
    let f = (t / 0.9).fract();
    let (a, z) = (beat % n, (beat * 7 + 3) % n);
    if a != z {
        let ((x0, y0), (x1, y1)) = (at(a), at(z));
        let (r0, g0, b0) = SPELLS[(beat % 8) as usize];
        let col = Rgba::rgb(r0, g0, b0);
        let p = (f * 1.6).min(1.0);
        let (x, y) = (x0 + (x1 - x0) * p, y0 + (y1 - y0) * p);
        let tail = (p - 0.25).max(0.0);
        let (tx, ty) = (x0 + (x1 - x0) * tail, y0 + (y1 - y0) * tail);
        if p < 1.0 {
            c.line(tx, ty, x, y, 1.5 * u, col);
            c.glow(x, y, 6.0 * u, col.fade(0.8));
        } else {
            c.glow(x1, y1, (4.0 + 10.0 * (f - 0.625)) * u, col.fade(1.0 - f));
        }
    }
}
