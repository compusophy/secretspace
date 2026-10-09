//! Each spell's icon, drawn in pixels: a tile in its colour and a glyph
//! on it. The page's bar and spellbook draw them; the spell cubes wear
//! them on every face.

use pixels::{Canvas, Rect, Rgba};
use render::V3;
use wandfall::laws::spell;

use crate::fx::colour;

const INK: Rgba = Rgba::rgb(250, 246, 236);

pub fn rgba(c: V3) -> Rgba {
    Rgba::rgb(
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
    )
}

/// A line through points, `w` wide.
fn stroke(c: &mut Canvas, pts: &[(f32, f32)], w: f32, col: Rgba) {
    for p in pts.windows(2) {
        c.line(p[0].0, p[0].1, p[1].0, p[1].1, w, col);
    }
}

/// A four-pointed glint at (x, y), `r` from its middle to a point.
fn glint(c: &mut Canvas, x: f32, y: f32, r: f32, col: Rgba) {
    let t = r * 0.22;
    c.poly(&[(x - r, y), (x, y - t), (x + r, y), (x, y + t)], col);
    c.poly(&[(x, y - r), (x + t, y), (x, y + r), (x - t, y)], col);
}

/// A spell's glyph in the box `b`, in `col` (`back`: the tile behind it,
/// for the cut-outs).
pub fn glyph(c: &mut Canvas, sp: u8, b: Rect, col: Rgba, back: Rgba) {
    let s = b.w;
    let at = |u: f32, v: f32| (b.x + u * s, b.y + v * s);
    let pts = |list: &[(f32, f32)]| list.iter().map(|&(u, v)| at(u, v)).collect::<Vec<_>>();
    match sp {
        spell::FIREBALL => {
            c.poly(
                &pts(&[
                    (0.5, 0.08),
                    (0.63, 0.3),
                    (0.76, 0.45),
                    (0.8, 0.62),
                    (0.73, 0.78),
                    (0.6, 0.88),
                    (0.5, 0.9),
                    (0.4, 0.88),
                    (0.27, 0.78),
                    (0.2, 0.62),
                    (0.26, 0.44),
                    (0.36, 0.52),
                    (0.37, 0.35),
                ]),
                col,
            );
            c.poly(
                &pts(&[
                    (0.52, 0.44),
                    (0.63, 0.6),
                    (0.63, 0.72),
                    (0.52, 0.8),
                    (0.41, 0.73),
                    (0.42, 0.6),
                ]),
                back,
            );
        }
        spell::LANCE => {
            c.poly(
                &pts(&[(0.88, 0.12), (0.7, 0.4), (0.12, 0.88), (0.6, 0.3)]),
                col,
            );
            let (x, y) = at(0.76, 0.24);
            glint(c, x, y, s * 0.2, col);
        }
        spell::FROST => {
            let (cx, cy) = at(0.5, 0.5);
            let w = s * 0.075;
            for k in 0..6 {
                let a = k as f32 * std::f32::consts::PI / 3.0 + std::f32::consts::FRAC_PI_2;
                let (dx, dy) = (a.cos(), a.sin());
                let end = (cx + dx * s * 0.38, cy + dy * s * 0.38);
                c.line(cx, cy, end.0, end.1, w, col);
                let m = (cx + dx * s * 0.24, cy + dy * s * 0.24);
                for side in [-1.0f32, 1.0] {
                    let b = a + side * 0.8;
                    let tip = (m.0 + b.cos() * s * 0.12, m.1 + b.sin() * s * 0.12);
                    c.line(m.0, m.1, tip.0, tip.1, w * 0.85, col);
                }
            }
            c.circle(cx, cy, s * 0.07, col);
        }
        spell::LIGHTNING => c.poly(
            &pts(&[
                (0.6, 0.06),
                (0.25, 0.54),
                (0.47, 0.54),
                (0.37, 0.94),
                (0.76, 0.42),
                (0.54, 0.42),
                (0.67, 0.06),
            ]),
            col,
        ),
        spell::BLINK => {
            let (ox, oy) = at(0.3, 0.68);
            c.ring(ox, oy, s * 0.13, s * 0.06, col.fade(0.55));
            for (k, u) in [0.36f32, 0.5, 0.64].into_iter().enumerate() {
                let (x, y) = at(0.3 + 0.38 * u * 1.0, 0.68 - 0.32 * u);
                c.circle(
                    x,
                    y,
                    s * (0.03 + 0.012 * k as f32),
                    col.fade(0.6 + 0.13 * k as f32),
                );
            }
            let (dx, dy) = at(0.66, 0.38);
            c.circle(dx, dy, s * 0.15, col);
            let (gx, gy) = at(0.84, 0.17);
            glint(c, gx, gy, s * 0.12, col);
        }
        spell::WARD => {
            let shield = [
                (0.5, 0.08),
                (0.82, 0.2),
                (0.79, 0.52),
                (0.5, 0.92),
                (0.21, 0.52),
                (0.18, 0.2),
            ];
            c.poly(&pts(&shield), col);
            let inset: Vec<(f32, f32)> = shield
                .iter()
                .map(|&(u, v)| (0.5 + (u - 0.5) * 0.62, 0.47 + (v - 0.47) * 0.62))
                .collect();
            c.poly(&pts(&inset), back);
            let inner: Vec<(f32, f32)> = shield
                .iter()
                .map(|&(u, v)| (0.5 + (u - 0.5) * 0.3, 0.46 + (v - 0.46) * 0.3))
                .collect();
            c.poly(&pts(&inner), col);
        }
        spell::MEND => {
            let w = s * 0.21;
            let (a, bb) = (at(0.5, 0.2), at(0.5, 0.8));
            c.line(a.0, a.1, bb.0, bb.1, w, col);
            let (a, bb) = (at(0.2, 0.5), at(0.8, 0.5));
            c.line(a.0, a.1, bb.0, bb.1, w, col);
            let (gx, gy) = at(0.82, 0.18);
            glint(c, gx, gy, s * 0.12, col);
        }
        spell::GUST => {
            let w = s * 0.075;
            for (y, x0, x1, r) in [
                (0.32, 0.1, 0.6, 0.12),
                (0.54, 0.2, 0.74, 0.1),
                (0.76, 0.12, 0.48, 0.08),
            ] {
                let mut line = vec![at(x0, y)];
                // Along, then a curl up and back over itself.
                let (cx, cy) = (x1, y - r);
                for k in 0..=12 {
                    let u = k as f32 / 12.0;
                    let a = std::f32::consts::FRAC_PI_2 - u * 1.5 * std::f32::consts::PI;
                    let rr = r * (1.0 - 0.4 * u);
                    line.push(at(cx + a.cos() * rr, cy + a.sin() * rr));
                }
                stroke(c, &line, w, col);
            }
        }
        spell::TETHER => {
            // A rope wavering up to a hook that bites.
            let w = s * 0.08;
            let line: Vec<(f32, f32)> = (0..=14)
                .map(|k| {
                    let u = k as f32 / 14.0;
                    let wave = (u * std::f32::consts::PI * 3.0).sin() * 0.06 * (1.0 - u);
                    at(0.14 + 0.5 * u + wave, 0.86 - 0.5 * u + wave)
                })
                .collect();
            stroke(c, &line, w, col);
            let tip = (0.76, 0.24);
            stroke(c, &pts(&[(0.62, 0.38), tip]), s * 0.1, col);
            for barb in [
                [(0.94, 0.22), (0.9, 0.36)],
                [(0.74, 0.06), (0.62, 0.12)],
                [(0.92, 0.07), (0.94, 0.1)],
            ] {
                stroke(c, &pts(&[tip, barb[0], barb[1]]), w, col);
            }
        }
        // The wand (what a knockout was dealt with): a spark.
        _ => {
            let (x, y) = at(0.5, 0.5);
            glint(c, x, y, s * 0.36, col);
            c.circle(x, y, s * 0.08, col);
        }
    }
}

