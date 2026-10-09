//! A wizard's skeleton for one frame, solved from how it moves (`Anim`)
//! and what it does (`Pose`). Its feet are placed first: each foot, in
//! turn, bears the weight (planted, moving back as the body goes over it)
//! then swings through in an arc, its step as long as the gait says; two
//! bones reach each foot from its hip (the knee bending forward), the
//! hips turned the way it goes. Above them the chest keeps its aim, leaning
//! and banking; the arms swing against the legs, elbows bending more at a
//! run, the wand arm rising along its aim to cast; the robe's panels
//! follow the thighs, and its hat's tip lags behind.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use render::geo::{self, V3};
use render::M4;

use super::gait::Anim;
use super::math::{bone, chain, dir, ease, point, reach, rx, ry, rz, sc, tr};

/// Hip height standing; thigh and shin (to the ankle); the ankle above the
/// sole; the hips' half width; the shoulders' height above the waist and
/// half width; upper arm and forearm.
pub const HIP: f32 = 0.95;
pub const THIGH: f32 = 0.47;
pub const SHIN: f32 = 0.44;
pub const ANKLE: f32 = 0.09;
const HIP_W: f32 = 0.11;
const SHOULDER: (f32, f32) = (0.37, 0.24);
pub const UPPER: f32 = 0.29;
pub const FORE: f32 = 0.27;
/// The wand's tip, down the forearm from the elbow.
pub const TIP: f32 = 0.77;
/// The most ground a planted foot covers (metres): a longer stride spends
/// less of its cycle on the ground (a run leaves it, a sprint more so),
/// so a leg never has to reach further than it is long.
const CONTACT: f32 = 0.7;

/// What it is doing this frame, besides moving.
pub struct Pose {
    /// Just hit: 1 flashes it white and flinches it back.
    pub flash: f32,
    /// Its wand arm raised to cast (0 at rest, 1 along its aim, `aim`
    /// radians up), and its wand's tip alight (a colour; 1 just cast).
    pub arm: f32,
    pub aim: f32,
    pub tip: (V3, f32),
    /// The spell it has just cast and how fresh (1 just now): its arms
    /// make that spell's gesture, easing back.
    pub spell: Option<(u8, f32)>,
    /// On its broomstick, dropping onto the island.
    pub glide: bool,
    /// Seconds (for breath and the broom's sway).
    pub t: f32,
}

/// Every part's place this frame (left first, then right): the root
/// (feet, facing), the hips (turned the way it goes) and the robe's four
/// panels (front left, front right, back right, back left), the chest,
/// head and hat's tip, each leg's thigh, shin and boot, each arm's upper
/// arm and forearm.
pub struct Frames {
    pub root: M4,
    pub pelvis: M4,
    pub panels: [M4; 4],
    pub chest: M4,
    pub head: M4,
    pub hat: M4,
    pub thigh: [M4; 2],
    pub shin: [M4; 2],
    pub boot: [M4; 2],
    pub upper: [M4; 2],
    pub fore: [M4; 2],
}

impl Frames {
    /// The wand's tip, where spells leave it.
    pub fn tip(&self) -> V3 {
        point(&self.fore[1], [0.0, -TIP, 0.0])
    }
}

/// Where a foot is through the cycle `u` (0 to 1): forward of the hip,
/// up off the ground, and its toe's pitch. A cycle is two steps; while
/// the foot bears the weight (`duty` of it) the body goes on by that much
/// of the cycle's distance, and the foot goes back as far, so it stays
/// where it was put.
fn foot(u: f32, step: f32, duty: f32, lift: f32) -> (f32, f32, f32) {
    let reach = 2.0 * duty * step;
    if u < duty {
        let s = u / duty;
        let pitch = 0.18 * (1.0 - s / 0.2).max(0.0) - 0.55 * ease(0.7, 1.0, s);
        (reach * (0.5 - s), 0.0, pitch)
    } else {
        // Swinging through, in an arc.
        let s = (u - duty) / (1.0 - duty);
        let x = -0.5 + s * s * (3.0 - 2.0 * s);
        (reach * x, lift * (PI * s).sin(), -0.45 + 0.65 * s)
    }
}

