//! One chunk of the island as one mesh: the ground (each tile its kind's
//! colour, land tinted by its claim's hue, natural ground a little uneven),
//! the cliffs where it meets the Dark, the island's rocky underside hanging
//! deeper toward the middle (with stalactites), everything that stands on
//! it, and the lights some of those give.

use std::collections::HashMap;

use lucilook::palette;
use luciphon::tiles::{ground, Tile, Tiles, CHUNK};
use pixels::Rgba;

use super::shapes::{hash, mix, rgb, scale, unit, Geo, V3};
use super::things::{self, Light};

const ROCK: V3 = rgb(58, 56, 72);
const VEIN: V3 = rgb(95, 211, 200);

pub struct Built {
    pub geo: Geo,
    pub lights: Vec<Light>,
}

pub fn colour(c: Rgba) -> V3 {
    rgb(c.0, c.1, c.2)
}

/// A soul's hue (0-255 a turn) as a colour.
pub fn hue(h: u8) -> V3 {
    colour(Rgba::hsl(h as f32 / 256.0 * 360.0, 0.6, 0.6))
}

fn natural(t: Tile) -> bool {
    matches!(
        t.kind(),
        ground::MEADOW | ground::MOSS | ground::DARK | ground::MUD
    )
}

/// The ground's height at a tile corner: natural ground is a little
/// uneven, everything else flat (so its edges meet).
fn rise(tiles: &Tiles, x: i32, y: i32) -> f32 {
    let around = [(x - 1, y - 1), (x, y - 1), (x - 1, y), (x, y)];
    if around.iter().all(|&(a, b)| natural(tiles.get(a, b))) {
        (unit(hash(x, y, 77)) - 0.5) * 0.09
    } else {
        0.0
    }
}

/// How far below the ground the island's underside is at a corner:
/// shallow at the Rim, deep under the middle.
fn depth(x: i32, y: i32) -> f32 {
    let r = ((x * x + y * y) as f32).sqrt();
    1.2 + (62.0 - r).max(0.0) * 0.55 + unit(hash(x, y, 91)) * 0.8
}

/// The colour of a tile's ground, its claim's hue mixed in.
fn ground_colour(t: Tile, x: i32, y: i32, hues: &HashMap<u16, u8>) -> (V3, f32) {
    let (c, var) = palette::ground(t.kind());
    let d = (unit(hash(x, y, 5)) - 0.5) * 2.0 * var as f32 / 255.0;
    let mut col = [
        (c.0 as f32 / 255.0 + d).clamp(0.0, 1.0),
        (c.1 as f32 / 255.0 + d).clamp(0.0, 1.0),
        (c.2 as f32 / 255.0 + d).clamp(0.0, 1.0),
    ];
    let mut glow = match t.kind() {
        ground::GLASS => 0.12,
        ground::WATER => 0.06,
        ground::ICE => 0.05,
        _ => 0.0,
    };
    if let Some(&h) = hues.get(&t.owner()) {
        let wick = t.wick().is_some();
        col = mix(col, hue(h), if wick { 0.45 } else { 0.22 });
        if wick {
            glow += 0.25;
        }
    }
    (col, glow)
}

