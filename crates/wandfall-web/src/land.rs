//! The island as it looks: its ground (painted where the places are,
//! grass held back from the plaza and the rift), everything standing on
//! it as statics (trees, rocks, mushrooms, ruins; the Spire's tower and
//! lamps, the circle's stones, the rift's obsidian and gate, the grove's
//! crystals; islets floating over it all), and what moves at the places
//! each frame: the beacon over the tower, rune rings, the gate's swirl,
//! embers, glimmer and their light.

use std::f32::consts::TAU;

use render::geo::{self, hash, mix, rgb, unit, Geo, STRIDE, V3};
use render::{m4, Item, Light, Material, Mesh, Pass, Renderer, Spark, Terrain};

use crate::fx::Draw;
use crate::look::Look;
use wandfall::laws::{MAP_HALF, RIFT_DEPTH, SEA};
use wandfall::map::{smooth as ease, Kind, Map};
use wandfall::places::{Place, Poi};

/// Metres between the ground's samples.
const TERRAIN_CELL: f32 = 1.25;

const VIOLET: V3 = rgb(178, 132, 255);
const CYAN: V3 = rgb(110, 225, 255);
const AMETHYST: V3 = rgb(140, 80, 255);
const AQUA: V3 = rgb(60, 190, 255);
const EMBER: V3 = rgb(255, 92, 30);
const GOLDEN: V3 = rgb(255, 206, 120);

/// The island's meshes and where its lights and moving things are.
pub struct Land {
    pub held: Vec<Mesh>,
    pois: Vec<Poi>,
    lamps: Vec<V3>,
    crystals: Vec<(V3, V3)>,
    /// The rift's gate: its foot and its turn.
    gate: (V3, f32),
    /// The beacon's crystal, and a ring of runes a metre across.
    gem: Mesh,
    runes: Mesh,
}

fn one(r: &mut Renderer, f: impl Fn(&mut Geo)) -> Mesh {
    let mut g = Geo::default();
    f(&mut g);
    r.mesh(&g)
}

fn smooth(r: &mut Renderer, f: impl Fn(&mut Geo)) -> Mesh {
    let mut g = Geo::default();
    f(&mut g);
    g.smooth();
    r.mesh(&g)
}

/// Lean the vertices from `from` on `lean` radians from upright, toward
/// `yaw`, then move them by `at`.
fn lean(g: &mut Geo, from: usize, lean: f32, yaw: f32, at: V3) {
    let (sl, cl) = lean.sin_cos();
    let (sy, cy) = yaw.sin_cos();
    let rot = |v: V3| {
        let (x, y, z) = (v[0] * cl + v[1] * sl, -v[0] * sl + v[1] * cl, v[2]);
        [x * cy - z * sy, y, x * sy + z * cy]
    };
    for k in from..g.len() {
        let o = k * STRIDE;
        let p = rot([g.v[o], g.v[o + 1], g.v[o + 2]]);
        let n = rot([g.v[o + 3], g.v[o + 4], g.v[o + 5]]);
        g.v[o..o + 3].copy_from_slice(&geo::add(p, at));
        g.v[o + 3..o + 6].copy_from_slice(&n);
    }
}

/// A six-sided crystal, `h` tall and `r` wide, pointed.
fn crystal(g: &mut Geo, r: f32, h: f32, col: V3, glow: f32) {
    g.column([0.0; 3], 6, (r, r * 0.86), h * 0.76, 0.3, col, glow, false);
    g.column(
        [0.0, h * 0.76, 0.0],
        6,
        (r * 0.86, 0.0),
        h * 0.24,
        0.3,
        col,
        glow,
        false,
    );
}

/// Glyphs about a ring a metre across, and the ring.
fn rune_ring(g: &mut Geo, glyphs: usize, seed: u32) {
    let at = |a: f32, r: f32| [a.cos() * r, 0.0, a.sin() * r];
    let n = 64;
    for k in 0..n {
        let (a0, a1) = (k as f32 / n as f32 * TAU, (k + 1) as f32 / n as f32 * TAU);
        g.quad(
            at(a0, 0.95),
            at(a0, 1.0),
            at(a1, 1.0),
            at(a1, 0.95),
            [1.0; 3],
            1.0,
        );
        g.quad(
            at(a0, 0.7),
            at(a0, 0.73),
            at(a1, 0.73),
            at(a1, 0.7),
            [1.0; 3],
            1.0,
        );
    }
    // Each glyph: a few strokes in a little box between the rings.
    for k in 0..glyphs {
        let mid = k as f32 / glyphs as f32 * TAU;
        let w = TAU / glyphs as f32 * 0.32;
        let p = |u: f32, v: f32| at(mid + (u - 0.5) * w, 0.76 + v * 0.15);
        for s in 0..3 {
            let h = |i| unit(hash(k as i32, s * 4 + i, seed));
            let (a, b) = (p(h(0), h(1)), p(h(2), h(3)));
            let d = geo::norm(geo::sub(b, a));
            let side = geo::scale([-d[2], 0.0, d[0]], 0.008);
            g.quad(
                geo::sub(a, side),
                geo::add(a, side),
                geo::add(b, side),
                geo::sub(b, side),
                [1.0; 3],
                1.0,
            );
        }
    }
}