/// The limbs' angles and body's place, before they become frames.
struct Body {
    /// The hips' height, sway to the side, turn (and their wobble).
    hip: (f32, f32),
    sway: f32,
    hips: f32,
    wobble: f32,
    /// Leaning forward, banking right, breathing (scale), the head's nod.
    lean: f32,
    bank: f32,
    breathe: f32,
    nod: f32,
    /// Each foot: where (in the hips' turn: forward, up, out) and its pitch.
    feet: [(V3, f32); 2],
    /// Each arm: forward swing, out, the elbow's bend.
    arms: [(f32, f32, f32); 2],
    /// The robe's drag, the hat's lag; knocked out: rolled back, sinking.
    cloth: f32,
    hat: f32,
    fall: f32,
    sink: f32,
}

fn standing(a: &Anim, p: &Pose, id: u16) -> Body {
    let run = a.run.x;
    let air = a.air;
    let moving = ease(0.25, 1.2, a.speed) * (1.0 - air);
    let crouch = a.crouch;
    let step = a.stride * moving;
    let duty = (0.6 - 0.24 * run).min(CONTACT / (2.0 * step).max(0.01));
    let lift = (0.11 + 0.15 * run) * (1.0 - 0.4 * crouch);
    // Each foot steps along the way it goes, off the hips.
    let (cs, cc) = a.course.sin_cos();
    let cyc = a.phase;
    let mut feet = [([0.0; 3], 0.0); 2];
    for (side, foot_at) in feet.iter_mut().enumerate() {
        let u = (cyc + 0.5 * side as f32).fract();
        let (along, y, pitch) = foot(u, step, duty, lift * moving);
        let (x, across) = (along * cc, along * cs);
        let out = (HIP_W + 0.02 + 0.06 * crouch - 0.025 * run) * if side == 0 { -1.0 } else { 1.0 };
        // In the air the legs tuck.
        let tuck = [0.1 + 0.08 * side as f32, 0.36 - 0.06 * side as f32];
        let at = [
            x + (tuck[0] - x) * air,
            y + (tuck[1] - y) * air,
            out + across * (1.0 - air),
        ];
        *foot_at = (at, pitch * moving * (1.0 - air));
    }
    // The hips ride over each step: highest over the planted foot at a
    // walk, lowest as it lands at a run; they sway over it at a walk.
    let w = 2.0 * TAU * (cyc - duty * 0.5);
    let bob = moving * (0.022 * (1.0 - run) * w.cos() - 0.035 * run * w.cos());
    let sway = -0.026 * moving * (1.0 - run) * (TAU * (cyc - duty * 0.5)).cos();
    // Lower going faster, so the legs reach (and it looks driven).
    let low = (0.34 * crouch + 0.22 * a.land).min(0.5)
        + moving * (0.04 + 0.05 * run + 0.03 * a.sprint.x) * (1.0 - crouch);
    let flinch = p.flash;
    // The arms swing against the legs, elbows bending more at a run.
    let swing = |side: usize| {
        let x = feet[side].0[0];
        let k = 0.3 + 0.35 * run + 0.25 * a.sprint.x;
        -(x / (duty * a.stride).max(0.2)) * k * moving
    };
    let sprint = a.sprint.x;
    let elbow = 0.18 + 1.05 * run * moving + 0.3 * sprint + 0.3 * crouch;
    // Out from the body, so the bell sleeves hang clear of the robe.
    let out = 0.3 + 0.05 * run + 0.5 * air + 0.2 * a.land;
    // The wand held forward, clear of the legs.
    let rest = (swing(1) + 0.15, out * 0.8, elbow.max(0.75));
    let up = FRAC_PI_2 + p.aim.clamp(-1.0, 1.0);
    let cast = (up + 0.25 * p.tip.1, 0.05, 0.12);
    let k = p.arm.clamp(0.0, 1.0);
    let right = (
        rest.0 + (cast.0 - rest.0) * k,
        rest.1 + (cast.1 - rest.1) * k,
        rest.2 + (cast.2 - rest.2) * k,
    );
    let mut b = Body {
        hip: (HIP - low + bob + 0.05 * air, a.land),
        sway,
        hips: a.hips.x,
        wobble: 0.09 * moving * (TAU * cyc).cos() * cc,
        lean: a.lean.x + 0.3 * crouch + 0.15 * a.land - 0.35 * flinch,
        bank: a.bank.x,
        breathe: 1.0 + 0.015 * (p.t * 2.4 + id as f32).sin() * (1.0 - moving),
        nod: 0.55 * p.aim.clamp(-0.9, 0.9) - 0.3 * flinch,
        feet,
        arms: [(swing(0) + 0.35 * crouch + 0.3 * air, out, elbow), right],
        cloth: a.cloth.x,
        hat: a.hat.x,
        fall: 0.0,
        sink: 0.0,
    };
    gesture(&mut b, p.spell, up);
    slid(&mut b, a.slide.x, k);
    b
}

