//! How a wizard stands (crouched, sliding, sprinting), what sprinting
//! spends, and how it steers: on the ground, sliding, in the air, on the
//! broom, climbing or hauled by a Tether. Speed over its pace is
//! momentum: held in the air, bled away on the ground, dragged off by
//! the sea and by Frost's chill.

use super::{flat, keys, Body, Input, Under, Wish};
use crate::laws::*;
use crate::map::smooth;
use crate::trig;

/// A velocity across the ground toward `want`, by at most `step` (the
/// same whichever way it faces).
fn toward(v: &mut [f32; 3], want: [f32; 2], step: f32) {
    let (dx, dz) = (want[0] - v[0], want[1] - v[2]);
    let d = (dx * dx + dz * dz).sqrt();
    if d <= step {
        (v[0], v[2]) = (want[0], want[1]);
    } else {
        v[0] += dx / d * step;
        v[2] += dz / d * step;
    }
}

/// A velocity across the ground going `sp`, made to go `to` instead.
fn scale(v: &mut [f32; 3], sp: f32, to: f32) {
    if sp > 1e-4 {
        v[0] *= to / sp;
        v[2] *= to / sp;
    }
}

/// How steeply it looks down, 0 (level, or up) to 1 (straight down): how
/// hard the broom dives.
pub(super) fn dive(i: &Input) -> f32 {
    (-trig::pitch_sin_cos(i.pitch).0).max(0.0)
}

/// Crouching, sliding (crouching at speed, as it starts or as it lands,
/// not in the sea), sprinting.
pub(super) fn stance(b: &mut Body, i: &Input, w: &Wish, u: &Under) {
    let crouch = i.has(keys::CROUCH) && !b.glide;
    let flat = flat(&b.v);
    let fresh = !b.crouch || b.coyote < COYOTE;
    if crouch && fresh && b.ground && !u.wading && !b.slide && flat >= SLIDE_MIN {
        b.slide = true;
        // A boost, once in a while, from no faster than a timed hop; not
        // chilled.
        if b.slide_cd == 0 && b.chill == 0 && flat < HOP_MAX {
            let k = (flat + SLIDE_BOOST).min(SLIDE_BOOST_TO) / flat;
            b.v[0] *= k;
            b.v[2] *= k;
            b.slide_cd = SLIDE_COOLDOWN;
        }
    }
    if b.slide && (!crouch || u.wading || (b.ground && flat < RUN * CROUCH_SLOW)) {
        b.slide = false;
    }
    b.slide_cd = b.slide_cd.saturating_sub(1);
    b.crouch = crouch;
    b.sprint = i.has(keys::SPRINT)
        && w.forward
        && !crouch
        && !i.has(keys::AIM)
        && !i.has(keys::FIRE)
        && !b.glide
        && !u.wading
        && b.chill == 0
        && !b.winded;
}

/// Sprinting spends stamina (on the ground: in the air it is neither
/// spent nor got back); a breath after, it comes back.
pub(super) fn stamina(b: &mut Body) {
    if b.sprint {
        if b.ground {
            b.spent = (b.spent + STAMINA_SPEND).min(STAMINA);
            b.breath = 0;
            b.winded = b.spent >= STAMINA;
        }
    } else {
        b.breath = b.breath.saturating_add(1);
        if b.breath >= STAMINA_BREATH {
            b.spent = b.spent.saturating_sub(STAMINA_BACK);
        }
        if b.winded && b.spent <= WINDED_UNTIL {
            b.winded = false;
        }
    }
}

/// Toward where it steers.
pub(super) fn steer(b: &mut Body, i: &Input, w: &Wish, u: &Under) {
    if b.mantle > 0 {
        // Climbing a ledge: carried over it, not steered.
    } else if b.tether > 0 {
        crate::tether::pull(b, w);
    } else if b.glide {
        glide(b, i, w);
    } else {
        if b.slide && b.ground {
            slide(b, w, u);
        } else {
            run(b, i, w, u);
        }
        chilled(b);
    }
}

/// Chilled, whatever speed it has over a chilled run drains away, on the
/// ground or in the air: Frost slows the hopper as it does the runner.
fn chilled(b: &mut Body) {
    let (sp, pace) = (flat(&b.v), RUN * CHILL_SLOW);
    if b.chill > 0 && sp > pace {
        scale(&mut b.v, sp, (sp - CHILL_DRAG * DT).max(pace));
    }
}

