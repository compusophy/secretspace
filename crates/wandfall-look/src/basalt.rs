//! The causeway's basalt: six-sided columns of stacked drums, a groove at
//! each joint, the colour varying drum to drum and darker, wetter, low
//! down toward the sea; capped by a dished top, moss over some in soft
//! patches. No two columns quite alike: their corners pulled in a little
//! (never out, so neighbours never meet), a few cuts of each. A column's
//! sides come in a few heights (the nearest stretched to fit, so the
//! joints stay about a metre apart); its cap is never stretched.

use std::f32::consts::TAU;

use render::geo::{hash, mix, rgb, unit, Geo, V3};
use render::sculpt::noise;
use render::{m4, Item, Mesh, Renderer};

use wandfall::laws::SEA;
use wandfall::map::Prop;

/// The heights the sides are made in (m).
const HEIGHTS: [f32; 7] = [1.5, 2.5, 3.5, 5.0, 7.0, 9.0, 12.5];
/// A column's look is this much of its blocking radius (its corners;
/// the grooves between columns show).
const WIDE: f32 = 0.955;
/// Nearer than this, the drums; farther, a plain prism.
const NEAR: f32 = 40.0;
/// Cuts of each column (their corners pulled in differently).
const CUTS: usize = 4;

const DARK: V3 = rgb(50, 50, 56);
const BASALT: V3 = rgb(74, 72, 78);
const WET: V3 = rgb(34, 36, 42);
const TOP: V3 = rgb(92, 92, 96);
const MOSS: V3 = rgb(70, 96, 52);

/// How far out each of a cut's six corners is (1 the full width, a
/// little less for the rest).
fn corners(cut: usize) -> [f32; 6] {
    std::array::from_fn(|k| 1.0 - 0.08 * unit(hash(cut as i32, k as i32, 0xba5)))
}

/// A ring of the six corners, `r` out (each as far as `cut` says) at
/// height `y`.
fn ring(r: f32, y: f32, cut: &[f32; 6]) -> [V3; 6] {
    std::array::from_fn(|k| {
        let a = TAU / 12.0 + k as f32 * TAU / 6.0;
        [a.cos() * r * cut[k], y, a.sin() * r * cut[k]]
    })
}

/// The sides between two rings, facing out.
fn band(g: &mut Geo, lo: [V3; 6], hi: [V3; 6], col: V3) {
    for k in 0..6 {
        let j = (k + 1) % 6;
        g.quad(lo[k], hi[k], hi[j], lo[j], col, 0.0);
    }
}

/// Sides `h` tall, unit wide (to the corners) as `cut` pulls them in:
/// drums from 1.2 m to 2.6 m, each a little narrower or wider, chamfered
/// into a fine groove at each joint; darker and wetter in the lowest
/// metres.
fn sides(g: &mut Geo, h: f32, seed: u32, cut: &[f32; 6]) {
    let (mut y, mut k) = (0.0, 0);
    while y < h - 0.05 {
        let u = |i| unit(hash(k, i, seed));
        let len = (1.2 + 1.4 * u(1)).min(h - y);
        let len = if h - (y + len) < 0.6 { h - y } else { len };
        let w = 0.985 + 0.015 * u(2);
        let c = mix(DARK, BASALT, 0.6 + 0.4 * u(3));
        let c = mix(c, WET, 0.6 * (1.0 - (y + len / 2.0) / 1.8).clamp(0.0, 1.0));
        let lip = 0.03f32.min(len / 4.0);
        let rings = [
            ring(w * 0.97, y, cut),
            ring(w, y + lip, cut),
            ring(w, y + len - lip, cut),
            ring(w * 0.97, y + len, cut),
        ];
        for r in rings.windows(2) {
            band(g, r[0], r[1], c);
        }
        y += len;
        k += 1;
    }
}

