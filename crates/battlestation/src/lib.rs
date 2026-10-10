//! Battlestation: a desk seen from the chair. Your own keyboard and mouse
//! move the ones on the desk: each key you press is pressed there by the
//! finger that types it (`keys`, `hands`), the mouse slides under a hand
//! that leaves the keys for it, and what you type lands in the terminal
//! on the monitor (`term`). Everything here is plain std, so the page and
//! its tests share it; the look is the page's.
//!
//! Space as the engine has it: metres, x east, y up, z south. You sit
//! facing north (-z); the desk's front edge is at z = 0.

pub mod demo;
pub mod hands;
pub mod keys;
pub mod laws;
pub mod term;

/// A point or a direction.
pub type V3 = [f32; 3];

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scale(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn len(a: V3) -> f32 {
    dot(a, a).sqrt()
}

/// `a` made a metre long (or straight ahead, north, if it has no length).
pub fn norm(a: V3) -> V3 {
    let l = len(a);
    if l < 1e-9 {
        [0.0, 0.0, -1.0]
    } else {
        scale(a, 1.0 / l)
    }
}

pub fn lerp(a: V3, b: V3, t: f32) -> V3 {
    add(a, scale(sub(b, a), t))
}

/// `a` turned `yaw` radians about the vertical (from -z toward +x: a
/// positive turn looks right).
pub fn turn(a: V3, yaw: f32) -> V3 {
    let (s, c) = yaw.sin_cos();
    [a[0] * c - a[2] * s, a[1], a[0] * s + a[2] * c]
}

pub fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A spring that settles without overshoot: `x` moving at `v` toward
/// `to`, `omega` its stiffness (about 4 / omega seconds to arrive),
/// stepped `dt` seconds. Stable at any step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spring {
    pub x: V3,
    pub v: V3,
}

impl Spring {
    pub fn at(x: V3) -> Spring {
        Spring { x, v: [0.0; 3] }
    }

    pub fn step(&mut self, to: V3, omega: f32, dt: f32) {
        let a = omega * dt;
        let e = 1.0 / (1.0 + a + 0.48 * a * a + 0.235 * a * a * a);
        for (k, &goal) in to.iter().enumerate() {
            let change = self.x[k] - goal;
            let temp = (self.v[k] + omega * change) * dt;
            self.v[k] = (self.v[k] - omega * temp) * e;
            self.x[k] = goal + (change + temp) * e;
        }
    }
}

/// One number on a spring (as `Spring`, in one dimension).
pub fn damp(x: &mut f32, v: &mut f32, to: f32, omega: f32, dt: f32) {
    let mut s = Spring {
        x: [*x, 0.0, 0.0],
        v: [*v, 0.0, 0.0],
    };
    s.step([to, 0.0, 0.0], omega, dt);
    (*x, *v) = (s.x[0], s.v[0]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spring_arrives_without_overshoot_at_any_step() {
        for dt in [0.001, 1.0 / 60.0, 0.1, 0.5] {
            let mut s = Spring::at([0.0; 3]);
            let mut most = 0.0f32;
            for _ in 0..(3.0 / dt) as usize {
                s.step([1.0, 0.0, 0.0], 30.0, dt);
                most = most.max(s.x[0]);
            }
            assert!((s.x[0] - 1.0).abs() < 1e-3, "arrives (dt {dt}): {}", s.x[0]);
            assert!(most < 1.0 + 1e-3, "never past (dt {dt}): {most}");
        }
    }

    #[test]
    fn turning_right_from_north_faces_east() {
        let e = turn([0.0, 0.0, -1.0], std::f32::consts::FRAC_PI_2);
        assert!((e[0] - 1.0).abs() < 1e-6 && e[2].abs() < 1e-6, "{e:?}");
    }
}