/// How the ground is painted at (x, z): a colour and how much of it
/// (above 1, it glows).
fn paint(map: &Map, x: f32, z: f32) -> (V3, f32) {
    for p in &map.pois {
        let d = p.dist(x, z);
        if d > p.r * 1.3 {
            continue;
        }
        let a = (z - p.z).atan2(x - p.x);
        return match p.place {
            Place::Spire => {
                // Paving in rings, a gold inlay about the tower.
                if (d - 10.0).abs() < 0.6 {
                    return (GOLDEN, 1.5);
                }
                let ring = (d / 2.4) as i32;
                let n = 10 + ring * 4;
                let sector = ((a / TAU + 0.5) * n as f32) as i32;
                let c = mix(
                    rgb(150, 144, 136),
                    rgb(186, 178, 166),
                    unit(hash(ring, sector, 3)),
                );
                (c, 0.95 * (1.0 - ease((d - p.r + 1.0) / 3.0)))
            }
            Place::Circle => (rgb(96, 112, 84), 0.4 * (1.0 - ease((d - p.r * 0.6) / 6.0))),
            Place::Rift => {
                // Lava where the gate stands, and cracks running out.
                let veins = (0..5).any(|k| {
                    let va = unit(hash(k, 7, map.seed as u32)) * TAU;
                    let (s, c) = va.sin_cos();
                    let (dx, dz) = (x - p.x, z - p.z);
                    let along = dx * c + dz * s;
                    along > 0.0 && along < p.r * 0.75 && (dz * c - dx * s).abs() < 0.7
                });
                if d < 3.8 || veins {
                    (EMBER, 1.0 + 1.6 * (1.0 - d / p.r).max(0.2))
                } else {
                    let c = mix(
                        rgb(22, 14, 14),
                        rgb(54, 30, 24),
                        unit(hash(x as i32, z as i32, 9)),
                    );
                    (c, 1.0 - ease((d - p.r * 0.8) / (p.r * 0.45)))
                }
            }
            Place::Grove => (rgb(120, 96, 170), 0.5 * (1.0 - ease((d - p.r * 0.7) / 5.0))),
        };
    }
    ([1.0; 3], 0.0)
}

/// How much grass grows at (x, z).
fn lush(map: &Map, x: f32, z: f32) -> f32 {
    let mut k: f32 = 1.0;
    for p in &map.pois {
        let d = p.dist(x, z);
        let (keep, edge) = match p.place {
            Place::Spire => (0.0, p.r + 1.0),
            Place::Rift => (0.0, p.r * 1.15),
            Place::Circle => (0.55, p.r * 0.8),
            Place::Grove => (0.2, p.r * 0.9),
        };
        let f = ease((d - edge) / 3.0);
        k = k.min(keep + (1.0 - keep) * f);
    }
    k
}