/// Build chunk (cx, cy) from the tiles the page knows.
pub fn build(tiles: &Tiles, cx: i32, cy: i32, hues: &HashMap<u16, u8>) -> Built {
    let mut g = Geo::default();
    let mut lights = Vec::new();
    let (x0, y0) = (cx * CHUNK - 64, cy * CHUNK - 64);
    for y in y0..y0 + CHUNK {
        for x in x0..x0 + CHUNK {
            let t = tiles.get(x, y);
            if t.void() {
                continue;
            }
            let (fx, fz) = (x as f32, y as f32);
            let corner = |a: i32, b: i32| [a as f32, rise(tiles, a, b), b as f32];
            let (a, b, c, d) = (
                corner(x, y),
                corner(x, y + 1),
                corner(x + 1, y + 1),
                corner(x + 1, y),
            );
            let (col, glow) = ground_colour(t, x, y, hues);
            g.quad(a, b, c, d, col, glow);
            // Cliffs down into the Dark, where the island ends.
            let inside = [fx + 0.5, -0.5, fz + 0.5];
            let cliff = scale(mix(ROCK, col, 0.25), 0.9);
            for (nx, ny, p, q) in [
                (x, y - 1, (x, y), (x + 1, y)),
                (x + 1, y, (x + 1, y), (x + 1, y + 1)),
                (x, y + 1, (x + 1, y + 1), (x, y + 1)),
                (x - 1, y, (x, y + 1), (x, y)),
            ] {
                if !tiles.get(nx, ny).void() {
                    continue;
                }
                let top = |(a, b): (i32, i32)| corner(a, b);
                let low = |(a, b): (i32, i32)| [a as f32, -depth(a, b), b as f32];
                let shade = 0.75 + 0.25 * unit(hash(nx, ny, 3));
                g.tri_out(top(p), top(q), low(q), inside, scale(cliff, shade), 0.0);
                g.tri_out(
                    top(p),
                    low(q),
                    low(p),
                    inside,
                    scale(cliff, shade * 0.8),
                    0.0,
                );
            }
            // The underside, and now and then a stalactite.
            let low = |a: i32, b: i32| [a as f32, -depth(a, b), b as f32];
            let up = [fx + 0.5, 0.0, fz + 0.5];
            let h = unit(hash(x, y, 13));
            let vein = h < 0.12;
            let under = if vein {
                mix(ROCK, VEIN, 0.35)
            } else {
                scale(ROCK, 0.6 + 0.3 * h)
            };
            let glow_under = if vein { 0.35 } else { 0.0 };
            let (la, lb, lc, ld) = (low(x, y), low(x, y + 1), low(x + 1, y + 1), low(x + 1, y));
            g.tri_out(la, lb, lc, up, under, glow_under);
            g.tri_out(la, lc, ld, up, under, glow_under);
            if h > 0.93 {
                let base = [fx + 0.5, (la[1] + lc[1]) / 2.0 + 0.1, fz + 0.5];
                let long = 1.0 + 5.0 * unit(hash(x, y, 14));
                g.spike(
                    base,
                    [base[0], base[1] - long, base[2]],
                    0.45,
                    5,
                    scale(ROCK, 0.7),
                    0.0,
                );
            }
            if t.obj != 0 {
                let owner = hues.get(&t.owner()).map_or(palette::INK, |&h| {
                    let c = hue(h);
                    Rgba(
                        (c[0] * 255.0) as u8,
                        (c[1] * 255.0) as u8,
                        (c[2] * 255.0) as u8,
                        255,
                    )
                });
                things::object(&mut g, t.obj, x, y, colour(owner));
                if let Some(l) = things::light_of(t.obj, x, y) {
                    lights.push(l);
                }
            }
        }
    }
    Built { geo: g, lights }
}

#[cfg(test)]
mod tests {
    use super::*;
    use luciphon::island;
    use luciphon::laws::LAWS;
    use luciphon::tiles::CHUNKS;

    #[test]
    fn the_whole_island_builds_and_the_luciphon_lights_it() {
        let (tiles, _) = island::generate(&LAWS, LAWS.world_seed as u64);
        let hues = HashMap::new();
        let (mut verts, mut lights) = (0, Vec::new());
        for cy in 0..CHUNKS {
            for cx in 0..CHUNKS {
                let b = build(&tiles, cx, cy, &hues);
                assert!(b.geo.v.iter().all(|v| v.is_finite()));
                verts += b.geo.len();
                lights.extend(b.lights);
            }
        }
        eprintln!("{verts} vertices, {} lights", lights.len());
        assert!(verts > 50_000 && verts < 2_000_000, "{verts}");
        assert!(lights.iter().any(|l| l.r > 20.0), "the Luciphon");
    }
}