/// The cap, at height 0, as `cut` pulls its corners in: a chamfer up
/// from the sides' last groove to a rim, dished a little toward its
/// middle; with `moss`, soft patches of it (thickest in the dish).
fn cap(g: &mut Geo, cut: &[f32; 6], moss: bool, seed: u32) {
    let rim = ring(0.93, 0.03, cut);
    band(g, ring(0.97, 0.0, cut), rim, mix(DARK, BASALT, 0.8));
    // The top, in rings toward the middle, each corner and the middle of
    // each edge, so the moss can lie in patches.
    let rings: Vec<(f32, f32)> = vec![(0.93, 0.03), (0.8, 0.04), (0.55, 0.03), (0.3, 0.02)];
    let at = |r: f32, y: f32, k: usize| {
        let (a, b) = (ring(r, y, cut)[k / 2], ring(r, y, cut)[(k / 2 + 1) % 6]);
        if k.is_multiple_of(2) {
            a
        } else {
            [(a[0] + b[0]) / 2.0, y, (a[2] + b[2]) / 2.0]
        }
    };
    let s = seed as f32 * 1.7;
    let paint = |p: V3, edge: f32| {
        if !moss {
            return TOP;
        }
        let patch = ((noise([p[0] * 1.6 + s, s, p[2] * 1.6]) + 0.15) * 1.8).clamp(0.0, 1.0);
        mix(TOP, MOSS, patch * (1.0 - edge))
    };
    let base = g.len() as u32;
    for (j, &(r, y)) in rings.iter().enumerate() {
        for k in 0..12 {
            let p = at(r, y, k);
            g.vertex(
                p,
                [0.0, 1.0, 0.0],
                paint(p, (j == 0) as i32 as f32 * 0.7),
                0.0,
            );
        }
    }
    let mid = g.vertex(
        [0.0, 0.015, 0.0],
        [0.0, 1.0, 0.0],
        paint([0.0; 3], 0.0),
        0.0,
    );
    let id = |j: usize, k: usize| base + (j * 12 + k % 12) as u32;
    for j in 0..rings.len() - 1 {
        for k in 0..12 {
            g.index(id(j, k), id(j + 1, k), id(j, k + 1));
            g.index(id(j, k + 1), id(j + 1, k), id(j + 1, k + 1));
        }
    }
    let last = rings.len() - 1;
    for k in 0..12 {
        g.index(mid, id(last, k + 1), id(last, k));
    }
}

pub(crate) struct Basalt {
    /// Each cut's sides at each height: drums, and a plain prism as tall
    /// (far off, and for the shadow).
    sides: Vec<Vec<(Mesh, Mesh, f32)>>,
    /// Each cut's caps, bare and mossy.
    caps: Vec<[Mesh; 2]>,
}

impl Basalt {
    pub(crate) fn new(r: &mut Renderer) -> Basalt {
        let cuts: Vec<[f32; 6]> = (0..CUTS).map(corners).collect();
        let sides = cuts
            .iter()
            .enumerate()
            .map(|(c, cut)| {
                HEIGHTS
                    .iter()
                    .enumerate()
                    .map(|(k, &h)| {
                        let mut g = Geo::default();
                        sides(&mut g, h, 70 + (k + c * 10) as u32, cut);
                        let mut plain = Geo::default();
                        band(&mut plain, ring(1.0, 0.0, cut), ring(1.0, h, cut), BASALT);
                        (r.mesh(&g), r.mesh(&plain), h)
                    })
                    .collect()
            })
            .collect();
        let caps = cuts
            .iter()
            .enumerate()
            .map(|(c, cut)| {
                [false, true].map(|moss| {
                    let mut g = Geo::default();
                    cap(&mut g, cut, moss, c as u32);
                    r.mesh(&g)
                })
            })
            .collect();
        Basalt { sides, caps }
    }

    /// The column `p` (the `k`th thing on the island), as statics: darker
    /// the nearer the sea it stands.
    pub(crate) fn put(&self, statics: &mut Vec<Item>, p: &Prop, k: usize) {
        let h = hash(k as i32, 9, 13);
        // Turned a sixth at a time: the same lattice, a different face.
        let yaw = p.yaw + (h % 6) as f32 * TAU / 6.0;
        let wide = p.r * WIDE;
        let cut = (h >> 8) as usize % CUTS;
        let &(near, far, tall) = self.sides[cut]
            .iter()
            .min_by(|a, b| (a.2 / p.h).ln().abs().total_cmp(&(b.2 / p.h).ln().abs()))
            .unwrap();
        let at = [p.x, p.y, p.z];
        let damp = 0.88 + 0.12 * ((p.y - SEA) / 3.0).clamp(0.0, 1.0);
        statics.push(
            Item::new(near, m4::place(at, yaw, [wide, p.h / tall, wide]))
                .far(far, NEAR)
                .tint([damp; 3], 1.0)
                .rough(0.8)
                .detail(0.3),
        );
        let top = [p.x, p.y + p.h, p.z];
        let moss = (h >> 4).is_multiple_of(3);
        statics.push(
            Item::new(
                self.caps[cut][moss as usize],
                m4::place(top, yaw, [wide, 1.0, wide]),
            )
            .rough(0.85)
            .detail(0.35),
        );
    }

    pub(crate) fn meshes(&self) -> impl Iterator<Item = Mesh> + '_ {
        self.sides
            .iter()
            .flatten()
            .flat_map(|s| [s.0, s.1])
            .chain(self.caps.iter().flatten().copied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_column_reaches_past_its_neighbour() {
        // Corners only ever come in, so the lattice's grooves never close
        // (a corner is all that comes near the next column).
        for c in 0..CUTS {
            let cut = corners(c);
            assert!(cut.iter().all(|k| (0.9..=1.0).contains(k)), "{cut:?}");
            for p in ring(WIDE, 0.0, &cut) {
                assert!(p[0].hypot(p[2]) <= WIDE + 1e-5);
            }
        }
        // And no two cuts alike.
        assert!(corners(0) != corners(1));
    }
}