/// Gravity down the slope; friction (more, come in over its top speed);
/// a little steering.
fn slide(b: &mut Body, w: &Wish, u: &Under) {
    let (gx, gz) = (u.gx, u.gz);
    let k = GRAVITY * DT / (1.0 + gx * gx + gz * gz);
    b.v[0] -= gx * k;
    b.v[2] -= gz * k;
    let sp = flat(&b.v);
    if sp > 1e-4 {
        let friction = if sp > SLIDE_MAX {
            SLIDE_OVER
        } else {
            SLIDE_FRICTION
        };
        let keep = (sp - friction * DT).max(0.0);
        let (mut dx, mut dz) = (b.v[0] / sp, b.v[2] / sp);
        if w.any {
            dx += w.x * SLIDE_STEER * DT;
            dz += w.z * SLIDE_STEER * DT;
            let d = (dx * dx + dz * dz).sqrt();
            (dx, dz) = (dx / d, dz / d);
        }
        b.v[0] = dx * keep;
        b.v[2] = dz * keep;
    }
}

/// On the broom: steered toward its pace (faster diving), drifting on
/// when let go, speed over its pace bleeding slowly away.
fn glide(b: &mut Body, i: &Input, w: &Wish) {
    let mut speed = GLIDE_SPEED * (1.0 + GLIDE_DIVE_FAST * dive(i));
    if b.chill > 0 {
        speed *= CHILL_SLOW;
    }
    let sp = flat(&b.v);
    if !w.any {
        scale(&mut b.v, sp, (sp - GLIDE_DRAG * DT).max(0.0));
    } else if sp > speed {
        let (dx, dz) = (
            b.v[0] / sp + w.x * GLIDE_ACCEL * DT / sp,
            b.v[2] / sp + w.z * GLIDE_ACCEL * DT / sp,
        );
        let d = (dx * dx + dz * dz).sqrt().max(1e-6);
        let keep = (sp - GLIDE_DRAG * DT).max(speed);
        b.v[0] = dx / d * keep;
        b.v[2] = dz / d * keep;
    } else {
        toward(&mut b.v, [w.x * speed, w.z * speed], GLIDE_ACCEL * DT);
    }
}

