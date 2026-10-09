//! The island's trees and stones, sculpted (`render::sculpt`), each made
//! near and far (coarser, for the many seen from afar):
//!
//! - a broadleaf tree: a trunk with ridged bark, roots flaring into the
//!   ground and branches reaching up into its crown, mossy at its foot;
//!   a crown of leafy masses melted together, ruffled by noise, lit as one
//!   soft mass (its normals bent out from its middle), lighter where the
//!   sky reaches it and darker beneath and between its masses;
//! - a pine: a slender trunk and tiers of ragged boughs, lighter at
//!   their tips;
//! - a boulder: a stone cut in facets, its edges worn round, moss on its
//!   top, darker in its cracks.

use std::f32::consts::TAU;

use render::geo::{hash, mix, rgb, unit, Geo, V3};
use render::sculpt::{bend, both, cone, ellipsoid, mesh, noise, sheet, smin};

/// How much coarser the far ones are.
pub const FAR: f32 = 2.6;

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
    bend(&mut g, [0.0, 4.0, 0.0], 0.55);
    g
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

/// A pine's tiers of boughs: each a skirt woven round, its rim ragged
/// and drooping, darker beneath (seen from under it).
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
        let rag = move |a: f32| {
            1.0 + 0.12 * (a * 9.0 + k as f32 * 1.7 + seed as f32).sin()
                + 0.05 * (a * 23.0 + k as f32).sin()
        };
        // Down from the tier's top to its rim, at `a`.
        let at = move |a: f32, v: f32, under: f32| {
            let r = w * rag(a) * v.powf(0.85) * (1.0 - 0.06 * under);
            let lift = 0.18 * under * v;
            [
                a.cos() * r,
                y + hgt * (1.0 - v) - 0.2 * v * v + lift,
                a.sin() * r,
            ]
        };
        let n = (steps(28, q), steps(5, q));
        sheet(
            &mut g,
            n,
            |u, v| at(u * TAU, v, 0.0),
            |u, v| {
                let tip = v * v * (0.8 + 0.2 * rag(u * TAU));
                mix(dark, light, 0.12 + 0.7 * tip)
            },
        );
        sheet(
            &mut g,
            n,
            |u, v| at(-u * TAU, v, 1.0),
            |_, v| render::geo::scale(dark, 0.55 + 0.25 * v),
        );
    }
    bend(&mut g, [0.0, 3.0, 0.0], 0.3);
    g
}

/// Steps for a sheet at this detail.
fn steps(n: usize, q: f32) -> usize {
    ((n as f32 / q).round() as usize).max(4)
}

/// A boulder (about 1 across and 0.85 tall before its instance's scale):
/// facets cut at random, edges worn, moss on top.
pub fn boulder(seed: u32, q: f32) -> Geo {
    let cuts: Vec<(V3, f32)> = (0..9)
        .map(|k| {
            let a = h(seed, 80 + k) * TAU;
            let up = h(seed, 90 + k) * 1.4 - 0.4;
            let (s, c) = a.sin_cos();
            let n = render::geo::norm([c, up, s]);
            (n, 0.5 + 0.16 * h(seed, 100 + k))
        })
        .collect();
    let f = move |p: V3| {
        let tall = 0.65 + 0.4 * h(seed, 4);
        let mut d = ellipsoid(p, [0.0, 0.3, 0.0], [1.0, tall, 0.95]);
        let q = [p[0], p[1] - 0.3, p[2]];
        for (n, off) in &cuts {
            let plane = q[0] * n[0] + q[1] * n[1] + q[2] * n[2] - off;
            d = both(d, plane, 0.035);
        }
        d + 0.04 * noise([p[0] * 3.0, p[1] * 3.0, p[2] * 3.0])
    };
    let stone = mix(rgb(128, 124, 118), rgb(150, 142, 128), h(seed, 3));
    let moss = rgb(74, 98, 50);
    mesh(
        &f,
        ([-1.2, -0.6, -1.2], [1.2, 1.25, 1.2]),
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
        // A tree near, all of it; far, a fifth as much.
        assert!(near[0] + near[1] < 12_000, "{near:?}");
        assert!(far[0] + far[1] < (near[0] + near[1]) / 4, "{far:?}");
        assert!(near[4] < 4_000, "{near:?}");
    }
}