/// A spell's icon: a tile in its colour, its glyph upon it.
pub fn icon(c: &mut Canvas, sp: u8, b: Rect) {
    let base = rgba(colour(sp));
    let dark = Rgba::rgb(14, 16, 26).mix(base, 0.32);
    let r = b.w * 0.2;
    c.round_rect(b, r, dark);
    c.glow(
        b.x + b.w / 2.0,
        b.y + b.h * 0.45,
        b.w * 0.5,
        base.fade(0.55),
    );
    c.round_rect(
        Rect::new(b.x + b.w * 0.08, b.y + b.h * 0.06, b.w * 0.84, b.h * 0.36),
        r * 0.7,
        Rgba(255, 255, 255, 14),
    );
    c.round_rect_line(
        b.grow(-0.5),
        r,
        (b.w / 26.0).max(1.0),
        base.mix(INK, 0.35).fade(0.9),
    );
    let g = b.grow(-b.w * 0.16);
    let off = (b.w / 28.0).max(1.0);
    let back = dark.mix(base, 0.25);
    glyph(
        c,
        sp,
        Rect::new(g.x + off, g.y + off, g.w, g.h),
        Rgba(0, 0, 0, 110),
        Rgba(0, 0, 0, 0),
    );
    glyph(c, sp, g, INK.mix(base, 0.12), back);
}
