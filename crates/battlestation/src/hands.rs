//! The hands: each finger is sent to the key you pressed (the finger a
//! touch typist would use), presses it as far as the real one went, and
//! lingers a moment before it goes home; a reach far from home brings the
//! wrist along. Move your mouse and the right hand leaves the keys for
//! the mouse on the desk (and comes back for the next key it must
//! type). Every fingertip is on a spring; the joints between are solved
//! from where the tip is (`pose`).

use crate::keys::{self, Finger, Side, INDEX, LITTLE, MIDDLE, THUMB};
use crate::laws::{
    COUPLE, DESK_TOP, FOREARM, HAND_PITCH, HAND_YAW, HOVER, KEY_OMEGA, KEY_TRAVEL, KEY_U, LINGER,
    MOST_BEND, MOUSE_H, MOUSE_WAKE, MOUSE_WINDOW, MOUSE_X, MOUSE_YAW, MOUSE_Z, PAD, RADII,
    REACH_FAR, REACH_NEAR, ROOTS, SEGMENTS, SHOULDER, SPLAY, SWAP_ARC, SWAP_OMEGA, THUMB_OUT,
    TIP_OMEGA, WRIST_BACK, WRIST_FAR, WRIST_NEAR, WRIST_OMEGA, WRIST_UP,
};
use crate::{add, cross, damp, dot, len, lerp, norm, scale, smoothstep, sub, turn, Spring, V3};

/// What a finger is doing: resting at home, pressing a key (by its place
/// in `keys::layout`), or hovering over one it just let go (until then).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Duty {
    Rest,
    Press(usize),
    Linger(usize, f32),
}

/// One hand.
#[derive(Clone, Debug)]
pub struct Hand {
    pub side: Side,
    wrist: Spring,
    tips: [Spring; 5],
    pub duty: [Duty; 5],
    /// 0 on the keys, 1 on the mouse (the right hand), and how fast it is
    /// getting there.
    on_mouse: f32,
    swap_v: f32,
    pub wants_mouse: bool,
}

/// A hand as it is drawn: its wrist, its axes (out toward the little
/// finger, up the back of the hand, forward along the fingers), each
/// finger's joints from its root to its tip, and the elbow its forearm
/// reaches back to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub wrist: V3,
    pub axes: [V3; 3],
    pub fingers: [[V3; 4]; 5],
    pub elbow: V3,
}

/// The mouse on the desk: where it is (x, z), and its buttons.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mouse {
    pub x: f32,
    pub z: f32,
    pub left: bool,
    pub right: bool,
}

impl Mouse {
    /// The middle of its foot.
    pub fn at(&self) -> V3 {
        [self.x, DESK_TOP + PAD, self.z]
    }
}

/// Both hands, the keys under them and the mouse.
#[derive(Clone, Debug)]
pub struct Hands {
    pub hands: [Hand; 2],
    pub mouse: Mouse,
    /// Each key (as `keys::layout`): held down, how far down it is, how
    /// fast it moves.
    held: Vec<bool>,
    depth: Vec<(f32, f32)>,
    /// Your mouse's movement lately (pixels, fading over `MOUSE_WINDOW`).
    stir: f32,
    time: f32,
}

fn side_index(s: Side) -> usize {
    match s {
        Side::Left => 0,
        Side::Right => 1,
    }
}

/// The hand's axes at this yaw: out, up, forward.
fn axes(side: Side, yaw: f32) -> [V3; 3] {
    let right = turn([1.0, 0.0, 0.0], yaw);
    let flat = turn([0.0, 0.0, -1.0], yaw);
    let (s, c) = HAND_PITCH.sin_cos();
    let fwd = [flat[0] * c, -s, flat[2] * c];
    let up = norm(cross(right, fwd));
    let out = match side {
        Side::Left => scale(right, -1.0),
        Side::Right => right,
    };
    [out, up, fwd]
}

/// A point given in a right hand's terms (out, up, forward) on this hand.
fn local(at: V3, ax: &[V3; 3], p: V3) -> V3 {
    add(
        at,
        add(
            scale(ax[0], p[0]),
            add(scale(ax[1], p[1]), scale(ax[2], p[2])),
        ),
    )
}

/// How much of a reach `d` metres from home the wrist makes.
fn wrist_share(d: f32) -> f32 {
    let t = (d / KEY_U - REACH_NEAR) / (REACH_FAR - REACH_NEAR);
    WRIST_NEAR + (WRIST_FAR - WRIST_NEAR) * smoothstep(t)
}

/// Where a hand rests on the keys: its wrist, and its fingertips.
struct Aim {
    wrist: V3,
    tips: [V3; 5],
    yaw: f32,
}

