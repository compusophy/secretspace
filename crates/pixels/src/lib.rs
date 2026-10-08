//! A software renderer: every game here draws its frame into one RGBA
//! buffer, pixel by pixel, and the page hands the whole buffer to the
//! screen at once. Shapes are antialiased at their edges; text is a 5x7
//! pixel font scaled by whole numbers. std only.

pub mod font;
pub mod sprite;

pub use sprite::Grid;

/// A colour, straight (not premultiplied) alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
        Rgba(r, g, b, 255)
    }

    /// The same colour, its alpha scaled by `a` (0..=1).
    pub fn fade(self, a: f32) -> Rgba {
        Rgba(
            self.0,
            self.1,
            self.2,
            (self.3 as f32 * a.clamp(0.0, 1.0)).round() as u8,
        )
    }

    /// From hue (degrees), saturation and lightness (0..=1).
    pub fn hsl(h: f32, s: f32, l: f32) -> Rgba {
        let h = h.rem_euclid(360.0) / 60.0;
        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let x = c * (1.0 - (h % 2.0 - 1.0).abs());
        let (r, g, b) = match h as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = l - c / 2.0;
        let k = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
        Rgba::rgb(k(r), k(g), k(b))
    }

    /// Part way from this colour to another.
    pub fn mix(self, o: Rgba, t: f32) -> Rgba {
        let t = t.clamp(0.0, 1.0);
        let k = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgba(
            k(self.0, o.0),
            k(self.1, o.1),
            k(self.2, o.2),
            k(self.3, o.3),
        )
    }
}

/// A box on the canvas, in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }

    /// Grown by `d` on every side.
    pub fn grow(&self, d: f32) -> Rect {
        Rect::new(self.x - d, self.y - d, self.w + 2.0 * d, self.h + 2.0 * d)
    }
}

#[derive(Clone)]
pub struct Canvas {
    pub w: i32,
    pub h: i32,
    /// RGBA bytes, row by row: what the screen's image wants.
    pub data: Vec<u8>,
}

impl Canvas {
    pub fn new(w: i32, h: i32) -> Canvas {
        let (w, h) = (w.max(1), h.max(1));
        Canvas {
            w,
            h,
            data: vec![0; (w * h * 4) as usize],
        }
    }

    pub fn resize(&mut self, w: i32, h: i32) {
        let (w, h) = (w.max(1), h.max(1));
        if (w, h) != (self.w, self.h) {
            *self = Canvas::new(w, h);
        }
    }

    pub fn clear(&mut self, c: Rgba) {
        for px in self.data.chunks_exact_mut(4) {
            px.copy_from_slice(&[c.0, c.1, c.2, 255]);
        }
    }

    /// Clear to nothing at all: a layer to draw over something else. A
    /// layer holds premultiplied colour (what `blend` writes over clear
    /// pixels), so light added to it (`add`) still shows.
    pub fn wipe(&mut self) {
        self.data.fill(0);
    }

    /// Blend `c` into pixel `i` with coverage `cover` (0..=255): source
    /// over, premultiplied, so an opaque picture stays opaque and a layer
    /// gains alpha.
    #[inline]
    fn blend(&mut self, i: usize, c: Rgba, cover: u32) {
        let a = c.3 as u32 * cover / 255;
        if a == 0 {
            return;
        }
        let p = &mut self.data[i * 4..i * 4 + 4];
        if a >= 255 {
            p[0] = c.0;
            p[1] = c.1;
            p[2] = c.2;
            p[3] = 255;
        } else {
            let k = |s: u8, d: u8| ((s as u32 * a + d as u32 * (255 - a) + 127) / 255) as u8;
            p[0] = k(c.0, p[0]);
            p[1] = k(c.1, p[1]);
            p[2] = k(c.2, p[2]);
            p[3] = (a + (p[3] as u32 * (255 - a) + 127) / 255) as u8;
        }
    }

