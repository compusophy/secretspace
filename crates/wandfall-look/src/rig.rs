//! A wizard, jointed and alive. Each part is its own mesh turning about
//! its joint (hips, knees, waist, neck, shoulders), posed every frame:
//!
//! - walking, its legs stride in the way it moves (forward, back, to the
//!   side), knees bending as each leg comes through, its boots under a
//!   robe that swings and trails, its arms swinging against its legs, the
//!   body bobbing and leaning into the run;
//! - standing, it breathes; in the air, it tucks its legs and opens its
//!   arms; hit, it flinches back; its head nods with its aim;
//! - casting, its wand arm rises along its aim and thrusts;
//! - dropping, it sits on a broomstick trailing sparks;
//! - knocked out, it falls on its back and sinks away.
//!
//! The model faces +x, up is +y, its right is +z (as `m4::place` turns
//! it).

use render::geo::{self, hash, mix, rgb, unit, Geo, V3};
use render::{m4, Item, Light, Mesh, Pass, Renderer, Spark, M4};

use crate::fx::Draw;

use std::f32::consts::{FRAC_PI_2, PI};

/// A wizard's colour from its id.
pub fn hue(id: u16) -> V3 {
    const HUES: [V3; 8] = [
        rgb(70, 110, 230),
        rgb(200, 60, 70),
        rgb(60, 160, 110),
        rgb(150, 80, 200),
        rgb(220, 140, 40),
        rgb(40, 170, 190),
        rgb(220, 90, 160),
        rgb(120, 120, 130),
    ];
    HUES[id as usize % HUES.len()]
}

const SKIN: V3 = rgb(214, 160, 120);
const GOLD: V3 = rgb(214, 170, 90);
const WHITE: V3 = rgb(248, 248, 252);
/// Hip, knee and waist heights; the robe's hem is between knee and foot.
const HIP: f32 = 0.95;
const THIGH: f32 = 0.47;

fn smooth(r: &mut Renderer, f: impl Fn(&mut Geo)) -> Mesh {
    let mut g = Geo::default();
    f(&mut g);
    g.smooth();
    r.mesh(&g)
}

fn flat(r: &mut Renderer, f: impl Fn(&mut Geo)) -> Mesh {
    let mut g = Geo::default();
    f(&mut g);
    r.mesh(&g)
}

// Turns and moves, as matrices, to chain from the feet up.
fn tr(v: V3) -> M4 {
    m4::basis(v, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0])
}

/// About the side axis (+z): positive lifts +x toward +y.
fn rz(a: f32) -> M4 {
    let (s, c) = a.sin_cos();
    m4::basis([0.0; 3], [c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0])
}

/// About the forward axis (+x): positive tips +y toward +z.
fn rx(a: f32) -> M4 {
    let (s, c) = a.sin_cos();
    m4::basis([0.0; 3], [1.0, 0.0, 0.0], [0.0, c, s], [0.0, -s, c])
}

/// About the upright, as headings turn.
fn ry(a: f32) -> M4 {
    let (s, c) = a.sin_cos();
    m4::basis([0.0; 3], [c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c])
}

fn sc(v: V3) -> M4 {
    m4::basis(
        [0.0; 3],
        [v[0], 0.0, 0.0],
        [0.0, v[1], 0.0],
        [0.0, 0.0, v[2]],
    )
}

fn chain(list: &[M4]) -> M4 {
    list.iter().skip(1).fold(list[0], |acc, m| m4::mul(&acc, m))
}

fn point(m: &M4, p: V3) -> V3 {
    [
        m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
        m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
        m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
    ]
}

