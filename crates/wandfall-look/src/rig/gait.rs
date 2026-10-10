//! How a wizard moves, as the page has watched it, eased by springs so
//! nothing jerks with each frame:
//!
//! - its velocity, and from it how fast it goes and which way, relative to
//!   where it faces (its aim);
//! - its hips, turning a little toward where it goes (backing away, a
//!   little toward the way it backs) while its chest keeps its aim; and
//!   its course off them, along which its feet step (sideways in a
//!   strafe, back when it backs away), so a planted foot never slides;
//! - standing, its feet held where they are as it turns to aim, until
//!   they are a good way round and two quick steps bring them after it;
//! - its stride: a step's length grows from a walk to a run, and the
//!   phase advances by distance travelled, so each foot stays planted on
//!   the ground while it bears the weight;
//! - its lean into speed and acceleration (taken in the world, so a
//!   steady turn leans it in), its bank into a turn, the drag of its robe
//!   and the lag of its hat's tip;
//! - in the air (rising or falling, and a jump made there, off a wall or
//!   not, unless something else threw it: a Tether's haul, a Gust's
//!   blast), crouched, or landing (how hard);
//! - where it faces and looks, eased: an aim follows what the crosshair
//!   is on, which can jump from the ground near by to the sky beyond.

use std::f32::consts::{FRAC_PI_2, PI};

use render::V3;
use wandfall::laws;

use super::math::{ease, wrap, Spring};
use super::pose::REST_DUTY;

/// Metres a step at a walk and at a run; speeds (m/s) from a walk to a
/// run (most of the laws' run), and from a run into the laws' sprint.
const STEP_WALK: f32 = 0.72;
const STEP_RUN: f32 = 1.45;
const WALK: f32 = 2.0;
const RUN: f32 = laws::RUN * 0.75;
const SPRINT: (f32, f32) = (laws::RUN + 0.6, laws::SPRINT - 0.4);
/// The furthest the hips turn from the aim, going forward and backing.
const HIPS_FWD: f32 = 0.6;
const HIPS_BACK: f32 = 0.45;
/// How quickly it turns to face (and look) where it aims (seconds).
const FACE: f32 = 0.06;
/// How quickly it goes up into the air and comes down out of it (each
/// second): a landing plants its feet at once.
const INTO_AIR: f32 = 9.0;
const OUT_OF_AIR: f32 = 45.0;
/// Standing, how far a foot stays turned off its aim before it steps
/// round after it (radians), and how long the two steps take (s).
const TWIST: f32 = 0.7;
const SHUFFLE: f32 = 0.4;
/// A jump in the air: how much quicker it rises than it was (m/s), and
/// how long its pose lasts (s); off a wall, its way turns at least this
/// much (radians).
const AIR_JUMP: f32 = 4.0;
const FLIP: f32 = 0.4;
const KICK_TURN: f32 = 1.05;
/// After it is shoved (`Anim::shove`), how long a rise or a turn in the
/// air is the shove's and not a jump of its own (s): a Tether lets go
/// with a hop up.
const SHOVE: f32 = 0.4;

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
    /// Which way it goes off its hips (radians): the feet step along it,
    /// so a strafe steps sideways and backing away steps back.
    pub course: f32,
    /// Through two steps, 0 to 1 (the left foot lands at 0).
    pub phase: f32,
    /// Metres a step now.
    pub stride: f32,
    /// A foot came down in the last step (on the ground, going, not
    /// sliding): a footfall to hear.
    pub footfall: bool,
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
    /// Rising (+) or falling (-), m/s, eased.
    pub vy: Spring,
    /// A jump made in the air (1 just now, to 0), and, if it kicked off a
    /// wall, which way the wall lay (radians off its facing, + right).
    pub flip: f32,
    pub wall: Option<f32>,
    /// Standing, each foot's heading held off its facing (radians) as it
    /// turns to aim; stepping round after it (1 as the steps begin, to 0).
    pub held: [f32; 2],
    pub shuffle: f32,
    /// Seconds yet that what it does in the air is not its own (`shove`).
    shoved: f32,
    aloft: f32,
    last: V3,
    was: (f32, f32),
    seen: bool,
}

