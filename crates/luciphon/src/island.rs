//! The Vale, from a seed (§6): a disc of radius 60 with a ragged Rim,
//! floating in the Dark, in four rings. The Sanctum (marble, the
//! Luciphon, four Keeper pillars, three teaching birches); the Glow (meadow,
//! birch groves, rocks, glow-moss, nothing that kills); the Dim (dark moss,
//! oak and birch, ice ponds, mud, a stream, chasms, brambles, the three
//! Wellspring sites); the Rim shelf (glass and crystal) to the edge.
//!
//! Generation runs only on the server (the page is sent tiles), so it may
//! use floats; the result is stored in snapshots, never regenerated.

use engine::rng::Rng;

use crate::laws::Laws;
use crate::tiles::{ground, obj, Tile, Tiles};

/// Which ring a point is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ring {
    Sanctum,
    Glow,
    Dim,
    Rim,
}

/// The ring at (x, y) tiles from the centre.
pub fn ring(l: &Laws, x: i32, y: i32) -> Ring {
    let r2 = x * x + y * y;
    if r2 < l.sanctum * l.sanctum {
        Ring::Sanctum
    } else if r2 < l.glow * l.glow {
        Ring::Glow
    } else if r2 < l.dim * l.dim {
        Ring::Dim
    } else {
        Ring::Rim
    }
}

/// Places that matter, besides the tiles.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sites {
    /// The three Wellspring sites in the Dim.
    pub wells: Vec<(i32, i32)>,
    /// The eight Rim checkpoints, a lap's waypoints.
    pub checkpoints: Vec<(i32, i32)>,
}

struct Gen {
    rng: Rng,
}

