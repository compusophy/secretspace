//! How a wizard moves, as the page has watched it, eased by springs so
//! nothing jerks with each frame:
//!
//! - its velocity, and from it how fast it goes and which way, relative to
//!   where it faces (its aim);
//! - its hips, turning toward where it goes (a strafe turns them up to a
//!   right angle's two thirds; backing away keeps them forward and plays
//!   the stride backward) while its chest keeps its aim;
//! - its stride: a step's length grows from a walk to a run, and the
//!   phase advances by distance travelled, so each foot stays planted on
//!   the ground while it bears the weight;
//! - its lean into speed and acceleration, its bank into a turn, the drag
//!   of its robe and the lag of its hat's tip;
//! - in the air, crouched, or landing (how hard);
//! - where it faces and looks, eased: an aim follows what the crosshair
//!   is on, which can jump from the ground near by to the sky beyond.

use std::f32::consts::{FRAC_PI_2, PI};

use render::V3;

use super::math::{ease, wrap, Spring};

/// Metres a step at a walk and at a run; speeds (m/s) from a walk to a
/// run.
const STEP_WALK: f32 = 0.72;
const STEP_RUN: f32 = 1.45;
const WALK: f32 = 2.0;
const RUN: f32 = 5.2;
/// The furthest the hips turn from the aim, going forward and backing.
const HIPS_FWD: f32 = 1.05;
const HIPS_BACK: f32 = 0.55;
/// How quickly it turns to face (and look) where it aims (seconds).
const FACE: f32 = 0.06;

#[derive(Clone, Copy, Debug, Default)]
pub struct Anim {
    vx: Spring,
    vz: Spring,
    /// Where it faces (radians, unwrapped) and looks up (radians), eased.
    pub face: Spring,
    pub pitch: Spring,
    /// Speed (m/s), and forward and to the right of its facing.
    pub speed: f32,
    pub fwd: f32,
    pub side: f32,
    /// Hips turned off its facing (radians, + toward its right); 0 going
    /// forward, 1 backing away.
    pub hips: Spring,
    pub back: Spring,
    /// 0 at a walk, 1 at a run; 0 to 1 into a sprint; sliding (0 to 1).
    pub run: Spring,
    pub sprint: Spring,
    pub slide: Spring,
    /// Through two steps, 0 to 1 (the left foot lands at 0).
    pub phase: f32,
    /// Metres a step now.
    pub stride: f32,
    /// Leaning forward, banking right (radians).
    pub lean: Spring,
    pub bank: Spring,
    /// The robe's drag behind, and the hat tip's lag (radians).
    pub cloth: Spring,
    pub hat: Spring,
    /// 0 on the ground, 1 in the air; crouched (0 to 1); the squat of a
    /// landing (1 a hard one, easing away).
    pub air: f32,
    pub crouch: f32,
    pub land: f32,
    /// On its broom (1) or on its feet (0), eased as it gets on or off.
    pub seat: Spring,
    aloft: f32,
    last: V3,
    was: (f32, f32),
    seen: bool,
}

