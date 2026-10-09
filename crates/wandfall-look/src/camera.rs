//! The camera over a wizard's shoulder (third person): a spring arm from a
//! pivot above its right shoulder, back along where you look; pulled in
//! at once when something stands between (never inside a tree, a stone,
//! the tower or the ground) and let out again gently; nearer and tighter
//! when aiming, lower crouched, further out on a broom. And what the
//! crosshair at the middle of the screen is on, so a wizard aims there
//! from its own eyes (the shoulder's offset taken out).

use render::geo::{self, V3};
use render::Camera;
use wandfall::map::Map;
use wandfall::proto::Seen;

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
/// Fields of view (radians, up and down): walking, aiming.
pub const FOV: f32 = 1.15;
pub const FOV_AIM: f32 = 0.78;

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
        // The shoulder's offset, cut short by anything beside the head.
        let side = geo::add(top, geo::scale(right, shoulder));
        let pivot = match map.strikes(top, side) {
            Some(t) => geo::add(top, geo::scale(right, (shoulder * t - SKIN).max(0.0))),
            None => side,
        };
        // The arm, cut short at once by anything behind; let out gently.
        let back = geo::sub(pivot, geo::scale(fwd, arm + SKIN));
        let room = map
            .strikes(pivot, back)
            .map_or(arm, |t| ((arm + SKIN) * t - SKIN).max(0.35));
        // Nor inside a tree's crown: in a step at a time till clear.
        let mut want = room.min(arm);
        while want > 0.7 && in_crown(map, geo::sub(pivot, geo::scale(fwd, want))) {
            want -= 0.25;
        }
        if want < self.arm.x {
            self.arm = Spring { x: want, v: 0.0 };
        } else {
            self.arm.step(want, 0.25, dt);
        }
        let lift = LIFT * (self.arm.x / arm).min(1.0);
        let eye = geo::add(
            geo::sub(pivot, geo::scale(fwd, self.arm.x)),
            [0.0, lift, 0.0],
        );
        Camera {
            eye,
            yaw,
            pitch,
            fov,
            aspect,
        }
    }
}

/// Whether `p` is among a tree's or a mushroom's leaves or cap.
fn in_crown(map: &Map, p: V3) -> bool {
    use wandfall::map::Kind;
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