/// How a wizard moves, as the page has watched it: smoothed so the pose
/// does not jerk with each frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Anim {
    /// The stride's phase (radians; a full turn is two steps).
    pub phase: f32,
    /// Speed forward and to the right of where it faces (m/s).
    pub fwd: f32,
    pub side: f32,
    /// 0 on the ground, 1 in the air.
    pub air: f32,
    /// Crouched (0 to 1, eased), and the squat of a landing (1 a hard
    /// one, easing away).
    pub crouch: f32,
    pub land: f32,
    /// Seconds in the air so far.
    aloft: f32,
    last: V3,
    seen: bool,
}

impl Anim {
    /// Watch it at `p`, facing `yaw`, crouched or not, `dt` seconds on.
    /// A landing says how hard it was (0 to 1).
    pub fn step(
        &mut self,
        p: V3,
        yaw: f32,
        (ground, crouch): (bool, bool),
        dt: f32,
    ) -> Option<f32> {
        if !self.seen {
            self.last = p;
            self.seen = true;
        }
        let dt = dt.max(1e-3);
        let (mut vx, mut vz) = ((p[0] - self.last[0]) / dt, (p[2] - self.last[2]) / dt);
        // A blink is not a stride.
        if vx * vx + vz * vz > 30.0 * 30.0 {
            (vx, vz) = (0.0, 0.0);
        }
        let (s, c) = yaw.sin_cos();
        let (f, r) = (vx * c + vz * s, -vx * s + vz * c);
        let k = 1.0 - (-dt * 10.0).exp();
        self.fwd += (f - self.fwd) * k;
        self.side += (r - self.side) * k;
        if ground {
            let speed = (self.fwd * self.fwd + self.side * self.side).sqrt();
            // About 1.3 m a step.
            self.phase = (self.phase + speed * dt * PI / 1.3) % (2.0 * PI * 64.0);
        }
        let up = if ground { 0.0 } else { 1.0 };
        self.air += (up - self.air) * (1.0 - (-dt * 8.0).exp());
        let low = if crouch { 1.0 } else { 0.0 };
        self.crouch += (low - self.crouch) * (1.0 - (-dt * 12.0).exp());
        self.land *= (-dt * 9.0).exp();
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

/// How far the hips must drop for the feet to stay on the ground with the
/// thighs forward by `thigh` and the knees bent by `knee`.
fn drop(thigh: f32, knee: f32) -> f32 {
    HIP - THIGH * thigh.cos() - 0.48 * (thigh - knee).cos()
}

/// What it is doing this frame, besides moving.
pub struct Pose {
    /// Just hit: 1 flashes it white and flinches it back.
    pub flash: f32,
    /// Its wand arm raised to cast (0 at rest, 1 along its aim, `aim`
    /// radians up), and its wand's tip alight (a colour; 1 just cast).
    pub arm: f32,
    pub aim: f32,
    pub tip: (V3, f32),
    /// On its broomstick, dropping onto the island.
    pub glide: bool,
    /// Seconds (for breath and the broom's sway).
    pub t: f32,
}

/// The joints' angles for one frame.
struct Joints {
    bob: f32,
    lean: f32,
    roll: f32,
    breathe: f32,
    /// Hips and knees (left, right), and the way the legs stride.
    thigh: [f32; 2],
    knee: [f32; 2],
    stride: f32,
    skirt: (f32, f32),
    /// Shoulders (forward, out) and the head's nod.
    arm: [(f32, f32); 2],
    nod: f32,
    /// Rolled over onto its back (knocked out).
    fall: f32,
    sink: f32,
}

/// Where each part is this frame: the feet, the hips and waist, the
/// robe below the waist, the body above it, the head, each thigh and
/// shin, each arm (left, right).
struct Frames {
    root: M4,
    skirt: M4,
    torso: M4,
    head: M4,
    hips: [M4; 2],
    knees: [M4; 2],
    arms: [M4; 2],
}

fn frames(at: V3, yaw: f32, j: &Joints) -> Frames {
    let root = chain(&[
        tr([at[0], at[1] + j.bob - j.sink, at[2]]),
        ry(yaw),
        rz(j.fall),
    ]);
    let body = chain(&[root, tr([0.0, HIP, 0.0]), rz(-j.lean), rx(j.roll)]);
    // The robe below the waist trails and sways; above it, the body.
    let skirt = chain(&[body, rz(j.skirt.0), rx(j.skirt.1)]);
    let torso = chain(&[body, sc([1.0, j.breathe, 1.0])]);
    let head = chain(&[torso, tr([0.0, 0.43, 0.0]), rz(j.nod)]);
    // Legs from the hips, striding in the way it moves.
    let leg = |side: usize, z: f32| {
        let hip = chain(&[
            root,
            tr([0.0, HIP, z]),
            rx(j.roll * 0.5),
            ry(j.stride),
            rz(j.thigh[side]),
            ry(-j.stride),
        ]);
        let knee = chain(&[
            hip,
            tr([0.0, -THIGH, 0.0]),
            ry(j.stride),
            rz(-j.knee[side]),
            ry(-j.stride),
        ]);
        (hip, knee)
    };
    let (l, r) = (leg(0, -0.12), leg(1, 0.12));
    // Arms from the shoulders: the left swings, the right holds the wand.
    // Out is away from the body: -z for the left, +z for the right.
    let arm = |side: usize, z: f32| {
        let (fwd, out) = j.arm[side];
        let spread = if side == 0 { out } else { -out };
        chain(&[torso, tr([0.02, 0.36, z]), rx(spread), rz(fwd)])
    };
    Frames {
        root,
        skirt,
        torso,
        head,
        hips: [l.0, r.0],
        knees: [l.1, r.1],
        arms: [arm(0, -0.27), arm(1, 0.27)],
    }
}

pub struct Rig {
    skirt: Mesh,
    hem: Mesh,
    torso: Mesh,
    belt: Mesh,
    mantle: Mesh,
    head: [Mesh; 2],
    hat: Mesh,
    sleeve: Mesh,
    hand: Mesh,
    wand: Mesh,
    thigh: Mesh,
    shin: Mesh,
    broom: Mesh,
    orb: Mesh,
    ball: Mesh,
}

impl Rig {
    pub fn new(r: &mut Renderer) -> Rig {
        Rig {
            skirt: smooth(r, |g| {
                let profile = [(0.44, 0.0), (0.4, 0.12), (0.32, 0.35), (0.285, 0.47)];
                g.lathe([0.0, -0.47, 0.0], &profile, 18, [1.0; 3], 0.0);
            }),
            hem: smooth(r, |g| {
                g.lathe(
                    [0.0, -0.47, 0.0],
                    &[(0.45, 0.0), (0.43, 0.06)],
                    18,
                    GOLD,
                    0.1,
                );
            }),
            torso: smooth(r, |g| {
                let profile = [
                    (0.285, 0.0),
                    (0.28, 0.17),
                    (0.25, 0.3),
                    (0.18, 0.42),
                    (0.0, 0.46),
                ];
                g.lathe([0.0; 3], &profile, 18, [1.0; 3], 0.0);
            }),
            belt: flat(r, |g| {
                let brown = rgb(90, 62, 40);
                g.column(
                    [0.0, -0.01, 0.0],
                    18,
                    (0.295, 0.295),
                    0.08,
                    0.0,
                    brown,
                    0.0,
                    false,
                );
                g.block(
                    [0.29, -0.015, 0.0],
                    [0.04, 0.09, 0.1],
                    0.0,
                    GOLD,
                    GOLD,
                    0.15,
                );
            }),
            mantle: smooth(r, |g| {
                let profile = [
                    (0.33, 0.0),
                    (0.35, 0.1),
                    (0.3, 0.2),
                    (0.17, 0.3),
                    (0.0, 0.33),
                ];
                g.lathe([0.0, 0.15, 0.0], &profile, 18, [1.0; 3], 0.0);
            }),
            head: [true, false].map(|beard| {
                smooth(r, |g| {
                    g.sphere(
                        [0.0, 0.14, 0.0],
                        [0.19, 0.21, 0.19],
                        (2, 4, 0.02),
                        SKIN,
                        0.0,
                    );
                    // Eyes (so you see which way it faces), brows, a nose.
                    for z in [-0.06f32, 0.06] {
                        g.sphere([0.172, 0.185, z], [0.024; 3], (1, 5, 0.0), WHITE, 0.0);
                        let dark = rgb(30, 40, 60);
                        g.sphere([0.19, 0.185, z], [0.012; 3], (1, 5, 0.0), dark, 0.0);
                        let (a, b) = ([0.17, 0.23, z - 0.03], [0.17, 0.235, z + 0.03]);
                        g.spike(a, b, 0.012, 4, WHITE, 0.0);
                    }
                    g.sphere(
                        [0.2, 0.12, 0.0],
                        [0.04, 0.035, 0.035],
                        (1, 6, 0.0),
                        SKIN,
                        0.0,
                    );
                    if beard {
                        g.spike([0.11, 0.07, 0.0], [0.22, -0.26, 0.0], 0.12, 10, WHITE, 0.0);
                    }
                    g.lathe(
                        [0.0, 0.345, 0.0],
                        &[(0.26, 0.0), (0.245, 0.06)],
                        18,
                        GOLD,
                        0.1,
                    );
                })
            }),
            hat: smooth(r, |g| {
                let profile = [
                    (0.47, 0.0),
                    (0.47, 0.035),
                    (0.27, 0.065),
                    (0.2, 0.3),
                    (0.11, 0.62),
                    (0.0, 0.86),
                ];
                g.lathe([0.0, 0.28, 0.0], &profile, 18, [1.0; 3], 0.0);
            }),
            sleeve: smooth(r, |g| {
                let profile = [(0.11, 0.0), (0.09, 0.22), (0.075, 0.5)];
                g.lathe([0.0, -0.5, 0.0], &profile, 10, [1.0; 3], 0.0);
            }),
            hand: smooth(r, |g| {
                g.sphere([0.0, -0.55, 0.0], [0.065; 3], (1, 8, 0.0), SKIN, 0.0);
            }),
            wand: smooth(r, |g| {
                g.sphere([0.0, -0.55, 0.0], [0.065; 3], (1, 8, 0.0), SKIN, 0.0);
                let wood = rgb(110, 76, 50);
                let profile = [(0.016, 0.0), (0.022, 0.25), (0.028, 0.47)];
                g.lathe([0.0, -1.0, 0.0], &profile, 8, wood, 0.0);
            }),
            thigh: smooth(r, |g| {
                let cloth = rgb(62, 54, 64);
                g.lathe(
                    [0.0, -THIGH, 0.0],
                    &[(0.075, 0.0), (0.095, THIGH)],
                    10,
                    cloth,
                    0.0,
                );
            }),
            shin: flat(r, |g| {
                let cloth = rgb(62, 54, 64);
                let leather = rgb(86, 58, 38);
                g.column(
                    [0.0, -0.36, 0.0],
                    8,
                    (0.07, 0.065),
                    0.36,
                    0.0,
                    cloth,
                    0.0,
                    false,
                );
                g.column(
                    [0.0, -0.4, 0.0],
                    8,
                    (0.08, 0.075),
                    0.12,
                    0.0,
                    leather,
                    0.0,
                    false,
                );
                g.block(
                    [0.05, -0.48, 0.0],
                    [0.27, 0.11, 0.14],
                    0.0,
                    leather,
                    leather,
                    0.0,
                );
            }),
            broom: smooth(r, |g| {
                let wood = rgb(120, 86, 56);
                let straw = rgb(206, 170, 96);
                let profile = [(0.035, 0.0), (0.03, 1.8), (0.018, 1.86)];
                g.lathe([0.0, -0.9, 0.0], &profile, 8, wood, 0.0);
                let bristles = [(0.03, 0.0), (0.2, 0.14), (0.18, 0.38), (0.06, 0.55)];
                g.lathe([0.0, -1.45, 0.0], &bristles, 12, straw, 0.0);
                g.lathe(
                    [0.0, -0.95, 0.0],
                    &[(0.065, 0.0), (0.06, 0.08)],
                    12,
                    GOLD,
                    0.1,
                );
            }),
            orb: smooth(r, |g| {
                g.sphere([0.0; 3], [1.0; 3], (2, 9, 0.0), rgb(255, 246, 225), 1.0)
            }),
            ball: smooth(r, |g| {
                g.sphere([0.0; 3], [1.0; 3], (2, 3, 0.0), [1.0; 3], 0.0)
            }),
        }
    }

    pub fn free(self, r: &mut Renderer) {
        let [h0, h1] = self.head;
        for m in [
            self.skirt,
            self.hem,
            self.torso,
            self.belt,
            self.mantle,
            h0,
            h1,
            self.hat,
            self.sleeve,
            self.hand,
            self.wand,
            self.thigh,
            self.shin,
            self.broom,
            self.orb,
            self.ball,
        ] {
            r.free(m);
        }
    }

    /// The joints for a wizard moving as `a` does, doing `p`.
    fn joints(a: &Anim, p: &Pose, id: u16) -> Joints {
        let speed = (a.fwd * a.fwd + a.side * a.side).sqrt();
        let k = (speed / 7.0).min(1.3);
        let ph = a.phase;
        // Backing away plays the stride backward.
        let (s, c) = ph.sin_cos();
        let s = if a.fwd < -0.3 { -s } else { s };
        let air = a.air;
        let walk = |v: f32| v * (1.0 - air);
        // Each knee bends as its leg swings through.
        let thigh = [
            walk(0.6 * k * s) + air * 0.6,
            walk(-0.6 * k * s) + air * 0.4,
        ];
        let knee = [
            walk(k * (0.12 + 0.85 * c.max(0.0))) + air * 1.0 + 0.04,
            walk(k * (0.12 + 0.85 * (-c).max(0.0))) + air * 0.8 + 0.04,
        ];
        // Crouched, and squatting from a landing: bent low, feet planted.
        let low = (a.crouch + a.land * 0.7).min(1.4);
        let (bend_t, bend_k) = (1.2 * low, 2.0 * low);
        let stride = 1.0 - 0.4 * a.crouch;
        let thigh = [thigh[0] * stride + bend_t, thigh[1] * stride + bend_t];
        let knee = [knee[0] * stride + bend_k, knee[1] * stride + bend_k];
        let sink = drop(bend_t, bend_k) * (1.0 - air);
        let flinch = p.flash;
        let rest = 0.5;
        let up = FRAC_PI_2 + p.aim.clamp(-1.0, 1.0);
        let cast = rest + (up - rest) * p.arm.clamp(0.0, 1.0) + 0.25 * p.tip.1;
        Joints {
            bob: walk(0.045 * k * (0.5 + 0.5 * (2.0 * ph).cos())) + air * 0.05 - sink,
            lean: 0.16 * (a.fwd / 7.0).clamp(-0.6, 1.0) - 0.35 * flinch + 0.3 * low,
            roll: -0.12 * (a.side / 7.0).clamp(-1.0, 1.0),
            breathe: 1.0 + 0.015 * (p.t * 2.4 + id as f32).sin(),
            thigh,
            knee,
            stride: a.side.atan2(a.fwd.abs().max(0.01)),
            skirt: (-0.14 * (a.fwd / 7.0).clamp(-0.6, 1.0), walk(0.05 * k * s)),
            arm: [
                (
                    0.12 - walk(0.5 * k * s) + air * 0.3 + 0.4 * low,
                    0.12 + air * 0.6 + 0.25 * a.land,
                ),
                (cast + walk(0.2 * k * s) * (1.0 - p.arm), 0.05 + air * 0.3),
            ],
            nod: 0.5 * p.aim.clamp(-0.9, 0.9) - 0.3 * flinch,
            fall: 0.0,
            sink: 0.0,
        }
    }

    /// The joints sitting on a broom.
    fn sitting(p: &Pose, id: u16) -> Joints {
        let sway = (p.t * 1.3 + id as f32).sin();
        Joints {
            bob: 0.08 * sway,
            lean: 0.22,
            roll: 0.06 * (p.t * 0.9 + id as f32).cos(),
            breathe: 1.0,
            thigh: [1.4, 1.35],
            knee: [1.5, 1.4],
            stride: 0.0,
            skirt: (0.5, 0.0),
            arm: [(1.05, 0.05), (1.0 + 0.4 * p.arm, 0.0)],
            nod: 0.15,
            fall: 0.0,
            sink: 0.0,
        }
    }

    /// A wizard at `at` (its feet), facing `yaw` (radians), moving as `a`
    /// says and doing what `p` says.
    pub fn wizard(&self, d: &mut Draw, id: u16, at: V3, yaw: f32, a: &Anim, p: &Pose) {
        let j = if p.glide {
            Rig::sitting(p, id)
        } else {
            Rig::joints(a, p, id)
        };
        self.draw(d, id, at, yaw, &j, p);
    }

    /// A wizard knocked out `t` seconds ago at `at`: it falls on its back
    /// and sinks away.
    pub fn fallen(&self, d: &mut Draw, id: u16, at: V3, yaw: f32, t: f32) {
        if t > 1.7 {
            return;
        }
        let k = (t / 0.45).min(1.0);
        let p = Pose {
            flash: 0.0,
            arm: 0.0,
            aim: 0.0,
            tip: (rgb(255, 214, 128), 0.0),
            glide: false,
            t,
        };
        let mut j = Rig::joints(&Anim::default(), &p, id);
        j.fall = k * k * 1.45;
        j.thigh = [0.25 * k, 0.4 * k];
        j.knee = [0.5 * k, 0.3 * k];
        j.arm = [(0.6 * k, 0.9 * k), (0.8 * k, 0.9 * k)];
        j.nod = 0.4 * k;
        j.sink = (t - 1.0).max(0.0) * 1.6;
        self.draw(d, id, at, yaw, &j, &p);
    }

    fn draw(&self, d: &mut Draw, id: u16, at: V3, yaw: f32, j: &Joints, p: &Pose) {
        let c = hue(id);
        let robe = mix(c, [1.0; 3], 0.7 * p.flash);
        let lit = 0.6 * p.flash;
        let dark = geo::scale(c, 0.62);
        let f = frames(at, yaw, j);
        let mut put = |mesh: Mesh, m: M4, tint: Option<V3>| {
            let it = Item::new(mesh, m).rough(0.8);
            d.items.push(match tint {
                Some(t) => it.tint(t, 1.0).glow(lit),
                None => it,
            });
        };
        put(self.skirt, f.skirt, Some(robe));
        put(self.hem, f.skirt, None);
        put(self.torso, f.torso, Some(robe));
        put(self.belt, f.torso, None);
        put(self.mantle, f.torso, Some(dark));
        put(self.head[id as usize % 2], f.head, None);
        put(self.hat, f.head, Some(dark));
        for side in 0..2 {
            put(self.thigh, f.hips[side], None);
            put(self.shin, f.knees[side], None);
        }
        put(self.sleeve, f.arms[0], Some(robe));
        put(self.hand, f.arms[0], None);
        put(self.sleeve, f.arms[1], Some(robe));
        put(self.wand, f.arms[1], None);
        let tip = point(&f.arms[1], [0.0, -1.02, 0.0]);
        let (col, flare) = p.tip;
        let k = 0.055 + 0.07 * flare;
        d.items.push(
            Item::new(self.orb, m4::place(tip, 0.0, [k; 3]))
                .tint(col, 1.0)
                .glow(1.0)
                .pass(Pass::Glow),
        );
        if flare > 0.05 {
            d.lights.push(Light {
                p: tip,
                r: 3.0 + 5.0 * flare,
                c: geo::scale(col, 2.5 * flare),
            });
        }
        if p.glide {
            self.broom(d, f.root, c, p.t, id);
        }
    }

    /// The broomstick under a sitting wizard, sparks trailing from its
    /// bristles.
    fn broom(&self, d: &mut Draw, root: M4, c: V3, t: f32, id: u16) {
        let m = chain(&[root, tr([0.08, HIP - 0.1, 0.0]), rz(-FRAC_PI_2 + 0.08)]);
        d.items.push(Item::new(self.broom, m).rough(0.85));
        let tail = point(&m, [0.0, -1.42, 0.0]);
        let back = geo::norm(geo::sub(tail, point(&m, [0.0, 0.0, 0.0])));
        let glow = mix(c, [1.0; 3], 0.5);
        for n in 0..14 {
            let u = ((t * 1.7 + n as f32 / 14.0) % 1.0).abs();
            let j = |q| (unit(hash(id as i32, n, q)) - 0.5) * 0.5 * u;
            d.sparks.push(Spark {
                p: [
                    tail[0] + back[0] * u * 2.2 + j(1),
                    tail[1] + back[1] * u * 2.2 + j(2) + u * 0.4,
                    tail[2] + back[2] * u * 2.2 + j(3),
                ],
                size: 0.12 * (1.0 - u) + 0.03,
                c: [glow[0], glow[1], glow[2], 1.0 - u],
            });
        }
        d.lights.push(Light {
            p: tail,
            r: 4.0,
            c: geo::scale(glow, 0.8),
        });
    }

    /// Your own broom as you drop: its handle reaching ahead below your
    /// eyes, your hands upon it.
    pub fn first_broom(&self, d: &mut Draw, cam: &render::Camera, t: f32) {
        let fwd = cam.forward();
        let (_, right, up) = cam.matrices(cam.fov);
        let at = |ahead: f32, side: f32, lift: f32| {
            geo::add(
                cam.eye,
                geo::add(
                    geo::scale(fwd, ahead),
                    geo::add(geo::scale(right, side), geo::scale(up, lift)),
                ),
            )
        };
        let sway = 0.02 * (t * 1.3).sin();
        let (a, b) = (at(0.1, 0.0, -0.62 + sway), at(1.5, 0.0, -0.4 + sway));
        let dir = geo::norm(geo::sub(b, a));
        let side = geo::norm(geo::cross(dir, up));
        let other = geo::cross(side, dir);
        let origin = geo::sub(b, geo::scale(dir, 0.9));
        d.items.push(
            Item::new(self.broom, m4::basis(origin, side, dir, other))
                .rough(0.85)
                .pass(Pass::View),
        );
        for s in [-0.09f32, 0.09] {
            let hand = geo::add(geo::add(a, geo::scale(dir, 0.55)), geo::scale(side, s));
            d.items.push(
                Item::new(self.ball, m4::place(hand, 0.0, [0.045; 3]))
                    .tint(SKIN, 1.0)
                    .pass(Pass::View),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stride_alternates_and_a_still_wizard_stands_straight() {
        let mut a = Anim::default();
        let p = pose();
        let still = Rig::joints(&a, &p, 1);
        assert!(still.thigh[0].abs() < 1e-3 && still.thigh[1].abs() < 1e-3);
        // Run east for a second.
        for k in 0..60 {
            a.step(
                [k as f32 * 7.0 / 60.0, 0.0, 0.0],
                0.0,
                (true, false),
                1.0 / 60.0,
            );
        }
        assert!(a.fwd > 6.0 && a.side.abs() < 0.5, "{a:?}");
        let mut seen = (false, false);
        for _ in 0..40 {
            a.phase += 0.2;
            let j = Rig::joints(&a, &p, 1);
            assert!((j.thigh[0] + j.thigh[1]).abs() < 1e-3, "the legs oppose");
            seen.0 |= j.thigh[0] > 0.3;
            seen.1 |= j.thigh[0] < -0.3;
        }
        assert!(seen.0 && seen.1, "each leg goes forward and back");
    }

    fn pose() -> Pose {
        Pose {
            flash: 0.0,
            arm: 0.0,
            aim: 0.0,
            tip: ([1.0; 3], 0.0),
            glide: false,
            t: 0.0,
        }
    }

    #[test]
    fn it_stands_on_its_feet_and_falls_on_its_back() {
        let j = Rig::joints(&Anim::default(), &pose(), 1);
        let f = frames([0.0; 3], 0.0, &j);
        let head = point(&f.head, [0.0, 0.14, 0.0]);
        assert!(head[1] > 1.4, "the head is up: {head:?}");
        for k in &f.knees {
            let sole = point(k, [0.0, -0.48, 0.0]);
            assert!(sole[1].abs() < 0.08, "the feet are on the ground: {sole:?}");
        }
        // Knocked out: on its back, head behind and low.
        let mut j = Rig::joints(&Anim::default(), &pose(), 1);
        j.fall = 1.45;
        let f = frames([0.0; 3], 0.0, &j);
        let head = point(&f.head, [0.0, 0.14, 0.0]);
        assert!(head[1] < 0.5 && head[0] < -1.0, "fallen backward: {head:?}");
        // Casting: the wand arm points along the aim.
        let mut p = pose();
        p.arm = 1.0;
        let j = Rig::joints(&Anim::default(), &p, 1);
        let f = frames([0.0; 3], 0.0, &j);
        let tip = point(&f.arms[1], [0.0, -1.02, 0.0]);
        let shoulder = point(&f.arms[1], [0.0; 3]);
        assert!(tip[0] - shoulder[0] > 0.9, "the wand points ahead: {tip:?}");
    }

    #[test]
    fn crouched_or_landing_its_feet_stay_on_the_ground() {
        let mut a = Anim {
            crouch: 1.0,
            ..Anim::default()
        };
        let j = Rig::joints(&a, &pose(), 1);
        let f = frames([0.0; 3], 0.0, &j);
        let head = point(&f.head, [0.0, 0.14, 0.0]);
        assert!(head[1] < 1.2, "crouched low: {head:?}");
        for k in &f.knees {
            let sole = point(k, [0.0, -0.48, 0.0]);
            assert!(sole[1].abs() < 0.08, "feet down: {sole:?}");
        }
        // A fall, then the ground: a landing, squatting.
        a.crouch = 0.0;
        for k in 0..40 {
            let y = 3.0 - k as f32 * 0.1;
            a.step([0.0, y, 0.0], 0.0, (false, false), 1.0 / 60.0);
        }
        let hard = a.step([0.0; 3], 0.0, (true, false), 1.0 / 60.0);
        assert!(hard.is_some_and(|h| h > 0.3), "{hard:?}");
        assert!(a.land > 0.3);
    }

    #[test]
    fn a_turn_is_a_turn() {
        let m = chain(&[tr([1.0, 2.0, 3.0]), ry(FRAC_PI_2), rz(0.0)]);
        let p = point(&m, [1.0, 0.0, 0.0]);
        // Facing a quarter turn round (+z), forward is +z.
        assert!(
            (p[0] - 1.0).abs() < 1e-5 && (p[2] - 4.0).abs() < 1e-5,
            "{p:?}"
        );
        let up = point(&rz(FRAC_PI_2), [1.0, 0.0, 0.0]);
        assert!((up[1] - 1.0).abs() < 1e-5, "rz lifts forward to up");
    }
}
