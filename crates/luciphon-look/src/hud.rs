//! The HUD lives in the world: a Flame arc under your feet with breath
//! inside it, the charge ring closing to a bright band in the perfect
//! window, a marker at the screen's edge toward the Luciphon, the Heart,
//! and the Underlight: the last picture in negative, your path as one
//! thread of light, your killer marked.

use pixels::{Canvas, Rect, Rgba};

use crate::palette::{GOLD, INK, RIM};

/// An arc of an ellipse under the feet (radii `r`), `k` of it filled.
fn arc(c: &mut Canvas, (sx, sy): (f32, f32), (rx, ry): (f32, f32), k: f32, on: Rgba, off: Rgba) {
    let steps = 28;
    for i in 0..steps {
        // From the left, under the feet, to the right.
        let t = i as f32 / (steps - 1) as f32;
        let a = std::f32::consts::PI * (1.0 - t);
        let (x, y) = (sx - a.cos() * rx, sy + a.sin() * ry);
        c.pixel(x as i32, y as i32, if t <= k { on } else { off });
    }
}

/// Your Flame and breath, under your feet at screen (sx, sy).
pub fn vitals(c: &mut Canvas, sx: f32, sy: f32, flame: f32, breath: f32) {
    let low = flame < 0.3;
    let fc = if low { Rgba::rgb(255, 122, 61) } else { GOLD };
    arc(c, (sx, sy), (11.0, 5.0), flame, fc, Rgba(255, 210, 122, 40));
    arc(
        c,
        (sx, sy),
        (7.0, 3.0),
        breath,
        RIM,
        Rgba(127, 224, 255, 30),
    );
}

/// The charge ring: closing from wide to tight by `charge_full`, a bright
/// band in the perfect window, grey past the overcharge.
pub fn charge(c: &mut Canvas, sx: f32, sy: f32, ch: u32, l: &luciphon::laws::Laws) {
    let k = (ch as f32 / l.charge_full as f32).min(1.0);
    let r = 22.0 - 14.0 * k;
    let perfect = (l.charge_full..=l.perfect_to).contains(&ch);
    let over = ch > l.charge_max;
    let col = if over {
        Rgba(160, 160, 170, 140)
    } else if perfect {
        Rgba(255, 245, 200, 255)
    } else if ch >= l.charge_min {
        Rgba(255, 210, 122, 200)
    } else {
        Rgba(255, 210, 122, 90)
    };
    c.ring(sx, sy - 8.0, r, if perfect { 2.5 } else { 1.2 }, col);
}

/// A throw's arrow, from you along `h`, `len` pixels; grey without glim.
pub fn arrow(c: &mut Canvas, sx: f32, sy: f32, h: u16, len: f32, lit: bool) {
    let (ux, uy) = engine::fixed::unit(h);
    let (ux, uy) = (ux.to_f32(), uy.to_f32());
    let (ex, ey) = (sx + ux * len, sy - 8.0 + uy * len);
    let col = if lit { GOLD } else { Rgba(150, 150, 160, 200) };
    c.line(sx, sy - 8.0, ex, ey, 1.5, col.fade(0.8));
    c.line(
        ex,
        ey,
        ex - ux * 5.0 - uy * 4.0,
        ey - uy * 5.0 + ux * 4.0,
        1.5,
        col,
    );
    c.line(
        ex,
        ey,
        ex - ux * 5.0 + uy * 4.0,
        ey - uy * 5.0 - ux * 4.0,
        1.5,
        col,
    );
}

/// A marker at the screen's edge toward a point off screen.
pub fn marker(c: &mut Canvas, x: f32, y: f32, col: Rgba) {
    let (w, h) = (c.w as f32, c.h as f32);
    if x >= 0.0 && y >= 0.0 && x < w && y < h {
        return;
    }
    let (cx, cy) = (w / 2.0, h / 2.0);
    let (dx, dy) = (x - cx, y - cy);
    let k = ((cx - 10.0) / dx.abs().max(1e-3)).min((cy - 10.0) / dy.abs().max(1e-3));
    let (mx, my) = (cx + dx * k, cy + dy * k);
    let a = dy.atan2(dx);
    let tip = (mx + a.cos() * 6.0, my + a.sin() * 6.0);
    let l = (mx + (a + 2.5).cos() * 5.0, my + (a + 2.5).sin() * 5.0);
    let r = (mx + (a - 2.5).cos() * 5.0, my + (a - 2.5).sin() * 5.0);
    c.line(tip.0, tip.1, l.0, l.1, 2.0, col);
    c.line(tip.0, tip.1, r.0, r.1, 2.0, col);
    c.glow_add(mx as i32, my as i32, 5, col);
}