impl Hand {
    fn new(side: Side) -> Hand {
        let mut h = Hand {
            side,
            wrist: Spring::default(),
            tips: [Spring::default(); 5],
            duty: [Duty::Rest; 5],
            on_mouse: 0.0,
            swap_v: 0.0,
            wants_mouse: false,
        };
        let aim = h.on_keys([0.0; 3]);
        h.wrist = Spring::at(aim.wrist);
        for d in 0..5 {
            h.tips[d] = Spring::at(aim.tips[d]);
        }
        h
    }

    fn finger(&self, digit: usize) -> Finger {
        Finger {
            side: self.side,
            digit,
        }
    }

    fn yaw(&self) -> f32 {
        match self.side {
            Side::Left => HAND_YAW,
            Side::Right => -HAND_YAW,
        }
    }

    /// The hand over its home keys, moved by `shift`, each finger at its
    /// duty.
    fn on_keys(&self, shift: V3) -> Aim {
        let ax = axes(self.side, self.yaw());
        let mut tips = [[0.0; 3]; 5];
        let mut sum = [0.0; 3];
        for (d, tip) in tips.iter_mut().enumerate() {
            let f = self.finger(d);
            let home = keys::home_key(f).spot(f);
            if d != THUMB {
                sum = add(sum, home);
            }
            let r = RADII[d];
            *tip = match self.duty[d] {
                Duty::Rest => add(add(home, shift), [0.0, r + HOVER, 0.0]),
                Duty::Press(k) => add(keys::layout()[k].spot(f), [0.0, r - KEY_TRAVEL, 0.003]),
                Duty::Linger(k, _) => add(keys::layout()[k].spot(f), [0.0, r + 0.004, 0.002]),
            };
        }
        let mid = scale(sum, 0.25);
        let flat = turn([0.0, 0.0, -1.0], self.yaw());
        let wrist = add(
            add(mid, shift),
            add(scale(flat, -WRIST_BACK), scale(ax[1], WRIST_UP)),
        );
        Aim {
            wrist,
            tips,
            yaw: self.yaw(),
        }
    }

    /// How far the wrist follows the fingers' reaches.
    fn shift(&self) -> V3 {
        let mut sum = [0.0; 3];
        let mut n = 0.0;
        for d in INDEX..=LITTLE {
            let k = match self.duty[d] {
                Duty::Press(k) | Duty::Linger(k, _) => k,
                Duty::Rest => continue,
            };
            let f = self.finger(d);
            let reach = sub(keys::layout()[k].spot(f), keys::home_key(f).spot(f));
            let flat = len([reach[0], 0.0, reach[2]]);
            sum = add(sum, scale(reach, wrist_share(flat)));
            n += 1.0;
        }
        if n > 0.0 {
            scale(sum, 1.0 / n)
        } else {
            [0.0; 3]
        }
    }

    /// The right hand on the mouse.
    fn on_mouse_aim(&self, m: &Mouse) -> Aim {
        let yaw = MOUSE_YAW;
        let at = m.at();
        let flat = turn([0.0, 0.0, -1.0], yaw);
        let right = turn([1.0, 0.0, 0.0], yaw);
        let up = [0.0, 1.0, 0.0];
        let p = |o: f32, u: f32, f: f32| {
            add(at, add(scale(right, o), add(scale(up, u), scale(flat, f))))
        };
        let top = MOUSE_H * 0.82;
        let click = |down: bool| if down { 0.0025 } else { 0.0 };
        let tips = [
            p(-0.036, 0.013, -0.018),
            p(-0.011, top + RADII[INDEX] - click(m.left), 0.044),
            p(0.011, top + RADII[MIDDLE] - click(m.right), 0.041),
            p(0.031, 0.018, 0.024),
            p(0.039, 0.010, 0.004),
        ];
        let wrist = p(0.006, 0.047, -0.122);
        Aim { wrist, tips, yaw }
    }
}

impl Default for Hands {
    fn default() -> Hands {
        Hands::new()
    }
}

impl Hands {
    pub fn new() -> Hands {
        let n = keys::layout().len();
        Hands {
            hands: [Hand::new(Side::Left), Hand::new(Side::Right)],
            mouse: Mouse {
                x: MOUSE_X,
                z: MOUSE_Z,
                left: false,
                right: false,
            },
            held: vec![false; n],
            depth: vec![(0.0, 0.0); n],
            stir: 0.0,
            time: 0.0,
        }
    }

    pub fn hand(&self, side: Side) -> &Hand {
        &self.hands[side_index(side)]
    }

