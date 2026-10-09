//! Shapes sculpted from distance fields, the way clay is: primitives
//! (spheres, ellipsoids, capsules, cones that round off, rounded boxes)
//! blended into each other smoothly or carved out, then made into a
//! smooth mesh (surface nets: a vertex in each cell the surface passes
//! through, pulled onto it, its normal the field's own slope). Each vertex
//! is painted by where it is, and darkened in creases and where shapes
//! meet (occlusion read from the field itself), so a sculpted thing has
//! depth before any light falls on it.

use crate::geo::{add, dot, norm, scale, sub, Geo, V3};

/// A distance field: how far a point is outside the shape (negative
/// inside).
pub trait Field: Fn(V3) -> f32 {}
impl<F: Fn(V3) -> f32> Field for F {}

fn len(a: V3) -> f32 {
    dot(a, a).sqrt()
}

pub fn sphere(p: V3, c: V3, r: f32) -> f32 {
    len(sub(p, c)) - r
}

/// An ellipsoid of radii `r` (close to a true distance near its surface).
pub fn ellipsoid(p: V3, c: V3, r: V3) -> f32 {
    let q = sub(p, c);
    let k0 = len([q[0] / r[0], q[1] / r[1], q[2] / r[2]]);
    let k1 = len([
        q[0] / (r[0] * r[0]),
        q[1] / (r[1] * r[1]),
        q[2] / (r[2] * r[2]),
    ]);
    if k1 < 1e-9 {
        return -r[0].min(r[1]).min(r[2]);
    }
    k0 * (k0 - 1.0) / k1
}

/// A capsule from `a` to `b`, `r` thick.
pub fn capsule(p: V3, a: V3, b: V3, r: f32) -> f32 {
    let (pa, ba) = (sub(p, a), sub(b, a));
    let h = (dot(pa, ba) / dot(ba, ba).max(1e-12)).clamp(0.0, 1.0);
    len(sub(pa, scale(ba, h))) - r
}

/// A cone from `a` (`ra` thick) to `b` (`rb`), its ends rounded.
pub fn cone(p: V3, a: V3, b: V3, ra: f32, rb: f32) -> f32 {
    let ba = sub(b, a);
    let l2 = dot(ba, ba).max(1e-12);
    let rr = ra - rb;
    let a2 = l2 - rr * rr;
    let il2 = 1.0 / l2;
    let pa = sub(p, a);
    let y = dot(pa, ba);
    let z = y - l2;
    let xv = sub(scale(pa, l2), scale(ba, y));
    let x2 = dot(xv, xv);
    let y2 = y * y * l2;
    let z2 = z * z * l2;
    let k = rr.signum() * rr * rr * x2;
    if z.signum() * a2 * z2 > k {
        return (x2 + z2).sqrt() * il2 - rb;
    }
    if y.signum() * a2 * y2 < k {
        return (x2 + y2).sqrt() * il2 - ra;
    }
    ((x2 * a2 * il2).sqrt() + y * rr) * il2 - ra
}

/// A box of half sizes `h` about `c`, its edges rounded by `r`.
pub fn rbox(p: V3, c: V3, h: V3, r: f32) -> f32 {
    let q = sub(p, c);
    let d = [
        q[0].abs() - h[0] + r,
        q[1].abs() - h[1] + r,
        q[2].abs() - h[2] + r,
    ];
    len([d[0].max(0.0), d[1].max(0.0), d[2].max(0.0)]) + d[0].max(d[1]).max(d[2]).min(0.0) - r
}

/// A ring about the upright through `c`: `big` across, `small` thick.
pub fn torus(p: V3, c: V3, big: f32, small: f32) -> f32 {
    let q = sub(p, c);
    let x = (q[0] * q[0] + q[2] * q[2]).sqrt() - big;
    (x * x + q[1] * q[1]).sqrt() - small
}

/// Two shapes melted together over `k` metres.
pub fn smin(a: f32, b: f32, k: f32) -> f32 {
    if k <= 0.0 {
        return a.min(b);
    }
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - h * h * k * 0.25
}

/// `a` with `b` carved out of it, the edge rounded over `k`.
pub fn carve(a: f32, b: f32, k: f32) -> f32 {
    -smin(-a, b, k)
}

/// Where two shapes overlap, rounded over `k`.
pub fn both(a: f32, b: f32, k: f32) -> f32 {
    -smin(-a, -b, k)
}

/// The field's slope at `p`: the surface's outward normal there.
pub fn normal(f: &impl Field, p: V3) -> V3 {
    let e = 1e-3;
    let d = |o: V3| f(add(p, o)) - f(sub(p, o));
    norm([d([e, 0.0, 0.0]), d([0.0, e, 0.0]), d([0.0, 0.0, e])])
}

