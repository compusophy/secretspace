//! The island's trees and stones, sculpted (`render::sculpt`), each made
//! near and far (coarser, for the many seen from afar):
//!
//! - a broadleaf tree: a trunk with ridged bark, roots flaring into the
//!   ground and branches reaching up into its crown, mossy at its foot;
//!   a crown of leafy masses melted together, ruffled by noise, lit as one
//!   soft mass (its normals bent out from its middle), lighter where the
//!   sky reaches it and darker beneath and between its masses;
//!   leaves standing out of it, breaking its outline;
//! - a pine: a slender trunk and tiers of boughs, lobed and drooping at
//!   their tips, lighter there;
//! - a boulder: a stone cut in facets, its edges worn round, moss on its
//!   top, darker in its cracks.

use std::f32::consts::TAU;

use render::geo::{hash, mix, rgb, unit, Geo, V3};
use render::sculpt::{bend, both, cone, ellipsoid, mesh, noise, sheet, smin};
use wandfall::laws::{ROCK_GIRTH, ROCK_TALL, ROCK_TOP};

use crate::make::steps;

/// How much coarser the far ones are.
pub const FAR: f32 = 2.6;
/// A boulder's proportions as set down (times its prop's scale).
pub const ROCK: V3 = [1.2, 1.4, 1.1];
/// Where feet stand on a boulder, in its own measure: its prop is
/// `ROCK_TALL` times its scale tall, and stood on `ROCK_TOP` of that up.
const ROCK_STAND: f32 = ROCK_TOP * ROCK_TALL / ROCK[1];

/// A broadleaf crown's colours: dark, light.
pub type Leaves = (V3, V3);

fn h(seed: u32, k: i32) -> f32 {
    unit(hash(seed as i32, k, 0x7ee5))
}

/// A broadleaf tree's trunk (a metre is a metre at scale 1; it stands
/// 4.4 tall), at detail `q` (1 near).
pub fn trunk(seed: u32, q: f32) -> Geo {
    let lean = [0.3 * (h(seed, 1) - 0.5), 0.0, 0.3 * (h(seed, 2) - 0.5)];
    let top = [lean[0], 3.6, lean[2]];
    let mid = [lean[0] * 0.3, 1.8, lean[2] * 0.3];
    let f = move |p: V3| {
        let mut d = smin(
            cone(p, [0.0, -0.3, 0.0], mid, 0.36, 0.25),
            cone(p, mid, top, 0.25, 0.15),
            0.15,
        );
        // Roots flaring into the ground.
        for k in 0..5 {
            let a = (k as f32 + h(seed, 10 + k)) / 5.0 * TAU;
            let end = [a.cos() * 0.75, -0.2, a.sin() * 0.75];
            d = smin(d, cone(p, [0.0, 0.45, 0.0], end, 0.15, 0.06), 0.25);
        }
        // Branches up into the crown.
        for k in 0..3 {
            let a = (k as f32 + 0.3 * h(seed, 20 + k)) / 3.0 * TAU;
            let end = [
                top[0] + a.cos() * 1.2,
                4.6 + 0.4 * h(seed, 30 + k),
                top[2] + a.sin() * 1.2,
            ];
            d = smin(
                d,
                cone(p, [lean[0] * 0.8, 2.9, lean[2] * 0.8], end, 0.11, 0.05),
                0.12,
            );
        }
        // Ridged bark.
        let a = p[2].atan2(p[0]);
        d + 0.018 * (a * 9.0 + p[1] * 0.7 + 0.6 * noise([p[0] * 2.0, p[1] * 0.6, p[2] * 2.0])).sin()
    };
    let bark = rgb(92, 70, 52);
    let moss = rgb(70, 92, 44);
    mesh(
        &f,
        ([-1.1, -0.35, -1.1], [1.8, 5.2, 1.8]),
        0.11 * q,
        &|p, n| {
            let low = (1.0 - p[1] / 0.9).clamp(0.0, 1.0) * (0.5 + 0.5 * n[1].max(0.0));
            let c = mix(bark, moss, 0.55 * low);
            (
                mix(
                    c,
                    rgb(120, 100, 80),
                    0.25 * (noise([p[0] * 3.0, p[1] * 3.0, p[2] * 3.0]) * 0.5 + 0.5),
                ),
                0.0,
            )
        },
        (0.6, 0.35),
    )
}