    /// A key of yours went down or up (`KeyboardEvent.code`); keys this
    /// keyboard does not have are let be.
    pub fn key(&mut self, code: &str, down: bool) {
        let Some(k) = keys::layout().iter().position(|key| key.code == code) else {
            return;
        };
        self.held[k] = down;
        if !down {
            let until = self.time + LINGER;
            for h in &mut self.hands {
                for duty in &mut h.duty {
                    if *duty == Duty::Press(k) {
                        *duty = Duty::Linger(k, until);
                    }
                }
            }
            return;
        }
        let mut finger = keys::layout()[k].finger;
        if finger.side == Side::Right && self.hands[1].wants_mouse {
            if code == "Space" {
                // The hand stays on the mouse; the left thumb takes it.
                finger = Finger {
                    side: Side::Left,
                    digit: THUMB,
                };
            } else {
                self.hands[1].wants_mouse = false;
            }
        }
        // A key held by one finger and pressed again by another moves.
        for h in &mut self.hands {
            for duty in &mut h.duty {
                if matches!(*duty, Duty::Press(j) | Duty::Linger(j, _) if j == k) {
                    *duty = Duty::Rest;
                }
            }
        }
        self.hands[side_index(finger.side)].duty[finger.digit] = Duty::Press(k);
    }

    /// Your mouse moved this many pixels: enough of that, and the right
    /// hand goes to it (unless it is holding a key down).
    pub fn stir(&mut self, px: f32) {
        self.stir += px.abs().min(1000.0);
        let typing = self.hands[1]
            .duty
            .iter()
            .any(|d| matches!(d, Duty::Press(_)));
        if self.stir > MOUSE_WAKE && !typing {
            self.hands[1].wants_mouse = true;
        }
    }

    /// A mouse button went down or up (0 left, 2 right).
    pub fn button(&mut self, b: i16, down: bool) {
        match b {
            0 => self.mouse.left = down,
            2 => self.mouse.right = down,
            _ => return,
        }
        if down {
            self.hands[1].wants_mouse = true;
        }
    }

    /// Put the mouse here on the desk (x, z).
    pub fn put_mouse(&mut self, x: f32, z: f32) {
        self.mouse.x = x;
        self.mouse.z = z;
    }

    /// How far down key `k` (of `keys::layout`) is.
    pub fn depth(&self, k: usize) -> f32 {
        self.depth
            .get(k)
            .map_or(0.0, |d| d.0.clamp(0.0, KEY_TRAVEL))
    }

    /// Whether key `k` is held down.
    pub fn held(&self, k: usize) -> bool {
        self.held.get(k).copied().unwrap_or(false)
    }

    /// How much of the way to the mouse the right hand is (0 to 1).
    pub fn on_mouse(&self) -> f32 {
        smoothstep(self.hands[1].on_mouse)
    }

    /// Move everything on by `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.25)
        } else {
            0.0
        };
        self.time += dt;
        self.stir *= (-dt / MOUSE_WINDOW).exp();
        for (k, d) in self.depth.iter_mut().enumerate() {
            let to = if self.held[k] { KEY_TRAVEL } else { 0.0 };
            damp(&mut d.0, &mut d.1, to, KEY_OMEGA, dt);
        }
        let mouse = self.mouse;
        let time = self.time;
        for h in &mut self.hands {
            for duty in &mut h.duty {
                if let Duty::Linger(_, until) = *duty {
                    if time >= until {
                        *duty = Duty::Rest;
                    }
                }
            }
            let want = if h.wants_mouse { 1.0 } else { 0.0 };
            damp(&mut h.on_mouse, &mut h.swap_v, want, SWAP_OMEGA, dt);
            let s = smoothstep(h.on_mouse);
            let keys_aim = h.on_keys(h.shift());
            let aim = if h.side == Side::Right && s > 0.0 {
                let m = h.on_mouse_aim(&mouse);
                let arc = SWAP_ARC * (std::f32::consts::PI * s).sin();
                let mut tips = keys_aim.tips;
                for (d, tip) in tips.iter_mut().enumerate() {
                    *tip = add(lerp(*tip, m.tips[d], s), [0.0, arc, 0.0]);
                }
                Aim {
                    wrist: add(lerp(keys_aim.wrist, m.wrist, s), [0.0, arc, 0.0]),
                    tips,
                    yaw: keys_aim.yaw + (m.yaw - keys_aim.yaw) * s,
                }
            } else {
                keys_aim
            };
            // Breathing moves the hands a little.
            let phase = if h.side == Side::Left { 0.0 } else { 1.7 };
            let sway = [
                0.0006 * (time * 0.9 + phase).sin(),
                0.0008 * (time * 1.3 + phase).sin(),
                0.0006 * (time * 0.7 + phase).cos(),
            ];
            h.wrist.step(add(aim.wrist, sway), WRIST_OMEGA, dt);
            for d in 0..5 {
                let mut to = add(aim.tips[d], sway);
                // Lift over the keys on the way to a far one.
                let gap = sub(to, h.tips[d].x);
                let flat = len([gap[0], 0.0, gap[2]]);
                to[1] += (flat * 0.6).min(0.012);
                h.tips[d].step(to, TIP_OMEGA, dt);
            }
        }
    }

    /// A hand as it is now, its joints solved from its fingertips.
    pub fn pose(&self, side: Side) -> Pose {
        let h = &self.hands[side_index(side)];
        let s = if side == Side::Right {
            smoothstep(h.on_mouse)
        } else {
            0.0
        };
        let yaw = h.yaw() + (MOUSE_YAW - h.yaw()) * s;
        let ax = axes(side, yaw);
        let wrist = h.wrist.x;
        let mut fingers = [[[0.0; 3]; 4]; 5];
        for (d, joints) in fingers.iter_mut().enumerate() {
            let root = local(wrist, &ax, ROOTS[d]);
            let tip = h.tips[d].x;
            *joints = if d == THUMB {
                let out = norm(local([0.0; 3], &ax, THUMB_OUT));
                let knuckle = add(root, scale(out, SEGMENTS[THUMB][0]));
                let lens = [SEGMENTS[THUMB][1], SEGMENTS[THUMB][2], 0.0];
                let fwd = norm(add(ax[2], scale(out, 0.6)));
                let j = solve(knuckle, tip, lens, ax[1], fwd, 0.9);
                [root, j[0], j[1], j[2]]
            } else {
                solve(root, tip, SEGMENTS[d], ax[1], ax[2], SPLAY)
            };
        }
        let shoulder = match side {
            Side::Left => [-SHOULDER[0], SHOULDER[1], SHOULDER[2]],
            Side::Right => SHOULDER,
        };
        let elbow = add(wrist, scale(norm(sub(shoulder, wrist)), FOREARM));
        Pose {
            wrist,
            axes: ax,
            fingers,
            elbow,
        }
    }
}