impl Land {
    /// Build the island's meshes and set it down as statics.
    pub fn new(r: &mut Renderer, map: &Map) -> Land {
        let mut t = Terrain::sample(
            [-MAP_HALF, -MAP_HALF],
            MAP_HALF * 2.0,
            TERRAIN_CELL,
            |x, z| map.height(x, z),
        );
        t.grow(|x, z| lush(map, x, z));
        r.terrain(&t);
        let ground = r.mesh(&t.mesh(SEA - 4.0, |x, z, _| paint(map, x, z)));
        let bark = rgb(96, 74, 56);
        let trunk = smooth(r, |g| {
            let profile = [
                (0.46, -0.3),
                (0.36, 0.4),
                (0.27, 1.6),
                (0.2, 3.2),
                (0.1, 4.4),
            ];
            g.lathe([0.0; 3], &profile, 9, bark, 0.0);
        });
        // Crowns: green, a deep teal, and the wizard-wood's violet.
        let leaves = [
            (rgb(60, 104, 42), rgb(98, 132, 54)),
            (rgb(34, 92, 78), rgb(60, 120, 96)),
            (rgb(96, 64, 150), rgb(140, 96, 190)),
        ];
        let crowns = [11u32, 29, 47].map(|s| {
            let (a, b) = leaves[(s as usize / 18) % 3];
            smooth(r, |g| {
                let c = mix(a, b, unit(hash(s as i32, 1, 7)));
                let dark = mix(c, rgb(24, 40, 40), 0.45);
                g.sphere([0.0, 3.9, 0.0], [2.1, 1.8, 2.1], (2, s, 0.3), c, 0.0);
                g.sphere(
                    [0.6, 5.1, -0.3],
                    [1.5, 1.3, 1.5],
                    (2, s + 1, 0.3),
                    mix(c, b, 0.6),
                    0.0,
                );
                g.sphere([-0.8, 4.6, 0.6], [1.3, 1.1, 1.3], (2, s + 2, 0.3), c, 0.0);
                g.sphere([0.3, 3.3, 1.1], [1.2, 1.0, 1.2], (2, s + 3, 0.3), dark, 0.0);
            })
        });
        let pines = smooth(r, |g| {
            for (y, w, h) in [
                (1.5, 2.3, 2.5),
                (2.9, 1.85, 2.3),
                (4.1, 1.4, 2.1),
                (5.2, 0.95, 1.9),
            ] {
                g.lathe(
                    [0.0, y, 0.0],
                    &[(w, 0.0), (w * 0.5, h * 0.45), (0.0, h)],
                    12,
                    rgb(30, 70, 56),
                    0.0,
                );
            }
        });
        let rocks = [5u32, 17, 23].map(|s| {
            smooth(r, |g| {
                g.sphere(
                    [0.0, 0.35, 0.0],
                    [1.0, 0.85, 1.0],
                    (3, s, 0.34),
                    rgb(130, 126, 120),
                    0.0,
                )
            })
        });
        let stone = rgb(208, 200, 184);
        let pillar = smooth(r, |g| {
            let profile = [
                (0.66, 0.0),
                (0.66, 0.06),
                (0.54, 0.12),
                (0.5, 0.5),
                (0.47, 0.94),
                (0.53, 0.97),
                (0.53, 1.0),
            ];
            g.lathe([0.0; 3], &profile, 16, stone, 0.0);
        });
        let cap = one(r, |g| {
            g.block(
                [0.0; 3],
                [1.35, 0.32, 1.35],
                0.0,
                stone,
                rgb(176, 168, 154),
                0.0,
            )
        });
        // Giant mushrooms, a metre tall (scaled to each): a pale stem, a
        // cap that glows a little, spots.
        let shrooms = [rgb(150, 80, 220), rgb(50, 170, 160), rgb(210, 70, 70)].map(|c| {
            smooth(r, |g| {
                let cream = rgb(226, 214, 190);
                g.lathe(
                    [0.0; 3],
                    &[(0.09, 0.0), (0.075, 0.4), (0.06, 0.8), (0.08, 0.95)],
                    10,
                    cream,
                    0.0,
                );
                let rim = [
                    (0.06, 0.9),
                    (0.42, 0.88),
                    (0.47, 0.94),
                    (0.4, 1.06),
                    (0.22, 1.16),
                    (0.0, 1.19),
                ];
                g.lathe([0.0; 3], &rim, 16, c, 0.35);
                for k in 0..7 {
                    let a = k as f32 * 2.4;
                    let rr = 0.18 + 0.12 * (k % 2) as f32;
                    let y = 1.16 - rr * 0.55;
                    g.sphere(
                        [a.cos() * rr, y, a.sin() * rr],
                        [0.045, 0.02, 0.045],
                        (1, k, 0.0),
                        cream,
                        0.2,
                    );
                }
            })
        });
        // The Spire's tower: a stone shaft, buttressed, a balcony with
        // battlements, a wizard's hat of a roof; windows lit up a spiral.
        let tower = smooth(r, |g| {
            let pale = rgb(172, 164, 160);
            let dark = rgb(120, 112, 132);
            g.lathe(
                [0.0; 3],
                &[(6.2, 0.0), (6.0, 1.2), (5.0, 1.3)],
                8,
                dark,
                0.0,
            );
            g.lathe(
                [0.0; 3],
                &[(5.0, 1.2), (4.7, 4.0), (4.5, 12.0), (4.3, 21.6)],
                24,
                pale,
                0.0,
            );
            for y in [7.0, 14.5] {
                g.lathe(
                    [0.0, y, 0.0],
                    &[(4.62, 0.0), (4.75, 0.2), (4.62, 0.4)],
                    24,
                    GOLDEN,
                    0.15,
                );
            }
            g.lathe(
                [0.0; 3],
                &[(4.2, 21.4), (5.9, 22.4), (5.9, 23.0), (3.7, 23.0)],
                24,
                dark,
                0.0,
            );
            g.lathe([0.0; 3], &[(3.7, 23.0), (3.5, 30.0)], 20, pale, 0.0);
            // A wizard's hat: a wide brim, its tip bent over.
            let felt = rgb(66, 44, 140);
            let hat = [
                (5.7, 29.8),
                (5.6, 30.3),
                (3.6, 32.4),
                (2.2, 35.4),
                (1.2, 38.6),
                (0.55, 41.0),
            ];
            g.lathe([0.0; 3], &hat, 24, felt, 0.0);
            g.spike([0.0, 40.8, 0.0], [2.2, 44.6, 0.5], 0.62, 10, felt, 0.0);
            g.lathe(
                [0.0, 30.0, 0.0],
                &[(5.66, 0.0), (5.75, 0.25), (5.66, 0.5)],
                24,
                GOLDEN,
                0.2,
            );
        });
        let trim = one(r, |g| {
            let dark = rgb(120, 112, 132);
            for k in 0..8 {
                let a = k as f32 / 8.0 * TAU;
                g.block(
                    [a.cos() * 4.9, 1.2, a.sin() * 4.9],
                    [1.4, 5.5, 0.8],
                    a,
                    dark,
                    dark,
                    0.0,
                );
            }
            for k in 0..20 {
                let a = (k as f32 + 0.5) / 20.0 * TAU;
                g.block(
                    [a.cos() * 5.6, 23.0, a.sin() * 5.6],
                    [0.5, 0.8, 0.9],
                    a,
                    dark,
                    dark,
                    0.0,
                );
            }
            // The door, and the windows climbing the shaft.
            g.block(
                [4.75, 1.2, 0.0],
                [0.5, 3.2, 1.8],
                0.0,
                rgb(40, 26, 44),
                rgb(40, 26, 44),
                0.0,
            );
            // A dark frame, and candlelight in it, pointed at the top.
            let window = |g: &mut Geo, a: f32, y: f32, r: f32, w: f32, h: f32| {
                let (c, s) = (a.cos(), a.sin());
                let pane = |g: &mut Geo, r: f32, w: f32, h: f32, col: V3, glow: f32| {
                    let o = [c * r, y, s * r];
                    let side = [-s * w / 2.0, 0.0, c * w / 2.0];
                    let up = [0.0, h / 2.0, 0.0];
                    let p = |i: f32, j: f32| {
                        geo::add(o, geo::add(geo::scale(side, i), geo::scale(up, j)))
                    };
                    g.quad(
                        p(-1.0, -1.0),
                        p(-1.0, 0.6),
                        p(1.0, 0.6),
                        p(1.0, -1.0),
                        col,
                        glow,
                    );
                    g.tri(p(-1.0, 0.6), p(0.0, 1.0), p(1.0, 0.6), col, glow);
                };
                pane(g, r, w + 0.3, h + 0.3, rgb(44, 34, 56), 0.0);
                pane(g, r + 0.03, w, h, rgb(255, 178, 90), 3.0);
            };
            for k in 0..13 {
                let y = 4.5 + k as f32 * 1.3;
                // Just proud of the shaft, which narrows as it climbs.
                let r = if y < 12.0 {
                    4.7 - (y - 4.0) * 0.025
                } else {
                    4.5 - (y - 12.0) * 0.0208
                };
                window(g, k as f32 * 1.05 + 1.0, y, r + 0.07, 0.55, 1.1);
            }
            for k in 0..4 {
                window(g, k as f32 / 4.0 * TAU + 0.4, 26.5, 3.68, 0.6, 1.4);
            }
        });
        let lamp = one(r, |g| {
            let iron = rgb(46, 42, 58);
            g.column([0.0; 3], 6, (0.2, 0.13), 2.8, 0.0, iron, 0.0, false);
            g.column([0.0, 2.8, 0.0], 6, (0.32, 0.32), 0.08, 0.0, iron, 0.0, true);
            g.sphere([0.0, 3.15, 0.0], [0.26; 3], (1, 4, 0.0), VIOLET, 2.5);
        });
        // A standing stone a unit tall, its runes on the side facing in.
        let menhir = one(r, |g| {
            // Five-sided, a face toward -x (inward once placed).
            let grey = rgb(112, 110, 106);
            let rot = 144f32.to_radians();
            g.column([0.0; 3], 5, (0.5, 0.36), 0.86, rot, grey, 0.0, false);
            g.column(
                [0.0, 0.86, 0.0],
                5,
                (0.36, 0.14),
                0.14,
                rot,
                grey,
                0.0,
                true,
            );
            for k in 0..5 {
                let y = 0.22 + k as f32 * 0.12;
                let x = -(0.809 * (0.5 - 0.14 * y / 0.86) + 0.008);
                for s in 0..3 {
                    let h = |i| unit(hash(k, s * 4 + i, 77)) - 0.5;
                    let a = [x, y + h(0) * 0.08, h(1) * 0.3];
                    let b = [x, y + h(2) * 0.08, h(3) * 0.3];
                    let d = geo::norm(geo::sub(b, a));
                    let side = [0.0, -d[2] * 0.012, d[1] * 0.012];
                    g.quad(
                        geo::sub(a, side),
                        geo::add(a, side),
                        geo::add(b, side),
                        geo::sub(b, side),
                        CYAN,
                        2.0,
                    );
                }
            }
        });
        let altar = one(r, |g| {
            let dark = rgb(96, 92, 100);
            g.block([0.0; 3], [1.7, 0.9, 1.1], 0.0, dark, dark, 0.0);
            g.block(
                [0.0, 0.9, 0.0],
                [2.1, 0.22, 1.5],
                0.0,
                rgb(130, 126, 134),
                dark,
                0.0,
            );
        });
        // Obsidian a unit tall, shards about its foot; the gate.
        let obsidian = rgb(24, 10, 14);
        let spike = one(r, |g| {
            g.spike([0.0; 3], [0.12, 1.0, 0.05], 1.0, 5, obsidian, 0.0);
            g.spike([0.7, 0.0, 0.3], [1.1, 0.4, 0.5], 0.4, 4, obsidian, 0.0);
            g.spike([-0.4, 0.0, -0.7], [-0.7, 0.3, -1.0], 0.35, 4, obsidian, 0.0);
            g.spike([0.0, 0.0, 0.0], [0.08, 0.6, 0.03], 0.3, 4, EMBER, 0.6);
        });
        let gate = one(r, |g| {
            g.block(
                [0.0; 3],
                [2.2, 0.6, 7.6],
                0.0,
                rgb(40, 30, 40),
                obsidian,
                0.0,
            );
            for z in [-3.1f32, 3.1] {
                g.spike([0.0, 0.5, z], [0.0, 7.6, z * 1.25], 0.7, 6, obsidian, 0.0);
            }
            // The ring the gate opens in.
            let (n, m, big, tube) = (32, 6, 2.7, 0.28);
            let at = |i: usize, j: usize| {
                let (a, b) = (i as f32 / n as f32 * TAU, j as f32 / m as f32 * TAU);
                let rr = big + tube * b.cos();
                [tube * b.sin(), 3.6 + rr * a.sin(), rr * a.cos()]
            };
            for i in 0..n {
                for j in 0..m {
                    let glow = if j == m / 2 { 1.4 } else { 0.0 };
                    let c = if glow > 0.0 { EMBER } else { obsidian };
                    g.quad(
                        at(i, j),
                        at(i + 1, j),
                        at(i + 1, j + 1),
                        at(i, j + 1),
                        c,
                        glow,
                    );
                }
            }
        });
        let crystals = [0u32, 1].map(|s| {
            one(r, |g| {
                crystal(g, 0.42, 1.0, [1.0; 3], 0.3);
                for k in 0..4 {
                    let from = g.len();
                    let h = 0.35 + 0.3 * unit(hash(k, 1, s));
                    crystal(g, 0.2, h, [1.0; 3], 0.3);
                    let a = k as f32 * 1.7 + s as f32;
                    lean(
                        g,
                        from,
                        0.5 + 0.4 * unit(hash(k, 2, s)),
                        a,
                        [a.cos() * 0.5, 0.0, a.sin() * 0.5],
                    );
                }
            })
        });
        // Islets floating over the island: earth hanging under a green
        // top, a crystal under some.
        let islet = smooth(r, |g| {
            let earth = [
                (0.0, -7.0),
                (1.4, -5.2),
                (3.0, -2.8),
                (4.2, -0.8),
                (4.5, 0.0),
            ];
            g.lathe([0.0; 3], &earth, 14, rgb(112, 92, 80), 0.0);
            g.lathe(
                [0.0; 3],
                &[(4.6, 0.0), (4.4, 0.35), (3.0, 0.6), (0.0, 0.7)],
                14,
                rgb(70, 112, 50),
                0.0,
            );
        });

        let mut statics = vec![Item::new(ground, m4::ID).material(Material::Terrain)];
        let mut lamps = Vec::new();
        let mut gems = Vec::new();
        let mut gate_at = ([0.0; 3], 0.0);
        for (k, p) in map.props.iter().enumerate() {
            let at = [p.x, p.y, p.z];
            let h = hash(k as i32, 3, 5);
            match p.kind {
                Kind::Tree => {
                    let s = p.scale;
                    statics.push(
                        Item::new(trunk, m4::place(at, p.yaw, [s; 3]))
                            .rough(0.9)
                            .detail(0.4),
                    );
                    let crown = match k % 7 {
                        0 | 4 => pines,
                        6 => crowns[2],
                        3 => crowns[1],
                        _ => crowns[0],
                    };
                    statics.push(
                        Item::new(crown, m4::place(at, p.yaw, [s; 3]))
                            .material(Material::Foliage)
                            .rough(0.7),
                    );
                }
                Kind::Rock => {
                    let s = p.scale;
                    let m = m4::place(at, p.yaw, [s * 1.2, s * 1.4, s * 1.1]);
                    statics.push(Item::new(rocks[k % 3], m).rough(0.85).detail(0.6));
                }
                Kind::Pillar => {
                    let m = m4::place(at, p.yaw, [1.0, p.h, 1.0]);
                    statics.push(Item::new(pillar, m).rough(0.7).detail(0.3));
                    if k % 2 == 0 {
                        let top = [p.x, p.y + p.h, p.z];
                        statics.push(
                            Item::new(cap, m4::place(top, p.yaw, [1.0; 3]))
                                .rough(0.75)
                                .detail(0.3),
                        );
                    }
                }
                Kind::Shroom => {
                    let m = m4::place(at, p.yaw, [p.h; 3]);
                    statics.push(Item::new(shrooms[k % 3], m).rough(0.6).detail(0.2));
                }
                Kind::Tower => {
                    let m = m4::place(at, 0.0, [1.0; 3]);
                    statics.push(Item::new(tower, m).rough(0.8).detail(0.12));
                    statics.push(Item::new(trim, m).rough(0.85).detail(0.1));
                }
                Kind::Lamp => {
                    statics.push(Item::new(lamp, m4::place(at, 0.0, [1.0; 3])).rough(0.5));
                    lamps.push([p.x, p.y + 3.15, p.z]);
                }
                Kind::Stone => {
                    let m = m4::place(at, p.yaw, [1.1, p.h, 1.9]);
                    statics.push(Item::new(menhir, m).rough(0.9).detail(0.2));
                }
                Kind::Altar => {
                    statics.push(
                        Item::new(altar, m4::place(at, 0.4, [1.0; 3]))
                            .rough(0.85)
                            .detail(0.5),
                    );
                }
                Kind::Spike => {
                    let m = m4::place(at, p.yaw, [p.r * 1.1, p.h, p.r * 1.1]);
                    statics.push(Item::new(spike, m).rough(0.55));
                }
                Kind::Portal => {
                    statics.push(Item::new(gate, m4::place(at, p.yaw, [1.0; 3])).rough(0.15));
                    gate_at = (at, p.yaw);
                }
                Kind::Crystal => {
                    let c = mix(AMETHYST, AQUA, unit(h));
                    let m = m4::place(at, p.yaw, [p.r * 1.3, p.h, p.r * 1.3]);
                    statics.push(Item::new(crystals[k % 2], m).tint(c, 1.0).rough(0.25));
                    gems.push(([p.x, p.y + p.h * 0.6, p.z], c));
                }
            }
        }
        for k in 0..7 {
            let a = (k as f32 + unit(hash(k, 1, map.seed as u32))) / 7.0 * TAU;
            let d = 36.0 + 34.0 * unit(hash(k, 2, map.seed as u32));
            let (x, z) = (a.cos() * d, a.sin() * d);
            let y = map.height(x, z).max(SEA) + 26.0 + 16.0 * unit(hash(k, 3, map.seed as u32));
            let s = 0.6 + 0.6 * unit(hash(k, 4, map.seed as u32));
            statics.push(
                Item::new(islet, m4::place([x, y, z], a, [s; 3]))
                    .rough(0.9)
                    .detail(0.6),
            );
            if k % 2 == 0 {
                let m = m4::place([x, y + 0.6 * s, z], a, [s * 0.8; 3]);
                statics.push(Item::new(trunk, m).rough(0.9));
                statics.push(Item::new(crowns[k as usize % 3], m).material(Material::Foliage));
            } else {
                let c = mix(AMETHYST, AQUA, (k % 3) as f32 / 2.0);
                let m = m4::basis(
                    [x, y - 4.0 * s, z],
                    [s * 1.4, 0.0, 0.0],
                    [0.0, -3.0 * s, 0.0],
                    [0.0, 0.0, s * 1.4],
                );
                statics.push(Item::new(crystals[0], m).tint(c, 1.0).rough(0.25));
                gems.push(([x, y - 6.0 * s, z], c));
            }
        }
        r.statics(statics);
        let gem = smooth(r, |g| {
            g.spike([0.0; 3], [0.0, 1.0, 0.0], 0.55, 4, [1.0; 3], 1.0);
            g.spike([0.0; 3], [0.0, -1.0, 0.0], 0.55, 4, [1.0; 3], 1.0);
        });
        let runes = one(r, |g| rune_ring(g, 18, 5));
        let mut held = vec![
            ground, trunk, pines, pillar, cap, tower, trim, lamp, menhir, altar, spike, gate, islet,
        ];
        held.extend(crowns);
        held.extend(rocks);
        held.extend(shrooms);
        held.extend(crystals);
        held.extend([gem, runes]);
        Land {
            held,
            pois: map.pois.clone(),
            lamps,
            crystals: gems,
            gate: gate_at,
            gem,
            runes,
        }
    }
}

