//! How a wizard moves, tick by tick: run and sprint (with momentum: speed
//! above your pace bleeds away, it does not vanish), slide (crouch at
//! speed; gravity carries it down a hill), walk slower up a slope and
//! faster down it, jump, glide down in the drop, wade in the shallows,
//! step up small rises, and never through trees, rocks or pillars.
//! Arithmetic only, so the page predicts its own wizard to the bit.

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
    pub keys: u16,
    /// Spells cast this input (a bit a slot).
    pub cast: u8,
}

pub mod cast {
    pub const SLOT: [u8; 4] = [1, 2, 4, 8];
}

pub mod keys {
    pub const FWD: u16 = 1;
    pub const BACK: u16 = 2;
    pub const LEFT: u16 = 4;
    pub const RIGHT: u16 = 8;
    pub const JUMP: u16 = 16;
    pub const FIRE: u16 = 32;
    /// Aiming down the wand.
    pub const AIM: u16 = 64;
    pub const CROUCH: u16 = 128;
    pub const SPRINT: u16 = 256;
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
    /// Crouching: lower, slower, harder to hit.
    pub crouch: bool,
    /// Ticks a jump still works since the ground was left; ticks an early
    /// jump waits for the ground.
    pub coyote: u8,
    pub buffer: u8,
    /// Sprinting; sliding; ticks before a slide boosts again.
    pub sprint: bool,
    pub slide: bool,
    pub slide_cd: u8,
}

impl Body {
    /// How tall it stands (crouching or not), and where its eyes are.
    pub fn tall(&self) -> f32 {
        if self.crouch {
            CROUCH_HEIGHT
        } else {
            HEIGHT
        }
    }

    pub fn eye(&self) -> f32 {
        if self.crouch {
            CROUCH_EYE
        } else {
            EYE
        }
    }
}

fn toward(v: f32, want: f32, step: f32) -> f32 {
    if v < want {
        (v + step).min(want)
    } else {
        (v - step).max(want)
    }
}

