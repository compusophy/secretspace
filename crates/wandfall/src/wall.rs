//! Wall jumps: in the air against the side of something standing (a
//! rock, a pillar, a trunk, the tower), a jump kicks you up and off it,
//! keeping some of your speed along it; a few before you land, none of
//! them spending the air jump. Pushing into the wall with the air jump
//! still to spend, the jump is the air jump (to climb it). Part of `motion::step`, so the page
//! predicts it to the bit.

use crate::laws::*;
use crate::motion::Body;

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

/// Whether steering `wish` (`len`: any held) pushes into the wall.
pub fn into(b: &Body, (wx, wz): (f32, f32), len: f32) -> bool {
    len > 1e-6 && wx * b.wall_n[0] + wz * b.wall_n[1] < -0.3
}

/// Up and off the wall.
pub fn kick(b: &mut Body) {
    let n = b.wall_n;
    // What of the speed runs along the wall, kept (some of it).
    let into = b.v[0] * n[0] + b.v[2] * n[1];
    let along = [b.v[0] - n[0] * into, b.v[2] - n[1] * into];
    b.v = [
        along[0] * WALL_KEEP + n[0] * WALL_KICK,
        WALL_JUMP,
        along[1] * WALL_KEEP + n[1] * WALL_KICK,
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
            .find(|q| q.kind == Kind::Pillar && q.h > 3.0)
            .expect("a tall pillar");
        // In the air beside it, a little off the ground, running at it.
        let start = [q.x - q.r - 1.5, q.y + 0.8, q.z];
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
}
