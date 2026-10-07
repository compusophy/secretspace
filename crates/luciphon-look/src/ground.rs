//! The ground, baked once per chunk (512x512 pixels) as its tiles arrive,
//! and copied to the screen in rows. Each kind has its colour and grain,
//! tile by tile; glow-moss is a decal on it; where land ends, a cliff face
//! drops into the Dark.

use std::collections::HashMap;

use luciphon::tiles::{ground, obj, Tiles, CHUNK};
use pixels::{Canvas, Rgba};

use crate::palette::{self, hash, shade};

pub const TILE: i32 = 16;
const SIDE: i32 = CHUNK * TILE;
const HALF: i32 = luciphon::tiles::SIZE / 2;

#[derive(Default)]
pub struct Ground {
    chunks: HashMap<(i32, i32), Canvas>,
}

fn tile_pixels(c: &mut Canvas, tiles: &Tiles, tx: i32, ty: i32, ox: i32, oy: i32) {
    let t = tiles.get(tx, ty);
    let kind = t.kind();
    let h = hash(tx, ty);
    if kind == ground::VOID {
        // Transparent, but under land a cliff face falls away.
        for py in 0..TILE {
            for px in 0..TILE {
                let i = (((oy + py) * c.w + ox + px) * 4) as usize;
                c.data[i + 3] = 0;
            }
        }
        let above = tiles.get(tx, ty - 1);
        if !above.void() {
            let (base, _) = palette::ground(above.kind());
            let earth = base.mix(Rgba::rgb(40, 30, 40), 0.7);
            for py in 0..7 {
                let band = shade(earth, -(py * 5) - ((py == 3) as i32) * 12);
                for px in 0..TILE {
                    let jag = (hash(tx * 16 + px, ty) % 3) as i32;
                    if py < 7 - jag {
                        c.fill_rect(ox + px, oy + py, 1, 1, band);
                    }
                }
            }
        }
        return;
    }
    let (base, var) = palette::ground(kind);
    let tone = shade(base, (h % (2 * var as u32 + 1)) as i32 - var);
    c.fill_rect(ox, oy, TILE, TILE, tone);
    // Grain: a few lighter and darker specks, fixed per tile.
    for k in 0..6 {
        let s = hash(tx * 7 + k, ty * 13 - k);
        let (px, py) = ((s % 16) as i32, ((s >> 8) % 16) as i32);
        let d = if s & 1 == 0 { 10 } else { -10 };
        c.fill_rect(ox + px, oy + py, 1, 1, shade(tone, d));
    }
    match kind {
        ground::MARBLE => {
            // Flagstones.
            let line = shade(tone, -18);
            c.fill_rect(ox, oy + TILE - 1, TILE, 1, line);
            c.fill_rect(ox + TILE - 1, oy, 1, TILE, line);
        }
        ground::MEADOW | ground::MOSS => {
            for k in 0..3 {
                let s = hash(tx + k * 31, ty - k * 17);
                let (px, py) = ((s % 14) as i32 + 1, ((s >> 6) % 13) as i32 + 2);
                let blade = shade(tone, 22);
                c.fill_rect(ox + px, oy + py, 1, 2, blade);
            }
        }
        ground::WATER => {
            let r = shade(tone, 24);
            let y = (h % 10) as i32 + 3;
            c.fill_rect(ox + 3, oy + y, 6, 1, r);
        }
        ground::ICE => {
            let r = shade(tone, 30);
            c.fill_rect(ox + (h % 9) as i32 + 2, oy + 4, 4, 1, r);
            c.fill_rect(ox + (h % 5) as i32 + 6, oy + 10, 3, 1, r);
        }
        ground::GLASS if h.is_multiple_of(3) => {
            c.fill_rect(
                ox + (h % 13) as i32 + 1,
                oy + ((h >> 5) % 13) as i32 + 1,
                1,
                1,
                palette::RIM,
            );
        }
        _ => {}
    }
    if t.obj == obj::GLOWMOSS {
        for k in 0..5 {
            let s = hash(tx * 3 + k, ty * 5 + k);
            let (px, py) = ((s % 14) as i32 + 1, ((s >> 7) % 14) as i32 + 1);
            c.fill_rect(ox + px, oy + py, 1, 1, palette::TEAL);
        }
    }
}

impl Ground {
    /// Bake chunk (cx, cy) from these tiles.
    pub fn bake(&mut self, tiles: &Tiles, cx: i32, cy: i32) {
        let mut c = Canvas::new(SIDE, SIDE);
        for y in 0..CHUNK {
            for x in 0..CHUNK {
                let (tx, ty) = (cx * CHUNK + x - HALF, cy * CHUNK + y - HALF);
                tile_pixels(&mut c, tiles, tx, ty, x * TILE, y * TILE);
            }
        }
        self.chunks.insert((cx, cy), c);
    }

    /// Redraw one tile of a baked chunk (and the cliff under it).
    pub fn retile(&mut self, tiles: &Tiles, tx: i32, ty: i32) {
        for (x, y) in [(tx, ty), (tx, ty + 1)] {
            let (cx, cy) = Tiles::chunk_of(x, y);
            if let Some(c) = self.chunks.get_mut(&(cx, cy)) {
                let (ox, oy) = (
                    (x + HALF - cx * CHUNK) * TILE,
                    (y + HALF - cy * CHUNK) * TILE,
                );
                tile_pixels(c, tiles, x, y, ox, oy);
            }
        }
    }

    pub fn forget(&mut self, cx: i32, cy: i32) {
        self.chunks.remove(&(cx, cy));
    }

    pub fn has(&self, cx: i32, cy: i32) -> bool {
        self.chunks.contains_key(&(cx, cy))
    }

    /// Copy the baked ground under a camera whose top-left screen pixel is
    /// world pixel (left, top).
    pub fn draw(&self, out: &mut Canvas, left: i32, top: i32) {
        for (&(cx, cy), c) in &self.chunks {
            let (wx, wy) = ((cx * CHUNK - HALF) * TILE, (cy * CHUNK - HALF) * TILE);
            let (dx, dy) = (wx - left, wy - top);
            let (x0, x1) = (dx.max(0), (dx + SIDE).min(out.w));
            let (y0, y1) = (dy.max(0), (dy + SIDE).min(out.h));
            if x0 >= x1 || y0 >= y1 {
                continue;
            }
            for y in y0..y1 {
                let src = (((y - dy) * SIDE + (x0 - dx)) * 4) as usize;
                let dst = ((y * out.w + x0) * 4) as usize;
                let n = ((x1 - x0) * 4) as usize;
                let (s, d) = (&c.data[src..src + n], &mut out.data[dst..dst + n]);
                for (sp, dp) in s.chunks_exact(4).zip(d.chunks_exact_mut(4)) {
                    if sp[3] != 0 {
                        dp.copy_from_slice(sp);
                    }
                }
            }
        }
    }
}
