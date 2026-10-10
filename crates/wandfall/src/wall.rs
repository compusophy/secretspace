//! Wall jumps: in the air against the side of something standing (a
//! rock, a pillar, a trunk, the tower), a jump kicks you up and off it,
//! turned the way you steer, keeping most of your speed along it; a few
//! before you land, none of them spending the air jump. Pushing into the
//! wall you slide down it slowly, lining up the kick; with the air jump
//! still to spend, the jump is the air jump (to climb it). Part of
//! `motion::step`, so the page predicts it to the bit.

use crate::laws::*;
use crate::motion::{Body, Wish};

/// Pushed out of something's side by `by` (x, z) in the air: touching
/// it, for a moment.
pub fn touch(b: &mut Body, by: [f32; 2]) {
    let d = (by[0] * by[0] + by[1] * by[1]).sqrt();
    if d > 1e-4 {
        b.wall = WALL_GRACE;
        b.wall_n = [by[0] / d, by[1] / d];
    }
}

/// Whether a jump now kicks off a wall.
pub fn can(b: &Body) -> bool {
    !b.ground && !b.glide && b.wall > 0 && b.walls < WALL_JUMPS && b.mantle == 0 && b.tether == 0
}

/// Whether steering `w` pushes into the wall.
pub fn into(b: &Body, w: &Wish) -> bool {
    w.any && w.x * b.wall_n[0] + w.z * b.wall_n[1] < -0.3
}

/// Up and off the wall, out from it turned toward `w` (never back into
/// it, nor so far round it is barely off it).
pub fn kick(b: &mut Body, w: &Wish) {
    let n = b.wall_n;
    // What of the speed runs along the wall, kept (most of it).
    let into = b.v[0] * n[0] + b.v[2] * n[1];
    let along = [b.v[0] - n[0] * into, b.v[2] - n[1] * into];
    let mut out = n;
    if w.any {
        let o = [n[0] + w.x * WALL_AIM, n[1] + w.z * WALL_AIM];
        let d = (o[0] * o[0] + o[1] * o[1]).sqrt();
        if d > 1e-4 && (o[0] * n[0] + o[1] * n[1]) / d >= 0.25 {
            out = [o[0] / d, o[1] / d];
        }
    }
    b.v = [
        along[0] * WALL_KEEP + out[0] * WALL_KICK,
        WALL_JUMP,
        along[1] * WALL_KEEP + out[1] * WALL_KICK,
    ];
    b.walls += 1;
    b.wall = 0;
}

#[cfg(test)]
mod tests {
    use crate::laws::*;
    use crate::map::{Kind, Map};
    use crate::motion::{keys, step, Body, Input};

    #[test]
    fn a_jump_against_a_pillar_kicks_off_it_a_few_times_at_most() {
        let map = Map::new(11);
        let q = *map
            .props
            .iter()
            .find(|q| q.kind == Kind::Pillar && q.h > 4.0)
            .expect("a tall pillar");
        // In the air beside it, well off the ground, running at it.
        let start = [q.x - q.r - 1.5, q.y + 2.0, q.z];
        let mut b = Body {
            p: start,
            v: [4.0, 2.0, 0.0],
            ..Body::default()
        };
        let at_it = Input {
            yaw: 0,
            keys: keys::FWD,
            ..Input::default()
        };
        for _ in 0..12 {
            step(&mut b, &at_it, &map);
            if b.wall > 0 {
                break;
            }
        }
        assert!(b.wall > 0 && b.wall_n[0] < -0.9, "touching it: {b:?}");
        // Pushing into it, the air jump to spend: that is the climb's.
        let mut climb = b;
        let up = Input {
            keys: keys::JUMP | keys::FWD,
            ..at_it
        };
        step(&mut climb, &up, &map);
        assert!(climb.air_jumped && climb.walls == 0, "{climb:?}");
        // Letting go of it: off the wall.
        let jump = Input {
            keys: keys::JUMP,
            ..at_it
        };
        step(&mut b, &jump, &map);
        assert_eq!(b.walls, 1);
        assert!(!b.air_jumped, "the air jump is kept");
        assert!(
            b.v[0] < -WALL_KICK * 0.5 && b.v[1] > 0.0,
            "off and up: {b:?}"
        );
        // Out of kicks: the press is the air jump instead.
        b.walls = WALL_JUMPS;
        b.wall = WALL_GRACE;
        step(&mut b, &at_it, &map);
        step(&mut b, &jump, &map);
        assert_eq!(b.walls, WALL_JUMPS);
        assert!(b.air_jumped);
    }

    #[test]
    fn a_kick_goes_where_you_steer_and_pushing_in_you_slide_down_slowly() {
        let map = Map::new(11);
        let q = *map
            .props
            .iter()
            .find(|q| q.kind == Kind::Pillar && q.h > 4.0)
            .expect("a tall pillar");
        // Against its west side, high up, falling, pushing into it.
        let mut b = Body {
            p: [q.x - q.r - 1.0, q.y + 3.5, q.z],
            v: [4.0, 0.0, 0.0],
            ..Body::default()
        };
        let at_it = Input {
            keys: keys::FWD,
            ..Input::default()
        };
        for _ in 0..20 {
            step(&mut b, &at_it, &map);
        }
        assert!(b.wall > 0 && !b.ground, "on the wall: {b:?}");
        assert!(b.v[1] >= -WALL_SLIDE_FALL, "sliding down slowly: {b:?}");
        // Kicked off steering left (north, -z) or right: off that way.
        b.air_jumped = true;
        for (k, way) in [(keys::LEFT, -1.0), (keys::RIGHT, 1.0)] {
            let mut c = b;
            let i = Input {
                keys: keys::JUMP | k,
                ..Input::default()
            };
            step(&mut c, &i, &map);
            assert!(c.walls == 1 && c.v[0] < -2.0, "off it: {c:?}");
            assert!(c.v[2] * way > 2.0, "the way it steered: {c:?}");
        }
    }
}