/// A finger from `root` reaching for `tip`: its three segments `l` bend
/// in one plane (the hand's `up` and the way to the tip, splayed at most
/// `splay` from `fwd`), the last joint bending `COUPLE` as much as the
/// middle one. Its four joints, root to tip; a tip out of reach is
/// pointed at.
pub fn solve(root: V3, tip: V3, l: [f32; 3], up: V3, fwd: V3, splay: f32) -> [V3; 4] {
    let v = sub(tip, root);
    let flat = sub(v, scale(up, dot(v, up)));
    let fwd = norm(sub(fwd, scale(up, dot(fwd, up))));
    let side = cross(fwd, up);
    let mut d = if len(flat) < 1e-6 { fwd } else { norm(flat) };
    let a = dot(d, side).atan2(dot(d, fwd)).clamp(-splay, splay);
    d = norm(add(scale(fwd, a.cos()), scale(side, a.sin())));
    let (x, y) = (dot(v, d), dot(v, up));
    // The chain's end with its first joint straight, bent `b` after.
    let end = |b: f32| {
        let (c2, s2) = (b.cos(), b.sin());
        let (c3, s3) = ((b * (1.0 + COUPLE)).cos(), (b * (1.0 + COUPLE)).sin());
        (l[0] + l[1] * c2 + l[2] * c3, -(l[1] * s2 + l[2] * s3))
    };
    let reach = |b: f32| {
        let (ex, ey) = end(b);
        (ex * ex + ey * ey).sqrt()
    };
    let r = (x * x + y * y).sqrt();
    let b = if r >= reach(0.0) {
        0.0
    } else if r <= reach(MOST_BEND) {
        MOST_BEND
    } else {
        let (mut lo, mut hi) = (0.0, MOST_BEND);
        for _ in 0..24 {
            let mid = (lo + hi) / 2.0;
            if reach(mid) > r {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        (lo + hi) / 2.0
    };
    let (ex, ey) = end(b);
    let t1 = ((-y).atan2(x) - (-ey).atan2(ex)).clamp(-0.6, 1.8);
    let dir = |t: f32| add(scale(d, t.cos()), scale(up, -t.sin()));
    let j1 = add(root, scale(dir(t1), l[0]));
    let j2 = add(j1, scale(dir(t1 + b), l[1]));
    let j3 = add(j2, scale(dir(t1 + b * (1.0 + COUPLE)), l[2]));
    [root, j1, j2, j3]
}

/// The desk under the mouse: how far it moves from home for the cursor
/// this far from the screen's middle (pixels).
pub fn mouse_for(cursor: (f32, f32), screen: (i32, i32)) -> (f32, f32) {
    let k = crate::laws::MOUSE_PER_PX;
    (
        MOUSE_X + (cursor.0 - screen.0 as f32 / 2.0) * k,
        MOUSE_Z + (cursor.1 - screen.1 as f32 / 2.0) * k,
    )
}

#[cfg(test)]
mod tests;