/// How open `p` is along `n` (1 open, toward 0 in a crease or where
/// something stands close over it).
pub fn openness(f: &impl Field, p: V3, n: V3, reach: f32) -> f32 {
    let mut shut = 0.0;
    let mut w = 1.0;
    for k in 1..=5 {
        let d = reach * k as f32 / 5.0;
        shut += w * (d - f(add(p, scale(n, d)))).max(0.0) / d;
        w *= 0.6;
    }
    (1.0 - 0.75 * shut).clamp(0.0, 1.0)
}

/// What a sculpt is painted with: the colour at a point (with its
/// normal) and its glow.
pub trait Paint: Fn(V3, V3) -> (V3, f32) {}
impl<P: Fn(V3, V3) -> (V3, f32)> Paint for P {}

/// The surface of `f` inside the box `lo`..`hi`, sampled every `cell`
/// metres, as a smooth mesh painted by `paint`; creases darkened by up to
/// `shade` (0 none, 1 black) over `reach` metres.
pub fn mesh(
    f: &impl Field,
    (lo, hi): (V3, V3),
    cell: f32,
    paint: &impl Paint,
    (shade, reach): (f32, f32),
) -> Geo {
    let n = [0, 1, 2].map(|k| (((hi[k] - lo[k]) / cell).ceil() as usize).max(1) + 1);
    let at = |i: usize, j: usize, k: usize| {
        [
            lo[0] + i as f32 * cell,
            lo[1] + j as f32 * cell,
            lo[2] + k as f32 * cell,
        ]
    };
    let idx = |i: usize, j: usize, k: usize| (k * n[1] + j) * n[0] + i;
    let mut d = vec![0.0f32; n[0] * n[1] * n[2]];
    for k in 0..n[2] {
        for j in 0..n[1] {
            for i in 0..n[0] {
                d[idx(i, j, k)] = f(at(i, j, k));
            }
        }
    }
    // A vertex in each cell the surface crosses: where its edges cross,
    // averaged, then pulled onto the surface.
    let cells = [n[0] - 1, n[1] - 1, n[2] - 1];
    let cid = |i: usize, j: usize, k: usize| (k * cells[1] + j) * cells[0] + i;
    let mut vert = vec![u32::MAX; cells[0] * cells[1] * cells[2]];
    let mut g = Geo::default();
    const EDGES: [(usize, usize); 12] = [
        (0, 1),
        (2, 3),
        (4, 5),
        (6, 7),
        (0, 2),
        (1, 3),
        (4, 6),
        (5, 7),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    for k in 0..cells[2] {
        for j in 0..cells[1] {
            for i in 0..cells[0] {
                let corner = |c: usize| (i + (c & 1), j + ((c >> 1) & 1), k + ((c >> 2) & 1));
                let v: [f32; 8] = std::array::from_fn(|c| {
                    let (x, y, z) = corner(c);
                    d[idx(x, y, z)]
                });
                let inside = v.iter().filter(|x| **x < 0.0).count();
                if inside == 0 || inside == 8 {
                    continue;
                }
                let mut sum = [0.0f32; 3];
                let mut m = 0.0;
                for (a, b) in EDGES {
                    if (v[a] < 0.0) != (v[b] < 0.0) {
                        let t = v[a] / (v[a] - v[b]);
                        let (pa, pb) = (corner(a), corner(b));
                        let (pa, pb) = (at(pa.0, pa.1, pa.2), at(pb.0, pb.1, pb.2));
                        sum = add(sum, add(pa, scale(sub(pb, pa), t)));
                        m += 1.0;
                    }
                }
                let mut p = scale(sum, 1.0 / m);
                for _ in 0..2 {
                    let nn = normal(f, p);
                    p = sub(p, scale(nn, f(p)));
                }
                let nn = normal(f, p);
                let (col, glow) = paint(p, nn);
                let lit = 1.0 - shade * (1.0 - openness(f, p, nn, reach));
                vert[cid(i, j, k)] = g.vertex(p, nn, scale(col, lit), glow);
            }
        }
    }
    // A quad across each grid edge the surface crosses, joining the four
    // cells about it, turned to face out.
    for k in 1..cells[2] {
        for j in 1..cells[1] {
            for i in 1..cells[0] {
                let here = d[idx(i, j, k)] < 0.0;
                let steps = [
                    (
                        d[idx(i + 1, j, k)] < 0.0,
                        [(i, j - 1, k - 1), (i, j, k - 1), (i, j, k), (i, j - 1, k)],
                    ),
                    (
                        d[idx(i, j + 1, k)] < 0.0,
                        [(i - 1, j, k - 1), (i, j, k - 1), (i, j, k), (i - 1, j, k)],
                    ),
                    (
                        d[idx(i, j, k + 1)] < 0.0,
                        [(i - 1, j - 1, k), (i, j - 1, k), (i, j, k), (i - 1, j, k)],
                    ),
                ];
                for (axis, (there, quad)) in steps.into_iter().enumerate() {
                    if here == there {
                        continue;
                    }
                    let q = quad.map(|(a, b, c)| vert[cid(a, b, c)]);
                    if q.contains(&u32::MAX) {
                        continue;
                    }
                    // The y edge's cells run the other way round.
                    let flip = here ^ (axis == 1);
                    if flip {
                        g.index(q[0], q[1], q[2]);
                        g.index(q[0], q[2], q[3]);
                    } else {
                        g.index(q[0], q[2], q[1]);
                        g.index(q[0], q[3], q[2]);
                    }
                }
            }
        }
    }
    g
}

/// A sheet woven over `u` and `v` (each 0 to 1, in `nu` and `nv` steps):
/// `at(u, v)` where it is, `paint(u, v)` its colour; its normals from its
/// own slopes (out along the `u` slope crossed with the `v` slope), so
/// light rolls across it. For cloth: folds cost no more than flat.
pub fn sheet(
    g: &mut Geo,
    (nu, nv): (usize, usize),
    at: impl Fn(f32, f32) -> V3,
    paint: impl Fn(f32, f32) -> V3,
) {
    let base = g.len() as u32;
    let e = 1e-3;
    for j in 0..=nv {
        let v = j as f32 / nv as f32;
        for i in 0..=nu {
            let u = i as f32 / nu as f32;
            let p = at(u, v);
            let du = sub(at((u + e).min(1.0), v), at((u - e).max(0.0), v));
            let dv = sub(at(u, (v + e).min(1.0)), at(u, (v - e).max(0.0)));
            let n = norm(crate::geo::cross(du, dv));
            g.vertex(p, n, paint(u, v), 0.0);
        }
    }
    let w = (nu + 1) as u32;
    for j in 0..nv as u32 {
        for i in 0..nu as u32 {
            let k = base + j * w + i;
            g.index(k, k + 1, k + w);
            g.index(k + 1, k + w + 1, k + w);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::STRIDE;

    fn faces_out(g: &Geo, c: V3) -> f32 {
        // How many triangles face away from the centre (they should all).
        let p = |k: u32| {
            let s = k as usize * STRIDE;
            [g.v[s], g.v[s + 1], g.v[s + 2]]
        };
        let mut out = 0;
        for t in g.i.chunks(3) {
            let (a, b, cc) = (p(t[0]), p(t[1]), p(t[2]));
            let n = crate::geo::cross(sub(b, a), sub(cc, a));
            let mid = scale(add(add(a, b), cc), 1.0 / 3.0);
            if dot(n, sub(mid, c)) > 0.0 {
                out += 1;
            }
        }
        out as f32 / (g.i.len() / 3) as f32
    }

    #[test]
    fn a_sphere_is_meshed_round_closed_and_facing_out() {
        let c = [0.1, 0.2, -0.3];
        let f = |p: V3| sphere(p, c, 0.25);
        let g = mesh(
            &f,
            ([-0.3, -0.2, -0.7], [0.5, 0.6, 0.1]),
            0.02,
            &|_, _| ([1.0; 3], 0.0),
            (0.0, 0.1),
        );
        assert!(g.triangles() > 1000, "{} triangles", g.triangles());
        for v in g.v.chunks(STRIDE) {
            let r = len(sub([v[0], v[1], v[2]], c));
            assert!((r - 0.25).abs() < 0.004, "on the surface: {r}");
            let out = dot(norm(sub([v[0], v[1], v[2]], c)), [v[3], v[4], v[5]]);
            assert!(out > 0.99, "its normal points out: {out}");
        }
        assert!(faces_out(&g, c) > 0.999, "{}", faces_out(&g, c));
    }

    #[test]
    fn a_crease_is_darker_than_open_ground() {
        // Two balls melted together: the waist between them is shut in.
        let f = |p: V3| {
            smin(
                sphere(p, [-0.2, 0.0, 0.0], 0.25),
                sphere(p, [0.2, 0.0, 0.0], 0.25),
                0.1,
            )
        };
        let top = [-0.2, 0.25, 0.0];
        let waist = [0.0, 0.17, 0.0];
        let open = openness(&f, top, normal(&f, top), 0.15);
        let p = sub(waist, scale(normal(&f, waist), f(waist)));
        let shut = openness(&f, p, normal(&f, p), 0.15);
        assert!(
            open > 0.95 && shut < open - 0.1,
            "open {open}, crease {shut}"
        );
    }

    #[test]
    fn a_cone_rounds_off_both_ends() {
        let (a, b) = ([0.0; 3], [0.0, 1.0, 0.0]);
        assert!((cone([0.3, 0.0, 0.0], a, b, 0.3, 0.1)).abs() < 1e-4);
        // Even, it is a capsule.
        assert!((cone([0.2, 0.5, 0.0], a, b, 0.2, 0.2)).abs() < 1e-4);
        assert!((cone([0.0, 1.1, 0.0], a, b, 0.3, 0.1)).abs() < 1e-3);
        assert!(cone([0.0, 0.5, 0.0], a, b, 0.3, 0.1) < 0.0);
    }
}
