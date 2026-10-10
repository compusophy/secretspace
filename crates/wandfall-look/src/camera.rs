//! The camera over a wizard's shoulder (third person): a spring arm from a
//! pivot above its right shoulder, back along where you look; pulled in
//! at once when a wall stands between (the ground, the tower, a stone)
//! and let out again gently; eased in out of a tree's crown or from under
//! a mushroom's cap; slid round a trunk or a post rather than pulled in by
//! it (a trunk passing behind a running wizard would jerk the view in and
//! out); nearer and tighter when aiming, lower crouched, further out and
//! looking down on a broom (the island you are choosing to land on before
//! you), back to the view on foot as you come in to land. And what the
//! crosshair at the middle of the screen is on, so a wizard aims there
//! from its own eyes (the shoulder's offset taken out).

use render::geo::{self, V3};
use render::Camera;
use wandfall::laws::{spell, SEA};
use wandfall::map::{Kind, Map, Prop};
use wandfall::proto::{Ev, Seen};

use crate::fx;
use crate::land::pine;
use crate::rig::Spring;

/// The arm's length (metres): walking, aiming, on a broom.
const ARM: f32 = 3.4;
const ARM_AIM: f32 = 1.7;
const ARM_GLIDE: f32 = 9.0;
/// The pivot: how far right of the head, and how high above the feet
/// (standing, crouched, on a broom).
const SHOULDER: f32 = 0.6;
const HEAD: f32 = 1.65;
const HEAD_CROUCH: f32 = 1.2;
const HEAD_GLIDE: f32 = 3.0;
/// How far down the view tips on a broom (radians), so the wizard sits
/// low and the island ahead and below fills it.
const GLIDE_TILT: f32 = 0.35;
/// Coming down on a broom, the view eases from the broom's to the one
/// on foot over the last this many metres, so landing neither cuts nor
/// tips it (nor moves your aim).
const GLIDE_LOW: f32 = 12.0;
/// How quickly the arm comes in (half-life, s) when the stance wants it
/// shorter (aiming, landing; a wall pulls it in at once), and lets out.
const ARM_IN: f32 = 0.08;
const ARM_OUT: f32 = 0.25;
/// How far the view's height may trail the feet's (m): a step up onto a
/// root or a stone (`laws::STEP`) is eased whole; a fall, a leap, a haul
/// that has trailed further than `RISE_LAG` for `RISE_HOLD` s is kept up
/// with, the leeway closing to `RISE_LAG` over about `RISE_CLOSE` s.
const STEP_LAG: f32 = wandfall::laws::STEP + 0.1;
const RISE_LAG: f32 = 0.35;
const RISE_HOLD: f32 = 0.04;
const RISE_CLOSE: f32 = 0.05;
/// How near the camera may come to what it would be inside; how far it
/// sits above the line of the arm (so the wizard stands low in the view).
const SKIN: f32 = 0.3;
const LIFT: f32 = 0.35;
/// Things narrower than this (radius, metres: trunks, stems, posts,
/// lamps) the camera slides round, eased aside and back (a half-life,
/// seconds); wider ones are walls.
const THIN: f32 = 0.6;
const ROUND: f32 = 0.05;
/// Fields of view (radians, up and down): walking, aiming.
pub const FOV: f32 = 1.15;
pub const FOV_AIM: f32 = 0.78;
/// A blast near you shakes the view: how far it is felt (m), how long
/// (ms), and how hard at its heart (radians).
const RUMBLE_REACH: f32 = 20.0;
const RUMBLE_FOR: f64 = 450.0;
const RUMBLE: f32 = 0.035;

/// How hard the blasts in `shows` shake a view at `at`, `now` ms: a
/// fireball bursting, lightning striking, a ward shattering, each by how
/// near and how fresh (0 none).
pub fn rumble(shows: &[(f64, Ev)], now: f64, at: V3) -> f32 {
    shows
        .iter()
        .filter_map(|&(when, e)| {
            let age = now - when;
            let (p, k) = match e {
                Ev::Cast {
                    spell: spell::FIREBALL,
                    stage: 1,
                    at,
                    ..
                } => (at, 1.0),
                Ev::Cast {
                    spell: spell::LIGHTNING,
                    stage: 1,
                    at,
                    ..
                } => (at, 1.2),
                Ev::Cast {
                    spell: spell::WARD,
                    stage: 2,
                    at,
                    ..
                } => (at, 0.5),
                _ => return None,
            };
            if !(0.0..RUMBLE_FOR).contains(&age) {
                return None;
            }
            let d = geo::dot(geo::sub(p, at), geo::sub(p, at)).sqrt();
            let near = (1.0 - d / RUMBLE_REACH).max(0.0);
            let fresh = 1.0 - (age / RUMBLE_FOR) as f32;
            Some(k * near * near * fresh * fresh)
        })
        .fold(0.0, f32::max)
}

