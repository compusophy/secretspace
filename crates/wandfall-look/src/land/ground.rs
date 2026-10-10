//! The ground at the places: how it is painted (the plaza's mortar, the
//! circle's green, the rift's scorched bowl going to ash and burnt grass,
//! the grove's violet, the causeway's black gravel) and where grass
//! grows; and what lies on it finer than its samples (1.25 m apart) can
//! show: the plaza's paving, slab by slab, with its gold inlay, and the
//! rift's lava running out in cracks that wander and fork.

use std::f32::consts::TAU;

use render::geo::{self, hash, mix, rgb, unit, Geo, V3};
use render::sculpt::noise;
use render::Terrain;

use wandfall::laws::PAD_R;
use wandfall::map::{smooth as ease, Map};
use wandfall::places::{Place, Poi};

use super::{EMBER, GOLDEN};

/// How the ground is painted at (x, z): a colour and how much of it
/// (above 1, it glows).
pub(super) fn paint(map: &Map, x: f32, z: f32) -> (V3, f32) {
    for p in &map.pois {
        let d = p.dist(x, z);
        if d > p.r * 1.3 {
            continue;
        }
        return match p.place {
            // The mortar between the paving's slabs.
            Place::Spire => (rgb(58, 54, 50), 0.95 * (1.0 - ease((d - p.r + 1.0) / 3.0))),
            Place::Circle => (rgb(96, 112, 84), 0.4 * (1.0 - ease((d - p.r * 0.6) / 6.0))),
            Place::Rift => {
                // A pool of lava where the gate stands; scorched dark in
                // the bowl, ash toward its lip, burnt grass past it.
                if d < 3.8 {
                    return (EMBER, 1.0 + 1.6 * (1.0 - d / p.r).max(0.2));
                }
                let dark = mix(
                    rgb(22, 14, 14),
                    rgb(54, 30, 24),
                    unit(hash(x as i32, z as i32, 9)),
                );
                let ash = mix(dark, rgb(70, 62, 58), ease((d - p.r * 0.55) / (p.r * 0.3)));
                let c = mix(ash, rgb(78, 70, 40), ease((d - p.r * 0.85) / (p.r * 0.2)));
                (c, 1.0 - ease((d - p.r * 1.05) / (p.r * 0.25)))
            }
            Place::Grove => (rgb(120, 96, 170), 0.5 * (1.0 - ease((d - p.r * 0.7) / 5.0))),
            // Black gravel and broken basalt.
            Place::Causeway => {
                let c = mix(
                    rgb(52, 52, 56),
                    rgb(88, 86, 82),
                    unit(hash((x * 1.7) as i32, (z * 1.7) as i32, 4)),
                );
                (c, 0.9 * (1.0 - ease((d - p.r * 0.7) / (p.r * 0.45))))
            }
        };
    }
    ([1.0; 3], 0.0)
}

/// How much grass grows at (x, z): none on the plaza and in the rift,
/// thinner at the other places.
pub(super) fn lush(map: &Map, x: f32, z: f32) -> f32 {
    let mut k: f32 = 1.0;
    for p in &map.pois {
        let d = p.dist(x, z);
        let (keep, edge) = match p.place {
            Place::Spire => (0.0, p.r + 1.0),
            Place::Rift => (0.0, p.r * 0.95),
            Place::Circle => (0.55, p.r * 0.8),
            Place::Grove => (0.2, p.r * 0.9),
            Place::Causeway => (0.3, p.r * 0.85),
        };
        let f = ease((d - edge) / 3.0);
        k = k.min(keep + (1.0 - keep) * f);
    }
    // None on the launch runes, so their circles show.
    for q in &map.pads {
        let d = ((x - q[0]).powi(2) + (z - q[2]).powi(2)).sqrt();
        k = k.min(ease((d - PAD_R * 1.3) / 1.5));
    }
    k
}