/// A broadleaf crown, its masses about (0, 4.3, 0).
pub fn crown(seed: u32, (dark, light): Leaves, q: f32) -> Geo {
    let centre = [0.0, 4.3, 0.0];
    let mut masses: Vec<(V3, V3)> = vec![(centre, [1.9, 1.5, 1.9])];
    for k in 0..6 {
        let a = (k as f32 + 0.4 * h(seed, 40 + k)) / 6.0 * TAU;
        let r = 1.35 + 0.35 * h(seed, 50 + k);
        let y = 3.9 + 0.9 * h(seed, 60 + k);
        let s = 1.0 + 0.35 * h(seed, 70 + k);
        masses.push(([a.cos() * r, y, a.sin() * r], [s, s * 0.85, s]));
    }
    masses.push(([0.3, 5.5, -0.2], [1.25, 1.05, 1.25]));
    let f = move |p: V3| {
        let mut d = f32::MAX;
        for (c, r) in &masses {
            d = smin(d, ellipsoid(p, *c, *r), 0.55);
        }
        // Leafy, ruffled.
        let s = seed as f32 * 0.37;
        d + 0.22 * noise([p[0] * 1.6 + s, p[1] * 1.6, p[2] * 1.6 - s])
            + 0.08 * noise([p[0] * 4.0 - s, p[1] * 4.0, p[2] * 4.0 + s])
    };
    let mut g = mesh(
        &f,
        ([-3.6, 1.8, -3.6], [3.6, 7.4, 3.6]),
        0.24 * q,
        &|p, _| {
            let up = ((p[1] - 3.0) / 3.2).clamp(0.0, 1.0);
            let speck = 0.5 + 0.5 * noise([p[0] * 2.3, p[1] * 2.3, p[2] * 2.3]);
            (mix(dark, light, 0.25 + 0.55 * up + 0.2 * speck), 0.0)
        },
        (0.75, 1.1),
    );
    // Leaves breaking its outline, at every third of its points: near,
    // small ones; far (fewer points), bigger ones, pointed (their shape
    // lost that far off), so it keeps its ragged edge for few triangles.
    if q < 1.5 {
        leaves(&mut g, 3, (0.36, 0.2), seed, false);
    } else {
        leaves(&mut g, 3, (0.75, 0.45), seed, true);
    }
    bend(&mut g, [0.0, 4.0, 0.0], 0.55);
    g
}

/// Leaves on a crown's surface: at every `every`th of its points (not
/// those facing down), a leaf `long` and `wide` standing out from it at
/// a slant, turned every way, its colour a little lighter or darker, lit
/// as the surface is; both its faces drawn. A leaf is a diamond, or
/// `pointed`, a triangle from its foot to its tip (half the triangles).
fn leaves(g: &mut Geo, every: usize, (long, wide): (f32, f32), seed: u32, pointed: bool) {
    use render::geo::{add, cross, norm, scale, STRIDE};
    let points = g.len();
    for k in (0..points).step_by(every) {
        let o = k * STRIDE;
        let at = [g.v[o], g.v[o + 1], g.v[o + 2]];
        let n = [g.v[o + 3], g.v[o + 4], g.v[o + 5]];
        if n[1] < -0.4 {
            continue;
        }
        let u = |i| unit(hash(k as i32, i, seed ^ 0x1eaf));
        let col = scale([g.v[o + 6], g.v[o + 7], g.v[o + 8]], 0.85 + 0.3 * u(0));
        let side = norm(cross(
            n,
            if n[1].abs() < 0.9 {
                [0.0, 1.0, 0.0]
            } else {
                [1.0, 0.0, 0.0]
            },
        ));
        let roll = u(1) * TAU;
        let along = add(scale(side, roll.cos()), scale(cross(n, side), roll.sin()));
        let along = norm(add(along, scale(n, 0.3 + 0.7 * u(2))));
        let across = scale(norm(cross(along, n)), wide / 2.0);
        let tip = add(at, scale(along, long));
        let base = g.len() as u32;
        if pointed {
            for c in [add(at, across), tip, add(at, scale(across, -1.0))] {
                g.vertex(c, n, col, 0.0);
            }
            g.index(base, base + 1, base + 2);
            g.index(base, base + 2, base + 1);
            continue;
        }
        let mid = add(at, scale(along, long / 2.0));
        for c in [at, add(mid, across), tip, add(mid, scale(across, -1.0))] {
            g.vertex(c, n, col, 0.0);
        }
        for (a, b, c) in [(0, 1, 2), (0, 2, 3), (0, 2, 1), (0, 3, 2)] {
            g.index(base + a, base + b, base + c);
        }
    }
}

