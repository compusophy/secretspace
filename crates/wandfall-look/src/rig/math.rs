//! Turns and moves as matrices, chained from the feet up, and points
//! carried through them. The model faces +x, up is +y, its right is +z.

use render::geo::{self, V3};
use render::{m4, M4};

pub fn tr(v: V3) -> M4 {
    m4::basis(v, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0])
}

/// About the side axis (+z): positive lifts +x toward +y.
pub fn rz(a: f32) -> M4 {
    let (s, c) = a.sin_cos();
    m4::basis([0.0; 3], [c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0])
}

/// About the forward axis (+x): positive tips +y toward +z.
pub fn rx(a: f32) -> M4 {
    let (s, c) = a.sin_cos();
    m4::basis([0.0; 3], [1.0, 0.0, 0.0], [0.0, c, s], [0.0, -s, c])
}

/// About the upright, as headings turn (+x toward +z).
pub fn ry(a: f32) -> M4 {
    let (s, c) = a.sin_cos();
    m4::basis([0.0; 3], [c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c])
}

pub fn sc(v: V3) -> M4 {
    m4::basis(
        [0.0; 3],
        [v[0], 0.0, 0.0],
        [0.0, v[1], 0.0],
        [0.0, 0.0, v[2]],
    )
}

pub fn chain(list: &[M4]) -> M4 {
    list.iter().skip(1).fold(list[0], |acc, m| m4::mul(&acc, m))
}

pub fn point(m: &M4, p: V3) -> V3 {
    [
        m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
        m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
        m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
    ]
}

/// A direction (no move) through a matrix.
pub fn dir(m: &M4, v: V3) -> V3 {
    [
        m[0] * v[0] + m[4] * v[1] + m[8] * v[2],
        m[1] * v[0] + m[5] * v[1] + m[9] * v[2],
        m[2] * v[0] + m[6] * v[1] + m[10] * v[2],
    ]
}

/// A bone from `from` to `to`: its mesh hangs down its -y from `from`,
/// its +x turned as near `fwd` as it can be.
pub fn bone(from: V3, to: V3, fwd: V3) -> M4 {
    let y = geo::norm(geo::sub(from, to));
    let x = geo::norm(geo::sub(fwd, geo::scale(y, geo::dot(fwd, y))));
    let z = geo::cross(x, y);
    m4::basis(from, x, y, z)
}

/// Two bones (`a` then `b` long) from `hip` reaching for `foot`, the knee
/// bent toward `pole`: where the knee goes, and where the foot can reach.
pub fn reach(hip: V3, foot: V3, pole: V3, (a, b): (f32, f32)) -> (V3, V3) {
    let d = geo::sub(foot, hip);
    let len = geo::dot(d, d).sqrt().clamp(0.05, a + b - 1e-3);
    let u = geo::norm(d);
    let foot = geo::add(hip, geo::scale(u, len));
    let ca = ((a * a + len * len - b * b) / (2.0 * a * len)).clamp(-1.0, 1.0);
    let sa = (1.0 - ca * ca).sqrt();
    let n = geo::norm(geo::sub(pole, geo::scale(u, geo::dot(pole, u))));
    let knee = geo::add(hip, geo::add(geo::scale(u, a * ca), geo::scale(n, a * sa)));
    (knee, foot)
}

/// An angle brought within a half turn of zero.
pub fn wrap(a: f32) -> f32 {
    let t = std::f32::consts::TAU;
    a - (a / t).round() * t
}

/// 0 below `a`, 1 above `b`, smooth between.
pub fn ease(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A critically damped spring: it moves toward where it is told, settling
/// in about `half` seconds, never overshooting (the way a body eases).
#[derive(Clone, Copy, Debug, Default)]
pub struct Spring {
    pub x: f32,
    pub v: f32,
}

impl Spring {
    pub fn step(&mut self, to: f32, half: f32, dt: f32) -> f32 {
        let y = 2.0 * std::f32::consts::LN_2 / half.max(1e-4);
        let j0 = self.x - to;
        let j1 = self.v + j0 * y;
        let e = (-y * dt).exp();
        self.x = e * (j0 + j1 * dt) + to;
        self.v = e * (self.v - j1 * y * dt);
        self.x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_bones_reach_and_a_spring_settles() {
        let (knee, foot) = reach(
            [0.0, 1.0, 0.0],
            [0.2, 0.1, 0.0],
            [1.0, 0.0, 0.0],
            (0.5, 0.5),
        );
        let len = |a: V3, b: V3| geo::dot(geo::sub(a, b), geo::sub(a, b)).sqrt();
        assert!((len([0.0, 1.0, 0.0], knee) - 0.5).abs() < 1e-4);
        assert!((len(knee, foot) - 0.5).abs() < 1e-4);
        assert!(knee[0] > 0.2, "the knee bends forward: {knee:?}");
        let mut s = Spring::default();
        for _ in 0..120 {
            s.step(1.0, 0.1, 1.0 / 60.0);
            assert!(s.x <= 1.0 + 1e-4, "no overshoot");
        }
        assert!((s.x - 1.0).abs() < 1e-3);
        let m = chain(&[tr([1.0, 2.0, 3.0]), ry(std::f32::consts::FRAC_PI_2)]);
        let p = point(&m, [1.0, 0.0, 0.0]);
        assert!(
            (p[0] - 1.0).abs() < 1e-5 && (p[2] - 4.0).abs() < 1e-5,
            "{p:?}"
        );
        assert!((wrap(7.0) - (7.0 - std::f32::consts::TAU)).abs() < 1e-5);
    }
}