impl Anim {
    /// Something other than its legs moves it now (a Tether hauls it, a
    /// Gust has thrown it): until a while after, a rise or a sharp turn
    /// in the air is not a jump of its own (no tuck, no kick off a wall
    /// that is not there).
    pub fn shove(&mut self) {
        self.shoved = SHOVE;
        // Thrown from where it kicked off, the wall is not by it now.
        self.wall = None;
    }

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
        let faced = self.face.x;
        let yaw = self.face.step(to, FACE, dt);
        self.pitch.step(pitch, FACE, dt);
        let (mut mx, mut mz) = ((p[0] - self.last[0]) / dt, (p[2] - self.last[2]) / dt);
        let mut my = (p[1] - self.last[1]) / dt;
        // A blink is not a stride (nor a leap).
        if mx * mx + mz * mz > 30.0 * 30.0 || my.abs() > 40.0 {
            (mx, mz, my) = (0.0, 0.0, 0.0);
        }
        // A jump made in the air: a while off the ground, it rises far
        // quicker than it was (and nothing else threw it); off a wall if
        // its way turned sharply too (the wall lies the way it was going).
        let own = self.shoved <= 0.0;
        self.shoved = (self.shoved - dt).max(0.0);
        if own && !ground && self.aloft > 0.12 && my - self.vy.x > AIR_JUMP {
            self.flip = 1.0;
            let (ox, oz) = (self.vx.x, self.vz.x);
            let fast = ox * ox + oz * oz > 4.0 && mx * mx + mz * mz > 4.0;
            let turn = wrap(mz.atan2(mx) - oz.atan2(ox));
            self.wall = (fast && turn.abs() > KICK_TURN).then(|| wrap(oz.atan2(ox) - yaw));
        }
        self.flip = (self.flip - dt / FLIP).max(0.0);
        if self.flip <= 0.0 {
            self.wall = None;
        }
        self.vy.step(my, 0.07, dt);
        let vx = self.vx.step(mx, 0.07, dt);
        let vz = self.vz.step(mz, 0.07, dt);
        let (s, c) = yaw.sin_cos();
        let (f, r) = (vx * c + vz * s, -vx * s + vz * c);
        // Its acceleration in the world (so a steady turn, which speeds it
        // toward the turn's middle, counts), then off its facing.
        let (ax, az) = ((vx - self.was.0) / dt, (vz - self.was.1) / dt);
        self.was = (vx, vz);
        let accel = (ax * c + az * s, -ax * s + az * c);
        (self.fwd, self.side) = (f, r);
        self.speed = (f * f + r * r).sqrt();
        let moving = ease(0.3, 1.2, self.speed);
        self.turn_feet(yaw - faced, ground && moving < 0.05, dt);
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
        let hips = self.hips.step(turn * moving, 0.11, dt);
        self.course = wrap(rel - hips);
        let run = self.run.step(ease(WALK, RUN, self.speed), 0.2, dt);
        let sprint = self
            .sprint
            .step(ease(SPRINT.0, SPRINT.1, self.speed), 0.2, dt);
        self.slide.step(slide as i32 as f32, 0.07, dt);
        let low = if crouch { 1.0 } else { 0.0 };
        self.crouch += (low - self.crouch) * (1.0 - (-dt * 12.0).exp());
        // Steps to the side are shorter (and quicker), so the feet never
        // cross.
        self.stride = (STEP_WALK + (STEP_RUN - STEP_WALK) * run + 0.35 * sprint)
            * (1.0 - 0.3 * self.crouch)
            * (1.0 - 0.4 * self.course.sin().abs());
        // Landing, the foot that led reaches the ground first: its stance
        // begins.
        if ground && self.aloft > 0.15 {
            let lead = (self.phase < 0.5) as i32 as f32;
            self.phase = (1.0 - 0.5 * lead).fract();
        }
        let was = self.phase;
        if ground {
            // Two steps a cycle: the feet move as far as the body does.
            self.phase = (self.phase + self.speed * dt / (2.0 * self.stride)).fract();
        }
        self.footfall = ground
            && !slide
            && self.speed > 0.6
            && (self.phase < was || (was < 0.5 && self.phase >= 0.5));
        let lean =
            0.12 * run + 0.14 * sprint + (accel.0 * 0.012).clamp(-0.12, 0.12) + 0.08 * run * moving;
        self.lean.step(lean.clamp(-0.2, 0.35), 0.15, dt);
        self.bank
            .step((accel.1 * 0.04).clamp(-0.3, 0.3) * moving, 0.12, dt);
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
        let (up, rate) = if ground {
            (0.0, OUT_OF_AIR)
        } else {
            (1.0, INTO_AIR)
        };
        self.air += (up - self.air) * (1.0 - (-dt * rate).exp());
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