/// The ground at (x, z) as it is drawn: across each square of samples,
/// two flat triangles (not the samples' smooth blend), so what lies on it
/// lies on what is seen.
pub(super) fn drawn(t: &Terrain, x: f32, z: f32) -> f32 {
    let fx = ((x - t.origin[0]) / t.cell).clamp(0.0, (t.n - 1) as f32 - 0.001);
    let fz = ((z - t.origin[1]) / t.cell).clamp(0.0, (t.n - 1) as f32 - 0.001);
    let (i, j) = (fx as usize, fz as usize);
    let (u, v) = (fx - i as f32, fz - j as f32);
    let at = |i: usize, j: usize| t.heights[j.min(t.n - 1) * t.n + i.min(t.n - 1)];
    let (a, b, c, d) = (at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1));
    if u + v <= 1.0 {
        a + (b - a) * u + (c - a) * v
    } else {
        d + (c - d) * (1.0 - u) + (b - d) * (1.0 - v)
    }
}

/// The rings of the plaza's paving (inner and outer radius, metres out
/// from the tower), and the gold inlay's between them.
const PAVING: [(f32, f32); 6] = [
    (4.7, 6.9),
    (6.9, 9.2),
    (9.8, 12.2),
    (12.2, 14.6),
    (14.6, 17.0),
    (17.0, 19.4),
];
const INLAY: (f32, f32) = (9.25, 9.75);
/// The mortar between slabs, and the bevel round each slab's top.
const JOINT: f32 = 0.05;
const BEVEL: f32 = 0.035;

/// The plaza about `p` (the Spire): rings of slabs, each its own shade,
/// bevelled and a little proud of the ground (the mortar dark between
/// them), more and more of them broken away toward the plaza's edge; the
/// gold inlay round the tower.
pub(super) fn paving(p: &Poi) -> Geo {
    let mut g = Geo::default();
    let (top, edge, under) = (p.level + 0.02, p.level - 0.015, p.level - 0.08);
    for (ring, &(r0, r1)) in PAVING.iter().enumerate() {
        let n = ((r0 + r1) / 2.0 * TAU / 2.0).round() as i32;
        // Broken away toward the edge.
        let gone = ease((r0 - 15.0) / 5.0) * 0.6;
        for k in 0..n {
            let u = |i: u32| unit(hash(ring as i32, k, 300 + i));
            if u(0) < gone {
                continue;
            }
            let c = mix(rgb(128, 122, 114), rgb(178, 170, 158), u(1));
            let c = mix(c, rgb(110, 116, 84), 0.3 * u(2) * gone);
            let (a0, a1) = (k as f32 / n as f32 * TAU, (k + 1) as f32 / n as f32 * TAU);
            slab(
                &mut g,
                p,
                (r0 + JOINT / 2.0, r1 - JOINT / 2.0),
                (a0, a1),
                (top, edge, under),
                c,
            );
        }
    }
    // The inlay: a band of gold, aglow.
    let n = 160;
    let at = |a: f32, r: f32| [p.x + a.cos() * r, p.level + 0.025, p.z + a.sin() * r];
    for k in 0..n {
        let (a0, a1) = (k as f32 / n as f32 * TAU, (k + 1) as f32 / n as f32 * TAU);
        g.quad(
            at(a0, INLAY.0),
            at(a1, INLAY.0),
            at(a1, INLAY.1),
            at(a0, INLAY.1),
            GOLDEN,
            1.5,
        );
    }
    g
}

