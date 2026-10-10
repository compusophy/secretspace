//! The shapes spells are drawn with, white (each tinted as it is drawn):
//! a soft ring (bright a little inside its edge, fading both ways), an
//! arcane circle (rings, ticks, runes and a six-pointed star), a cut ice
//! crystal, a smooth beam (in lengths, joined), a ray drawn to a point at
//! each end, and a shaft of light fading up.

use std::f32::consts::{PI, TAU};

use render::geo::{add, hash, norm, sub, Geo, V3};

use super::{UP, WHITE};

fn at(a: f32, r: f32) -> V3 {
    [a.cos() * r, 0.0, a.sin() * r]
}

/// A flat ring a metre across, lying in x and z: dark at 0.55, bright at
/// 0.88, dark again at its edge (added light: dark is clear).
pub fn soft_ring() -> Geo {
    let mut g = Geo::default();
    let n = 72;
    let bands = [(0.55, 0.0), (0.88, 1.0), (1.0, 0.0)];
    for k in 0..n {
        let (a0, a1) = (k as f32 / n as f32 * TAU, (k + 1) as f32 / n as f32 * TAU);
        for w in bands.windows(2) {
            let ((r0, b0), (r1, b1)) = (w[0], w[1]);
            let v = |g: &mut Geo, a: f32, r: f32, b: f32| g.vertex(at(a, r), UP, [b; 3], b);
            let p = v(&mut g, a0, r0, b0);
            let q = v(&mut g, a1, r0, b0);
            let s = v(&mut g, a1, r1, b1);
            let t = v(&mut g, a0, r1, b1);
            g.index(p, s, q);
            g.index(p, t, s);
        }
    }
    g
}

/// A line from `p` to `q`, `w` wide, flat in x and z.
fn stroke(g: &mut Geo, p: V3, q: V3, w: f32) {
    let d = norm(sub(q, p));
    let s = [-d[2] * w * 0.5, 0.0, d[0] * w * 0.5];
    g.quad(add(p, s), add(q, s), sub(q, s), sub(p, s), WHITE, 1.0);
}

/// A band round between radii `r0` and `r1`.
fn band(g: &mut Geo, (r0, r1): (f32, f32), n: usize) {
    for k in 0..n {
        let (a0, a1) = (k as f32 / n as f32 * TAU, (k + 1) as f32 / n as f32 * TAU);
        g.quad(at(a0, r0), at(a0, r1), at(a1, r1), at(a1, r0), WHITE, 1.0);
    }
}

/// An arcane circle a metre across, lying in x and z: an outer ring, a
/// ring of runes between ticks, a six-pointed star inside, a small ring
/// and a square at its heart.
pub fn sigil() -> Geo {
    let mut g = Geo::default();
    band(&mut g, (0.955, 1.0), 96);
    band(&mut g, (0.80, 0.822), 96);
    band(&mut g, (0.29, 0.31), 48);
    for k in 0..60 {
        let a = k as f32 / 60.0 * TAU;
        let len = if k % 5 == 0 { 0.05 } else { 0.025 };
        stroke(&mut g, at(a, 0.822), at(a, 0.822 + len), 0.01);
    }
    // Runes: a dozen, each a few strokes in its own cell, from a hash.
    for k in 0..12 {
        let mid = (k as f32 + 0.5) / 12.0 * TAU;
        let (lo, hi) = (0.875, 0.94);
        let span = 0.16;
        let p = |u: f32, v: f32| at(mid + (u - 0.5) * span, lo + (hi - lo) * v);
        let bits = hash(k, 7, 0x5161);
        let strokes: [((f32, f32), (f32, f32)); 8] = [
            ((0.0, 0.0), (0.0, 1.0)),
            ((0.5, 0.0), (0.5, 1.0)),
            ((1.0, 0.0), (1.0, 1.0)),
            ((0.0, 1.0), (1.0, 1.0)),
            ((0.0, 0.5), (1.0, 0.5)),
            ((0.0, 0.0), (1.0, 1.0)),
            ((1.0, 0.0), (0.0, 1.0)),
            ((0.0, 0.0), (1.0, 0.0)),
        ];
        let mut drawn = 0;
        for (n, &(a, b)) in strokes.iter().enumerate() {
            if bits >> n & 1 == 1 || (n == 1 && drawn == 0) {
                stroke(&mut g, p(a.0, a.1), p(b.0, b.1), 0.012);
                drawn += 1;
            }
        }
    }
    // The star: two triangles.
    for t in 0..2 {
        let off = PI / 2.0 + t as f32 * PI / 3.0;
        for k in 0..3 {
            let a = off + k as f32 * TAU / 3.0;
            stroke(&mut g, at(a, 0.8), at(a + TAU / 3.0, 0.8), 0.016);
        }
    }
    for k in 0..4 {
        let a = PI / 4.0 + k as f32 * PI / 2.0;
        stroke(&mut g, at(a, 0.3), at(a + PI / 2.0, 0.3), 0.012);
    }
    band(&mut g, (0.04, 0.07), 24);
    g
}