/// The Heart: the one button, a glyph in a ring.
pub fn heart(c: &mut Canvas, b: Rect, pressed: bool, now: f64) {
    let (cx, cy, r) = (b.x + b.w / 2.0, b.y + b.h / 2.0, b.w.min(b.h) / 2.0);
    let glow = if pressed {
        255
    } else {
        150 + ((now / 700.0).sin() * 40.0) as i32
    } as u8;
    c.circle(cx, cy, r, Rgba(20, 18, 34, 170));
    c.ring(cx, cy, r - 1.0, 1.5, Rgba(255, 210, 122, glow));
    // A small heart of light.
    let s = r * 0.32;
    c.circle(
        cx - s * 0.55,
        cy - s * 0.2,
        s * 0.6,
        Rgba(255, 210, 122, glow),
    );
    c.circle(
        cx + s * 0.55,
        cy - s * 0.2,
        s * 0.6,
        Rgba(255, 210, 122, glow),
    );
    for k in 0..(s * 1.4) as i32 {
        let half = s * 1.1 * (1.0 - k as f32 / (s * 1.4));
        c.line(
            cx - half,
            cy + k as f32 * 0.8,
            cx + half,
            cy + k as f32 * 0.8,
            1.2,
            Rgba(255, 210, 122, glow),
        );
    }
}

/// The Underlight: the frame you fell in, in negative, held for the
/// Descent with your path drawn as one thread of light.
#[derive(Default)]
pub struct Underlight {
    frame: Option<Canvas>,
    pub since: f64,
    pub active: bool,
}

impl Underlight {
    /// Fallen: keep this picture.
    pub fn begin(&mut self, c: &Canvas, now: f64) {
        let mut f = c.clone();
        f.invert();
        self.frame = Some(f);
        self.since = now;
        self.active = true;
    }

    pub fn end(&mut self) {
        self.active = false;
        self.frame = None;
    }

    /// Draw it over everything: a white flash, then the negative with the
    /// thread (screen points, oldest first) and the killer's mark.
    pub fn draw(
        &self,
        c: &mut Canvas,
        path: &[(f32, f32)],
        killer: Option<(f32, f32)>,
        now: f64,
        u: i32,
    ) {
        let Some(f) = &self.frame else {
            return;
        };
        let t = now - self.since;
        if t < 300.0 {
            let k = (1.0 - t / 300.0) as f32;
            c.fill_rect(0, 0, c.w, c.h, Rgba(255, 252, 240, (k * 255.0) as u8));
            return;
        }
        if f.w == c.w && f.h == c.h {
            c.data.copy_from_slice(&f.data);
        }
        for w in path.windows(2) {
            c.line(
                w[0].0,
                w[0].1,
                w[1].0,
                w[1].1,
                1.5,
                Rgba(255, 236, 170, 220),
            );
        }
        if let Some(&(x, y)) = path.last() {
            c.glow_add(x as i32, y as i32, 6, GOLD);
        }
        if let Some((x, y)) = killer {
            c.ring(x, y - 8.0, 10.0, 2.0, Rgba(255, 90, 60, 230));
            c.line(
                x - 6.0,
                y - 14.0,
                x + 6.0,
                y - 2.0,
                2.0,
                Rgba(255, 90, 60, 230),
            );
            c.line(
                x + 6.0,
                y - 14.0,
                x - 6.0,
                y - 2.0,
                2.0,
                Rgba(255, 90, 60, 230),
            );
        }
        let msg = "the underlight";
        let s = pixels::fit_scale(msg, c.w - 20, 2 * u);
        c.text_centred(c.w / 2, c.h / 5, msg, s, INK.fade(0.85));
    }
}
