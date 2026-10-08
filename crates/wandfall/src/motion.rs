//! How a wizard moves, tick by tick: run, jump, glide down in the drop,
//! wade in the shallows, step up small rises, and never through trees,
//! rocks or pillars. Arithmetic only, so the page predicts its own wizard
//! to the bit.

use crate::laws::*;
use crate::map::Map;
use crate::trig;

/// What a page says each tick: which input this is, where it looks, and
/// the keys held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Input {
    pub seq: u16,
    pub yaw: u16,
    pub pitch: i16,
    pub keys: u8,
    /// Spells cast this input (a bit a slot) and taking a scroll.
    pub cast: u8,
}

pub mod cast {
    pub const SLOT: [u8; 4] = [1, 2, 4, 8];
    pub const TAKE: u8 = 16;
}

pub mod keys {
    pub const FWD: u8 = 1;
    pub const BACK: u8 = 2;
    pub const LEFT: u8 = 4;
    pub const RIGHT: u8 = 8;
    pub const JUMP: u8 = 16;
    pub const FIRE: u8 = 32;
    /// Aiming down the wand.
    pub const AIM: u8 = 64;
}

/// A wizard's body: feet, velocity, and whether it stands.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    pub p: [f32; 3],
    pub v: [f32; 3],
    pub ground: bool,
    /// Falling slowly from the drop until it lands.
    pub glide: bool,
    /// Ticks left chilled (by Frost): slower.
    pub chill: u16,
}

fn toward(v: f32, want: f32, step: f32) -> f32 {
    if v < want {
        (v + step).min(want)
    } else {
        (v - step).max(want)
    }
}

/// One tick of a body under an input.
pub fn step(b: &mut Body, i: &Input, map: &Map) {
    let (s, c) = trig::sin_cos(i.yaw);
    let has = |k| i.keys & k != 0;
    let f = has(keys::FWD) as i32 as f32 - has(keys::BACK) as i32 as f32;
    let r = has(keys::RIGHT) as i32 as f32 - has(keys::LEFT) as i32 as f32;
    // Forward is (c, s) on the ground; right is (-s, c).
    let mut wx = c * f - s * r;
    let mut wz = s * f + c * r;
    let len = (wx * wx + wz * wz).sqrt();
    if len > 1e-6 {
        wx /= len;
        wz /= len;
    }
    let under = map.height(b.p[0], b.p[2]);
    let wading = b.ground && under < SEA;
    let mut speed = if b.glide {
        GLIDE_SPEED
    } else if wading {
        RUN * WADE
    } else {
        RUN
    };
    if b.chill > 0 {
        speed *= CHILL_SLOW;
    }
    if i.keys & keys::AIM != 0 && !b.glide {
        speed *= AIM_SLOW;
    }
    b.chill = b.chill.saturating_sub(1);
    let accel = if b.ground || b.glide {
        ACCEL_GROUND
    } else {
        ACCEL_AIR
    } * DT;
    b.v[0] = toward(b.v[0], wx * speed, accel);
    b.v[2] = toward(b.v[2], wz * speed, accel);
    if b.ground && has(keys::JUMP) {
        b.v[1] = JUMP;
        b.ground = false;
    }
    b.v[1] -= GRAVITY * DT;
    if b.glide && b.v[1] < -GLIDE_FALL {
        b.v[1] = -GLIDE_FALL;
    }
    let was = b.ground;
    let mut p = [
        b.p[0] + b.v[0] * DT,
        b.p[1] + b.v[1] * DT,
        b.p[2] + b.v[2] * DT,
    ];
    let lim = MAP_HALF - 1.0;
    p[0] = p[0].clamp(-lim, lim);
    p[2] = p[2].clamp(-lim, lim);
    map.push_out(&mut p);
    // The ground (the sea floor too: you wade, you do not swim).
    let floor = map.height(p[0], p[2]).max(SEA - 0.9);
    if p[1] <= floor || (was && b.v[1] <= 0.0 && p[1] - floor < STEP) {
        p[1] = floor;
        b.v[1] = 0.0;
        b.ground = true;
        b.glide = false;
    } else {
        b.ground = false;
    }
    b.p = p;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stand(map: &Map) -> Body {
        let [x, z] = map.spot(&mut engine::rng::Rng::new(5));
        Body {
            p: [x, map.height(x, z), z],
            ground: true,
            ..Body::default()
        }
    }

    #[test]
    fn runs_where_it_looks_and_lands_from_a_jump() {
        let map = Map::new(11);
        let mut b = stand(&map);
        let start = b.p;
        let run = Input {
            keys: keys::FWD,
            ..Input::default()
        };
        let mut aim = b;
        let mut cold = b;
        cold.chill = 30;
        for _ in 0..30 {
            step(&mut b, &run, &map);
        }
        let moved = b.p[0] - start[0];
        assert!(moved > 3.0, "east, about RUN a second: {moved}");
        let aiming = Input {
            keys: keys::FWD | keys::AIM,
            ..Input::default()
        };
        for _ in 0..30 {
            step(&mut aim, &aiming, &map);
            step(&mut cold, &run, &map);
        }
        assert!(aim.p[0] - start[0] < moved * 0.8, "aiming is slower");
        assert!(cold.p[0] - start[0] < moved * 0.8, "chilled is slower");
        assert_eq!(cold.chill, 0, "and it wears off");
        let jump = Input {
            keys: keys::JUMP,
            ..Input::default()
        };
        step(&mut b, &jump, &map);
        assert!(!b.ground && b.v[1] > 0.0);
        for _ in 0..60 {
            step(&mut b, &Input::default(), &map);
        }
        assert!(b.ground, "down again");
    }

    #[test]
    fn the_drop_glides_down_slowly() {
        let map = Map::new(11);
        let mut b = stand(&map);
        b.p[1] += DROP_HEIGHT;
        b.ground = false;
        b.glide = true;
        let mut ticks = 0;
        while !b.ground && ticks < 2000 {
            step(&mut b, &Input::default(), &map);
            ticks += 1;
        }
        let secs = ticks as f32 / TICK_HZ as f32;
        assert!(
            secs > DROP_HEIGHT / GLIDE_FALL * 0.8,
            "it takes a while: {secs}"
        );
        assert!(b.ground && !b.glide);
    }
}