impl Gen {
    fn unit(&mut self) -> f64 {
        (self.rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.unit()
    }

    /// A point in the ring between radii a and b.
    fn spot(&mut self, a: f64, b: f64) -> (i32, i32) {
        let t = self.unit() * std::f64::consts::TAU;
        let r = self.range(a, b);
        ((t.cos() * r).round() as i32, (t.sin() * r).round() as i32)
    }
}

fn polar(r: f64, deg: f64) -> (i32, i32) {
    let t = deg.to_radians();
    ((t.cos() * r).round() as i32, (t.sin() * r).round() as i32)
}

/// The island from a seed.
pub fn generate(l: &Laws, seed: u64) -> (Tiles, Sites) {
    let mut g = Gen {
        rng: Rng::new(seed ^ 0x15_1a_4d),
    };
    let mut t = Tiles::default();
    let half = l.size / 2;

    // The Rim: the island's radius wanders with the angle.
    let knots: Vec<f64> = (0..32)
        .map(|_| g.range(-l.rim_noise as f64, l.rim_noise as f64))
        .collect();
    let rim = |a: f64| {
        let f = (a / std::f64::consts::TAU).rem_euclid(1.0) * 32.0;
        let (i, k) = (f.floor() as usize % 32, f.fract());
        let s = k * k * (3.0 - 2.0 * k);
        l.island as f64 + knots[i] * (1.0 - s) + knots[(i + 1) % 32] * s
    };
    for y in -half..half {
        for x in -half..half {
            let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
            let r = (fx * fx + fy * fy).sqrt();
            if r > rim(fy.atan2(fx)) {
                continue;
            }
            let kind = match ring(l, x, y) {
                Ring::Sanctum => ground::MARBLE,
                Ring::Glow => ground::MEADOW,
                Ring::Dim => ground::DARK,
                Ring::Rim => ground::GLASS,
            };
            t.set(x, y, Tile::new(kind, obj::NONE));
        }
    }

    let (glow0, glow1) = (l.sanctum as f64 + 1.0, l.glow as f64 - 0.5);
    let (dim0, dim1) = (l.glow as f64 + 0.5, l.dim as f64 - 0.5);
    let wells: Vec<(i32, i32)> = (0..3)
        .map(|k| polar(44.0, 30.0 + 120.0 * k as f64))
        .collect();
    let checkpoints: Vec<(i32, i32)> = (0..8).map(|k| polar(56.0, 45.0 * k as f64)).collect();
    let near = |p: (i32, i32), q: (i32, i32), d: i32| {
        (p.0 - q.0) * (p.0 - q.0) + (p.1 - q.1) * (p.1 - q.1) <= d * d
    };
    let kept_clear = |p: (i32, i32)| wells.iter().any(|&w| near(p, w, 4));

    // Ground patches in the Dim: ice ponds, mud fields, chasms; moss in the
    // Glow. Chasms keep 6 tiles from every Wellspring site.
    let blob = |t: &mut Tiles, c: (i32, i32), r: f64, kind: u8, ok: &dyn Fn(Tile) -> bool| {
        let ri = r.ceil() as i32 + 1;
        for y in c.1 - ri..=c.1 + ri {
            for x in c.0 - ri..=c.0 + ri {
                let (dx, dy) = ((x - c.0) as f64, (y - c.1) as f64);
                let wobble = 0.6 * ((x * 7 + y * 13) as f64).sin();
                if (dx * dx + dy * dy).sqrt() <= r + wobble && ok(t.get(x, y)) {
                    t.set(x, y, Tile::new(kind, obj::NONE));
                }
            }
        }
    };
    let dim_ground = |tile: Tile| tile.kind() == ground::DARK;
    for _ in 0..3 {
        let c = g.spot(dim0 + 4.0, dim1 - 4.0);
        let r = g.range(3.0, 5.0);
        blob(&mut t, c, r, ground::ICE, &dim_ground);
    }
    for _ in 0..2 {
        let c = g.spot(dim0 + 4.0, dim1 - 4.0);
        let r = g.range(3.0, 5.0);
        blob(&mut t, c, r, ground::MUD, &dim_ground);
    }
    let mut chasms = 0;
    while chasms < 4 {
        let c = g.spot(dim0 + 5.0, dim1 - 2.0);
        if wells.iter().any(|&w| near(c, w, 11)) {
            continue;
        }
        let r = g.range(2.0, 4.0);
        let outer = |tile: Tile| {
            matches!(
                tile.kind(),
                ground::DARK | ground::ICE | ground::MUD | ground::GLASS
            )
        };
        blob(&mut t, c, r, ground::VOID, &outer);
        chasms += 1;
    }
    for _ in 0..10 {
        let c = g.spot(glow0 + 2.0, glow1);
        let r = g.range(2.0, 4.0);
        blob(&mut t, c, r, ground::MOSS, &|tile| {
            tile.kind() == ground::MEADOW
        });
    }
    // One shallow stream, wandering across the Dim.
    let mut a = g.unit() * std::f64::consts::TAU;
    let mut r = dim0 + 2.0;
    while r < dim1 {
        let (x, y) = ((a.cos() * r).round() as i32, (a.sin() * r).round() as i32);
        for (dx, dy) in [(0, 0), (1, 0), (0, 1)] {
            if dim_ground(t.get(x + dx, y + dy)) {
                t.set(x + dx, y + dy, Tile::new(ground::WATER, obj::NONE));
            }
        }
        r += 0.5;
        a += g.range(-0.03, 0.05);
    }

    // Things on the ground.
    let place = |t: &mut Tiles, p: (i32, i32), o: u8, on: &[u8]| {
        let tile = t.get(p.0, p.1);
        if tile.obj == obj::NONE && on.contains(&tile.kind()) && !kept_clear(p) {
            t.set(p.0, p.1, Tile::new(tile.ground, o));
            true
        } else {
            false
        }
    };
    // The Sanctum: the Luciphon, the pillars, the teaching birches.
    for y in -1..=1 {
        for x in -1..=1 {
            t.set(x, y, Tile::new(ground::MARBLE, obj::LUCIPHON));
        }
    }
    for k in 0..4 {
        place(
            &mut t,
            polar(7.0, 45.0 + 90.0 * k as f64),
            obj::PILLAR,
            &[ground::MARBLE],
        );
    }
    for k in 0..3 {
        place(
            &mut t,
            polar(9.0, 90.0 + 120.0 * k as f64),
            obj::BIRCH,
            &[ground::MARBLE],
        );
    }
    // The Glow: birches in groves, rocks, glow-moss.
    let glow_tiles = |t: &Tiles| {
        t.t.iter()
            .filter(|x| matches!(x.kind(), ground::MEADOW | ground::MOSS))
            .count()
    };
    let n = glow_tiles(&t) as f64;
    let mut groves = 0;
    while (groves as f64) < n * 0.04 {
        let c = g.spot(glow0 + 1.0, glow1);
        for _ in 0..8 {
            let p = (
                c.0 + g.range(-3.0, 3.0).round() as i32,
                c.1 + g.range(-3.0, 3.0).round() as i32,
            );
            if place(&mut t, p, obj::BIRCH, &[ground::MEADOW, ground::MOSS]) {
                groves += 1;
            }
        }
    }
    for (share, o) in [(0.02, obj::ROCK), (0.015, obj::GLOWMOSS)] {
        let mut k = 0;
        while (k as f64) < n * share {
            let p = g.spot(glow0, glow1);
            if place(&mut t, p, o, &[ground::MEADOW, ground::MOSS]) {
                k += 1;
            }
        }
    }
    // The Dim: oak and birch, rocks, moss, brambles.
    let dim_n = t.t.iter().filter(|x| x.kind() == ground::DARK).count() as f64;
    for (share, o) in [
        (0.035, obj::OAK),
        (0.025, obj::BIRCH),
        (0.03, obj::ROCK),
        (0.01, obj::GLOWMOSS),
    ] {
        let mut k = 0;
        while (k as f64) < dim_n * share {
            let p = g.spot(dim0, dim1);
            if place(&mut t, p, o, &[ground::DARK]) {
                k += 1;
            }
        }
    }
    for _ in 0..6 {
        let c = g.spot(dim0 + 2.0, dim1 - 1.0);
        for _ in 0..7 {
            let p = (
                c.0 + g.range(-2.0, 2.0).round() as i32,
                c.1 + g.range(-2.0, 2.0).round() as i32,
            );
            place(&mut t, p, obj::BRAMBLE, &[ground::DARK]);
        }
    }
    // The Rim shelf: crystal and rocks, never on the very edge.
    let rim_n = t.t.iter().filter(|x| x.kind() == ground::GLASS).count() as f64;
    for (share, o) in [(0.03, obj::CRYSTAL), (0.01, obj::ROCK)] {
        let mut k = 0;
        let mut tries = 0;
        while (k as f64) < rim_n * share && tries < 10_000 {
            tries += 1;
            let p = g.spot(l.dim as f64 + 0.5, l.island as f64 - 3.0);
            let edge = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|&(dx, dy)| t.void(p.0 + dx, p.1 + dy));
            if !edge && place(&mut t, p, o, &[ground::GLASS]) {
                k += 1;
            }
        }
    }
    (t, Sites { wells, checkpoints })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::laws::LAWS;

    #[test]
    fn the_vale_is_an_island_in_rings() {
        let (t, sites) = generate(&LAWS, 7);
        let land = t.t.iter().filter(|x| !x.void()).count();
        // About pi * 60^2, less the chasms.
        assert!((10_000..12_500).contains(&land), "{land} tiles of land");
        // The centre is the Luciphon; the spawn ring around it is clear.
        assert_eq!(t.get(0, 0).obj, obj::LUCIPHON);
        assert!(!t.void(0, 4) && !t.solid(0, 4));
        // No void and no brambles inside the Dim.
        for y in -34..34 {
            for x in -34..34 {
                if x * x + y * y < 34 * 34 {
                    let tile = t.get(x, y);
                    assert!(!tile.void() && tile.obj != obj::BRAMBLE, "{x},{y}");
                }
            }
        }
        // The edge is out past the Dim.
        assert!(t.void(70, 0) && t.void(0, -70));
        assert_eq!(sites.wells.len(), 3);
        for w in &sites.wells {
            assert!(!t.void(w.0, w.1) && !t.solid(w.0, w.1));
        }
        assert_eq!(generate(&LAWS, 7).0, t, "a seed makes one island");
    }
}