    pub fn pixel(&mut self, x: i32, y: i32, c: Rgba) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.blend((y * self.w + x) as usize, c, 255);
        }
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) {
        let (x0, y0) = (x.max(0), y.max(0));
        let (x1, y1) = ((x + w).min(self.w), (y + h).min(self.h));
        for yy in y0..y1 {
            let row = (yy * self.w) as usize;
            for xx in x0..x1 {
                self.blend(row + xx as usize, c, 255);
            }
        }
    }

    pub fn hline(&mut self, x0: i32, x1: i32, y: i32, c: Rgba) {
        self.fill_rect(x0, y, x1 - x0, 1, c);
    }

    pub fn vline(&mut self, x: i32, y0: i32, y1: i32, c: Rgba) {
        self.fill_rect(x, y0, 1, y1 - y0, c);
    }

    /// Visit every pixel whose centre lies within `pad` of the box, with
    /// the pixel's centre; `f` returns coverage 0..=1.
    fn shade(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: Rgba, f: impl Fn(f32, f32) -> f32) {
        let xa = (x0.floor() as i32).max(0);
        let ya = (y0.floor() as i32).max(0);
        let xb = (x1.ceil() as i32).min(self.w - 1);
        let yb = (y1.ceil() as i32).min(self.h - 1);
        for y in ya..=yb {
            let row = (y * self.w) as usize;
            let py = y as f32 + 0.5;
            for x in xa..=xb {
                let cover = f(x as f32 + 0.5, py);
                if cover > 0.0 {
                    self.blend(row + x as usize, c, (cover.min(1.0) * 255.0) as u32);
                }
            }
        }
    }

    /// A filled disc with a soft edge.
    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, c: Rgba) {
        if r <= 0.0 {
            return;
        }
        self.shade(
            cx - r - 1.0,
            cy - r - 1.0,
            cx + r + 1.0,
            cy + r + 1.0,
            c,
            |x, y| {
                let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                r - d + 0.5
            },
        );
    }

    /// A ring `width` wide, centred on radius `r`.
    pub fn ring(&mut self, cx: f32, cy: f32, r: f32, width: f32, c: Rgba) {
        let half = width / 2.0;
        let out = r + half + 1.0;
        self.shade(cx - out, cy - out, cx + out, cy + out, c, |x, y| {
            let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
            half - (d - r).abs() + 0.5
        });
    }

    /// A soft glow: full at the centre, fading to nothing at `r`.
    pub fn glow(&mut self, cx: f32, cy: f32, r: f32, c: Rgba) {
        if r <= 0.0 {
            return;
        }
        self.shade(cx - r, cy - r, cx + r, cy + r, c, |x, y| {
            let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() / r;
            (1.0 - d).max(0.0).powi(2)
        });
    }

    /// A line `width` wide with round ends.
    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, c: Rgba) {
        let half = width / 2.0;
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len2 = (dx * dx + dy * dy).max(1e-6);
        let pad = half + 1.0;
        self.shade(
            x0.min(x1) - pad,
            y0.min(y1) - pad,
            x0.max(x1) + pad,
            y0.max(y1) + pad,
            c,
            |x, y| {
                let t = (((x - x0) * dx + (y - y0) * dy) / len2).clamp(0.0, 1.0);
                let d = ((x - x0 - t * dx).powi(2) + (y - y0 - t * dy).powi(2)).sqrt();
                half - d + 0.5
            },
        );
    }

    /// A filled rectangle with rounded corners.
    pub fn round_rect(&mut self, b: Rect, r: f32, c: Rgba) {
        let Rect { x, y, w, h } = b;
        self.shade(x - 1.0, y - 1.0, x + w + 1.0, y + h + 1.0, c, |px, py| {
            rounded(b, r, px, py)
        });
    }

    /// Part of a rounded rectangle: what lies between `from` and `to`
    /// turns about its centre (0 at the top, clockwise), as a clock's
    /// hand sweeps it.
    pub fn sweep(&mut self, b: Rect, r: f32, from: f32, to: f32, c: Rgba) {
        let Rect { x, y, w, h } = b;
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        self.shade(x - 1.0, y - 1.0, x + w + 1.0, y + h + 1.0, c, |px, py| {
            let t = (px - cx).atan2(cy - py) / std::f32::consts::TAU;
            let t = if t < 0.0 { t + 1.0 } else { t };
            if t < from || t > to {
                return 0.0;
            }
            rounded(b, r, px, py)
        });
    }

    /// A filled polygon (a simple outline, either way round) with soft
    /// edges.
    pub fn poly(&mut self, pts: &[(f32, f32)], c: Rgba) {
        if pts.len() < 3 {
            return;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &(x, y) in pts {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
        let n = pts.len();
        self.shade(x0 - 1.0, y0 - 1.0, x1 + 1.0, y1 + 1.0, c, |px, py| {
            let mut inside = false;
            let mut d2 = f32::MAX;
            for i in 0..n {
                let (ax, ay) = pts[i];
                let (bx, by) = pts[(i + 1) % n];
                if (ay > py) != (by > py) && px < ax + (py - ay) * (bx - ax) / (by - ay) {
                    inside = !inside;
                }
                let (dx, dy) = (bx - ax, by - ay);
                let t = (((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy).max(1e-9))
                    .clamp(0.0, 1.0);
                d2 = d2.min((px - ax - t * dx).powi(2) + (py - ay - t * dy).powi(2));
            }
            let d = d2.sqrt();
            if inside {
                d + 0.5
            } else {
                0.5 - d
            }
        });
    }

    /// The outline of a rounded rectangle, `width` wide.
    pub fn round_rect_line(&mut self, b: Rect, r: f32, width: f32, c: Rgba) {
        let Rect { x, y, w, h } = b;
        let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
        let (ix0, iy0, ix1, iy1) = (x + r, y + r, x + w - r, y + h - r);
        let half = width / 2.0;
        self.shade(
            x - width,
            y - width,
            x + w + width,
            y + h + width,
            c,
            |px, py| {
                let qx = (ix0 - px).max(px - ix1);
                let qy = (iy0 - py).max(py - iy1);
                // Signed distance to the rounded box's boundary.
                let outside =
                    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r;
                half - outside.abs() + 0.5
            },
        );
    }

    /// Darken everything outside a circle (the arena's edge, say).
    pub fn outside_circle(&mut self, cx: f32, cy: f32, r: f32, c: Rgba) {
        for y in 0..self.h {
            let dy = y as f32 + 0.5 - cy;
            let span = (r * r - dy * dy).max(0.0).sqrt();
            let (a, b) = if r * r > dy * dy {
                ((cx - span).floor() as i32, (cx + span).ceil() as i32)
            } else {
                (self.w, self.w)
            };
            self.hline(0, a.min(self.w), y, c);
            if b < self.w {
                self.hline(b.max(0), self.w, y, c);
            }
        }
    }

    /// Copy another canvas in at `(x, y)`, its corners rounded by `r`.
    pub fn blit(&mut self, src: &Canvas, x: i32, y: i32, r: f32) {
        let (w, h) = (src.w as f32, src.h as f32);
        for sy in 0..src.h {
            let ty = y + sy;
            if ty < 0 || ty >= self.h {
                continue;
            }
            let py = sy as f32 + 0.5;
            for sx in 0..src.w {
                let tx = x + sx;
                if tx < 0 || tx >= self.w {
                    continue;
                }
                let px = sx as f32 + 0.5;
                // Coverage: whole, except in the rounded corners.
                let qx = (r - px).max(px - (w - r)).max(0.0);
                let qy = (r - py).max(py - (h - r)).max(0.0);
                let cover = if qx > 0.0 && qy > 0.0 {
                    (r - (qx * qx + qy * qy).sqrt() + 0.5).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let i = ((sy * src.w + sx) * 4) as usize;
                let c = Rgba(src.data[i], src.data[i + 1], src.data[i + 2], 255);
                self.blend((ty * self.w + tx) as usize, c, (cover * 255.0) as u32);
            }
        }
    }

    /// Text in the pixel font at a whole-number scale. Returns its width.
    pub fn text(&mut self, x: i32, y: i32, s: &str, scale: i32, c: Rgba) -> i32 {
        let scale = scale.max(1);
        let mut cx = x;
        for ch in s.chars() {
            let g = font::glyph(ch);
            for (row, bits) in g.iter().enumerate() {
                for col in 0..5 {
                    if bits & (0x10 >> col) != 0 {
                        self.fill_rect(cx + col * scale, y + row as i32 * scale, scale, scale, c);
                    }
                }
            }
            cx += font::CELL_W * scale;
        }
        cx - x
    }

    /// Text with a one-pixel shadow under it, for reading over anything.
    pub fn text_shadowed(&mut self, x: i32, y: i32, s: &str, scale: i32, c: Rgba) -> i32 {
        self.text(
            x + scale.max(1),
            y + scale.max(1),
            s,
            scale,
            Rgba(0, 0, 0, c.3 / 2 + c.3 / 4),
        );
        self.text(x, y, s, scale, c)
    }

    /// Text centred on `cx`.
    pub fn text_centred(&mut self, cx: i32, y: i32, s: &str, scale: i32, c: Rgba) {
        let w = text_width(s, scale);
        self.text_shadowed(cx - w / 2, y, s, scale, c);
    }
}

/// How wide `s` is at this scale, without the trailing column of air.
pub fn text_width(s: &str, scale: i32) -> i32 {
    let n = s.chars().count() as i32;
    if n == 0 {
        0
    } else {
        (n * font::CELL_W - 1) * scale.max(1)
    }
}

/// The biggest whole scale, up to `max`, at which `s` fits in `width`.
pub fn fit_scale(s: &str, width: i32, max: i32) -> i32 {
    (1..=max.max(1))
        .rev()
        .find(|&k| text_width(s, k) <= width)
        .unwrap_or(1)
}

/// `s` broken into lines no wider than `width` at this scale, at spaces
/// where it can be (a word longer than a line is cut).
pub fn wrap(s: &str, width: i32, scale: i32) -> Vec<String> {
    let per = ((width / scale.max(1) + 1) / font::CELL_W).max(1) as usize;
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in s.split(' ') {
        let mut word = word.to_string();
        while word.chars().count() > per {
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            let head: String = word.chars().take(per).collect();
            word = word.chars().skip(per).collect();
            lines.push(head);
        }
        let need = line.chars().count() + usize::from(!line.is_empty()) + word.chars().count();
        if need > per && !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&word);
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}

/// How much of the pixel centred at `(px, py)` a rounded rectangle covers.
fn rounded(b: Rect, r: f32, px: f32, py: f32) -> f32 {
    let Rect { x, y, w, h } = b;
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let (ix0, iy0, ix1, iy1) = (x + r, y + r, x + w - r, y + h - r);
    let qx = (ix0 - px).max(px - ix1).max(0.0);
    let qy = (iy0 - py).max(py - iy1).max(0.0);
    // Straight edges: coverage by distance to the box's own edge.
    let edge = (px - x).min(x + w - px).min(py - y).min(y + h - py);
    if qx > 0.0 && qy > 0.0 {
        r - (qx * qx + qy * qy).sqrt() + 0.5
    } else {
        edge + 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_layer_keeps_its_alpha_and_a_picture_stays_opaque() {
        let mut c = Canvas::new(2, 1);
        c.wipe();
        c.pixel(0, 0, Rgba(200, 100, 0, 128));
        // Premultiplied: half of the colour, half covered.
        assert_eq!(&c.data[0..4], &[100, 50, 0, 128]);
        c.pixel(0, 0, Rgba(200, 100, 0, 128));
        assert_eq!(c.data[3], 192);
        assert_eq!(&c.data[4..8], &[0, 0, 0, 0]);
        c.clear(Rgba::rgb(0, 0, 0));
        c.pixel(1, 0, Rgba(200, 100, 0, 128));
        assert_eq!(&c.data[4..8], &[100, 50, 0, 255]);
    }

    #[test]
    fn shapes_stay_inside_the_buffer() {
        let mut c = Canvas::new(40, 30);
        c.clear(Rgba::rgb(0, 0, 0));
        c.circle(-5.0, -5.0, 20.0, Rgba::rgb(255, 0, 0));
        c.circle(1e9, 1e9, 3.0, Rgba::rgb(255, 0, 0));
        c.line(-100.0, 15.0, 100.0, 15.0, 3.0, Rgba::rgb(0, 255, 0));
        c.round_rect(
            Rect::new(-10.0, 20.0, 100.0, 50.0),
            6.0,
            Rgba::rgb(0, 0, 255),
        );
        c.round_rect_line(
            Rect::new(2.0, 2.0, 30.0, 20.0),
            5.0,
            2.0,
            Rgba::rgb(9, 9, 9),
        );
        c.outside_circle(20.0, 15.0, 10.0, Rgba(0, 0, 0, 128));
        c.text(-3, -3, "Hi! zoë", 2, Rgba::rgb(255, 255, 255));
        c.glow(39.0, 29.0, 8.0, Rgba::rgb(255, 255, 0));
        assert_eq!(c.data.len(), 40 * 30 * 4);
    }

    #[test]
    fn a_disc_is_solid_inside_and_soft_at_the_edge() {
        let mut c = Canvas::new(21, 21);
        c.clear(Rgba::rgb(0, 0, 0));
        c.circle(10.5, 10.5, 8.0, Rgba::rgb(255, 255, 255));
        let at = |x: i32, y: i32| c.data[((y * 21 + x) * 4) as usize];
        assert_eq!(at(10, 10), 255);
        assert_eq!(at(0, 0), 0);
        let edge = at(10, 2);
        assert!(edge > 0 && edge < 255, "{edge}");
    }

    #[test]
    fn a_polygon_fills_inside_and_a_sweep_cuts_like_a_clock() {
        let mut c = Canvas::new(20, 20);
        c.clear(Rgba::rgb(0, 0, 0));
        let tri = [(2.0, 2.0), (18.0, 2.0), (2.0, 18.0)];
        c.poly(&tri, Rgba::rgb(255, 255, 255));
        let at = |c: &Canvas, x: i32, y: i32| c.data[((y * 20 + x) * 4) as usize];
        assert_eq!(at(&c, 5, 5), 255);
        assert_eq!(at(&c, 16, 16), 0);
        // Either way round, the same.
        let mut d = Canvas::new(20, 20);
        d.clear(Rgba::rgb(0, 0, 0));
        d.poly(&[tri[2], tri[1], tri[0]], Rgba::rgb(255, 255, 255));
        assert_eq!(c.data, d.data);
        // The first quarter of a sweep: the top right, not the top left.
        let mut s = Canvas::new(20, 20);
        s.clear(Rgba::rgb(0, 0, 0));
        s.sweep(
            Rect::new(0.0, 0.0, 20.0, 20.0),
            3.0,
            0.0,
            0.25,
            Rgba::rgb(255, 0, 0),
        );
        assert_eq!(at(&s, 15, 4), 255);
        assert_eq!(at(&s, 4, 4), 0);
        assert_eq!(at(&s, 15, 15), 0);
    }

    #[test]
    fn half_alpha_mixes_halfway() {
        let mut c = Canvas::new(1, 1);
        c.clear(Rgba::rgb(0, 0, 0));
        c.pixel(0, 0, Rgba(200, 100, 50, 128));
        assert_eq!(&c.data[..3], &[100, 50, 25]);
    }

    #[test]
    fn hsl_hits_the_primaries() {
        assert_eq!(Rgba::hsl(0.0, 1.0, 0.5), Rgba::rgb(255, 0, 0));
        assert_eq!(Rgba::hsl(120.0, 1.0, 0.5), Rgba::rgb(0, 255, 0));
        assert_eq!(Rgba::hsl(240.0, 1.0, 0.5), Rgba::rgb(0, 0, 255));
    }

    #[test]
    fn wrapping_keeps_every_line_inside() {
        let s = "eat the glow, grow long, make them run into you";
        for width in [30, 60, 100, 300] {
            let lines = wrap(s, width, 2);
            assert!(
                lines.iter().all(|l| text_width(l, 2) <= width.max(10)),
                "{width}: {lines:?}"
            );
            assert_eq!(lines.join(" ").replace(' ', ""), s.replace(' ', ""));
        }
        assert_eq!(fit_scale("SECRETSPACE", 390, 9), 6);
        assert_eq!(fit_scale("SECRETSPACE", 389, 9), 5);
    }

    #[test]
    fn text_measures_what_it_draws() {
        let mut c = Canvas::new(200, 20);
        assert_eq!(c.text(0, 0, "abc", 2, Rgba::rgb(1, 1, 1)), 36);
        assert_eq!(text_width("abc", 2), 34);
    }
}