/// An ice crystal along +x, a unit across: six faces, cut to a long point
/// at 1 and a short one at -0.3, faceted (it glints face by face).
pub fn crystal() -> Geo {
    let mut g = Geo::default();
    let ring = |x: f32, r: f32| -> Vec<V3> {
        (0..6)
            .map(|k| {
                let a = k as f32 / 6.0 * TAU;
                [x, a.cos() * r, a.sin() * r]
            })
            .collect()
    };
    let (r0, r1) = (ring(0.0, 1.0), ring(0.5, 0.82));
    let (tip, back) = ([1.0, 0.0, 0.0], [-0.3, 0.0, 0.0]);
    let inside = [0.2, 0.0, 0.0];
    for k in 0..6 {
        let j = (k + 1) % 6;
        g.tri_out(back, r0[k], r0[j], inside, WHITE, 0.6);
        g.tri_out(r0[k], r1[k], r1[j], inside, WHITE, 0.6);
        g.tri_out(r0[k], r1[j], r0[j], inside, WHITE, 0.6);
        g.tri_out(r1[k], tip, r1[j], inside, WHITE, 0.6);
    }
    g
}

/// A beam a metre up y, a unit across, smooth.
pub fn beam() -> Geo {
    let mut g = Geo::default();
    g.lathe([0.0; 3], &[(1.0, 0.0), (1.0, 1.0)], 16, WHITE, 1.0);
    g.smooth();
    g
}

/// A ray a metre up y, a unit across: drawn to a point at each end, so a
/// lone beam of light has no flat cap to it.
pub fn ray() -> Geo {
    rings(&[
        (0.2, 0.0, 1.0),
        (1.0, 0.05, 1.0),
        (1.0, 0.95, 1.0),
        (0.3, 1.0, 1.0),
    ])
}

/// A shaft of light a metre up y, a unit across: brightest at its foot,
/// fading to nothing at its top (added light: dark is clear).
pub fn shaft() -> Geo {
    rings(&[
        (1.0, 0.0, 1.0),
        (1.0, 0.08, 0.85),
        (1.0, 0.35, 0.3),
        (1.0, 1.0, 0.0),
    ])
}

/// Rings up y, each (radius, height, brightness), joined round, open at
/// the ends.
fn rings(list: &[(f32, f32, f32)]) -> Geo {
    let mut g = Geo::default();
    let n = 16;
    for &(r, y, b) in list {
        for k in 0..n {
            let a = k as f32 / n as f32 * TAU;
            g.vertex(
                [a.cos() * r, y, a.sin() * r],
                [a.cos(), 0.0, a.sin()],
                [b; 3],
                b,
            );
        }
    }
    let at = |j: usize, k: usize| (j * n + k % n) as u32;
    for j in 0..list.len() - 1 {
        for k in 0..n {
            g.index(at(j, k), at(j + 1, k), at(j, k + 1));
            g.index(at(j, k + 1), at(j + 1, k), at(j + 1, k + 1));
        }
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spell_shapes_are_made() {
        for (name, g) in [
            ("ring", soft_ring()),
            ("sigil", sigil()),
            ("crystal", crystal()),
            ("beam", beam()),
            ("ray", ray()),
            ("shaft", shaft()),
        ] {
            assert!(!g.is_empty(), "{name}");
            assert!(g.triangles() < 4_000, "{name}: {}", g.triangles());
        }
    }

    #[test]
    fn a_shaft_fades_to_nothing_at_its_top_and_a_ray_comes_to_points() {
        // Each vertex: place (3), facing (3), colour (3), glow.
        let verts = |g: &Geo| g.v.chunks(10).map(|v| v.to_vec()).collect::<Vec<_>>();
        for v in verts(&shaft()) {
            if v[1] > 0.99 {
                assert_eq!((v[6], v[9]), (0.0, 0.0));
            }
        }
        for v in verts(&ray()) {
            if v[1] < 0.01 || v[1] > 0.99 {
                assert!(v[0].hypot(v[2]) < 0.35);
            }
        }
    }
}