    /// Its feet as it turns `turned` radians: standing (`still`), each
    /// stays where it is until one is a good way round; then two quick
    /// steps, each foot turning to its facing while it is up (the one on
    /// the side it turns to first). Going, or in the air, they come round.
    fn turn_feet(&mut self, turned: f32, still: bool, dt: f32) {
        if self.shuffle > 0.0 {
            self.shuffle = (self.shuffle - dt / SHUFFLE).max(0.0);
            self.phase = (self.phase + dt / SHUFFLE).fract();
            for (side, held) in self.held.iter_mut().enumerate() {
                *held -= turned;
                if (self.phase + 0.5 * side as f32).fract() >= REST_DUTY {
                    *held *= (-dt / 0.035).exp();
                }
            }
        } else if still {
            for held in &mut self.held {
                *held -= turned;
            }
            if self.held.iter().any(|h| h.abs() > TWIST) {
                self.shuffle = 1.0;
                // The right foot first turning right (its feet held left).
                let first = (self.held[0] + self.held[1] < 0.0) as usize;
                self.phase = (REST_DUTY + 0.5 * first as f32).fract();
            }
        } else {
            for held in &mut self.held {
                *held *= (-dt / 0.1).exp();
            }
        }
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
        // Facing +x, running to its right (+z): the hips turn a little
        // right, and the feet step the rest of the way.
        let mut a = Anim::default();
        walk(&mut a, (0.0, 6.0), 0.0, 1.0);
        assert!(a.hips.x > 0.5 && a.hips.x < 0.7, "{a:?}");
        assert!((a.hips.x + a.course - FRAC_PI_2).abs() < 0.05, "{a:?}");
        assert!(a.back.x < 0.1);
        // Backing away (-x): hips forward, the stride played back.
        let mut a = Anim::default();
        walk(&mut a, (-5.0, 0.0), 0.0, 1.0);
        assert!(a.hips.x.abs() < 0.1 && a.back.x > 0.9, "{a:?}");
        assert!(a.course.abs() > 3.0, "it steps back: {a:?}");
        // Running forward: a run, a long stride, leaning in.
        let mut a = Anim::default();
        walk(&mut a, (7.0, 0.0), 0.0, 1.0);
        assert!(a.run.x > 0.9 && a.stride > 1.3 && a.lean.x > 0.1, "{a:?}");
    }

    #[test]
    fn a_steady_turn_banks_into_it() {
        // Round a 10 m circle at 7 m/s, facing the way it goes: banked in
        // (+ toward its right) and held there, a turn to the right or left.
        for way in [1.0f32, -1.0] {
            let mut a = Anim::default();
            let (r, v) = (10.0f32, 7.0f32);
            let mut banks = Vec::new();
            for k in 0..180 {
                let t = k as f32 / 60.0;
                let a0 = way * v / r * t;
                let p = [r * a0.sin() * way, 0.0, way * r * (1.0 - a0.cos())];
                a.step(p, (a0, 0.0), (true, false, false), 1.0 / 60.0);
                if k > 90 {
                    banks.push(a.bank.x * way);
                }
            }
            let (lo, hi) = banks
                .iter()
                .fold((9.0f32, -9.0f32), |(l, h), &b| (l.min(b), h.max(b)));
            assert!(
                lo > 0.12 && hi < 0.3 && hi - lo < 0.03,
                "{way}: banked {lo} to {hi}"
            );
        }
        // Running straight, none; a flick of the aim does not bank it.
        let mut a = Anim::default();
        walk(&mut a, (7.0, 0.0), 0.0, 1.0);
        walk(&mut a, (7.0, 0.0), 0.8, 0.3);
        assert!(a.bank.x.abs() < 0.02, "{a:?}");
    }