/// The ground's rise a metre east and a metre south at (x, z).
fn slope(map: &Map, x: f32, z: f32) -> (f32, f32) {
    let e = 0.5;
    (
        (map.height(x + e, z) - map.height(x - e, z)) / (2.0 * e),
        (map.height(x, z + e) - map.height(x, z - e)) / (2.0 * e),
    )
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
    // On a hill (not a deck, not the sea floor): which way is up.
    let on_land = b.ground && !wading && map.floor(b.p[0], b.p[2], b.p[1]) <= under + 0.05;
    let (gx, gz) = if on_land {
        slope(map, b.p[0], b.p[2])
    } else {
        (0.0, 0.0)
    };
    let crouch = has(keys::CROUCH) && !b.glide;
    let flat = (b.v[0] * b.v[0] + b.v[2] * b.v[2]).sqrt();
    // A slide: crouching at speed, as it starts or as it lands.
    let fresh = !b.crouch || b.coyote < COYOTE;
    if crouch && fresh && b.ground && !b.slide && flat >= SLIDE_MIN {
        b.slide = true;
        if b.slide_cd == 0 {
            let k = (flat + SLIDE_BOOST) / flat;
            b.v[0] *= k;
            b.v[2] *= k;
        }
        b.slide_cd = SLIDE_COOLDOWN;
    }
    if b.slide && (!crouch || (b.ground && flat < RUN * CROUCH_SLOW)) {
        b.slide = false;
    }
    b.slide_cd = b.slide_cd.saturating_sub(1);
    b.crouch = crouch;
    b.sprint = has(keys::SPRINT)
        && f > 0.0
        && !crouch
        && !has(keys::AIM)
        && !has(keys::FIRE)
        && !b.glide
        && !wading
        && b.chill == 0;
    if b.slide && b.ground {
        // Gravity down the slope; friction; a little steering, no faster.
        let k = GRAVITY * DT / (1.0 + gx * gx + gz * gz);
        b.v[0] -= gx * k;
        b.v[2] -= gz * k;
        let sp = (b.v[0] * b.v[0] + b.v[2] * b.v[2]).sqrt();
        if sp > 1e-4 {
            let keep = (sp - SLIDE_FRICTION * DT).max(0.0);
            let (mut dx, mut dz) = (b.v[0] / sp, b.v[2] / sp);
            if len > 1e-6 {
                dx += wx * SLIDE_STEER * DT;
                dz += wz * SLIDE_STEER * DT;
                let d = (dx * dx + dz * dz).sqrt();
                (dx, dz) = (dx / d, dz / d);
            }
            let keep = keep.min(SLIDE_MAX);
            b.v[0] = dx * keep;
            b.v[2] = dz * keep;
        }
    } else {
        let mut speed = if b.glide {
            GLIDE_SPEED
        } else if wading {
            RUN * WADE
        } else if b.sprint {
            SPRINT
        } else {
            RUN
        };
        if b.chill > 0 {
            speed *= CHILL_SLOW;
        }
        if has(keys::AIM) && !b.glide {
            speed *= AIM_SLOW;
        }
        if b.crouch && b.ground {
            speed *= CROUCH_SLOW;
        }
        // Up a hill slower, down it faster.
        if len > 1e-6 {
            let up = gx * wx + gz * wz;
            speed *= (1.0 - HILL * up).clamp(0.6, 1.35);
        }
        let air = !(b.ground || b.glide);
        let accel = if air {
            ACCEL_AIR
        } else if len > 1e-6 {
            ACCEL_GROUND
        } else {
            BRAKE
        } * DT;
        // Faster than your pace, still steering: the speed holds in the
        // air and bleeds slowly away on the ground (momentum).
        let sp = (b.v[0] * b.v[0] + b.v[2] * b.v[2]).sqrt();
        let along = if sp > 1e-4 {
            (b.v[0] * wx + b.v[2] * wz) / sp
        } else {
            0.0
        };
        if air && len < 1e-6 {
            // Nothing held in the air: it drifts on as it was going.
        } else if sp > speed && len > 1e-6 && along > 0.2 && !b.glide {
            let (mut dx, mut dz) = (b.v[0] / sp, b.v[2] / sp);
            dx += wx * accel / sp;
            dz += wz * accel / sp;
            let d = (dx * dx + dz * dz).sqrt();
            let keep = if air {
                sp
            } else {
                (sp - OVERSPEED * DT).max(speed)
            };
            b.v[0] = dx / d * keep;
            b.v[2] = dz / d * keep;
        } else {
            b.v[0] = toward(b.v[0], wx * speed, accel);
            b.v[2] = toward(b.v[2], wz * speed, accel);
        }
    }
    b.chill = b.chill.saturating_sub(1);
    // A jump pressed a moment early waits for the ground; one a moment
    // after running off an edge still goes.
    b.buffer = if has(keys::JUMP) {
        JUMP_BUFFER
    } else {
        b.buffer.saturating_sub(1)
    };
    b.coyote = if b.ground {
        COYOTE
    } else {
        b.coyote.saturating_sub(1)
    };
    if b.buffer > 0 && (b.ground || b.coyote > 0) && b.v[1] <= 0.5 && !b.glide {
        // Out of a slide, the slide's speed goes with it.
        b.v[1] = JUMP;
        b.ground = false;
        b.slide = false;
        b.coyote = 0;
        b.buffer = 0;
    }
    let pull = if b.v[1] > 0.0 && has(keys::JUMP) {
        GRAVITY_UP
    } else {
        GRAVITY
    };
    b.v[1] -= pull * DT;
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
    map.push_out(&mut p, b.tall());
    // The ground, or a deck under you (the sea floor too: you wade, you
    // do not swim).
    let floor = map.floor(p[0], p[2], b.p[1]).max(SEA - 0.9);
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

    /// Flat open ground (across the Spire's plaza, east of its west
    /// edge, clear of the tower) and a hill to slide down: where each is
    /// (x, z) and which way is downhill.
    fn grounds(map: &Map) -> ([f32; 2], ([f32; 2], [f32; 2])) {
        let flat = [-17.0, -9.0];
        let mut hill = None;
        let g = |x: f32, z: f32| {
            let (gx, gz) = slope(map, x, z);
            (gx, gz, (gx * gx + gz * gz).sqrt())
        };
        for k in 0..6400 {
            let (x, z) = ((k % 80) as f32 * 3.0 - 120.0, (k / 80) as f32 * 3.0 - 120.0);
            if !map.land(x, z) || map.near(x, z, 1.5).next().is_some() || !map.wild(x, z, 2.0) {
                continue;
            }
            let (gx, gz, s) = g(x, z);
            if s > 0.18 && hill.is_none() && map.height(x, z) > SEA + 4.0 {
                hill = Some(([x, z], [-gx / s, -gz / s]));
            }
        }
        (flat, hill.expect("a hill"))
    }

    fn on(map: &Map, at: [f32; 2], v: [f32; 2]) -> Body {
        Body {
            p: [at[0], map.height(at[0], at[1]), at[1]],
            v: [v[0], 0.0, v[1]],
            ground: true,
            coyote: COYOTE,
            ..Body::default()
        }
    }

    fn east(keys: u16) -> Input {
        Input {
            keys,
            yaw: 0,
            ..Input::default()
        }
    }

    #[test]
    fn sprinting_is_faster_and_a_slide_boosts_then_slows() {
        let map = Map::new(6);
        let (flat, _) = grounds(&map);
        let mut b = on(&map, flat, [0.0, 0.0]);
        for _ in 0..30 {
            step(&mut b, &east(keys::FWD | keys::SPRINT), &map);
        }
        assert!(
            b.sprint && (b.v[0] - SPRINT).abs() < 0.6,
            "sprinting: {b:?}"
        );
        // Crouch: a slide, faster at first, slowing on the flat.
        step(&mut b, &east(keys::FWD | keys::SPRINT | keys::CROUCH), &map);
        assert!(b.slide && b.v[0] > SPRINT + 1.0, "boosted: {b:?}");
        let start = b.v[0];
        for _ in 0..20 {
            step(&mut b, &east(keys::FWD | keys::CROUCH), &map);
        }
        assert!(b.slide && b.v[0] < start, "slowing: {b:?}");
        // And it ends, crouched, below a crouch's pace.
        for _ in 0..120 {
            step(&mut b, &east(keys::FWD | keys::CROUCH), &map);
        }
        assert!(!b.slide && b.crouch, "{b:?}");
    }

    #[test]
    fn down_a_hill_a_slide_gathers_speed_and_a_jump_keeps_it() {
        let map = Map::new(6);
        let (_, (hill, down)) = grounds(&map);
        let fast = [down[0] * 9.0, down[1] * 9.0];
        let mut b = on(&map, hill, fast);
        let i = Input {
            keys: keys::CROUCH,
            ..Input::default()
        };
        step(&mut b, &i, &map);
        assert!(b.slide, "{b:?}");
        let mut most: f32 = 0.0;
        for _ in 0..20 {
            step(&mut b, &i, &map);
            most = most.max((b.v[0] * b.v[0] + b.v[2] * b.v[2]).sqrt());
        }
        assert!(most > 9.0 + SLIDE_BOOST + 0.5, "gathering speed: {most}");
        // Jumping out of it: the speed goes along, held in the air.
        let before = (b.v[0] * b.v[0] + b.v[2] * b.v[2]).sqrt();
        let jump = Input {
            keys: keys::JUMP,
            ..Input::default()
        };
        step(&mut b, &jump, &map);
        for _ in 0..8 {
            step(&mut b, &Input::default(), &map);
        }
        let after = (b.v[0] * b.v[0] + b.v[2] * b.v[2]).sqrt();
        assert!(
            !b.ground && after > before - 1.5,
            "kept: {before} -> {after}"
        );
    }

    #[test]
    fn momentum_bleeds_away_it_does_not_vanish() {
        let map = Map::new(6);
        let (flat, _) = grounds(&map);
        // Running east at 14 m/s (out of a slide, say), holding forward.
        let mut b = on(&map, flat, [14.0, 0.0]);
        step(&mut b, &east(keys::FWD), &map);
        assert!(b.v[0] > 13.5, "not snapped back to a run: {b:?}");
        for _ in 0..60 {
            step(&mut b, &east(keys::FWD), &map);
        }
        assert!(
            b.v[0] < 14.0 - 3.0 && b.v[0] >= RUN - 0.01,
            "bled toward a run: {b:?}"
        );
    }

    #[test]
    fn walks_up_the_spire_to_its_balcony() {
        let map = Map::new(4);
        let r = TOWER_RADIUS + STAIR_WIDTH / 2.0;
        let a0 = STAIR_FROM + 0.05;
        let mut b = Body {
            p: [a0.cos() * r, PLATEAU_TOP, a0.sin() * r],
            ground: true,
            ..Body::default()
        };
        let top = map.height(0.0, 0.0) - 0.3 + BALCONY;
        for _ in 0..40 * TICK_HZ {
            // Facing along the stair, leaning in to keep to it.
            let a = b.p[2].atan2(b.p[0]);
            let d = (b.p[0] * b.p[0] + b.p[2] * b.p[2]).sqrt();
            let yaw = a + std::f32::consts::FRAC_PI_2 + (d - r) * 0.3;
            let i = Input {
                keys: keys::FWD,
                yaw: crate::trig::heading(yaw),
                ..Input::default()
            };
            step(&mut b, &i, &map);
            if b.p[1] >= top - 0.01 {
                break;
            }
        }
        assert!(b.ground, "standing");
        assert!(
            (b.p[1] - top).abs() < 0.05,
            "on the balcony: {:?} of {top}",
            b.p
        );
    }

    #[test]
    fn crouching_is_lower_and_slower_and_jumps_forgive() {
        let map = Map::new(11);
        let mut walk = stand(&map);
        let mut low = walk;
        let start = walk.p;
        let go = |k| Input {
            keys: k,
            ..Input::default()
        };
        for _ in 0..30 {
            step(&mut walk, &go(keys::FWD), &map);
            step(&mut low, &go(keys::FWD | keys::CROUCH), &map);
        }
        assert!(low.crouch && low.tall() < walk.tall() && low.eye() < walk.eye());
        assert!(low.p[0] - start[0] < (walk.p[0] - start[0]) * 0.7, "slower");
        // Held, a jump rises higher than tapped.
        let top = |hold: bool| {
            let mut b = stand(&map);
            step(&mut b, &go(keys::JUMP), &map);
            let mut best = b.p[1];
            for _ in 0..40 {
                step(&mut b, &go(if hold { keys::JUMP } else { 0 }), &map);
                best = best.max(b.p[1]);
                if b.ground {
                    break;
                }
            }
            best - stand(&map).p[1]
        };
        assert!(top(true) > top(false) * 1.2, "{} {}", top(true), top(false));
        // Pressed a moment before landing, it still jumps on landing.
        let mut b = stand(&map);
        b.p[1] += 0.15;
        b.ground = false;
        step(&mut b, &go(keys::JUMP), &map);
        let mut jumped = false;
        for _ in 0..JUMP_BUFFER {
            step(&mut b, &go(0), &map);
            jumped |= b.v[1] > 1.0;
        }
        assert!(jumped, "the early press counted");
        // Just off an edge (in the air, no longer on the ground), it still
        // jumps.
        let mut b = stand(&map);
        step(&mut b, &go(0), &map);
        b.ground = false;
        b.v[1] = 0.0;
        step(&mut b, &go(keys::JUMP), &map);
        assert!(b.v[1] > 1.0, "coyote time");
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
