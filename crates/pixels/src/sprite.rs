//! Sprites: little pictures written as text grids in Rust source, one
//! character a pixel, coloured at draw time by a palette function (so one
//! grid serves every hue). Drawing them is integer only: alpha-keyed, or
//! added for glows. And two whole-canvas passes: `invert` (the Underlight)
//! and `light` (an integer multiply by a low-resolution light map, with a
//! 4x4 Bayer dither so its steps never band).

use crate::{Canvas, Rgba};

/// A grid of characters, '.' or ' ' for nothing.
#[derive(Clone, Copy, Debug)]
pub struct Grid {
    pub rows: &'static [&'static str],
}

impl Grid {
    pub const fn new(rows: &'static [&'static str]) -> Grid {
        Grid { rows }
    }

    pub fn w(&self) -> i32 {
        self.rows.iter().map(|r| r.len()).max().unwrap_or(0) as i32
    }

    pub fn h(&self) -> i32 {
        self.rows.len() as i32
    }
}

const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

impl Canvas {
    /// Draw a grid with its top-left at (x, y), each pixel `scale` square,
    /// mirrored if `flip`. `paint` colours a character (None: nothing).
    pub fn grid(
        &mut self,
        g: &Grid,
        x: i32,
        y: i32,
        scale: i32,
        flip: bool,
        paint: &dyn Fn(u8) -> Option<Rgba>,
    ) {
        let w = g.w();
        for (gy, row) in g.rows.iter().enumerate() {
            for (gx, ch) in row.bytes().enumerate() {
                if ch == b'.' || ch == b' ' {
                    continue;
                }
                let Some(c) = paint(ch) else {
                    continue;
                };
                let gx = if flip { w - 1 - gx as i32 } else { gx as i32 };
                let (px, py) = (x + gx * scale, y + gy as i32 * scale);
                if scale == 1 {
                    self.pixel(px, py, c);
                } else {
                    self.fill_rect(px, py, scale, scale, c);
                }
            }
        }
    }

    /// Add light: each channel gains `c` scaled by `k` (0-255), saturating.
    #[inline]
    pub fn add(&mut self, x: i32, y: i32, c: Rgba, k: u32) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        let i = ((y * self.w + x) * 4) as usize;
        let k = k * c.3 as u32 / 255;
        for (ch, v) in [c.0, c.1, c.2].into_iter().enumerate() {
            let p = &mut self.data[i + ch];
            *p = (*p as u32 + v as u32 * k / 255).min(255) as u8;
        }
    }

    /// A soft additive glow, integer only: brightest at the centre.
    pub fn glow_add(&mut self, cx: i32, cy: i32, r: i32, c: Rgba) {
        if r <= 0 {
            return;
        }
        let r2 = r * r;
        for dy in -r..=r {
            let y = cy + dy;
            if y < 0 || y >= self.h {
                continue;
            }
            for dx in -r..=r {
                let d2 = dx * dx + dy * dy;
                if d2 < r2 {
                    let f = (r2 - d2) as u32 * 255 / r2 as u32;
                    self.add(cx + dx, y, c, f * f / 255);
                }
            }
        }
    }

    /// The whole picture in negative.
    pub fn invert(&mut self) {
        for px in self.data.chunks_exact_mut(4) {
            px[0] = 255 - px[0];
            px[1] = 255 - px[1];
            px[2] = 255 - px[2];
        }
    }

    /// Multiply the picture by a light map of `lw` x `lh` cells, each
    /// `cell` pixels square, holding 0-255 per channel (r, g, b). Cells are
    /// blended to their neighbours with a Bayer dither.
    pub fn light(&mut self, map: &[[u8; 3]], lw: i32, lh: i32, cell: i32) {
        if lw <= 0 || lh <= 0 || map.len() < (lw * lh) as usize {
            return;
        }
        let cell = cell.max(1);
        for y in 0..self.h {
            for x in 0..self.w {
                // Dither between this cell and the next one along.
                let t = BAYER[(y & 3) as usize][(x & 3) as usize] as i32 * cell / 16;
                let lx = ((x + t) / cell).min(lw - 1);
                let ly = ((y + t) / cell).min(lh - 1);
                let l = map[(ly * lw + lx) as usize];
                let i = ((y * self.w + x) * 4) as usize;
                for (ch, &k) in l.iter().enumerate() {
                    self.data[i + ch] = (self.data[i + ch] as u32 * k as u32 / 255) as u8;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grids_draw_flip_and_skip_dots() {
        let g = Grid::new(&["ab.", ".a."]);
        let mut c = Canvas::new(4, 2);
        c.clear(Rgba::rgb(0, 0, 0));
        let paint = |ch: u8| match ch {
            b'a' => Some(Rgba::rgb(255, 0, 0)),
            b'b' => Some(Rgba::rgb(0, 255, 0)),
            _ => None,
        };
        c.grid(&g, 0, 0, 1, false, &paint);
        assert_eq!(&c.data[0..3], &[255, 0, 0]);
        assert_eq!(&c.data[4..7], &[0, 255, 0]);
        assert_eq!(&c.data[8..11], &[0, 0, 0]);
        c.clear(Rgba::rgb(0, 0, 0));
        c.grid(&g, 0, 0, 1, true, &paint);
        assert_eq!(&c.data[8..11], &[255, 0, 0], "mirrored");
        c.invert();
        assert_eq!(&c.data[0..3], &[255, 255, 255]);
        c.light(&[[128, 128, 128]], 1, 1, 8);
        assert_eq!(c.data[0], 128);
        c.add(0, 0, Rgba::rgb(255, 255, 255), 255);
        assert_eq!(c.data[0], 255);
    }
}