/// The view shaken by `k` (`rumble`), `now` ms: turned a little this way
/// and that, quickly and unevenly.
pub fn shake(cam: &mut Camera, k: f32, now: f64) {
    if k <= 0.0 {
        return;
    }
    let t = now as f32;
    let a = RUMBLE * k;
    cam.yaw += ((t * 0.071).sin() + 0.6 * (t * 0.113 + 1.7).sin()) * a;
    cam.pitch += ((t * 0.089 + 0.4).sin() + 0.5 * (t * 0.131).sin()) * a;
}

/// What the wizard is doing, for the camera.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stance {
    pub aiming: bool,
    pub crouch: bool,
    pub glide: bool,
    /// Going fast (sprinting or sliding): the view widens a touch.
    pub fast: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Chase {
    arm: Spring,
    shoulder: Spring,
    head: Spring,
    fov: Spring,
    rise: Spring,
    /// How long the view's height has trailed the feet's by more than
    /// `RISE_LAG` (s).
    trail: f32,
    tilt: Spring,
    round: [Spring; 2],
    started: bool,
}

/// Forward along a look, and to its right on the ground.
pub fn axes(yaw: f32, pitch: f32) -> (V3, V3) {
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    ([cp * cy, sp, cp * sy], [-sy, 0.0, cy])
}