/// Each spell's gesture, `k` of the way into it (fresh casts most): fire
/// thrust forward with both hands, lightning called down from overhead,
/// frost swept across, a ward spread wide, mending drawn to the chest,
/// a gust flung out, a blink crouched into; the Lance's aim is the cast
/// itself. `up` is the aim's angle for the wand arm.
fn gesture(b: &mut Body, spell: Option<(u8, f32)>, up: f32) {
    use wandfall::laws::spell::*;
    let Some((sp, fresh)) = spell else {
        return;
    };
    let k = fresh.clamp(0.0, 1.0);
    let k = k * k * (3.0 - 2.0 * k);
    let to = |a: &mut (f32, f32, f32), t: (f32, f32, f32), k: f32| {
        *a = (
            a.0 + (t.0 - a.0) * k,
            a.1 + (t.1 - a.1) * k,
            a.2 + (t.2 - a.2) * k,
        );
    };
    let [left, wand] = &mut b.arms;
    match sp {
        FIREBALL => {
            to(left, (up - 0.1, 0.3, 0.25), k);
            b.lean += 0.15 * k;
        }
        LIGHTNING => {
            // Overhead as it is called, coming down as it fades.
            to(wand, (2.9, 0.1, 0.05), k);
            to(left, (0.4, 1.1, 0.3), k);
        }
        FROST => {
            to(wand, (up, -0.35 + 0.7 * (1.0 - k), 0.15), k);
            to(left, (0.3, 0.9, 0.4), k);
        }
        WARD => {
            to(wand, (0.9, 1.3, 0.2), k);
            to(left, (0.9, 1.3, 0.2), k);
            b.lean -= 0.1 * k;
        }
        MEND => {
            to(wand, (1.0, -0.2, 1.9), k);
            to(left, (1.0, -0.2, 1.9), k);
            b.nod += 0.25 * k;
        }
        GUST => {
            to(wand, (0.5, 1.5, 0.1), k);
            to(left, (0.5, 1.5, 0.1), k);
        }
        BLINK => {
            to(wand, (-0.6, 0.4, 0.3), k);
            to(left, (-0.6, 0.4, 0.3), k);
            b.lean += 0.3 * k;
        }
        _ => {}
    }
}

/// Sliding (`s` of the way into it): low, the lead leg out ahead, the
/// other tucked under, leaning back, the free arm out for balance, the
/// robe flying; the wand arm still casts (`cast` of the way up).
fn slid(b: &mut Body, s: f32, cast: f32) {
    if s <= 0.0 {
        return;
    }
    let mix = |a: f32, z: f32| a + (z - a) * s;
    b.hip.0 = mix(b.hip.0, HIP - 0.5);
    b.sway *= 1.0 - s;
    b.wobble *= 1.0 - s;
    b.lean = mix(b.lean, -0.42);
    let lead = ([0.62, 0.0, 0.09], -0.1);
    let tuck = ([-0.12, 0.1, -0.06], -0.5);
    for (side, (to, pitch)) in [(0, tuck), (1, lead)] {
        let (at, p) = b.feet[side];
        b.feet[side] = (
            [mix(at[0], to[0]), mix(at[1], to[1]), mix(at[2], to[2])],
            mix(p, pitch),
        );
    }
    let (sw, out, el) = b.arms[0];
    b.arms[0] = (mix(sw, -0.5), mix(out, 1.0), mix(el, 0.35));
    let (sw, out, el) = b.arms[1];
    let k = s * (1.0 - cast);
    b.arms[1] = (
        sw + (0.9 - sw) * k,
        out + (0.25 - out) * k,
        el + (0.3 - el) * k,
    );
    b.cloth = mix(b.cloth, 0.9);
    b.hat = mix(b.hat, 0.5);
}