/// A slab of paving between radii `r` and headings `a`, its top at
/// `y.0`, bevelled down to `y.1` at its edges, its sides down to `y.2`.
fn slab(g: &mut Geo, p: &Poi, r: (f32, f32), a: (f32, f32), y: (f32, f32, f32), c: V3) {
    let (top, edge, under) = y;
    let mid = (r.0 + r.1) / 2.0;
    let gap = JOINT / 2.0 / mid;
    let (a0, a1) = (a.0 + gap, a.1 - gap);
    let bev = BEVEL / mid;
    let at = |a: f32, r: f32, y: f32| [p.x + a.cos() * r, y, p.z + a.sin() * r];
    let inside = at((a0 + a1) / 2.0, mid, under - 1.0);
    let segs = (((a1 - a0) * mid / 0.7).ceil() as usize).max(1);
    let side = geo::scale(c, 0.7);
    for s in 0..segs {
        let t0 = a0 + (a1 - a0) * s as f32 / segs as f32;
        let t1 = a0 + (a1 - a0) * (s + 1) as f32 / segs as f32;
        // Inset at the slab's ends.
        let (i0, i1) = (
            if s == 0 { t0 + bev } else { t0 },
            if s + 1 == segs { t1 - bev } else { t1 },
        );
        let ri = (r.0 + BEVEL, r.1 - BEVEL);
        let face = |g: &mut Geo, q: [V3; 4], col: V3| {
            g.tri_out(q[0], q[1], q[2], inside, col, 0.0);
            g.tri_out(q[0], q[2], q[3], inside, col, 0.0);
        };
        face(
            g,
            [
                at(i0, ri.0, top),
                at(i1, ri.0, top),
                at(i1, ri.1, top),
                at(i0, ri.1, top),
            ],
            c,
        );
        // The bevels in and out, and the sides under them.
        face(
            g,
            [
                at(t0, r.0, edge),
                at(t1, r.0, edge),
                at(i1, ri.0, top),
                at(i0, ri.0, top),
            ],
            c,
        );
        face(
            g,
            [
                at(i0, ri.1, top),
                at(i1, ri.1, top),
                at(t1, r.1, edge),
                at(t0, r.1, edge),
            ],
            c,
        );
        face(
            g,
            [
                at(t0, r.0, under),
                at(t1, r.0, under),
                at(t1, r.0, edge),
                at(t0, r.0, edge),
            ],
            side,
        );
        face(
            g,
            [
                at(t0, r.1, edge),
                at(t1, r.1, edge),
                at(t1, r.1, under),
                at(t0, r.1, under),
            ],
            side,
        );
    }
    // Its ends.
    for (t, i) in [(a0, a0 + bev), (a1, a1 - bev)] {
        let ri = (r.0 + BEVEL, r.1 - BEVEL);
        g.tri_out(
            at(t, r.0, edge),
            at(i, ri.0, top),
            at(i, ri.1, top),
            inside,
            c,
            0.0,
        );
        g.tri_out(
            at(t, r.0, edge),
            at(i, ri.1, top),
            at(t, r.1, edge),
            inside,
            c,
            0.0,
        );
        g.tri_out(
            at(t, r.0, under),
            at(t, r.0, edge),
            at(t, r.1, edge),
            inside,
            side,
            0.0,
        );
        g.tri_out(
            at(t, r.0, under),
            at(t, r.1, edge),
            at(t, r.1, under),
            inside,
            side,
            0.0,
        );
    }
}

/// The rift's lava about `p`, laid on the ground as drawn (`t`): from the
/// pool under the gate, five cracks run out, wandering and forking, each
/// a ribbon narrowing as it goes, hot in its middle.
pub(super) fn lava(p: &Poi, seed: u32, t: &Terrain) -> Geo {
    let mut g = Geo::default();
    for k in 0..5 {
        let a = unit(hash(k, 7, seed)) * TAU;
        let len = p.r * (0.55 + 0.25 * unit(hash(k, 8, seed)));
        let start = [p.x + a.cos() * 3.2, p.z + a.sin() * 3.2];
        let fork = crack(&mut g, t, (start, a), (len, 0.26), (k, seed));
        // A fork off it, part way out.
        let side = if unit(hash(k, 9, seed)) < 0.5 {
            -0.7
        } else {
            0.7
        };
        crack(
            &mut g,
            t,
            (fork.0, fork.1 + side),
            (len * 0.4, 0.17),
            (k + 50, seed),
        );
    }
    g
}