    #[test]
    fn a_shove_in_the_air_is_not_a_jump() {
        // Running (+z), off a ledge and falling a while; then thrown back
        // (-x) and up as a Gust throws (`laws::GUST_LIFT`), or hauled
        // away by a Tether and let go with its hop. Watched alone, that
        // is a kick off a wall; shoved, it is neither a tuck nor a kick.
        let throw = |a: &mut Anim, shoved: bool| {
            walk(a, (0.0, 6.0), 0.0, 1.0);
            let mut p = a.last;
            for _ in 0..12 {
                p = [p[0], p[1] - 2.0 / 60.0, p[2] + 6.0 / 60.0];
                a.step(p, (0.0, 0.0), (false, false, false), 1.0 / 60.0);
            }
            let mut flipped = false;
            for k in 0..20 {
                if shoved && k < 6 {
                    a.shove();
                }
                p = [p[0] - 9.0 / 60.0, p[1] + laws::GUST_LIFT / 60.0, p[2]];
                a.step(p, (0.0, 0.0), (false, false, false), 1.0 / 60.0);
                flipped |= a.flip > 0.0 || a.wall.is_some();
            }
            flipped
        };
        assert!(throw(&mut Anim::default(), false), "unshoved, a kick");
        assert!(!throw(&mut Anim::default(), true), "shoved, none");
        // Hauled level by a Tether (its own way), then let go with its
        // hop up a frame after the last shove: still not its own.
        let mut a = Anim::default();
        walk(&mut a, (6.0, 0.0), 0.0, 1.0);
        let mut p = a.last;
        for _ in 0..12 {
            p = [p[0] + 6.0 / 60.0, p[1] - 1.0 / 60.0, p[2]];
            a.step(p, (0.0, 0.0), (false, false, false), 1.0 / 60.0);
        }
        for k in 0..30 {
            let hauled = k < 20;
            if hauled {
                a.shove();
            }
            let v = if hauled {
                [0.0, 0.0, -laws::TETHER_SPEED]
            } else {
                [0.0, laws::TETHER_POP, -12.0]
            };
            p = [p[0] + v[0] / 60.0, p[1] + v[1] / 60.0, p[2] + v[2] / 60.0];
            a.step(p, (0.0, 0.0), (false, false, false), 1.0 / 60.0);
            assert!(a.flip == 0.0 && a.wall.is_none(), "{k}: {a:?}");
        }
    }

    #[test]
    fn a_foot_falls_each_stride() {
        // Running at 7 m/s for 4 s: a footfall each stride's length.
        let mut a = Anim::default();
        walk(&mut a, (7.0, 0.0), 0.0, 1.0);
        let (mut falls, mut p) = (0, a.last);
        for _ in 0..240 {
            p = [p[0] + 7.0 / 60.0, 0.0, p[2]];
            a.step(p, (0.0, 0.0), (true, false, false), 1.0 / 60.0);
            falls += a.footfall as i32;
        }
        let want = 28.0 / a.stride;
        assert!(
            (falls as f32 - want).abs() <= 1.5,
            "{falls} falls, {want} strides"
        );
        // Standing, or in the air: none.
        for ground in [true, false] {
            for _ in 0..60 {
                p = [p[0] + if ground { 0.0 } else { 0.1 }, 0.0, p[2]];
                a.step(p, (0.0, 0.0), (ground, false, false), 1.0 / 60.0);
                assert!(!a.footfall || ground && a.speed > 0.6);
            }
        }
    }
}
