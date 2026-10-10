//! What the front page stands on, and its name. Deep space rather than
//! confetti: a faint haze in the shelf's own two colours (wyrm's green
//! low on one side, Wandfall's violet high on the other), painted once
//! for the screen's size, and three depths of stars drifting past at
//! their own speeds (the nearer, the faster, and the more they move as the
//! page scrolls), each twinkling, fading in at one edge and out at the
//! other. The name's letters hold to the same band, green to violet, and
//! a slow glint of light crosses them now and then.

use pixels::{Canvas, Rgba};

use crate::shelf::TITLE;

const BG: Rgba = Rgba::rgb(7, 10, 18);
/// The shelf's band: wyrm's hue to Wandfall's.
const FROM: f32 = 140.0;
const TO: f32 = 265.0;
/// Seconds between glints crossing the name.
const GLINT_EVERY: f32 = 7.0;

/// 0..1, the same for the same k and salt.
fn hash(k: u32, salt: u32) -> f32 {
    let mut x = k.wrapping_mul(0x9e37_79b9) ^ salt.wrapping_mul(0x85eb_ca6b);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2c1b_3c6d);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297a_2d39);
    x ^= x >> 15;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// A depth of stars: how many, how big and bright, how fast they drift
/// (buffer pixels a second, a text pixel each), and how much they follow
/// the page's scroll.
struct Depth {
    n: u32,
    r: f32,
    alpha: f32,
    speed: f32,
    follow: f32,
}

const DEPTHS: [Depth; 3] = [
    Depth {
        n: 70,
        r: 0.45,
        alpha: 0.32,
        speed: 1.2,
        follow: 0.08,
    },
    Depth {
        n: 28,
        r: 0.75,
        alpha: 0.5,
        speed: 3.5,
        follow: 0.2,
    },
    Depth {
        n: 9,
        r: 1.1,
        alpha: 0.7,
        speed: 8.0,
        follow: 0.45,
    },
];

#[derive(Default)]
pub struct Backdrop {
    /// The haze, painted for the screen's size.
    haze: Option<Canvas>,
}

impl Backdrop {
    /// The haze and the stars over the whole of `c`, at time `t` (s), the
    /// page scrolled `scroll`, text scale `u`.
    pub fn draw(&mut self, c: &mut Canvas, t: f32, scroll: f32, u: f32) {
        if self.haze.as_ref().is_none_or(|h| (h.w, h.h) != (c.w, c.h)) {
            self.haze = Some(haze(c.w, c.h));
        }
        if let Some(h) = &self.haze {
            c.data.copy_from_slice(&h.data);
        }
        let (w, ht) = (c.w as f32, c.h as f32);
        let edge = 24.0 * u;
        for (d, depth) in DEPTHS.iter().enumerate() {
            for k in 0..depth.n {
                let id = k + 1000 * d as u32;
                let x = (hash(id, 1) * (w + edge) + t * depth.speed * u).rem_euclid(w + edge);
                let x = x - edge / 2.0;
                let y = (hash(id, 2) * ht - scroll * depth.follow).rem_euclid(ht);
                // In at one edge, out at the other, never popping.
                let fade = (x.min(w - x) / edge).clamp(0.0, 1.0);
                let twinkle = 0.6 + 0.4 * (t * (0.5 + hash(id, 3) * 1.5) + hash(id, 4) * 6.3).sin();
                // Most are pale; a few near ones carry the shelf's colours.
                let tint = hash(id, 5);
                let col = if d == 2 && tint < 0.5 {
                    Rgba::hsl(if tint < 0.25 { FROM } else { TO }, 0.6, 0.75)
                } else {
                    Rgba::hsl(220.0, 0.35, 0.86)
                };
                let a = depth.alpha * (0.5 + 0.5 * hash(id, 6)) * twinkle * fade;
                c.circle(x, y, depth.r * u, col.fade(a));
            }
        }
    }
}

