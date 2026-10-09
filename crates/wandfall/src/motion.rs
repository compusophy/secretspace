//! How a wizard moves, tick by tick: run and sprint (with momentum: speed
//! above your pace bleeds away, it does not vanish), slide (crouch at
//! speed; gravity carries it down a hill), walk slower up a slope and
//! faster down it, jump, glide down in the drop, wade in the shallows,
//! step up small rises, and never through trees, rocks or pillars; a
//! Tether hauls you (`tether`).
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
    /// Stamina spent (0 fresh, `STAMINA` none left), ticks since it last
    /// sprinted, and winded (it cannot sprint till some comes back).
    pub spent: u16,
    pub breath: u8,
    pub winded: bool,
    /// Jump held last tick (a jump is a press); ticks on the ground since
    /// it landed; its air jump spent (till it lands).
    pub held: bool,
    pub landed: u8,
    pub air_jumped: bool,
    /// Ticks left of a climb onto a ledge (steering held till it is over).
    pub mantle: u8,
    /// Ticks left pulled by a Tether, toward where it caught.
    pub tether: u8,
    pub anchor: [f32; 3],
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
        && b.chill == 0
        && !b.winded;
    // Sprinting spends stamina; a breath after, it comes back.
    if b.sprint {
        b.spent = (b.spent + STAMINA_SPEND).min(STAMINA);
        b.breath = 0;
        b.winded = b.spent >= STAMINA;
    } else {
        b.breath = b.breath.saturating_add(1);
        if b.breath >= STAMINA_BREATH {
            b.spent = b.spent.saturating_sub(STAMINA_BACK);
        }
        if b.winded && b.spent <= WINDED_UNTIL {
            b.winded = false;
        }
    }
    if b.mantle > 0 {
        // Climbing a ledge: carried over it, not steered.
    } else if b.tether > 0 {
        crate::tether::pull(b);
    } else if b.slide && b.ground {
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
    // A jump is a press: holding it does not hop again. Pressed a moment
    // early it waits for the ground; a moment after running off an edge
    // it still goes.
    let mut press = has(keys::JUMP) && !b.held;
    b.held = has(keys::JUMP);
    if press && b.tether > 0 {
        crate::tether::let_go(b);
        press = false;
    }
    b.buffer = if press {
        JUMP_BUFFER
    } else {
        b.buffer.saturating_sub(1)
    };
    b.coyote = if b.ground {
        COYOTE
    } else {
        b.coyote.saturating_sub(1)
    };
    if b.ground {
        b.landed = b.landed.saturating_add(1);
        b.air_jumped = false;
    }
    if b.buffer > 0 && (b.ground || b.coyote > 0) && b.v[1] <= 0.5 && !b.glide {
        // Out of a slide, the slide's speed goes with it. Timed to the
        // landing, a hop keeps its speed and gains a little.
        let sp = (b.v[0] * b.v[0] + b.v[2] * b.v[2]).sqrt();
        if b.landed <= HOP_WINDOW && sp > RUN * 0.8 && sp < HOP_MAX {
            let k = (sp + HOP_BOOST).min(HOP_MAX) / sp;
            b.v[0] *= k;
            b.v[2] *= k;
        }
        b.v[1] = JUMP;
        b.ground = false;
        b.slide = false;
        b.coyote = 0;
        b.buffer = 0;
    } else if press
        && !b.ground
        && b.coyote == 0
        && !b.glide
        && !b.air_jumped
        && b.mantle == 0
        && !b.winded
        && b.spent + AIR_JUMP_STAMINA <= STAMINA
    {
        // The air jump: up again, turned the way you steer.
        b.air_jumped = true;
        b.spent += AIR_JUMP_STAMINA;
        b.breath = 0;
        b.v[1] = AIR_JUMP;
        if len > 1e-6 {
            let sp = (b.v[0] * b.v[0] + b.v[2] * b.v[2]).sqrt().max(RUN * 0.8);
            let (mut dx, mut dz) = if sp > 1e-4 {
                (
                    b.v[0] / sp * 0.35 + wx * 0.65,
                    b.v[2] / sp * 0.35 + wz * 0.65,
                )
            } else {
                (wx, wz)
            };
            let d = (dx * dx + dz * dz).sqrt().max(1e-6);
            (dx, dz) = (dx / d, dz / d);
            b.v[0] = dx * sp;
            b.v[2] = dz * sp;
        }
        b.buffer = 0;
    }
    // A launch rune underfoot: up into the sky, onto your broom.
    if b.ground && !b.glide && map.pad_under(b.p).is_some() {
        b.tether = 0;
        b.v[1] = PAD_UP;
        b.glide = true;
        b.ground = false;
        b.slide = false;
        b.coyote = 0;
        b.buffer = 0;
    }
    // A ledge within reach as you push toward it in the air (and would
    // fall short of it): up and over, timed to come down on its middle,
    // steering held till then.
    if b.mantle > 0 {
        b.mantle -= 1;
    } else if !b.ground && !b.glide && b.tether == 0 && len > 1e-6 {
        if let Some((top, mid)) = map.ledge(b.p, (wx, wz)) {
            let need = (2.0 * GRAVITY * (top + MANTLE_OVER - b.p[1])).sqrt();
            if b.v[1] < need {
                let (dx, dz) = (mid[0] - b.p[0], mid[1] - b.p[2]);
                let d = (dx * dx + dz * dz).sqrt().max(1e-4);
                // Up against its side till the feet clear it, then over
                // in the time they take to rise past its top and come
                // back down to it.
                let over = (2.0 * MANTLE_OVER / GRAVITY).sqrt();
                let push = (d / (2.0 * over)).min(MANTLE_PUSH);
                b.v = [dx / d * push, need, dz / d * push];
                b.mantle = ((need / GRAVITY + over) / DT) as u8 + 2;
            }
        }
    }
    let pull = if b.v[1] > 0.0 && has(keys::JUMP) && b.mantle == 0 {
        GRAVITY_UP
    } else {
        GRAVITY
    };
    // A Tether bears most of your weight.
    let bear = if b.tether > 0 { TETHER_GRAVITY } else { 1.0 };
    b.v[1] -= pull * bear * DT;
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
    // Out of anything standing there; coming down onto a top from over
    // it, it holds you rather than pushing you off it.
    let mut out = [p[0], p[1].max(b.p[1]), p[2]];
    map.push_out(&mut out, b.tall());
    (p[0], p[2]) = (out[0], out[2]);
    // The ground, or a deck under you (the sea floor too: you wade, you
    // do not swim).
    let floor = map.floor(p[0], p[2], b.p[1]).max(SEA - 0.9);
    if p[1] <= floor || (was && b.v[1] <= 0.0 && p[1] - floor < STEP) {
        p[1] = floor;
        b.v[1] = 0.0;
        if !was {
            b.landed = 0;
        }
        b.ground = true;
        b.glide = false;
        b.mantle = 0;
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

    /// A pillar whose top stands `lo..hi` over open ground just west of
    /// it (where a wizard faces it, looking east): it, and that spot.
    fn pillar(map: &Map, (lo, hi): (f32, f32)) -> (crate::map::Prop, [f32; 2]) {
        map.props
            .iter()
            .filter(|q| q.kind == crate::map::Kind::Pillar)
            .find_map(|q| {
                let at = [q.x - (q.r + RADIUS + 0.7), q.z];
                let rise = q.y + q.h - map.height(at[0], at[1]);
                let clear = map.near(at[0], at[1], RADIUS + 0.5).next().is_none();
                let (gx, gz) = slope(map, at[0], at[1]);
                (rise > lo
                    && rise < hi
                    && clear
                    && map.land(at[0], at[1])
                    && gx * gx + gz * gz < 0.04)
                    .then_some((*q, at))
            })
            .expect("a pillar")
    }

    #[test]
    fn a_jump_at_a_pillar_climbs_onto_it_and_it_holds_you() {
        let map = Map::new(11);
        let (q, at) = pillar(&map, (2.0, 2.8));
        let top = q.y + q.h;
        let mut b = on(&map, at, [0.0, 0.0]);
        // Walking into it, you do not climb it.
        for _ in 0..30 {
            step(&mut b, &east(keys::FWD), &map);
        }
        assert!(b.p[1] < top - 1.0, "walked into, not up: {b:?}");
        // Jumping at it: up and onto its top.
        let mut up = false;
        for _ in 0..40 {
            step(&mut b, &east(keys::FWD | keys::JUMP), &map);
            if b.ground && (b.p[1] - top).abs() < 0.01 {
                up = true;
                break;
            }
        }
        assert!(up, "onto the top at {top}: {b:?}");
        // And it holds you.
        for _ in 0..30 {
            step(&mut b, &east(0), &map);
        }
        assert!(b.ground && (b.p[1] - top).abs() < 0.01, "{b:?}");
    }

    #[test]
    fn a_pillar_too_tall_for_one_jump_takes_an_air_jump_too() {
        let map = Map::new(11);
        let (q, at) = pillar(&map, (3.4, 4.0));
        let top = q.y + q.h;
        let climb = |air: bool| {
            let mut b = on(&map, at, [0.0, 0.0]);
            for k in 0..60 {
                // Jump (held, for the higher arc), let go near its top,
                // and (`air`) jump again, held.
                let jump = k <= 10 || (air && k >= 12);
                let keys = keys::FWD | if jump { keys::JUMP } else { 0 };
                step(&mut b, &east(keys), &map);
                if b.ground && (b.p[1] - top).abs() < 0.01 {
                    return true;
                }
            }
            false
        };
        assert!(!climb(false), "one jump falls short");
        assert!(climb(true), "an air jump reaches it");
    }

    #[test]
    fn a_launch_rune_throws_you_up_onto_your_broom_and_you_glide_down() {
        let map = Map::new(11);
        let q = map.pads[0];
        let mut b = on(&map, [q[0], q[2]], [0.0, 0.0]);
        step(&mut b, &east(0), &map);
        assert!(b.glide && !b.ground && b.v[1] > 20.0, "{b:?}");
        let mut top = b.p[1];
        let mut k = 0;
        while !b.ground && k < 600 {
            step(&mut b, &east(keys::FWD), &map);
            top = top.max(b.p[1]);
            k += 1;
        }
        assert!(top > q[1] + 14.0, "high: {top} over {}", q[1]);
        assert!(b.ground && !b.glide, "down again, off the broom: {b:?}");
        let went = (b.p[0] - q[0]).hypot(b.p[2] - q[2]);
        assert!(went > 30.0, "and far: {went}");
    }

    /// A body on the plaza (flat), and a step under `keys` heading `yaw`.
    fn plaza(map: &Map) -> Body {
        let (flat, _) = grounds(map);
        Body {
            p: [flat[0], map.height(flat[0], flat[1]), flat[1]],
            ground: true,
            ..Body::default()
        }
    }

    fn go(b: &mut Body, keys: u16, yaw: u16, map: &Map) {
        step(
            b,
            &Input {
                seq: 0,
                yaw,
                pitch: 0,
                keys,
                cast: 0,
            },
            map,
        );
    }

    #[test]
    fn holding_jump_hops_once_not_again_and_again() {
        let map = Map::new(3);
        let mut b = plaza(&map);
        let mut hops = 0;
        for _ in 0..90 {
            let was = b.ground;
            go(&mut b, keys::JUMP, 0, &map);
            hops += (was && !b.ground) as i32;
        }
        assert_eq!(hops, 1, "held: one hop");
        // Pressed again (released between), it hops again.
        go(&mut b, 0, 0, &map);
        go(&mut b, keys::JUMP, 0, &map);
        assert!(b.v[1] > 1.0, "a fresh press hops");
    }

    #[test]
    fn hops_timed_to_the_landing_build_speed_and_late_ones_do_not() {
        let map = Map::new(3);
        // Back and forth across the plaza would turn it; run east from
        // its west edge, hopping each time it lands (on time, or late).
        let hop = |late: u32| {
            let mut b = plaza(&map);
            b.p[0] -= 12.0;
            b.p[1] = map.height(b.p[0], b.p[2]);
            for _ in 0..30 {
                go(&mut b, keys::FWD | keys::SPRINT, 0, &map);
            }
            let mut wait = 0;
            for _ in 0..90 {
                let mut k = keys::FWD;
                if b.ground {
                    wait += 1;
                    if wait > late {
                        k |= keys::JUMP;
                        wait = 0;
                    }
                }
                go(&mut b, k, 0, &map);
            }
            (b.v[0] * b.v[0] + b.v[2] * b.v[2]).sqrt()
        };
        let (timed, late) = (hop(0), hop(8));
        assert!(timed > SPRINT + 1.0, "timed hops gain: {timed}");
        assert!(timed <= HOP_MAX + 0.01, "but not past the most: {timed}");
        assert!(late < SPRINT, "late ones do not: {late}");
    }

    #[test]
    fn one_air_jump_turns_you_and_costs_stamina() {
        let map = Map::new(3);
        let mut b = plaza(&map);
        go(&mut b, keys::JUMP, 0, &map);
        for _ in 0..6 {
            go(&mut b, 0, 0, &map);
        }
        assert!(!b.ground && !b.air_jumped);
        // Pressed in the air, steering right: up again, turned right.
        go(&mut b, keys::JUMP | keys::RIGHT, 0, &map);
        assert!(b.air_jumped && b.v[1] > AIR_JUMP - 1.5, "{b:?}");
        assert!(b.v[2] > 3.0, "turned the way it steers: {b:?}");
        assert!(b.spent >= AIR_JUMP_STAMINA);
        // Only once till it lands.
        go(&mut b, 0, 0, &map);
        let up = b.v[1];
        go(&mut b, keys::JUMP, 0, &map);
        assert!(b.v[1] < up, "no second air jump");
        for _ in 0..90 {
            go(&mut b, 0, 0, &map);
        }
        assert!(b.ground && !b.air_jumped, "landed, it has it again");
    }

    #[test]
    fn sprinting_tires_and_a_breath_brings_it_back() {
        let map = Map::new(3);
        let (flat, _) = grounds(&map);
        let mut b = Body {
            p: [flat[0], map.height(flat[0], flat[1]), flat[1]],
            ground: true,
            ..Body::default()
        };
        let run = |b: &mut Body, keys: u16, ticks: u32| {
            for k in 0..ticks {
                // Back and forth on the plaza, so it never leaves it.
                let yaw = if (k / 45) % 2 == 0 { 0 } else { 32768 };
                step(
                    b,
                    &Input {
                        seq: 0,
                        yaw,
                        pitch: 0,
                        keys,
                        cast: 0,
                    },
                    &map,
                );
            }
        };
        let full = (STAMINA / STAMINA_SPEND) as u32;
        run(&mut b, keys::FWD | keys::SPRINT, full - 2);
        assert!(b.sprint && !b.winded, "still sprinting: {b:?}");
        run(&mut b, keys::FWD | keys::SPRINT, 4);
        assert!(b.winded && !b.sprint, "winded: {b:?}");
        // Holding sprint winded is only running; a rest brings it back.
        run(&mut b, keys::FWD | keys::SPRINT, 10);
        assert!(!b.sprint);
        run(&mut b, 0, STAMINA_BREATH as u32 + 60);
        assert!(!b.winded && b.spent < STAMINA / 2, "rested: {b:?}");
        run(&mut b, keys::FWD | keys::SPRINT, 2);
        assert!(b.sprint, "sprinting again");
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
