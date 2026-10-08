//! Geometry: low-poly shapes into one vertex list, flat shaded: position (3),
//! normal (3), colour (3) and glow (1, how much it lights itself) a
//! vertex. Space: x east, y up, z south. Every face winds counter-clockwise seen
//! from outside, so its normal points out.

pub type V3 = [f32; 3];

/// Floats a vertex, and how they split into attributes.
pub const STRIDE: usize = 10;
pub const LAYOUT: [i32; 3] = [3, 3, 4];

pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn scale(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn norm(a: V3) -> V3 {
    scale(a, 1.0 / dot(a, a).sqrt().max(1e-9))
}

/// A colour from bytes.
pub const fn rgb(r: u8, g: u8, b: u8) -> V3 {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

pub fn mix(a: V3, b: V3, t: f32) -> V3 {
    add(scale(a, 1.0 - t), scale(b, t))
}

/// A cheap, fixed hash.
pub fn hash(x: i32, y: i32, k: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9e37_79b1)
        ^ (y as u32).wrapping_mul(0x85eb_ca77)
        ^ k.wrapping_mul(0xc2b2_ae3d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^ (h >> 12)
}

/// A hash as 0..1.
pub fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Vertices and the triangles between them (indices, three a triangle).
/// Flat shapes give each face its own corners; smooth ones (`sphere`,
/// `smooth`) share them, so light rolls across.
#[derive(Default)]
pub struct Geo {
    pub v: Vec<f32>,
    pub i: Vec<u32>,
}

impl Geo {
    /// A vertex: where, its normal, its colour and glow; its index.
    pub fn vertex(&mut self, p: V3, n: V3, col: V3, glow: f32) -> u32 {
        let k = (self.v.len() / STRIDE) as u32;
        self.v.extend_from_slice(&[
            p[0], p[1], p[2], n[0], n[1], n[2], col[0], col[1], col[2], glow,
        ]);
        k
    }

    pub fn tri(&mut self, a: V3, b: V3, c: V3, col: V3, glow: f32) {
        let n = norm(cross(sub(b, a), sub(c, a)));
        for p in [a, b, c] {
            let k = self.vertex(p, n, col, glow);
            self.i.push(k);
        }
    }

    /// Triangles over vertices already added.
    pub fn index(&mut self, a: u32, b: u32, c: u32) {
        self.i.extend_from_slice(&[a, b, c]);
    }

    /// Smooth every vertex's normal from the faces that share it (by
    /// index), and give it the noise-free shading a curved thing has.
    pub fn smooth(&mut self) {
        let n = self.v.len() / STRIDE;
        let mut acc = vec![[0.0f32; 3]; n];
        let at = |v: &Vec<f32>, k: u32| {
            let o = k as usize * STRIDE;
            [v[o], v[o + 1], v[o + 2]]
        };
        for t in self.i.chunks(3) {
            let (a, b, c) = (at(&self.v, t[0]), at(&self.v, t[1]), at(&self.v, t[2]));
            // Area-weighted: the cross product's length.
            let f = cross(sub(b, a), sub(c, a));
            for &k in t {
                let s = &mut acc[k as usize];
                *s = add(*s, f);
            }
        }
        for (k, s) in acc.iter().enumerate() {
            let m = norm(*s);
            self.v[k * STRIDE + 3..k * STRIDE + 6].copy_from_slice(&m);
        }
    }

    /// A smooth lump: a sphere of radii `r` about `at`, subdivided
    /// `detail` times from an icosahedron, its surface pushed in and out
    /// by `rough` (0 a ball, 0.3 a rock), the same for the same `seed`.
    pub fn sphere(
        &mut self,
        at: V3,
        r: V3,
        (detail, seed, rough): (u32, u32, f32),
        col: V3,
        glow: f32,
    ) {
        let t = (1.0 + 5.0f32.sqrt()) / 2.0;
        let mut pts: Vec<V3> = [
            [-1.0, t, 0.0],
            [1.0, t, 0.0],
            [-1.0, -t, 0.0],
            [1.0, -t, 0.0],
            [0.0, -1.0, t],
            [0.0, 1.0, t],
            [0.0, -1.0, -t],
            [0.0, 1.0, -t],
            [t, 0.0, -1.0],
            [t, 0.0, 1.0],
            [-t, 0.0, -1.0],
            [-t, 0.0, 1.0],
        ]
        .into_iter()
        .map(norm)
        .collect();
        let mut faces: Vec<[usize; 3]> = vec![
            [0, 11, 5],
            [0, 5, 1],
            [0, 1, 7],
            [0, 7, 10],
            [0, 10, 11],
            [1, 5, 9],
            [5, 11, 4],
            [11, 10, 2],
            [10, 7, 6],
            [7, 1, 8],
            [3, 9, 4],
            [3, 4, 2],
            [3, 2, 6],
            [3, 6, 8],
            [3, 8, 9],
            [4, 9, 5],
            [2, 4, 11],
            [6, 2, 10],
            [8, 6, 7],
            [9, 8, 1],
        ];
        for _ in 0..detail {
            let mut mid = std::collections::HashMap::new();
            let mut half = |a: usize, b: usize, pts: &mut Vec<V3>| {
                *mid.entry((a.min(b), a.max(b))).or_insert_with(|| {
                    pts.push(norm(add(pts[a], pts[b])));
                    pts.len() - 1
                })
            };
            let mut next = Vec::with_capacity(faces.len() * 4);
            for [a, b, c] in faces {
                let (ab, bc, ca) = (
                    half(a, b, &mut pts),
                    half(b, c, &mut pts),
                    half(c, a, &mut pts),
                );
                next.extend_from_slice(&[[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
            }
            faces = next;
        }
        let base = (self.v.len() / STRIDE) as u32;
        for d in &pts {
            let q = |v: f32| (v * 6.0).round() as i32;
            let bumps = unit(hash(q(d[0]) * 31 + q(d[1]), q(d[2]), seed)) - 0.5;
            let fine = unit(hash(
                q(d[0] * 3.0),
                q(d[1] * 3.0) * 7 + q(d[2] * 3.0),
                seed ^ 9,
            )) - 0.5;
            let k = 1.0 + rough * (bumps + 0.35 * fine);
            let p = [
                at[0] + d[0] * r[0] * k,
                at[1] + d[1] * r[1] * k,
                at[2] + d[2] * r[2] * k,
            ];
            self.vertex(p, *d, col, glow);
        }
        for [a, b, c] in faces {
            self.index(base + a as u32, base + b as u32, base + c as u32);
        }
    }

    /// A triangle facing away from `inside`.
    pub fn tri_out(&mut self, a: V3, b: V3, c: V3, inside: V3, col: V3, glow: f32) {
        let n = cross(sub(b, a), sub(c, a));
        if dot(n, sub(a, inside)) < 0.0 {
            self.tri(a, c, b, col, glow);
        } else {
            self.tri(a, b, c, col, glow);
        }
    }

    /// Four corners in order, counter-clockwise from outside.
    pub fn quad(&mut self, a: V3, b: V3, c: V3, d: V3, col: V3, glow: f32) {
        self.tri(a, b, c, col, glow);
        self.tri(a, c, d, col, glow);
    }

    /// A flat square on the ground's plane at height `y`, facing up.
    pub fn floor(
        &mut self,
        (x0, z0): (f32, f32),
        (x1, z1): (f32, f32),
        y: f32,
        col: V3,
        glow: f32,
    ) {
        self.quad(
            [x0, y, z0],
            [x0, y, z1],
            [x1, y, z1],
            [x1, y, z0],
            col,
            glow,
        );
    }

    /// An `n`-sided column standing at `at`: radius `r0` at its foot and
    /// `r1` at its head, `h` tall, turned by `rot`; with a lid if `lid`.
    #[allow(clippy::too_many_arguments)]
    pub fn column(
        &mut self,
        at: V3,
        n: usize,
        (r0, r1): (f32, f32),
        h: f32,
        rot: f32,
        col: V3,
        glow: f32,
        lid: bool,
    ) {
        let ring = |r: f32, y: f32, k: usize| {
            let a = rot + k as f32 / n as f32 * std::f32::consts::TAU;
            [at[0] + a.cos() * r, at[1] + y, at[2] + a.sin() * r]
        };
        let top = [at[0], at[1] + h, at[2]];
        for k in 0..n {
            let (p0, p1) = (ring(r0, 0.0, k), ring(r0, 0.0, k + 1));
            let (q0, q1) = (ring(r1, h, k), ring(r1, h, k + 1));
            if r1 > 0.001 {
                self.quad(p0, q0, q1, p1, col, glow);
                if lid {
                    self.tri(top, q1, q0, col, glow);
                }
            } else {
                self.tri(p0, top, p1, col, glow);
            }
        }
    }

    /// A box with its foot's centre at `at`, sized (x, y, z), turned by
    /// `rot`: its top one colour, its sides another.
    pub fn block(&mut self, at: V3, size: V3, rot: f32, top: V3, side: V3, glow: f32) {
        let (s, c) = rot.sin_cos();
        let (hx, hz) = (size[0] / 2.0, size[2] / 2.0);
        let p = |x: f32, y: f32, z: f32| [at[0] + x * c - z * s, at[1] + y, at[2] + x * s + z * c];
        let h = size[1];
        let (a, b, cc, d) = (
            p(-hx, 0.0, -hz),
            p(hx, 0.0, -hz),
            p(hx, 0.0, hz),
            p(-hx, 0.0, hz),
        );
        let (e, f, g, k) = (p(-hx, h, -hz), p(hx, h, -hz), p(hx, h, hz), p(-hx, h, hz));
        self.quad(e, k, g, f, top, glow);
        self.quad(a, e, f, b, side, glow);
        self.quad(b, f, g, cc, side, glow);
        self.quad(cc, g, k, d, side, glow);
        self.quad(d, k, e, a, side, glow);
    }

    /// A low-poly lump about `at`, radii `r`, its faces jittered by
    /// `seed` (a split octahedron: 32 faces).
    pub fn blob(&mut self, at: V3, r: V3, seed: u32, col: V3, glow: f32) {
        let axes: [V3; 6] = [
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ];
        // The same direction always jitters the same way.
        let place = |d: V3| {
            let d = norm(d);
            let q = |v: f32| (v * 8.0).round() as i32;
            let j = 0.82 + 0.3 * unit(hash(q(d[0]) * 17 + q(d[1]), q(d[2]), seed));
            [
                at[0] + d[0] * r[0] * j,
                at[1] + d[1] * r[1] * j,
                at[2] + d[2] * r[2] * j,
            ]
        };
        for &x in &[axes[0], axes[1]] {
            for &y in &[axes[2], axes[3]] {
                for &z in &[axes[4], axes[5]] {
                    let (xy, yz, zx) = (add(x, y), add(y, z), add(z, x));
                    for (a, b, c) in [(x, xy, zx), (y, yz, xy), (z, zx, yz), (xy, yz, zx)] {
                        let (a, b, c) = (place(a), place(b), place(c));
                        // Shade a little by height, as light falls from above.
                        let k = 0.85 + 0.25 * ((a[1] + b[1] + c[1]) / 3.0 - at[1]).signum();
                        self.tri_out(a, b, c, at, scale(col, k), glow);
                    }
                }
            }
        }
    }

    /// A spike from a base of radius `r` about `base` to `tip`.
    pub fn spike(&mut self, base: V3, tip: V3, r: f32, n: usize, col: V3, glow: f32) {
        let up = norm(sub(tip, base));
        let side = norm(cross(
            up,
            if up[1].abs() < 0.9 {
                [0.0, 1.0, 0.0]
            } else {
                [1.0, 0.0, 0.0]
            },
        ));
        let other = cross(up, side);
        let mid = add(base, scale(sub(tip, base), 0.3));
        for k in 0..n {
            let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
            let a1 = (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
            let p = |a: f32| {
                add(
                    base,
                    add(scale(side, a.cos() * r), scale(other, a.sin() * r)),
                )
            };
            self.tri_out(p(a0), p(a1), tip, mid, col, glow);
        }
    }

    /// A surface turned on a lathe: `profile` is (radius, height) from
    /// the foot up, turned `n` times about the upright through `at`; its
    /// rings share their vertices, so it shades smooth once `smooth` is
    /// called. The ends are closed where their radius is above 0.
    pub fn lathe(&mut self, at: V3, profile: &[(f32, f32)], n: usize, col: V3, glow: f32) {
        let base = (self.v.len() / STRIDE) as u32;
        for &(r, y) in profile {
            for k in 0..n {
                let a = k as f32 / n as f32 * std::f32::consts::TAU;
                let d = [a.cos(), 0.0, a.sin()];
                self.vertex(
                    [at[0] + d[0] * r, at[1] + y, at[2] + d[2] * r],
                    d,
                    col,
                    glow,
                );
            }
        }
        let ring = |j: usize, k: usize| base + (j * n + k % n) as u32;
        for j in 0..profile.len().saturating_sub(1) {
            for k in 0..n {
                let (a, b, c, d) = (
                    ring(j, k),
                    ring(j, k + 1),
                    ring(j + 1, k),
                    ring(j + 1, k + 1),
                );
                // A point (radius 0) needs one triangle, not two.
                if profile[j + 1].0 > 0.001 || profile[j].0 <= 0.001 {
                    self.index(b, c, d);
                }
                if profile[j].0 > 0.001 {
                    self.index(a, c, b);
                }
            }
        }
        // Lids: a centre and a fan, facing out of the ends.
        if let (Some(&(r0, y0)), Some(&(r1, y1))) = (profile.first(), profile.last()) {
            if r0 > 0.001 {
                let c = self.vertex([at[0], at[1] + y0, at[2]], [0.0, -1.0, 0.0], col, glow);
                for k in 0..n {
                    self.index(c, ring(0, k), ring(0, k + 1));
                }
            }
            if r1 > 0.001 {
                let j = profile.len() - 1;
                let c = self.vertex([at[0], at[1] + y1, at[2]], [0.0, 1.0, 0.0], col, glow);
                for k in 0..n {
                    self.index(c, ring(j, k + 1), ring(j, k));
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        self.v.len() / STRIDE
    }

    pub fn triangles(&self) -> usize {
        self.i.len() / 3
    }

    pub fn is_empty(&self) -> bool {
        self.v.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every face's normal points away from the shape's centre.
    fn outward(g: &Geo, centre: V3) -> bool {
        let at = |k: u32| {
            let o = k as usize * STRIDE;
            (
                [g.v[o], g.v[o + 1], g.v[o + 2]],
                [g.v[o + 3], g.v[o + 4], g.v[o + 5]],
            )
        };
        g.i.chunks(3).all(|t| {
            let ((a, na), (b, _), (c, _)) = (at(t[0]), at(t[1]), at(t[2]));
            let mid = scale(add(add(a, b), c), 1.0 / 3.0);
            // The face winds outward, and its first normal agrees.
            let face = cross(sub(b, a), sub(c, a));
            dot(face, sub(mid, centre)) > 0.0 && dot(na, sub(mid, centre)) > 0.0
        })
    }

    #[test]
    fn shapes_face_outward() {
        let mut g = Geo::default();
        g.block([0.0; 3], [1.0, 2.0, 1.0], 0.4, [1.0; 3], [1.0; 3], 0.0);
        assert!(outward(&g, [0.0, 1.0, 0.0]));
        let mut g = Geo::default();
        g.column([0.0; 3], 7, (0.5, 0.3), 2.0, 0.1, [1.0; 3], 0.0, true);
        assert!(outward(&g, [0.0, 1.0, 0.0]));
        let mut g = Geo::default();
        g.column([0.0; 3], 6, (0.5, 0.0), 1.0, 0.0, [1.0; 3], 0.0, false);
        assert!(outward(&g, [0.0, 0.3, 0.0]));
        let mut g = Geo::default();
        g.blob([1.0, 1.0, 1.0], [0.5, 0.3, 0.5], 7, [1.0; 3], 0.0);
        assert_eq!(g.len(), 32 * 3);
        assert!(outward(&g, [1.0, 1.0, 1.0]));
        let mut g = Geo::default();
        g.sphere([0.0, 2.0, 0.0], [1.0, 0.7, 1.0], (2, 5, 0.2), [1.0; 3], 0.0);
        assert_eq!(g.triangles(), 20 * 16);
        assert_eq!(g.len(), 162, "shared corners: smooth");
        assert!(outward(&g, [0.0, 2.0, 0.0]));
        g.smooth();
        assert!(outward(&g, [0.0, 2.0, 0.0]), "still outward once smoothed");
        let mut g = Geo::default();
        g.lathe(
            [0.0; 3],
            &[(0.5, 0.0), (0.45, 1.0), (0.3, 2.0), (0.0, 2.6)],
            10,
            [1.0; 3],
            0.0,
        );
        g.smooth();
        assert!(outward(&g, [0.0, 1.2, 0.0]), "a lathed shape faces out");
        let mut g = Geo::default();
        g.floor((0.0, 0.0), (1.0, 1.0), 0.0, [1.0; 3], 0.0);
        assert!(g.v.chunks(STRIDE).all(|v| v[4] > 0.99), "a floor faces up");
    }
}