impl Look {
    /// What moves at the places, and their light, `t` seconds in.
    pub fn places(&self, d: &mut Draw, t: f32) {
        let l = &self.land;
        let glow = |d: &mut Draw, mesh: Mesh, m: render::M4, c: V3, a: f32| {
            d.items
                .push(Item::new(mesh, m).tint(c, a).glow(1.0).pass(Pass::Glow));
        };
        for &p in &l.lamps {
            d.lights.push(Light {
                p,
                r: 9.0,
                c: geo::scale(VIOLET, 1.6),
            });
        }
        for &(p, c) in &l.crystals {
            d.lights.push(Light {
                p,
                r: 8.0,
                c: geo::scale(c, 1.2),
            });
        }
        for p in &l.pois {
            let base = [p.x, p.level, p.z];
            match p.place {
                Place::Spire => {
                    // The beacon: a crystal turning over the hat, its
                    // light up into the sky, runes wheeling about it.
                    let foot = base[1] - 0.3;
                    let top = [p.x, foot + 45.0 + (t * 1.3).sin() * 0.4, p.z];
                    let m = m4::place(top, t * 0.8, [1.3, 2.0, 1.3]);
                    d.items
                        .push(Item::new(l.gem, m).tint(VIOLET, 1.0).glow(2.2));
                    glow(
                        d,
                        self.beam,
                        m4::place(top, 0.0, [0.5, 260.0, 0.5]),
                        VIOLET,
                        0.25,
                    );
                    glow(
                        d,
                        self.beam,
                        m4::place(top, 0.0, [1.6, 260.0, 1.6]),
                        VIOLET,
                        0.06,
                    );
                    d.lights.push(Light {
                        p: top,
                        r: 40.0,
                        c: geo::scale(VIOLET, 4.0),
                    });
                    for (y, r, w) in [
                        (foot + 31.4, 6.2, 0.35),
                        (foot + 35.4, 4.4, -0.5),
                        (top[1], 2.4, 0.9),
                    ] {
                        glow(
                            d,
                            l.runes,
                            m4::place([p.x, y, p.z], t * w, [r; 3]),
                            GOLDEN,
                            0.8,
                        );
                    }
                    let floor = [p.x, base[1] + 0.04, p.z];
                    glow(
                        d,
                        l.runes,
                        m4::place(floor, -t * 0.05, [9.0; 3]),
                        GOLDEN,
                        0.35,
                    );
                }
                Place::Circle => {
                    let at = [p.x, base[1] + 0.5 + 1.9 + (t * 1.7).sin() * 0.12, p.z];
                    d.items.push(
                        Item::new(self.orb, m4::place(at, 0.0, [0.28; 3]))
                            .tint(CYAN, 1.0)
                            .glow(2.0),
                    );
                    glow(d, l.runes, m4::place(at, t * 0.6, [1.4; 3]), CYAN, 0.9);
                    let floor = [p.x, base[1] + 0.56, p.z];
                    glow(
                        d,
                        l.runes,
                        m4::place(floor, -t * 0.1, [p.r * 0.4; 3]),
                        CYAN,
                        0.25,
                    );
                    d.lights.push(Light {
                        p: at,
                        r: 14.0,
                        c: geo::scale(CYAN, 2.5),
                    });
                }
                Place::Rift => {
                    // The gate's swirl, and embers rising off the lava.
                    let floor = base[1] - RIFT_DEPTH;
                    let (foot, yaw) = l.gate;
                    let (s, c) = yaw.sin_cos();
                    let mid = [foot[0], foot[1] + 3.6, foot[2]];
                    let face = |r: f32| {
                        m4::basis(
                            mid,
                            [c * 0.06, 0.0, s * 0.06],
                            [0.0, r, 0.0],
                            [-s * r, 0.0, c * r],
                        )
                    };
                    let pulse = 0.8 + 0.2 * (t * 3.0).sin();
                    // A void, a red rim about it, runes wheeling in it.
                    d.items.push(
                        Item::new(self.ball, face(2.45))
                            .tint(rgb(255, 50, 20), 0.55 * pulse)
                            .glow(1.0)
                            .material(Material::Rim)
                            .pass(Pass::Glow),
                    );
                    d.items.push(
                        Item::new(self.ball, face(2.25))
                            .tint(rgb(14, 2, 8), 1.0)
                            .rough(0.3),
                    );
                    for (r, w) in [(2.3, 0.7), (1.5, -1.1), (0.8, 1.6)] {
                        let (sa, ca) = (t * w).sin_cos();
                        let up = [0.0, ca * r, 0.0];
                        let across = [-s * sa * r, 0.0, c * sa * r];
                        let u = geo::add(up, across);
                        let v = geo::sub([-s * ca * r, 0.0, c * ca * r], [0.0, sa * r, 0.0]);
                        let m = m4::basis(
                            geo::add(mid, [c * 0.08, 0.0, s * 0.08]),
                            u,
                            [c * 0.02, 0.0, s * 0.02],
                            v,
                        );
                        let m2 = m4::basis(
                            geo::sub(mid, [c * 0.08, 0.0, s * 0.08]),
                            u,
                            [c * 0.02, 0.0, s * 0.02],
                            v,
                        );
                        glow(d, l.runes, m, EMBER, 0.8 * pulse);
                        glow(d, l.runes, m2, EMBER, 0.8 * pulse);
                    }
                    d.lights.push(Light {
                        p: mid,
                        r: 18.0,
                        c: geo::scale(EMBER, 3.5 * pulse),
                    });
                    d.lights.push(Light {
                        p: [p.x, floor + 0.6, p.z],
                        r: 12.0,
                        c: geo::scale(EMBER, 2.0),
                    });
                    for k in 0..48 {
                        let u = |i| unit(hash(k, i, 31));
                        let f = (t / (3.0 + 2.0 * u(0)) + u(1)).fract();
                        let a = u(2) * TAU;
                        let r = 1.0 + 9.0 * u(3) + f * 1.5;
                        d.sparks.push(Spark {
                            p: [
                                p.x + a.cos() * r,
                                floor + 0.3 + f * (6.0 + 6.0 * u(4)),
                                p.z + a.sin() * r,
                            ],
                            size: 0.07 + 0.06 * u(5),
                            c: [
                                EMBER[0],
                                EMBER[1] * (1.0 - f * 0.5),
                                EMBER[2],
                                (1.0 - f) * 0.9,
                            ],
                        });
                    }
                }
                Place::Grove => {
                    for k in 0..36 {
                        let u = |i| unit(hash(k, i, 41));
                        let a = u(0) * TAU + t * 0.05 * (u(1) - 0.5);
                        let r = p.r * u(2).sqrt();
                        let y = base[1]
                            + 0.8
                            + 3.5 * u(3)
                            + (t * (0.5 + u(4)) + u(5) * TAU).sin() * 0.4;
                        let tw = 0.5 + 0.5 * (t * 2.0 + u(6) * TAU).sin();
                        let c = mix(VIOLET, CYAN, u(7));
                        d.sparks.push(Spark {
                            p: [p.x + a.cos() * r, y, p.z + a.sin() * r],
                            size: 0.06,
                            c: [c[0], c[1], c[2], 0.8 * tw],
                        });
                    }
                }
            }
        }
    }
}