/// Sitting on its broom: legs forward, hands on the handle.
fn sitting(p: &Pose, id: u16) -> Body {
    let sway = (p.t * 1.3 + id as f32).sin();
    let foot = |z: f32| ([0.42, 0.28, z], 0.3);
    Body {
        hip: (HIP - 0.04 + 0.08 * sway, 0.0),
        sway: 0.0,
        hips: 0.0,
        wobble: 0.0,
        lean: 0.22,
        bank: 0.06 * (p.t * 0.9 + id as f32).cos(),
        breathe: 1.0,
        nod: 0.15,
        feet: [foot(-0.15), foot(0.15)],
        arms: [(1.05, 0.08, 0.55), (1.0 + 0.4 * p.arm, 0.04, 0.5)],
        cloth: 0.5,
        hat: 0.35,
        fall: 0.0,
        sink: 0.0,
    }
}

/// The frames for a body at `at`, facing `yaw`.
fn frames(at: V3, yaw: f32, b: &Body) -> Frames {
    let root = chain(&[tr([at[0], at[1] - b.sink, at[2]]), ry(yaw), rz(b.fall)]);
    let hips = ry(b.hips);
    let turn = ry(b.hips + b.wobble);
    let hip_y = b.hip.0;
    let pelvis = chain(&[tr([0.0, hip_y, b.sway]), turn, rz(-0.3 * b.lean)]);
    let mut legs = [(root, root, root); 2];
    // How far each thigh swings forward (radians), in the hips' turn; and
    // the legs' points the robe must hang clear of.
    let mut swings = [0.0f32; 2];
    let mut shins: Vec<(V3, f32)> = Vec::with_capacity(10);
    let (sh, ch) = b.hips.sin_cos();
    for (side, &(f, pitch)) in b.feet.iter().enumerate() {
        let s = if side == 0 { -1.0 } else { 1.0 };
        let hip = point(&pelvis, [0.0, 0.0, s * HIP_W]);
        let ankle = geo::add(point(&hips, f), [0.0, ANKLE, 0.0]);
        let pole = dir(&hips, [1.0, 0.0, 0.12 * s]);
        let (knee, ankle) = reach(hip, ankle, pole, (THIGH, SHIN));
        let d = geo::sub(knee, hip);
        swings[side] = (d[0] * ch + d[2] * sh).atan2(-d[1]);
        let toe = b.hips + 0.1 * s;
        let boot = chain(&[tr(ankle), ry(toe), rz(pitch)]);
        let mid = |a: V3, b: V3| geo::scale(geo::add(a, b), 0.5);
        shins.extend([
            (mid(hip, knee), 0.1),
            (knee, 0.09),
            (mid(knee, ankle), 0.085),
            (ankle, 0.085),
            (point(&boot, [0.22, -0.05, 0.0]), 0.06),
        ]);
        legs[side] = (
            m4mul(&root, &bone(hip, knee, pole)),
            m4mul(&root, &bone(knee, ankle, pole)),
            chain(&[root, tr(ankle), ry(toe), rz(pitch)]),
        );
    }
    let body = chain(&[root, tr([0.0, hip_y, b.sway])]);
    let pelvis_w = m4mul(&root, &pelvis);
    // The robe's panels: a front one swings out with its thigh going
    // forward, a back one with it going back; all drag behind at speed.
    let [sl, sr] = swings;
    let panel = |c: f32, tilt: f32| {
        let tilt = tilt.max(clear(c, &shins, &unpelvis(b, hip_y)));
        chain(&[pelvis_w, ry(c), rz(tilt), ry(-c)])
    };
    let drag = b.cloth;
    let panels = [
        panel(-PI / 4.0, sl.max(0.0) * 0.85 + 0.04 - 0.1 * drag),
        panel(PI / 4.0, sr.max(0.0) * 0.85 + 0.04 - 0.1 * drag),
        panel(3.0 * PI / 4.0, (-sr).max(0.0) * 0.7 + 0.04 + 0.35 * drag),
        panel(-3.0 * PI / 4.0, (-sl).max(0.0) * 0.7 + 0.04 + 0.35 * drag),
    ];
    let chest = chain(&[
        body,
        ry(-0.5 * b.wobble),
        rz(-b.lean),
        rx(b.bank),
        sc([1.0, b.breathe, 1.0]),
    ]);
    let head = chain(&[chest, tr([0.0, 0.46, 0.0]), rz(b.nod), rx(-0.6 * b.bank)]);
    let hat = chain(&[head, tr([0.0, 0.73, 0.0]), rz(-b.hat), rx(0.3 * b.bank)]);
    let arm = |side: usize| {
        let s = if side == 0 { -1.0 } else { 1.0 };
        let (swing, out, elbow) = b.arms[side];
        let upper = chain(&[
            chest,
            tr([0.0, SHOULDER.0, s * SHOULDER.1]),
            rx(-s * out),
            rz(swing),
        ]);
        let fore = chain(&[upper, tr([0.0, -UPPER, 0.0]), rz(elbow)]);
        (upper, fore)
    };
    let (l, r) = (arm(0), arm(1));
    Frames {
        root,
        pelvis: pelvis_w,
        panels,
        chest,
        head,
        hat,
        thigh: [legs[0].0, legs[1].0],
        shin: [legs[0].1, legs[1].1],
        boot: [legs[0].2, legs[1].2],
        upper: [l.0, r.0],
        fore: [l.1, r.1],
    }
}