/// The haze: the shelf's two colours, very faint, over the page's dark,
/// dithered so it does not band.
fn haze(w: i32, h: i32) -> Canvas {
    let mut c = Canvas::new(w, h);
    c.clear(BG);
    let (wf, hf) = (w as f32, h as f32);
    let big = wf.max(hf);
    c.glow(
        0.8 * wf,
        0.12 * hf,
        0.6 * big,
        Rgba::hsl(TO, 0.55, 0.32).fade(0.22),
    );
    c.glow(
        0.12 * wf,
        0.92 * hf,
        0.55 * big,
        Rgba::hsl(FROM, 0.5, 0.26).fade(0.16),
    );
    c.glow(
        0.5 * wf,
        0.5 * hf,
        0.7 * big,
        Rgba::hsl(225.0, 0.5, 0.2).fade(0.12),
    );
    for (i, px) in c.data.chunks_exact_mut(4).enumerate() {
        let d = (hash(i as u32, 7) * 3.0) as i32 - 1;
        for v in &mut px[..3] {
            *v = (*v as i32 + d).clamp(0, 255) as u8;
        }
    }
    c
}

/// The name at (`x`, `y`), `scale` a font pixel: each letter its place in
/// the band, and a glint crossing it every `GLINT_EVERY` seconds.
pub fn wordmark(c: &mut Canvas, x: i32, y: i32, scale: i32, t: f32) {
    let n = TITLE.chars().count() as f32;
    // From well before the first letter to well after the last: a pause.
    let glint = (t / GLINT_EVERY).fract() * 2.0 - 0.5;
    let mut at = x;
    for (i, ch) in TITLE.chars().enumerate() {
        let p = (i as f32 + 0.5) / n;
        let base = Rgba::hsl(FROM + (TO - FROM) * i as f32 / (n - 1.0), 0.66, 0.62);
        let k = (1.0 - (p - glint).abs() / 0.12).max(0.0).powi(2);
        if k > 0.0 {
            let cx = at as f32 + 2.5 * scale as f32;
            let cy = y as f32 + 3.5 * scale as f32;
            c.glow(
                cx,
                cy,
                6.0 * scale as f32,
                Rgba::rgb(255, 248, 235).fade(0.12 * k),
            );
        }
        let col = base.mix(Rgba::rgb(255, 250, 240), 0.7 * k);
        at += c.text_shadowed(at, y, &ch.to_string(), scale, col);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_haze_is_dark_and_the_stars_are_few() {
        // Dark everywhere: a haze, never a wash of colour.
        let h = haze(320, 200);
        assert!(h
            .data
            .chunks_exact(4)
            .all(|p| p[0] < 60 && p[1] < 60 && p[2] < 80 && p[3] == 255));
        let mut c = Canvas::new(320, 200);
        let mut b = Backdrop::default();
        for t in [0.0, 1.5, 60.0, 3600.0] {
            b.draw(&mut c, t, 120.0, 1.0);
            // Opaque, and the stars a sprinkle, not a field.
            assert!(c.data.chunks_exact(4).all(|p| p[3] == 255));
            let lit = c
                .data
                .chunks_exact(4)
                .zip(h.data.chunks_exact(4))
                .filter(|(p, q)| p[2] > q[2] + 8)
                .count();
            assert!(lit > 10 && lit < 320 * 200 / 100, "{t}: {lit}");
        }
        // A new size, a new haze.
        let mut d = Canvas::new(100, 300);
        b.draw(&mut d, 1.0, 0.0, 2.0);
        assert_eq!(b.haze.as_ref().map(|h| (h.w, h.h)), Some((100, 300)));
    }

    #[test]
    fn the_name_holds_to_the_band_between_glints() {
        let mut c = Canvas::new(400, 40);
        c.clear(BG);
        // Halfway through the pause after a glint: no letter lit.
        wordmark(&mut c, 0, 0, 2, GLINT_EVERY * 0.9);
        let lit = c
            .data
            .chunks_exact(4)
            .any(|p| p[0] > 230 && p[1] > 230 && p[2] > 230);
        assert!(!lit);
    }
}
