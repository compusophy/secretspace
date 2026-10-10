//! A wizard's skeleton for one frame, solved from how it moves (`Anim`)
//! and what it does (`Pose`). Its feet are placed first: each foot, in
//! turn, bears the weight (planted, moving back as the body goes over it)
//! then swings through in an arc, its step as long as the gait says; in
//! the air they leap, tuck and reach down for the ground; two bones reach
//! each foot from its hip (the knee bending forward), the hips turned the
//! way it goes. Above them the chest keeps its aim, leaning and banking;
//! the arms swing against the legs, elbows bending more at a run, the
//! wand arm rising along its aim to cast (however the chest leans); the
//! robe's panels follow the thighs, and its hat's tip lags behind.

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
/// How much of a cycle a foot bears the weight at a walk (and stepping
/// where it stands).
pub const REST_DUTY: f32 = 0.6;

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
    /// The hips' height, sway to the side, turn (and their wobble); each
    /// foot's heading off the hips' (standing, as it turns where it is).
    hip: f32,
    sway: f32,
    hips: f32,
    wobble: f32,
    turn: [f32; 2],
    /// Leaning forward, banking right, breathing (scale), the head's nod.
    lean: f32,
    bank: f32,
    breathe: f32,
    nod: f32,
    /// Each foot: where (in the hips' turn: forward, up, out) and its pitch.
    feet: [(V3, f32); 2],
    /// Each arm: forward swing, out, the elbow's bend.
    arms: [(f32, f32, f32); 2],
    /// The robe's drag, the hat's lag; the whole of it tipped back (as it
    /// falls, or noses up on its broom), rolled to its right, and sunk.
    cloth: f32,
    hat: f32,
    fall: f32,
    roll: f32,
    sink: f32,
}