/// From the root's frame into the pelvis's (as `frames` places it).
fn unpelvis(b: &Body, hip_y: f32) -> M4 {
    chain(&[
        rz(0.3 * b.lean),
        ry(-(b.hips + b.wobble)),
        tr([0.0, -hip_y, -b.sway]),
    ])
}

/// How far the robe's panel about angle `c` must swing out (radians) to
/// hang clear of the legs' points (each with how thick the leg is
/// there), brought into the pelvis's frame by `into`: none if they are
/// all inside it, or not before it.
fn clear(c: f32, legs: &[(V3, f32)], into: &M4) -> f32 {
    let span = PI / 4.0 + 0.15;
    let mut most: f32 = 0.0;
    for &(at, thick) in legs {
        let q = point(into, at);
        let h = -q[1];
        if h < 0.06 {
            continue;
        }
        let a = q[2].atan2(q[0]);
        if super::math::wrap(a - c).abs() > span {
            continue;
        }
        // The robe at that depth, swung out by t, reaches
        // r cos t + h sin t from the middle; it must pass the leg.
        let need = (q[0] * q[0] + q[2] * q[2]).sqrt() + thick + 0.015;
        let r = super::parts::robe_r((h / super::parts::HEM).min(1.0)) * 0.97;
        if need <= r {
            continue;
        }
        let l = (r * r + h * h).sqrt();
        let t = (need / l).min(1.0).asin() - r.atan2(h);
        most = most.max(t.min(1.2));
    }
    most
}

fn m4mul(a: &M4, b: &M4) -> M4 {
    render::m4::mul(a, b)
}

/// A wizard moving as `a` does, doing `p`; getting on or off its broom,
/// eased from sitting to standing.
pub fn wizard(at: V3, yaw: f32, a: &Anim, p: &Pose, id: u16) -> Frames {
    let k = a.seat.x.clamp(0.0, 1.0);
    let b = if k > 0.99 {
        sitting(p, id)
    } else if k < 0.01 {
        standing(a, p, id)
    } else {
        blend(&standing(a, p, id), &sitting(p, id), k)
    };
    frames(at, yaw, &b)
}

/// `a` turned `k` of the way into `b`.
fn blend(a: &Body, b: &Body, k: f32) -> Body {
    let m = |x: f32, y: f32| x + (y - x) * k;
    let v = |x: V3, y: V3| [m(x[0], y[0]), m(x[1], y[1]), m(x[2], y[2])];
    let foot = |s: usize| (v(a.feet[s].0, b.feet[s].0), m(a.feet[s].1, b.feet[s].1));
    let arm = |s: usize| {
        let (x, y) = (a.arms[s], b.arms[s]);
        (m(x.0, y.0), m(x.1, y.1), m(x.2, y.2))
    };
    Body {
        hip: (m(a.hip.0, b.hip.0), m(a.hip.1, b.hip.1)),
        sway: m(a.sway, b.sway),
        hips: m(a.hips, b.hips),
        wobble: m(a.wobble, b.wobble),
        lean: m(a.lean, b.lean),
        bank: m(a.bank, b.bank),
        breathe: m(a.breathe, b.breathe),
        nod: m(a.nod, b.nod),
        feet: [foot(0), foot(1)],
        arms: [arm(0), arm(1)],
        cloth: m(a.cloth, b.cloth),
        hat: m(a.hat, b.hat),
        fall: m(a.fall, b.fall),
        sink: m(a.sink, b.sink),
    }
}

