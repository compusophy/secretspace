//! How a wizard stands (crouched, sliding, sprinting), what sprinting
//! spends, and how it steers: on the ground, sliding, in the air, on the
//! broom, climbing or hauled by a Tether.

use super::{flat, keys, Body, Input, Under, Wish};
use crate::laws::*;
use crate::map::smooth;

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

/// Crouching, sliding (crouching at speed, as it starts or as it lands),
/// sprinting.
pub(super) fn stance(b: &mut Body, i: &Input, w: &Wish, u: &Under) {
    let crouch = i.has(keys::CROUCH) && !b.glide;
    let flat = flat(&b.v);
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
        crate::tether::pull(b);
    } else if b.slide && b.ground {
        slide(b, w, u);
    } else {
        run(b, i, w, u);
    }
}

/// Gravity down the slope; friction; a little steering, no faster.
fn slide(b: &mut Body, w: &Wish, u: &Under) {
    let (gx, gz) = (u.gx, u.gz);
    let k = GRAVITY * DT / (1.0 + gx * gx + gz * gz);
    b.v[0] -= gx * k;
    b.v[2] -= gz * k;
    let sp = flat(&b.v);
    if sp > 1e-4 {
        let keep = (sp - SLIDE_FRICTION * DT).max(0.0);
        let (mut dx, mut dz) = (b.v[0] / sp, b.v[2] / sp);
        if w.any {
            dx += w.x * SLIDE_STEER * DT;
            dz += w.z * SLIDE_STEER * DT;
            let d = (dx * dx + dz * dz).sqrt();
            (dx, dz) = (dx / d, dz / d);
        }
        let keep = keep.min(SLIDE_MAX);
        b.v[0] = dx * keep;
        b.v[2] = dz * keep;
    }
}

/// Running, in the air, on the broom.
fn run(b: &mut Body, i: &Input, w: &Wish, u: &Under) {
    let mut speed = if b.glide {
        GLIDE_SPEED
    } else if u.wading {
        RUN * WADE
    } else if b.sprint {
        SPRINT
    } else {
        RUN
    };
    if b.chill > 0 {
        speed *= CHILL_SLOW;
    }
    if i.has(keys::AIM) && !b.glide {
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
    let air = !(b.ground || b.glide);
    let accel = if air {
        ACCEL_AIR
    } else if w.any {
        ACCEL_GROUND
    } else {
        BRAKE
    } * DT;
    // Faster than your pace, still steering more or less its way: the
    // speed holds in the air (steering curves it) and bleeds away on the
    // ground, slowly while you steer along it, quicker as you turn
    // (momentum).
    let sp = flat(&b.v);
    let along = if sp > 1e-4 {
        (b.v[0] * w.x + b.v[2] * w.z) / sp
    } else {
        0.0
    };
    let (on, off) = MOMENTUM_ALONG;
    if air && !w.any {
        // Nothing held in the air: it drifts on as it was going.
    } else if sp > speed && w.any && along > if air { AIR_ALONG } else { off } && !b.glide {
        let (mut dx, mut dz) = (b.v[0] / sp, b.v[2] / sp);
        dx += w.x * accel / sp;
        dz += w.z * accel / sp;
        let d = (dx * dx + dz * dz).sqrt();
        let keep = if air {
            sp
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
    use super::super::tests::{go, plaza};
    use super::super::{flat, keys};
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
}