impl Chase {
    /// The camera for a wizard with its feet at `feet`, looked at along
    /// `yaw` and `pitch`, `dt` seconds on.
    pub fn view(
        &mut self,
        map: &Map,
        feet: V3,
        (yaw, pitch): (f32, f32),
        stance: Stance,
        aspect: f32,
        dt: f32,
    ) -> Camera {
        // On a broom high over the island: further out, higher, looking
        // down on it; coming in to land, eased to the view on foot.
        let high = if stance.glide {
            let under = map.floor(feet[0], feet[2], feet[1]).max(SEA);
            ((feet[1] - under) / GLIDE_LOW).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (arm, shoulder, head, fov) = match stance {
            Stance { glide: true, .. } => (
                ARM + (ARM_GLIDE - ARM) * high,
                SHOULDER * (1.0 - high),
                HEAD + (HEAD_GLIDE - HEAD) * high,
                FOV,
            ),
            Stance {
                aiming: true,
                crouch,
                ..
            } => (
                ARM_AIM,
                SHOULDER + 0.05,
                if crouch { HEAD_CROUCH } else { HEAD },
                FOV_AIM,
            ),
            Stance { crouch, .. } => (ARM, SHOULDER, if crouch { HEAD_CROUCH } else { HEAD }, FOV),
        };
        let tilt = -GLIDE_TILT * high;
        if !self.started {
            self.arm.x = arm;
            self.shoulder.x = shoulder;
            self.head.x = head;
            self.fov.x = fov;
            self.rise.x = feet[1];
            self.tilt.x = tilt;
            self.started = true;
        }
        let shoulder = self.shoulder.step(shoulder, 0.12, dt);
        let head = self.head.step(head, 0.1, dt);
        let fov = self
            .fov
            .step(fov + if stance.fast { 0.1 } else { 0.0 }, 0.12, dt);
        // Tipped down gently, and back up quickly (it is your aim).
        let half = if tilt > self.tilt.x { 0.06 } else { 0.25 };
        let pitch = (pitch + self.tilt.step(tilt, half, dt)).clamp(-1.5, 1.5);
        // The feet's height eased, so a step up does not jolt the view;
        // but a fall or a leap does not leave the wizard at the bottom of
        // it for long.
        let y = self.rise.step(feet[1], 0.06, dt);
        let off = y - feet[1];
        self.trail = if off.abs() > RISE_LAG {
            self.trail + dt
        } else {
            0.0
        };
        let held = (self.trail - RISE_HOLD).max(0.0);
        let leeway = RISE_LAG + (STEP_LAG - RISE_LAG) * (-held / RISE_CLOSE).exp();
        if off.abs() > leeway {
            self.rise = Spring {
                x: feet[1] + off.clamp(-leeway, leeway),
                v: 0.0,
            };
        }
        let y = self.rise.x;
        let (fwd, right) = axes(yaw, pitch);
        let top = [feet[0], y + head, feet[2]];
        let wall = |q: &Prop| q.r >= THIN;
        // The shoulder's offset, cut short by a wall beside the head.
        let side = geo::add(top, geo::scale(right, shoulder));
        let pivot = match map.strikes_if(top, side, wall) {
            Some(t) => geo::add(top, geo::scale(right, (shoulder * t - SKIN).max(0.0))),
            None => side,
        };
        // The arm, cut short at once by a wall behind (as far back as it
        // reaches now, if the stance is drawing it in); let out gently.
        let reach = arm.max(self.arm.x);
        let back = geo::sub(pivot, geo::scale(fwd, reach + SKIN));
        let hit = map
            .strikes_if(pivot, back, wall)
            .map(|t| ((reach + SKIN) * t - SKIN).max(0.35));
        if let Some(h) = hit.filter(|&h| h < self.arm.x) {
            self.arm = Spring { x: h, v: 0.0 };
        }
        let room = hit.map_or(arm, |h| h.min(arm));
        // Nor among a tree's leaves or under a cap: eased in till clear,
        // as gently as let out (leaves, not walls, so a moment among them
        // does no harm, and a yank into the head would).
        let mut want = room;
        while want > 0.7 && in_crown(map, geo::sub(pivot, geo::scale(fwd, want))) {
            want -= 0.25;
        }
        let half = if arm < self.arm.x { ARM_IN } else { ARM_OUT };
        self.arm.step(want, half, dt);
        let lift = LIFT * (self.arm.x / arm).min(1.0);
        let at = geo::add(
            geo::sub(pivot, geo::scale(fwd, self.arm.x)),
            [0.0, lift, 0.0],
        );
        // Round a trunk behind, eased aside and back (a trunk passing
        // close by would swing it round its side in a frame).
        let out = clear(map, at);
        let round = [0, 1].map(|k| self.round[k].step(out[k * 2] - at[k * 2], ROUND, dt));
        let eye = [at[0] + round[0], at[1], at[2] + round[1]];
        Camera {
            eye,
            yaw,
            pitch,
            fov,
            aspect,
        }
    }
}

/// `p` moved out of any trunk, stem or post it is in (to the side, the
/// shortest way), so the camera slides round them.
fn clear(map: &Map, mut p: V3) -> V3 {
    for q in map.near(p[0], p[2], THIN + SKIN) {
        let trunk = match q.kind {
            Kind::Tree | Kind::Shroom => q.h * 0.35,
            _ => q.h,
        };
        if q.r >= THIN || p[1] < q.y || p[1] > q.y + trunk {
            continue;
        }
        let (dx, dz) = (p[0] - q.x, p[2] - q.z);
        let d = (dx * dx + dz * dz).sqrt();
        let need = q.r + SKIN;
        if d < need {
            let (nx, nz) = if d > 1e-4 {
                (dx / d, dz / d)
            } else {
                (1.0, 0.0)
            };
            p[0] = q.x + nx * need;
            p[2] = q.z + nz * need;
        }
    }
    p
}

/// Whether `p` is among a tree's leaves or in a mushroom's cap, as each
/// is drawn: a broadleaf's crown, a pine's cone of boughs, a cap (not
/// merely near a trunk or a stem under them).
pub(crate) fn in_crown(map: &Map, p: V3) -> bool {
    map.near_indexed(p[0], p[2], 4.2).any(|(k, q)| {
        let (dx, dy, dz) = (p[0] - q.x, p[1] - q.y, p[2] - q.z);
        let s = q.scale;
        let within = |up: f32, wide: f32, tall: f32| {
            (dx * dx + dz * dz) / (wide * wide) + (dy - up).powi(2) / (tall * tall) < 1.0
        };
        match q.kind {
            Kind::Shroom => within(1.05 * q.h, 0.5 * q.h, 0.2 * q.h),
            Kind::Tree if pine(k) => {
                let u = (dy / s - 0.6) / 6.7;
                (0.0..1.0).contains(&u) && dx * dx + dz * dz < (2.6 * s * (1.0 - u)).powi(2)
            }
            Kind::Tree => within(4.6 * s, 2.9 * s, 2.0 * s),
            _ => false,
        }
    })
}

/// What the crosshair is on, looking from `cam` (the wizard `me` with its
/// eyes at `eye` left out), as far as `range`: the point, and the wizard
/// there if one. Things between the camera and the wizard are passed
/// over, so a wizard never aims behind itself.
pub fn crosshair(
    map: &Map,
    others: &[Seen],
    me: u16,
    cam: &Camera,
    eye: V3,
    range: f32,
) -> (V3, Option<u16>) {
    let fwd = cam.forward();
    let ahead = geo::dot(geo::sub(eye, cam.eye), fwd).max(0.0);
    let from = geo::add(cam.eye, geo::scale(fwd, ahead));
    fx::sight(map, others, me, (from, fwd), range)
}

/// The yaw and pitch (radians) from `eye` to `at`; from the camera's
/// own when `at` is too near to tell.
pub fn toward(eye: V3, at: V3, cam: &Camera) -> (f32, f32) {
    let d = geo::sub(at, eye);
    let flat = (d[0] * d[0] + d[2] * d[2]).sqrt();
    if flat < 1.5 {
        return (cam.yaw, cam.pitch);
    }
    (d[2].atan2(d[0]), d[1].atan2(flat))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blast_near_you_shakes_the_view_and_far_or_old_ones_do_not() {
        let burst = |at: V3| Ev::Cast {
            by: 1,
            spell: spell::FIREBALL,
            stage: 1,
            at,
        };
        let me = [0.0, 0.0, 0.0];
        let near = rumble(&[(1000.0, burst([2.0, 0.0, 0.0]))], 1050.0, me);
        let far = rumble(&[(1000.0, burst([40.0, 0.0, 0.0]))], 1050.0, me);
        let old = rumble(&[(1000.0, burst([2.0, 0.0, 0.0]))], 2000.0, me);
        assert!(near > 0.6, "{near}");
        assert_eq!((far, old), (0.0, 0.0));
        let mut cam = Camera::default();
        let was = (cam.yaw, cam.pitch);
        shake(&mut cam, near, 1050.0);
        assert!((cam.yaw, cam.pitch) != was);
    }

    #[test]
    fn running_through_a_wood_the_view_never_leaps() {
        // Through the thickest wood on the island at a sprint, 144 frames
        // a second: trunks pass behind, crowns overhead; the camera slides
        // and eases, never jumping more than the wizard moves by much.
        let map = Map::new(3);
        let (fps, v) = (144.0, 10.0);
        let trees = |x: f32, z: f32| {
            (0..30)
                .filter(|k| {
                    let x = x + *k as f32 * 2.0;
                    map.land(x, z) && map.near(x, z, 6.0).any(|q| q.kind == Kind::Tree)
                })
                .count()
        };
        let mut best = (0, 0.0, 0.0);
        for k in 0..900 {
            let (x, z) = ((k % 30) as f32 * 6.0 - 90.0, (k / 30) as f32 * 6.0 - 90.0);
            let n = trees(x, z);
            if n > best.0 {
                best = (n, x, z);
            }
        }
        let (_, x0, z) = best;
        let mut c = Chase::default();
        let mut last: Option<V3> = None;
        let mut worst: f32 = 0.0;
        // The feet go up and down the ground (and over a root or a stone)
        // no faster than a step a tick, as the page draws them.
        let climb = wandfall::laws::STEP * wandfall::laws::TICK_HZ as f32 / fps;
        let mut y = map.floor(x0, z, map.height(x0, z) + 1.0);
        for k in 0..(fps as usize * 6) {
            let x = x0 + v * k as f32 / fps;
            y += (map.floor(x, z, map.height(x, z) + 1.0) - y).clamp(-climb, climb);
            let feet = [x, y, z];
            let cam = c.view(&map, feet, (0.0, 0.0), Stance::default(), 1.6, 1.0 / fps);
            if let Some(l) = last {
                let d = geo::sub(cam.eye, l);
                worst = worst.max(geo::dot(d, d).sqrt());
            }
            last = Some(cam.eye);
        }
        assert!(worst < 3.0 * v / fps, "the view leapt {worst} m in a frame");
    }

    #[test]
    fn a_fall_or_a_leap_does_not_leave_the_wizard_at_the_bottom_of_the_view() {
        let map = Map::new(3);
        let (fps, g) = (60.0, wandfall::laws::GRAVITY);
        // Off a cliff, then thrown up at 20 m/s, for two seconds: the eye
        // stays within a step's leeway of where it sits over the feet at
        // rest, and once falling a while, within a little of it.
        for v0 in [0.0, 20.0] {
            let mut c = Chase::default();
            let rest = c.view(
                &map,
                [0.0, 120.0, 0.0],
                (0.0, 0.0),
                Stance::default(),
                1.6,
                0.0,
            );
            let over = rest.eye[1] - 120.0;
            let (mut y, mut v) = (120.0, v0);
            for k in 0..fps as usize * 2 {
                v -= g / fps;
                y += v / fps;
                let cam = c.view(
                    &map,
                    [0.0, y, 0.0],
                    (0.0, 0.0),
                    Stance::default(),
                    1.6,
                    1.0 / fps,
                );
                let off = cam.eye[1] - y - over;
                let most = if k < fps as usize * 3 / 2 {
                    STEP_LAG + 1e-3
                } else {
                    RISE_LAG + 0.02
                };
                assert!(off.abs() < most, "{off} m off at {y} m, {v} m/s");
            }
        }
    }

    #[test]
    fn a_step_up_onto_a_root_does_not_jolt_the_view() {
        // A step's height in a tick, drawn between ticks as the page
        // draws the feet: the eye rises smoothly, a little a frame.
        let map = Map::new(3);
        for fps in [30.0, 60.0, 144.0] {
            let mut c = Chase::default();
            let tick = 1.0 / wandfall::laws::TICK_HZ as f32;
            let mut last: Option<f32> = None;
            let mut worst: f32 = 0.0;
            for k in 0..fps as usize {
                let t = k as f32 / fps - 0.2;
                let y = 60.0 + wandfall::laws::STEP * (t / tick).clamp(0.0, 1.0);
                let cam = c.view(
                    &map,
                    [0.0, y, 0.0],
                    (0.0, 0.0),
                    Stance::default(),
                    1.6,
                    1.0 / fps,
                );
                if let Some(l) = last {
                    worst = worst.max(cam.eye[1] - l);
                }
                last = Some(cam.eye[1]);
            }
            assert!(worst < 6.0 / fps, "{worst} m in a frame at {fps} fps");
        }
    }

    #[test]
    fn coming_down_off_a_broom_the_view_neither_cuts_nor_tips() {
        // Down onto open ground on a broom, falling as fast as a broom
        // does, then on foot: the eye moves a little a frame all the way,
        // and once down the view's pitch (your aim) stays where it was.
        let map = Map::new(3);
        let (x, z) = (0..400)
            .map(|k| ((k % 20) as f32 * 9.0 - 90.0, (k / 20) as f32 * 9.0 - 90.0))
            .find(|&(x, z)| map.land(x, z) && map.near(x, z, 12.0).next().is_none())
            .expect("open ground");
        let ground = map.floor(x, z, map.height(x, z) + 1.0);
        let fps = 60.0;
        let mut c = Chase::default();
        let mut y = ground + 30.0;
        let (mut last, mut worst): (Option<V3>, f32) = (None, 0.0);
        let mut landed: Option<(usize, f32)> = None;
        for k in 0..fps as usize * 8 {
            y = (y - wandfall::laws::GLIDE_FALL / fps).max(ground);
            let stance = Stance {
                glide: y > ground,
                ..Stance::default()
            };
            let cam = c.view(&map, [x, y, z], (0.0, -0.1), stance, 1.6, 1.0 / fps);
            if let Some(l) = last {
                let d = geo::sub(cam.eye, l);
                worst = worst.max(geo::dot(d, d).sqrt());
            }
            last = Some(cam.eye);
            match landed {
                None if !stance.glide => landed = Some((k, cam.pitch)),
                Some((at, pitch)) if k < at + (fps * 0.3) as usize => {
                    let drift = (cam.pitch - pitch).abs();
                    assert!(drift < 0.02, "the aim drifted {drift} rad");
                }
                _ => {}
            }
        }
        assert!(landed.is_some(), "down on the ground");
        assert!(worst < 0.2, "the view moved {worst} m in a frame");
        assert!((c.arm.x - ARM).abs() < 0.05 && c.tilt.x.abs() < 1e-3);
    }

    #[test]
    fn at_the_range_s_spawn_the_view_stands_back_whichever_way_you_look() {
        // A mushroom stands a few metres off: beside its stem, not in its
        // cap, the camera is not pulled into your head.
        let mut room = wandfall::room::Wandfall::practice(0x5eed_0007);
        let w = room.world();
        let at = w.practice.as_ref().expect("a range").spawn;
        let feet = [at[0], w.map.height(at[0], at[1]), at[1]];
        for k in 0..8 {
            let yaw = k as f32 * std::f32::consts::TAU / 8.0;
            let mut c = Chase::default();
            for _ in 0..60 {
                c.view(&w.map, feet, (yaw, 0.0), Stance::default(), 1.6, 1.0 / 60.0);
            }
            assert!(c.arm.x > ARM - 0.05, "looking {k}/8 round: {}", c.arm.x);
        }
    }

    #[test]
    fn on_a_broom_it_stands_back_and_looks_down_on_the_island() {
        let map = Map::new(3);
        let feet = [0.0, 70.0, 0.0];
        let glide = Stance {
            glide: true,
            ..Stance::default()
        };
        let mut c = Chase::default();
        let mut cam = c.view(&map, feet, (0.0, 0.0), glide, 1.6, 1.0 / 60.0);
        for _ in 0..120 {
            cam = c.view(&map, feet, (0.0, 0.0), glide, 1.6, 1.0 / 60.0);
        }
        assert!((cam.pitch + GLIDE_TILT).abs() < 0.01, "{}", cam.pitch);
        assert!(cam.eye[0] < -ARM_GLIDE * 0.9 && cam.eye[1] > feet[1] + HEAD_GLIDE + 2.0);
    }

    #[test]
    fn beside_a_mushroom_s_stem_is_not_in_its_cap() {
        let map = Map::new(3);
        let alone = |q: &&Prop| map.near(q.x, q.z, 8.0).count() == 1;
        let q = map
            .props
            .iter()
            .filter(alone)
            .find(|q| q.kind == Kind::Shroom)
            .expect("a mushroom on its own");
        assert!(!in_crown(&map, [q.x + 1.5, q.y + 2.0, q.z]));
        // Each prop near comes with its own place among the island's (a
        // pine is told from a broadleaf by it).
        assert!(map
            .near_indexed(q.x, q.z, 40.0)
            .all(|(k, p)| std::ptr::eq(p, &map.props[k])));
        let cap = [q.x + 0.3 * q.h, q.y + 1.05 * q.h, q.z];
        assert!(in_crown(&map, cap));
        // And under a broadleaf's crown, beside its trunk, is clear; in it
        // is not.
        let tree = map
            .props
            .iter()
            .enumerate()
            .find(|(k, q)| q.kind == Kind::Tree && !pine(*k) && alone(q))
            .map(|(_, q)| q)
            .expect("a broadleaf on its own");
        assert!(!in_crown(&map, [tree.x + 1.5, tree.y + 2.0, tree.z]));
        assert!(in_crown(
            &map,
            [tree.x + 1.5, tree.y + 4.6 * tree.scale, tree.z]
        ));
    }

    #[test]
    fn it_looks_over_the_shoulder_and_never_inside_the_tower() {
        let map = Map::new(3);
        let mut c = Chase::default();
        // Out on open ground, looking east: behind, above, to the right.
        let open = (0..400)
            .map(|k| ((k % 20) as f32 * 9.0 - 90.0, (k / 20) as f32 * 9.0 - 90.0))
            .find(|&(x, z)| {
                map.land(x, z)
                    && map.near(x, z, 7.0).next().is_none()
                    && map
                        .strikes(
                            [x - 5.0, map.height(x, z) + 2.5, z],
                            [x, map.height(x, z) + 2.0, z],
                        )
                        .is_none()
            })
            .expect("open ground");
        let (x, z) = open;
        let feet = [x, map.height(x, z), z];
        let cam = c.view(&map, feet, (0.0, 0.0), Stance::default(), 1.6, 1.0 / 60.0);
        assert!(
            cam.eye[0] < feet[0] - 1.0 && cam.eye[1] > feet[1] + 1.2,
            "{cam:?}"
        );
        // Backed against the tower, looking away from it: the arm pulls in.
        let r = wandfall::laws::TOWER_RADIUS + wandfall::laws::RADIUS + 0.05;
        let top = wandfall::laws::PLATEAU_TOP;
        let mut c = Chase::default();
        let cam = c.view(
            &map,
            [r, top, 0.0],
            (0.0, 0.0),
            Stance::default(),
            1.6,
            1.0 / 60.0,
        );
        let d = (cam.eye[0].powi(2) + cam.eye[2].powi(2)).sqrt();
        assert!(
            d > wandfall::laws::TOWER_RADIUS,
            "outside the tower: {cam:?}"
        );
    }
}