/// Knocked out `t` seconds ago: it falls on its back and sinks away.
pub fn fallen(at: V3, yaw: f32, p: &Pose, id: u16, t: f32) -> Frames {
    let k = (t / 0.45).min(1.0);
    let mut b = standing(&Anim::default(), p, id);
    b.fall = k * k * 1.45;
    b.feet = [
        ([0.25 * k, 0.15 * k, -0.15], 0.0),
        ([0.4 * k, 0.08 * k, 0.16], 0.0),
    ];
    b.arms = [(0.6 * k, 0.9 * k, 0.4), (0.8 * k, 0.9 * k, 0.3)];
    b.nod = 0.4 * k;
    b.sink = (t - 1.0).max(0.0) * 1.6;
    frames(at, yaw, &b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose() -> Pose {
        Pose {
            spell: None,
            flash: 0.0,
            arm: 0.0,
            aim: 0.0,
            tip: ([1.0; 3], 0.0),
            glide: false,
            t: 0.0,
        }
    }

    fn sole(f: &Frames, side: usize) -> V3 {
        point(&f.boot[side], [0.0, -ANKLE, 0.0])
    }

    fn run(a: &mut Anim, from: V3, v: (f32, f32), yaw: f32, frames: usize) -> V3 {
        let mut p = from;
        for _ in 0..frames {
            p = [p[0] + v.0 / 60.0, 0.0, p[2] + v.1 / 60.0];
            a.step(p, (yaw, 0.0), (true, false, false), 1.0 / 60.0);
        }
        p
    }

    #[test]
    fn it_stands_on_its_feet_and_falls_on_its_back() {
        let f = wizard([0.0; 3], 0.0, &Anim::default(), &pose(), 1);
        let head = point(&f.head, [0.0, 0.14, 0.0]);
        assert!(head[1] > 1.4, "the head is up: {head:?}");
        for side in 0..2 {
            assert!(
                sole(&f, side)[1].abs() < 0.02,
                "on the ground: {:?}",
                sole(&f, side)
            );
        }
        let f = fallen([0.0; 3], 0.0, &pose(), 1, 1.0);
        let head = point(&f.head, [0.0, 0.14, 0.0]);
        assert!(head[1] < 0.5 && head[0] < -1.0, "fallen backward: {head:?}");
    }

    #[test]
    fn a_cast_points_the_wand_along_its_aim() {
        let mut p = pose();
        p.arm = 1.0;
        let f = wizard([0.0; 3], 0.0, &Anim::default(), &p, 1);
        let shoulder = point(&f.upper[1], [0.0; 3]);
        assert!(f.tip()[0] - shoulder[0] > 0.8, "ahead: {:?}", f.tip());
    }

    #[test]
    fn a_planted_foot_does_not_slide() {
        // Facing east and running every way (ahead, strafing, backing
        // away, on the diagonals): while a foot bears the weight it stays
        // put on the ground, though the body moves on over it.
        for (vx, vz) in [
            (6.5, 0.0),
            (0.0, 6.0),
            (0.0, -6.0),
            (-4.5, 0.0),
            (4.5, 4.5),
            (-3.5, 3.5),
        ] {
            let mut a = Anim::default();
            let mut at = run(&mut a, [0.0; 3], (vx, vz), 0.0, 90);
            let mut planted: Option<(V3, usize)> = None;
            let mut worst: f32 = 0.0;
            for _ in 0..60 {
                at = run(&mut a, at, (vx, vz), 0.0, 1);
                let f = wizard(at, 0.0, &a, &pose(), 1);
                let left = sole(&f, 0);
                if left[1] < 0.01 {
                    match planted {
                        Some((p, n)) => {
                            let d = ((left[0] - p[0]).powi(2) + (left[2] - p[2]).powi(2)).sqrt();
                            worst = worst.max(d);
                            planted = Some((p, n + 1));
                        }
                        None => planted = Some((left, 0)),
                    }
                } else if planted.is_some_and(|p| p.1 > 3) {
                    break;
                } else {
                    planted = None;
                }
            }
            let v = (vx, vz);
            assert!(planted.is_some_and(|p| p.1 > 3), "{v:?}: it plants a foot");
            assert!(worst < 0.12, "{v:?}: and it stays: slid {worst}");
        }
    }

    #[test]
    fn crouched_or_landing_its_feet_stay_on_the_ground() {
        let mut a = Anim::default();
        a.crouch = 1.0;
        let f = wizard([0.0; 3], 0.0, &a, &pose(), 1);
        let head = point(&f.head, [0.0, 0.14, 0.0]);
        assert!(head[1] < 1.3, "crouched low: {head:?}");
        for side in 0..2 {
            assert!(sole(&f, side)[1].abs() < 0.03, "feet down");
        }
    }

    #[test]
    fn running_at_any_pace_nothing_jumps_between_frames() {
        // At 144 frames a second, from a walk to past a sprint: no joint
        // moves (against the body) much faster than the body does, and the
        // hips rise and fall in a wave, never turning sharply; a knee that
        // snaps straight, or hips that drop when a foot is far, flicker.
        for v in [2.0f32, 5.0, 7.0, 10.0, 14.0] {
            let fps = 144.0;
            let mut a = Anim::default();
            let mut p = [0.0f32; 3];
            let mut was: Option<(Vec<V3>, f32, f32)> = None;
            for k in 0..(fps as usize * 3) {
                p[0] += v / fps;
                a.step(p, (0.0, 0.0), (true, false, false), 1.0 / fps);
                let f = wizard(p, 0.0, &a, &pose(), 1);
                let joints: Vec<V3> = (0..2)
                    .flat_map(|s| {
                        let hand = point(&f.fore[s], [0.0, -FORE, 0.0]);
                        [sole(&f, s), point(&f.shin[s], [0.0; 3]), hand]
                    })
                    .map(|j| geo::sub(j, p))
                    .collect();
                let hip = point(&f.pelvis, [0.0; 3])[1];
                let mut rise = 0.0;
                if let Some((old, oh, or)) = &was {
                    rise = (hip - oh) * fps;
                    if k > fps as usize {
                        for (j, o) in joints.iter().zip(old) {
                            let d = geo::dot(geo::sub(*j, *o), geo::sub(*j, *o)).sqrt() * fps;
                            assert!(d < 1.25 * v + 3.0, "{v} m/s: a joint at {d} m/s");
                        }
                        let turn = (rise - or).abs() * fps;
                        // A wave quickens with the pace; the old flicker
                        // turned at some 1,800.
                        let most = 60.0 + 0.5 * v * v;
                        assert!(turn < most, "{v} m/s: the hips turned at {turn} m/s/s");
                    }
                }
                was = Some((joints, hip, rise));
            }
        }
    }

    #[test]
    fn an_aim_that_jumps_turns_it_smoothly() {
        // What the crosshair is on can jump (near ground, the sky): the
        // body turns after it, never in a frame.
        let mut a = Anim::default();
        let fps = 144.0;
        let mut last = 0.0;
        for k in 0..300 {
            let aim = if (k / 20) % 2 == 0 { 0.0 } else { 0.3 };
            a.step([0.0; 3], (aim, 0.0), (true, false, false), 1.0 / fps);
            let turn = (a.face.x - last).abs();
            assert!(k == 0 || turn < 0.06, "turned {turn} in a frame");
            last = a.face.x;
        }
    }

    #[test]
    fn sliding_it_sits_low_with_a_leg_out_ahead() {
        let mut a = Anim::default();
        let mut p = [0.0; 3];
        for _ in 0..30 {
            p = [p[0] + 11.0 / 60.0, 0.0, 0.0];
            a.step(p, (0.0, 0.0), (true, true, true), 1.0 / 60.0);
        }
        let f = wizard(p, 0.0, &a, &pose(), 1);
        let hip = point(&f.root, [0.0, HIP, 0.0]);
        let pelvis = point(&f.thigh[1], [0.0; 3]);
        assert!(pelvis[1] < 0.6, "low: {pelvis:?} ({hip:?})");
        let lead = sole(&f, 1);
        assert!(lead[0] - p[0] > 0.4, "the lead foot ahead: {lead:?}");
        assert!(lead[1].abs() < 0.12, "and on the ground: {lead:?}");
    }

    #[test]
    fn strafing_its_legs_go_its_way_and_its_chest_keeps_its_aim() {
        // Facing +x, running right (+z).
        let mut a = Anim::default();
        let at = run(&mut a, [0.0; 3], (0.0, 6.0), 0.0, 60);
        let f = wizard(at, 0.0, &a, &pose(), 1);
        let hips_fwd = dir(&f.pelvis, [1.0, 0.0, 0.0]);
        let chest_fwd = dir(&f.chest, [1.0, 0.0, 0.0]);
        assert!(
            hips_fwd[2] > 0.4 && hips_fwd[2] < 0.7,
            "hips turned partly right: {hips_fwd:?}"
        );
        assert!(chest_fwd[0] > 0.9, "chest ahead: {chest_fwd:?}");
    }
}