impl Anim {
    /// Watch it at `p`, facing `yaw` and looking `pitch` up (radians), on
    /// the ground or not, crouched, sliding, `dt` seconds on. A landing
    /// says how hard it was (0 to 1).
    pub fn step(
        &mut self,
        p: V3,
        (yaw, pitch): (f32, f32),
        (ground, crouch, slide): (bool, bool, bool),
        dt: f32,
    ) -> Option<f32> {
        if !self.seen {
            self.last = p;
            self.seen = true;
            self.stride = STEP_WALK;
            self.face.x = yaw;
            self.pitch.x = pitch;
        }
        let dt = dt.max(1e-3);
        let to = self.face.x + wrap(yaw - self.face.x);
        let yaw = self.face.step(to, FACE, dt);
        self.pitch.step(pitch, FACE, dt);
        let (mut mx, mut mz) = ((p[0] - self.last[0]) / dt, (p[2] - self.last[2]) / dt);
        // A blink is not a stride.
        if mx * mx + mz * mz > 30.0 * 30.0 {
            (mx, mz) = (0.0, 0.0);
        }
        let vx = self.vx.step(mx, 0.07, dt);
        let vz = self.vz.step(mz, 0.07, dt);
        let (s, c) = yaw.sin_cos();
        let (f, r) = (vx * c + vz * s, -vx * s + vz * c);
        let accel = ((f - self.was.0) / dt, (r - self.was.1) / dt);
        self.was = (f, r);
        (self.fwd, self.side) = (f, r);
        self.speed = (f * f + r * r).sqrt();
        let moving = ease(0.3, 1.2, self.speed);
        // Which way it goes, off its facing; backing away past 110°, and
        // forward again inside 90° (so it does not flicker between).
        let rel = r.atan2(f);
        let backing = if rel.abs() > 1.92 {
            1.0
        } else if rel.abs() < FRAC_PI_2 {
            0.0
        } else {
            (self.back.x > 0.5) as i32 as f32
        };
        let back = self.back.step(backing * moving, 0.12, dt);
        let turn = if back > 0.5 {
            wrap(rel - PI).clamp(-HIPS_BACK, HIPS_BACK)
        } else {
            rel.clamp(-HIPS_FWD, HIPS_FWD)
        };
        self.hips.step(turn * moving, 0.11, dt);
        let run = self.run.step(ease(WALK, RUN, self.speed), 0.2, dt);
        let sprint = self.sprint.step(ease(7.6, 9.6, self.speed), 0.2, dt);
        self.slide.step(slide as i32 as f32, 0.07, dt);
        let low = if crouch { 1.0 } else { 0.0 };
        self.crouch += (low - self.crouch) * (1.0 - (-dt * 12.0).exp());
        self.stride =
            (STEP_WALK + (STEP_RUN - STEP_WALK) * run + 0.35 * sprint) * (1.0 - 0.3 * self.crouch);
        if ground {
            // Two steps a cycle: the feet move as far as the body does.
            self.phase = (self.phase + self.speed * dt / (2.0 * self.stride)).fract();
        }
        let lean =
            0.12 * run + 0.14 * sprint + (accel.0 * 0.012).clamp(-0.12, 0.12) + 0.08 * run * moving;
        self.lean.step(lean.clamp(-0.2, 0.35), 0.15, dt);
        self.bank
            .step((-accel.1 * 0.01).clamp(-0.2, 0.2) * moving, 0.12, dt);
        self.cloth.step(
            0.35 * (self.speed / 7.0).min(1.0) + 0.15 * self.air,
            0.18,
            dt,
        );
        self.hat.step(
            0.25 * (f / 7.0).clamp(-0.5, 1.0) + (accel.0 * 0.01).clamp(-0.2, 0.2),
            0.28,
            dt,
        );
        let up = if ground { 0.0 } else { 1.0 };
        self.air += (up - self.air) * (1.0 - (-dt * 9.0).exp());
        self.land *= (-dt * 8.0).exp();
        self.last = p;
        // Down again after a while up: how hard, by how long it fell.
        let landed = if ground && self.aloft > 0.25 {
            let hard = ((self.aloft - 0.2) / 0.8).clamp(0.15, 1.0);
            self.land = self.land.max(hard);
            Some(hard)
        } else {
            None
        };
        self.aloft = if ground { 0.0 } else { self.aloft + dt };
        landed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walk(a: &mut Anim, v: (f32, f32), yaw: f32, secs: f32) {
        let mut p = a.last;
        for _ in 0..(secs * 60.0) as usize {
            p = [p[0] + v.0 / 60.0, 0.0, p[2] + v.1 / 60.0];
            a.step(p, (yaw, 0.0), (true, false, false), 1.0 / 60.0);
        }
    }

    #[test]
    fn hips_turn_to_a_strafe_and_stay_forward_backing_away() {
        // Facing +x, running to its right (+z): the hips turn right.
        let mut a = Anim::default();
        walk(&mut a, (0.0, 6.0), 0.0, 1.0);
        assert!(a.hips.x > 0.9, "{a:?}");
        assert!(a.back.x < 0.1);
        // Backing away (-x): hips forward, the stride played back.
        let mut a = Anim::default();
        walk(&mut a, (-5.0, 0.0), 0.0, 1.0);
        assert!(a.hips.x.abs() < 0.1 && a.back.x > 0.9, "{a:?}");
        // Running forward: a run, a long stride, leaning in.
        let mut a = Anim::default();
        walk(&mut a, (7.0, 0.0), 0.0, 1.0);
        assert!(a.run.x > 0.9 && a.stride > 1.3 && a.lean.x > 0.1, "{a:?}");
    }
}