/// A pine's slender trunk.
pub fn pine_trunk(q: f32) -> Geo {
    let f = |p: V3| {
        let d = cone(p, [0.0, -0.3, 0.0], [0.0, 5.8, 0.0], 0.24, 0.06);
        let a = p[2].atan2(p[0]);
        d + 0.012 * (a * 11.0 + p[1] * 0.9).sin()
    };
    mesh(
        &f,
        ([-0.4, -0.35, -0.4], [0.4, 6.0, 0.4]),
        0.13 * q,
        &|_, _| (rgb(86, 62, 46), 0.0),
        (0.4, 0.2),
    )
}

/// A pine's tiers of boughs: each a skirt woven round, lobed into
/// boughs whose tips droop, darker beneath (seen from under it).
pub fn pine(seed: u32, q: f32) -> Geo {
    let tiers = [
        (0.9, 2.5, 2.0),
        (2.1, 2.05, 1.85),
        (3.2, 1.65, 1.7),
        (4.25, 1.25, 1.55),
        (5.2, 0.85, 1.35),
        (6.0, 0.45, 1.2),
    ];
    let (dark, light) = (rgb(20, 54, 42), rgb(64, 112, 74));
    let mut g = Geo::default();
    for (k, &(y, w, hgt)) in tiers.iter().enumerate() {
        // Boughs: lobes round the tier (1 at a bough's tip, 0 between).
        let lobes = (7 + (k + seed as usize) % 3) as f32;
        let turn = seed as f32 * 1.3 + k as f32 * 0.7;
        let bough = move |a: f32| (a * lobes / 2.0 + turn).cos().abs();
        let rag = move |a: f32| {
            let b = 1.0 - bough(a);
            1.0 - 0.3 * b * b + 0.04 * (a * 23.0 + k as f32).sin()
        };
        // Down from the tier's top to its rim, at `a`, its boughs' tips
        // drooping.
        let at = move |a: f32, v: f32, under: f32| {
            let r = w * rag(a) * v.powf(0.85) * (1.0 - 0.06 * under);
            let lift = 0.18 * under * v;
            let droop = 0.25 * v * v * bough(a);
            [
                a.cos() * r,
                y + hgt * (1.0 - v) - 0.2 * v * v - droop + lift,
                a.sin() * r,
            ]
        };
        // Round, steps enough for every bough; down from its top, fewer
        // beneath (seen only from under it), and fewer far off.
        let round = steps(42, q);
        let down = |n: f32, least: usize| ((n / q).round() as usize).max(least);
        sheet(
            &mut g,
            (round, down(5.0, 3)),
            |u, v| at(u * TAU, v, 0.0),
            |u, v| {
                let tip = v * v * (0.8 + 0.2 * rag(u * TAU));
                mix(dark, light, 0.12 + 0.7 * tip)
            },
        );
        sheet(
            &mut g,
            (round, down(3.0, 2)),
            |u, v| at(-u * TAU, v, 1.0),
            |_, v| render::geo::scale(dark, 0.55 + 0.25 * v),
        );
    }
    bend(&mut g, [0.0, 3.0, 0.0], 0.3);
    g
}