fn standing(a: &Anim, p: &Pose, id: u16) -> Body {
    let run = a.run.x;
    let air = a.air;
    let moving = ease(0.25, 1.2, a.speed) * (1.0 - air);
    let crouch = a.crouch;
    let step = a.stride * moving;
    let duty = (REST_DUTY - 0.24 * run).min(CONTACT / (2.0 * step).max(0.01));
    // Two quick steps lift the feet as it turns where it stands.
    let shuffle = (a.shuffle > 0.0) as i32 as f32;
    let lift = (0.11 + 0.15 * run) * (1.0 - 0.4 * crouch) * moving + 0.07 * shuffle;
    // Its feet stay where they stood; the hips turn between them.
    let held = (a.held[0] + a.held[1]) / 2.0;
    let hips = a.hips.x + held;
    // Each foot steps along the way it goes, off the hips.
    let (cs, cc) = a.course.sin_cos();
    let cyc = a.phase;
    let outs: [f32; 2] = std::array::from_fn(|side| {
        (HIP_W + 0.02 + 0.06 * crouch - 0.025 * run) * if side == 0 { -1.0 } else { 1.0 }
    });
    let flying = aloft(a, outs, hips);
    let mut feet = [([0.0; 3], 0.0); 2];
    for (side, foot_at) in feet.iter_mut().enumerate() {
        let u = (cyc + 0.5 * side as f32).fract();
        let (along, y, pitch) = foot(u, step, duty, lift);
        let on = [along * cc, y, outs[side] + along * cs];
        let at = geo::add(on, geo::scale(geo::sub(flying[side], on), air));
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
    // A jump made in the air flings the arms out and tips it forward.
    let flip = (a.flip * 2.0).min(1.0) * air;
    // Out from the body, so the bell sleeves hang clear of the robe.
    let out = 0.3 + 0.05 * run + 0.5 * air + 0.2 * a.land + 0.5 * flip;
    // The wand held forward, clear of the legs.
    let rest = (swing(1) + 0.15, out * 0.8, elbow.max(0.75));
    let up = FRAC_PI_2 + p.aim.clamp(-1.0, 1.0);
    // Along the aim: the upper arm short of it by the elbow's bend.
    let cast = (up - 0.12 + 0.25 * p.tip.1, 0.05, 0.12);
    let k = p.arm.clamp(0.0, 1.0);
    let right = (
        rest.0 + (cast.0 - rest.0) * k,
        rest.1 + (cast.1 - rest.1) * k,
        rest.2 + (cast.2 - rest.2) * k,
    );
    let mut b = Body {
        hip: HIP - low + bob + 0.05 * air,
        sway,
        hips,
        wobble: 0.09 * moving * (TAU * cyc).cos() * cc,
        turn: [a.held[0] - held, a.held[1] - held],
        lean: a.lean.x + 0.3 * crouch + 0.15 * a.land - 0.35 * flinch + 0.3 * flip,
        bank: a.bank.x,
        breathe: 1.0 + 0.015 * (p.t * 2.4 + id as f32).sin() * (1.0 - moving),
        nod: 0.55 * p.aim.clamp(-0.9, 0.9) - 0.3 * flinch,
        feet,
        arms: [(swing(0) + 0.35 * crouch + 0.3 * air, out, elbow), right],
        cloth: a.cloth.x,
        hat: a.hat.x,
        fall: 0.0,
        roll: 0.0,
        sink: 0.0,
    };
    gesture(&mut b, p.spell, up);
    slid(&mut b, a.slide.x, k);
    // However the chest leans, the wand keeps to the aim and the head
    // mostly level.
    b.arms[1].0 += k * b.lean;
    b.nod += 0.7 * b.lean;
    b
}

/// Where each foot goes in the air (forward, up, out, in the hips' turn
/// `hips`; `out` each foot's way out from them): tucked at the top of a
/// jump; rising, the leading knee up and the other leg trailing; falling,
/// both reaching down for the ground; a jump made in the air tucks both
/// knees up, and a kick off a wall stretches the leg on its side back
/// toward it.
fn aloft(a: &Anim, out: [f32; 2], hips: f32) -> [V3; 2] {
    let rise = ease(0.5, 4.0, a.vy.x);
    let fall = ease(-1.0, -5.0, a.vy.x);
    let flip = (a.flip * 2.0).min(1.0);
    // The foot that was swinging forward leads.
    let lead = (a.phase < 0.5) as usize;
    let mix =
        |a: [f32; 2], b: [f32; 2], k: f32| [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k];
    std::array::from_fn(|side| {
        let tuck = [0.1 + 0.08 * side as f32, 0.36 - 0.06 * side as f32];
        let (leap, reach) = if side == lead {
            ([0.3, 0.44], [0.16, 0.05])
        } else {
            ([-0.3, 0.2], [-0.06, 0.1])
        };
        let knees_up = [0.22, 0.55 - 0.05 * side as f32];
        let f = mix(mix(mix(tuck, leap, rise), reach, fall), knees_up, flip);
        let mut at = [f[0], f[1], out[side]];
        if let Some(w) = a.wall {
            let (s, c) = (w - hips).sin_cos();
            if (s < 0.0) == (side == 0) {
                let kick = [c * 0.55, 0.3, s * 0.55];
                at = geo::add(at, geo::scale(geo::sub(kick, at), flip));
            }
        }
        at
    })
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
    b.hip = mix(b.hip, HIP - 0.5);
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

/// Sitting on its broom: legs forward, hands on the handle, bobbing
/// (broom and all), banking into a turn and dipping its nose as it drops.
fn sitting(a: &Anim, p: &Pose, id: u16) -> Body {
    let sway = (p.t * 1.3 + id as f32).sin();
    let foot = |z: f32| ([0.42, 0.28, z], 0.3);
    Body {
        hip: HIP - 0.04,
        sway: 0.0,
        hips: 0.0,
        wobble: 0.0,
        turn: [0.0; 2],
        lean: 0.22,
        bank: 0.0,
        breathe: 1.0,
        nod: 0.15,
        feet: [foot(-0.15), foot(0.15)],
        arms: [(1.05, 0.08, 0.55), (1.0 + 0.4 * p.arm, 0.04, 0.5)],
        cloth: 0.5,
        hat: 0.35,
        fall: (0.025 * a.vy.x).clamp(-0.3, 0.2),
        roll: (1.6 * a.bank.x).clamp(-0.45, 0.45) + 0.03 * (p.t * 0.9 + id as f32).cos(),
        sink: -0.08 * sway,
    }
}

/// The frames for a body at `at`, facing `yaw`.
fn frames(at: V3, yaw: f32, b: &Body) -> Frames {
    let root = chain(&[
        tr([at[0], at[1] - b.sink, at[2]]),
        ry(yaw),
        rz(b.fall),
        rx(b.roll),
    ]);
    let turn = ry(b.hips + b.wobble);
    let hip_y = b.hip;
    let pelvis = chain(&[tr([0.0, hip_y, b.sway]), turn, rz(-0.3 * b.lean)]);
    let mut legs = [(root, root, root); 2];
    // How far each thigh swings forward (radians), in the hips' turn; and
    // the legs' points the robe must hang clear of.
    let mut swings = [0.0f32; 2];
    let mut shins: Vec<(V3, f32)> = Vec::with_capacity(10);
    let (sh, ch) = b.hips.sin_cos();
    for (side, &(f, pitch)) in b.feet.iter().enumerate() {
        let s = if side == 0 { -1.0 } else { 1.0 };
        let heading = b.hips + b.turn[side];
        let hips = ry(heading);
        let hip = point(&pelvis, [0.0, 0.0, s * HIP_W]);
        let ankle = geo::add(point(&hips, f), [0.0, ANKLE, 0.0]);
        let pole = dir(&hips, [1.0, 0.0, 0.12 * s]);
        let (knee, ankle) = reach(hip, ankle, pole, (THIGH, SHIN));
        let d = geo::sub(knee, hip);
        swings[side] = (d[0] * ch + d[2] * sh).atan2(-d[1]);
        let toe = heading + 0.1 * s;
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
    let into = unpelvis(b, hip_y);
    let panel = |c: f32, tilt: f32| {
        let tilt = tilt.max(clear(c, &shins, &into));
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
        sitting(a, p, id)
    } else if k < 0.01 {
        standing(a, p, id)
    } else {
        blend(&standing(a, p, id), &sitting(a, p, id), k)
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
        hip: m(a.hip, b.hip),
        sway: m(a.sway, b.sway),
        hips: m(a.hips, b.hips),
        wobble: m(a.wobble, b.wobble),
        turn: [m(a.turn[0], b.turn[0]), m(a.turn[1], b.turn[1])],
        lean: m(a.lean, b.lean),
        bank: m(a.bank, b.bank),
        breathe: m(a.breathe, b.breathe),
        nod: m(a.nod, b.nod),
        feet: [foot(0), foot(1)],
        arms: [arm(0), arm(1)],
        cloth: m(a.cloth, b.cloth),
        hat: m(a.hat, b.hat),
        fall: m(a.fall, b.fall),
        roll: m(a.roll, b.roll),
        sink: m(a.sink, b.sink),
    }
}

/// Knocked out `t` seconds ago: its knees give (its feet staying put),
/// then it topples back, turning `twist` radians as it goes, bounces once
/// on the ground, and sinks away.
pub fn fallen(at: V3, yaw: f32, p: &Pose, id: u16, t: f32, twist: f32) -> Frames {
    let give = ease(0.0, 0.16, t);
    let k = ((t - 0.1) / 0.42).clamp(0.0, 1.0);
    let over = k * k;
    let down = (t - 0.52).max(0.0);
    let bounce = 0.12 * (-down * 9.0).exp() * (down * 18.0).sin().max(0.0);
    let mut b = standing(&Anim::default(), p, id);
    b.hip = HIP - 0.38 * give;
    b.fall = over * 1.45 - bounce;
    b.feet = [
        ([0.08 * give + 0.17 * over, 0.15 * over, -0.15], 0.0),
        ([0.12 * give + 0.28 * over, 0.08 * over, 0.16], 0.0),
    ];
    b.arms = [
        (0.3 * give + 0.3 * over, 0.4 * give + 0.5 * over, 0.4),
        (0.4 * give + 0.4 * over, 0.4 * give + 0.5 * over, 0.3),
    ];
    b.lean = 0.25 * give * (1.0 - over);
    b.nod = 0.4 * over;
    b.sink = (t - 1.0).max(0.0) * 1.6;
    frames(at, yaw + twist * ease(0.0, 1.0, k), &b)
}

#[cfg(test)]
mod tests;
