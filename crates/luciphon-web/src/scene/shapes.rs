//! Low-poly shapes into one vertex list, flat shaded: position (3),
//! normal (3), colour (3) and glow (1, how much it lights itself) a
//! vertex. Space is the renderer's: x east, y up, z south (a tile (x, y)
//! of the world is x, z here). Every face winds counter-clockwise seen
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

#[derive(Default)]
pub struct Geo {
    pub v: Vec<f32>,
}

impl Geo {
    pub fn tri(&mut self, a: V3, b: V3, c: V3, col: V3, glow: f32) {
        let n = norm(cross(sub(b, a), sub(c, a)));
        for p in [a, b, c] {
            self.v.extend_from_slice(&[
                p[0], p[1], p[2], n[0], n[1], n[2], col[0], col[1], col[2], glow,
            ]);
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

    pub fn len(&self) -> usize {
        self.v.len() / STRIDE
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
        g.v.chunks(STRIDE * 3).all(|t| {
            let mid = scale(
                add(
                    add([t[0], t[1], t[2]], [t[10], t[11], t[12]]),
                    [t[20], t[21], t[22]],
                ),
                1.0 / 3.0,
            );
            dot([t[3], t[4], t[5]], sub(mid, centre)) > 0.0
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
        g.floor((0.0, 0.0), (1.0, 1.0), 0.0, [1.0; 3], 0.0);
        assert!(g.v.chunks(STRIDE).all(|v| v[4] > 0.99), "a floor faces up");
    }
}