/// One crack of lava from `from` heading `a` (radians), `len` long and
/// `wide` at its start: where it is and which way it heads two fifths of
/// the way along (for a fork).
fn crack(
    g: &mut Geo,
    t: &Terrain,
    (from, a): ([f32; 2], f32),
    (len, wide): (f32, f32),
    (k, seed): (i32, u32),
) -> ([f32; 2], f32) {
    const STEP: f32 = 0.3;
    let n = (len / STEP) as usize;
    let mut at = from;
    let mut fork = (from, a);
    let mut was: Option<[V3; 3]> = None;
    for i in 0..=n {
        let s = i as f32 * STEP;
        let head = a + 0.9 * noise([s * 0.12, k as f32 * 5.3, seed as f32 * 0.37]);
        if i == n * 2 / 5 {
            fork = (at, head);
        }
        let f = s / len;
        let w = wide * (1.0 - f).powf(0.7) + 0.06;
        let (sn, cs) = head.sin_cos();
        let on = |dx: f32, dz: f32| {
            let (x, z) = (at[0] + dx, at[1] + dz);
            [x, drawn(t, x, z) + 0.03, z]
        };
        let row = [on(sn * w, -cs * w), on(0.0, 0.0), on(-sn * w, cs * w)];
        if let Some(prev) = was {
            let hot = 2.4 * (1.0 - 0.6 * f);
            let edge = (rgb(46, 16, 12), 0.0);
            let core = (mix(EMBER, rgb(255, 200, 90), 0.6), hot);
            for (l, r) in [(0, 1), (1, 2)] {
                let c = |j: usize| if j == 1 { core } else { edge };
                let quad = [prev[l], prev[r], row[r], row[l]];
                let shade = [c(l), c(r), c(r), c(l)];
                let base = g.len() as u32;
                for (q, (col, glow)) in quad.iter().zip(shade) {
                    g.vertex(*q, [0.0, 1.0, 0.0], col, glow);
                }
                // Facing up, whichever way the crack runs.
                let n = geo::cross(geo::sub(quad[1], quad[0]), geo::sub(quad[2], quad[0]));
                if n[1] > 0.0 {
                    g.index(base, base + 1, base + 2);
                    g.index(base, base + 2, base + 3);
                } else {
                    g.index(base, base + 2, base + 1);
                    g.index(base, base + 3, base + 2);
                }
            }
        }
        was = Some(row);
        at = [at[0] + cs * STEP, at[1] + sn * STEP];
    }
    fork
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_lies_on_the_ground_lies_on_it_as_drawn() {
        let m = Map::new(7);
        let t = Terrain::sample([-60.0, -60.0], 120.0, 1.25, |x, z| m.height(x, z));
        // At the samples, the ground as drawn is the samples.
        for &(x, z) in &[(-10.0f32, 5.0f32), (2.5, -3.75), (20.0, 20.0)] {
            assert!((drawn(&t, x, z) - m.height(x, z)).abs() < 1e-3);
        }
        // Between them, on the very triangles: a straight slope is flat.
        let flat = Terrain::sample([0.0, 0.0], 10.0, 1.0, |x, z| x * 0.5 + z * 0.25);
        assert!((drawn(&flat, 2.3, 4.6) - (1.15 + 1.15)).abs() < 1e-4);
        // The paving lies flat on the plaza, just over it; slabs all round.
        let p = m.pois[0];
        let g = paving(&p);
        let ys = g.v.chunks(geo::STRIDE).map(|v| v[1]);
        let (lo, hi) = ys.fold((f32::MAX, f32::MIN), |(l, h), y| (l.min(y), h.max(y)));
        assert!(
            lo > p.level - 0.1 && hi < p.level + 0.03,
            "{lo} to {hi} over {}",
            p.level
        );
        assert!(
            g.triangles() > 2_000 && g.triangles() < 12_000,
            "{}",
            g.triangles()
        );
        // The lava never sinks under the ground drawn.
        let rift = *m.pois.iter().find(|q| q.place == Place::Rift).unwrap();
        let t = Terrain::sample([rift.x - 30.0, rift.z - 30.0], 60.0, 1.25, |x, z| {
            m.height(x, z)
        });
        let g = lava(&rift, 7, &t);
        assert!(g.triangles() > 200, "{}", g.triangles());
        for v in g.v.chunks(geo::STRIDE) {
            assert!(v[1] > drawn(&t, v[0], v[2]) + 0.02, "buried at {v:?}");
        }
    }
}