/// Running, and in the air.
fn run(b: &mut Body, i: &Input, w: &Wish, u: &Under) {
    let mut speed = if u.wading {
        RUN * WADE
    } else if b.sprint {
        SPRINT
    } else {
        RUN
    };
    if b.chill > 0 {
        speed *= CHILL_SLOW;
    }
    if i.has(keys::AIM) {
        speed *= AIM_SLOW;
    }
    if b.crouch && b.ground {
        speed *= CROUCH_SLOW;
    }
    // Up a hill slower, down it faster.
    if w.any {
        let up = u.gx * w.x + u.gz * w.z;
        speed *= (1.0 - HILL * up).clamp(0.6, 1.35);
    }
    let air = !b.ground;
    let accel = if air {
        ACCEL_AIR
    } else if w.any {
        ACCEL_GROUND
    } else {
        BRAKE
    } * DT;
    // Faster than your pace, still steering more or less its way: the
    // speed holds in the air (steering curves it) and bleeds away on the
    // ground, slowly while you steer along it, quicker as you turn; the
    // sea drags at every step (momentum).
    let sp = flat(&b.v);
    let along = if sp > 1e-4 {
        (b.v[0] * w.x + b.v[2] * w.z) / sp
    } else {
        0.0
    };
    let (on, off) = MOMENTUM_ALONG;
    if air && !w.any {
        // Nothing held in the air: it drifts on as it was going.
    } else if sp > speed && w.any && along > if air { AIR_ALONG } else { off } {
        let (mut dx, mut dz) = (b.v[0] / sp, b.v[2] / sp);
        dx += w.x * accel / sp;
        dz += w.z * accel / sp;
        let d = (dx * dx + dz * dz).sqrt();
        let keep = if air {
            sp
        } else if u.wading {
            speed + (sp - speed) * WADE_KEEP
        } else {
            let bleed = OVERSPEED + (ACCEL_GROUND - OVERSPEED) * smooth((on - along) / (on - off));
            (sp - bleed * DT).max(speed)
        };
        b.v[0] = dx / d * keep;
        b.v[2] = dz / d * keep;
    } else {
        toward(&mut b.v, [w.x * speed, w.z * speed], accel);
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{go, lap, plaza};
    use super::super::{flat, keys, Body};
    use crate::laws::*;
    use crate::map::Map;

    #[test]
    fn it_speeds_up_and_stops_alike_whichever_way_it_faces() {
        let map = Map::new(3);
        let ticks = |yaw: u16| {
            let mut b = plaza(&map);
            let mut up = 0;
            while flat(&b.v) < RUN * 0.95 {
                go(&mut b, keys::FWD, yaw, &map);
                up += 1;
            }
            let mut stop = 0;
            while flat(&b.v) > 0.05 {
                go(&mut b, 0, yaw, &map);
                stop += 1;
            }
            (up, stop)
        };
        // East, a sixteenth of a turn round, an eighth.
        let all = [0, 4096, 8192].map(ticks);
        assert!(all.iter().all(|&t| t == all[0]), "{all:?}");
    }

    #[test]
    fn steering_square_to_it_in_the_air_curves_it_and_keeps_its_speed() {
        let map = Map::new(3);
        let mut b = plaza(&map);
        b.p[1] += 30.0;
        b.ground = false;
        b.v = [12.5, 0.0, 0.0];
        for _ in 0..10 {
            go(&mut b, keys::RIGHT, 0, &map);
        }
        assert!(flat(&b.v) > 12.4 && b.v[2] > 1.0, "curved, kept: {b:?}");
        // Pulling back still brakes it.
        for _ in 0..10 {
            go(&mut b, keys::BACK, 0, &map);
        }
        assert!(flat(&b.v) < 11.0, "{b:?}");
    }

    /// Hopping east across the plaza from `v`, each hop on its landing,
    /// chilled for `chill` ticks: its speed after `ticks`.
    fn hopping(map: &Map, v: f32, chill: u16, ticks: u32) -> f32 {
        let mut b = plaza(map);
        b.v[0] = v;
        b.chill = chill;
        for _ in 0..ticks {
            let k = keys::FWD | if b.ground { keys::JUMP } else { 0 };
            go(&mut b, k, 0, map);
            lap(&mut b);
        }
        flat(&b.v)
    }

    #[test]
    fn chilled_a_hopper_slows_as_a_runner_does() {
        let map = Map::new(3);
        assert!(hopping(&map, 12.0, 0, 15) > 11.5, "unchilled it keeps on");
        let cold = hopping(&map, 12.0, CHILL_TICKS, 15);
        assert!(
            cold < RUN * CHILL_SLOW + 0.1,
            "chilled, half a second on: {cold}"
        );
    }

    #[test]
    fn slides_and_hops_in_turn_hold_their_speed_on_the_flat_not_run_away() {
        let map = Map::new(3);
        // Sliding so many ticks each landing, then hopping.
        let rhythm = |n: u32| {
            let mut b = plaza(&map);
            for _ in 0..30 {
                go(&mut b, keys::FWD | keys::SPRINT, 0, &map);
                lap(&mut b);
            }
            let mut on = 0;
            for _ in 0..8 * TICK_HZ {
                let mut k = keys::FWD;
                if b.ground {
                    on += 1;
                    k |= if on <= n { keys::CROUCH } else { keys::JUMP };
                    on = if on > n { 0 } else { on };
                }
                go(&mut b, k, 0, &map);
                lap(&mut b);
            }
            flat(&b.v)
        };
        for n in [6, 8, 14, 16] {
            let v = rhythm(n);
            assert!(v > SPRINT && v < SLIDE_BOOST_TO, "{n} ticks a slide: {v}");
        }
        // A slide within the cooldown of the last boost is not boosted,
        // and does not put the next one off.
        let mut b = plaza(&map);
        for _ in 0..30 {
            go(&mut b, keys::FWD | keys::SPRINT, 0, &map);
            lap(&mut b);
        }
        go(&mut b, keys::FWD | keys::CROUCH, 0, &map);
        assert!(b.slide && b.slide_cd > 0, "boosted: {b:?}");
        go(&mut b, keys::FWD, 0, &map);
        let before = flat(&b.v);
        go(&mut b, keys::FWD | keys::CROUCH, 0, &map);
        assert!(b.slide && flat(&b.v) < before, "not boosted again: {b:?}");
        let cd = b.slide_cd;
        go(&mut b, keys::FWD, 0, &map);
        assert_eq!(b.slide_cd, cd - 1, "the cooldown runs on");
    }

    #[test]
    fn the_sea_drags_at_hops_and_slides() {
        let map = Map::new(3);
        // Out in the shallows, east of the island's west shore.
        let z = 0.0;
        let x = (0..200)
            .map(|k| -SHORE - 30.0 + k as f32)
            .find(|&x| map.height(x, z) < SEA - 0.5 && map.height(x + 25.0, z) < SEA - 0.5)
            .expect("the sea");
        let wet = |keys: u16, v: f32| {
            let mut b = Body {
                p: [x, map.floor(x, z, SEA).max(SEA - 0.9), z],
                v: [v, 0.0, 0.0],
                ground: true,
                ..Body::default()
            };
            for _ in 0..TICK_HZ {
                let hop = if b.ground { keys & keys::JUMP } else { 0 };
                go(&mut b, (keys & !keys::JUMP) | hop, 0, &map);
            }
            b
        };
        let hops = wet(keys::FWD | keys::JUMP, 12.5);
        assert!(flat(&hops.v) < RUN + 0.5, "hopping in the sea: {hops:?}");
        let slid = wet(keys::FWD | keys::CROUCH, 10.0);
        assert!(
            !slid.slide && flat(&slid.v) < RUN,
            "no slide in the sea: {slid:?}"
        );
    }
}