/// A boulder (about 2 across and 1 tall before its instance's scale,
/// `ROCK`), as its prop blocks and is stood on: facets cut at random,
/// its sides out as far as its prop blocks, its top worn flat where feet
/// stand on it (`ROCK_STAND`), edges worn, moss on top.
pub fn boulder(seed: u32, q: f32) -> Geo {
    let mid = [0.0, 0.2, 0.0];
    let cuts: Vec<(V3, f32)> = (0..9)
        .map(|k| {
            let a = h(seed, 80 + k) * TAU;
            let up = h(seed, 90 + k) * 1.4 - 0.4;
            let (s, c) = a.sin_cos();
            let n = render::geo::norm([c, up, s]);
            // Its sides cut close to its girth; its shoulders past the
            // flat of its top, so feet find it where they stand.
            let off = if up.abs() < 0.3 {
                ROCK_GIRTH / ROCK[0] * (0.94 + 0.09 * h(seed, 100 + k))
            } else if up > 0.0 {
                0.75 * n[0].hypot(n[2]) + (ROCK_STAND - mid[1] - 0.05) * n[1]
            } else {
                0.6 + 0.16 * h(seed, 100 + k)
            };
            (n, off)
        })
        .collect();
    let f = move |p: V3| {
        let tall = 1.25 + 0.2 * h(seed, 4);
        let mut d = ellipsoid(p, mid, [1.0, tall, 0.95]);
        let q = [p[0], p[1] - mid[1], p[2]];
        for (n, off) in &cuts {
            let plane = q[0] * n[0] + q[1] * n[1] + q[2] * n[2] - off;
            d = both(d, plane, 0.035);
        }
        let d = d + 0.04 * noise([p[0] * 3.0, p[1] * 3.0, p[2] * 3.0]);
        both(d, p[1] - ROCK_STAND, 0.08)
    };
    let stone = mix(rgb(128, 124, 118), rgb(150, 142, 128), h(seed, 3));
    let moss = rgb(74, 98, 50);
    mesh(
        &f,
        ([-1.2, -0.6, -1.2], [1.2, 1.2, 1.2]),
        0.1 * q,
        &|p, n| {
            let top = ((n[1] - 0.55) / 0.3).clamp(0.0, 1.0)
                * (0.6 + 0.4 * noise([p[0] * 2.0, p[1] * 2.0, p[2] * 2.0]));
            let grain = 0.85 + 0.15 * noise([p[0] * 6.0, p[1] * 6.0, p[2] * 6.0]);
            (render::geo::scale(mix(stone, moss, 0.8 * top), grain), 0.0)
        },
        (0.65, 0.3),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flora_is_sculpted_light_enough_for_a_forest() {
        let leaves = (rgb(40, 80, 30), rgb(90, 130, 50));
        let count = |q: f32| {
            [
                trunk(1, q),
                crown(1, leaves, q),
                pine_trunk(q),
                pine(1, q),
                boulder(1, q),
            ]
            .map(|g| {
                assert!(!g.is_empty());
                g.triangles()
            })
        };
        let (near, far) = (count(1.0), count(FAR));
        println!("near {near:?}, far {far:?}");
        // A tree near, all of it; far, a fifth as much. Far, the crowns,
        // pines and boulders are what the sun's shadow draws for all the
        // island's hundreds: about as light as before they had leaves,
        // boughs and their full girth.
        assert!(near[0] + near[1] < 12_000, "{near:?}");
        assert!(far[0] + far[1] < (near[0] + near[1]) / 4, "{far:?}");
        assert!(near[4] < 4_000, "{near:?}");
        assert!(far[1] < 700 && far[3] < 1_000 && far[4] < 450, "{far:?}");
        // Far, a crown keeps its size and ragged edge (no shrinking as it
        // swaps).
        let reach = |g: &Geo| {
            g.v.chunks(render::geo::STRIDE)
                .map(|v| v[0].hypot(v[2]))
                .fold(0.0f32, f32::max)
        };
        for seed in [1, 11, 29, 47] {
            let (n, f) = (
                reach(&crown(seed, leaves, 1.0)),
                reach(&crown(seed, leaves, FAR)),
            );
            assert!(f > n * 0.95 && f < n * 1.1, "near {n}, far {f}");
        }
    }

    #[test]
    fn a_boulder_stands_where_its_prop_does() {
        // As set down (scale 1): its top, across the middle of it, where
        // feet stand on its prop; its sides out about as far as its prop
        // blocks (`ROCK_GIRTH`), low down where wizards meet them.
        for seed in [5, 17, 23] {
            let g = boulder(seed, 1.0);
            let world = |v: &[f32]| [v[0] * ROCK[0], v[1] * ROCK[1], v[2] * ROCK[2]];
            let stand = ROCK_STAND * ROCK[1];
            for v in g.v.chunks(render::geo::STRIDE).map(world) {
                if v[0].hypot(v[2]) < 0.6 && v[1] > stand - 0.4 {
                    assert!(
                        (v[1] - stand).abs() < 0.1,
                        "{seed}: its top at {v:?}, not {stand}"
                    );
                }
            }
            let mut out = [0.0f32; 12];
            for v in g.v.chunks(render::geo::STRIDE).map(world) {
                let up = v[1] - 0.2;
                if (0.1..0.6).contains(&up) {
                    let k = ((v[2].atan2(v[0]) / TAU + 0.5) * 12.0) as usize % 12;
                    out[k] = out[k].max(v[0].hypot(v[2]));
                }
            }
            for r in out {
                assert!(
                    r > ROCK_GIRTH * 0.77 && r < ROCK_GIRTH * 1.18,
                    "{seed}: its sides {out:?}"
                );
            }
        }
    }
}
