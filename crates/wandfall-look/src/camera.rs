//! The camera over a wizard's shoulder (third person): a spring arm from a
//! pivot above its right shoulder, back along where you look; pulled in
//! at once when a wall stands between (the ground, the tower, a stone)
//! and let out again gently; eased in out of a tree's crown; slid round a
//! trunk or a post rather than pulled in by it (a trunk passing behind a
//! running wizard would jerk the view in and out); nearer and tighter
//! when aiming, lower crouched, further out on a broom. And what the
//! crosshair at the middle of the screen is on, so a wizard aims there
//! from its own eyes (the shoulder's offset taken out).

use render::geo::{self, V3};
use render::Camera;
use wandfall::laws::spell;
use wandfall::map::{Kind, Map, Prop};
use wandfall::proto::{Ev, Seen};

use crate::fx;
use crate::rig::Spring;

/// The arm's length (metres): walking, aiming, on a broom.
const ARM: f32 = 3.4;
const ARM_AIM: f32 = 1.7;
const ARM_GLIDE: f32 = 5.6;
/// The pivot: how far right of the head, and how high above the feet
/// (standing, crouched, on a broom).
const SHOULDER: f32 = 0.6;
const HEAD: f32 = 1.65;
const HEAD_CROUCH: f32 = 1.2;
const HEAD_GLIDE: f32 = 1.9;
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
        let (arm, shoulder, head, fov) = match stance {
            Stance { glide: true, .. } => (ARM_GLIDE, 0.0, HEAD_GLIDE, FOV),
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
        if !self.started {
            self.arm.x = arm;
            self.shoulder.x = shoulder;
            self.head.x = head;
            self.fov.x = fov;
            self.rise.x = feet[1];
            self.started = true;
        }
        let shoulder = self.shoulder.step(shoulder, 0.12, dt);
        let head = self.head.step(head, 0.1, dt);
        let fov = self
            .fov
            .step(fov + if stance.fast { 0.1 } else { 0.0 }, 0.12, dt);
        // The feet's height eased a touch, so steps and landings do not
        // jolt the view.
        let y = self.rise.step(feet[1], 0.05, dt);
        let (fwd, right) = axes(yaw, pitch);
        let top = [feet[0], y + head, feet[2]];
        let wall = |q: &Prop| q.r >= THIN;
        // The shoulder's offset, cut short by a wall beside the head.
        let side = geo::add(top, geo::scale(right, shoulder));
        let pivot = match map.strikes_if(top, side, wall) {
            Some(t) => geo::add(top, geo::scale(right, (shoulder * t - SKIN).max(0.0))),
            None => side,
        };
        // The arm, cut short at once by a wall behind; let out gently.
        let back = geo::sub(pivot, geo::scale(fwd, arm + SKIN));
        let room = map
            .strikes_if(pivot, back, wall)
            .map_or(arm, |t| ((arm + SKIN) * t - SKIN).max(0.35))
            .min(arm);
        // Nor inside a tree's crown: eased in till clear (leaves, not
        // walls, so a moment among them does no harm).
        let mut want = room;
        while want > 0.7 && in_crown(map, geo::sub(pivot, geo::scale(fwd, want))) {
            want -= 0.25;
        }
        if room < self.arm.x {
            self.arm = Spring { x: room, v: 0.0 };
        }
        let half = if want < self.arm.x { 0.08 } else { 0.25 };
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

/// Whether `p` is among a tree's or a mushroom's leaves or cap.
fn in_crown(map: &Map, p: V3) -> bool {
    map.near(p[0], p[2], 2.0).any(|q| {
        matches!(q.kind, Kind::Tree | Kind::Shroom)
            && p[1] > q.y + q.h * 0.3
            && p[1] < q.y + q.h * 1.05
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
        for k in 0..(fps as usize * 6) {
            let x = x0 + v * k as f32 / fps;
            let feet = [x, map.floor(x, z, map.height(x, z) + 1.0), z];
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
