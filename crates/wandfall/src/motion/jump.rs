//! Jumps. A jump is a press: holding it does not hop again, and a press
//! is never lost. Pressed a moment early it waits for the ground (it is
//! not the air jump, if the ground is that near; if the ground does not
//! come after all, it is); a moment after running off an edge it still
//! goes. Timed to a landing, a hop keeps its speed and gains a little;
//! against a wall it kicks off it (`wall`); in the air, once, it is the
//! air jump.

use super::{flat, keys, Body, Input, Under, Wish};
use crate::laws::*;
use crate::map::Map;

pub(super) fn jump(b: &mut Body, i: &Input, w: &Wish, u: &Under, map: &Map) {
    let mut press = i.has(keys::JUMP) && !b.held;
    b.held = i.has(keys::JUMP);
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
        b.walls = 0;
    }
    if b.buffer > 0 && (b.ground || b.coyote > 0) && b.v[1] <= 0.5 && !b.glide {
        hop(b, u);
    } else if press
        && b.coyote == 0
        && crate::wall::can(b)
        && (b.air_jumped || !crate::wall::into(b, w))
    {
        // Off a wall (pushing into it with the air jump to spend, that
        // is the climb's: the air jump).
        crate::wall::kick(b, w);
        b.buffer = 0;
    } else if !b.ground
        && b.coyote == 0
        && !b.glide
        && !b.air_jumped
        && b.mantle == 0
        && !b.winded
        && b.spent + AIR_JUMP_STAMINA <= STAMINA
        && if press {
            !landing(b, map)
        } else {
            b.buffer == 1
        }
    {
        // Pressed in the air, the air jump; held for a landing that has
        // not come as the press runs out (it went on off the edge), the
        // air jump all the same: a press is never lost.
        air_jump(b, w);
    }
}

/// Whether it comes down onto something before an early press runs out
/// (so the press is a hop then, not the air jump): tick by tick, falling
/// as it falls and going on as it goes, onto what will be under it then.
/// Skimming off a top at speed, the top it is leaving does not count.
fn landing(b: &Body, map: &Map) -> bool {
    if b.v[1] > 0.0 {
        return false;
    }
    let (mut p, mut vy) = (b.p, b.v[1]);
    for _ in 1..JUMP_BUFFER {
        vy -= GRAVITY * DT;
        let y = p[1] + vy * DT;
        (p[0], p[2]) = (p[0] + b.v[0] * DT, p[2] + b.v[2] * DT);
        if y <= map.floor(p[0], p[2], p[1]).max(SEA - 0.9) {
            return true;
        }
        p[1] = y;
    }
    false
}

/// Off the ground. Out of a slide, the slide's speed goes with it. Timed
/// to the landing, a hop keeps its speed and gains a little (not chilled,
/// not in the sea, where it is lower too).
fn hop(b: &mut Body, u: &Under) {
    let sp = flat(&b.v);
    let free = b.chill == 0 && !u.wading;
    if b.landed <= HOP_WINDOW && sp > RUN * 0.8 && sp < HOP_MAX && free {
        let k = (sp + HOP_BOOST).min(HOP_MAX) / sp;
        b.v[0] *= k;
        b.v[2] *= k;
    }
    b.v[1] = if u.wading { JUMP * WADE_JUMP } else { JUMP };
    b.ground = false;
    b.slide = false;
    b.coyote = 0;
    b.buffer = 0;
}

/// The air jump: up again, turned the way you steer; turned back on
/// itself, it keeps less of the speed it had over a run.
fn air_jump(b: &mut Body, w: &Wish) {
    b.air_jumped = true;
    b.spent += AIR_JUMP_STAMINA;
    b.breath = 0;
    b.v[1] = AIR_JUMP;
    if w.any {
        let base = RUN * 0.8;
        let sp = flat(&b.v).max(base);
        let (ox, oz) = (b.v[0] / sp, b.v[2] / sp);
        let (mut dx, mut dz) = (ox * 0.35 + w.x * 0.65, oz * 0.35 + w.z * 0.65);
        let d = (dx * dx + dz * dz).sqrt().max(1e-6);
        (dx, dz) = (dx / d, dz / d);
        // How far it turns: straight on keeps it all, straight back
        // `AIR_JUMP_KEEP` of it.
        let turn = (1.0 + ox * dx + oz * dz) * 0.5;
        let keep = AIR_JUMP_KEEP + (1.0 - AIR_JUMP_KEEP) * turn;
        let sp = base + (sp - base) * keep;
        b.v[0] = dx * sp;
        b.v[2] = dz * sp;
    }
    b.buffer = 0;
}

#[cfg(test)]
mod tests {
    use super::super::tests::{go, plaza};
    use super::super::{flat, keys};
    use crate::laws::*;
    use crate::map::Map;

    #[test]
    fn an_air_jump_turned_back_keeps_less_of_its_speed() {
        let map = Map::new(3);
        let fly = |keys: u16| {
            let mut b = plaza(&map);
            go(&mut b, keys::JUMP, 0, &map);
            b.v[0] = 12.5;
            go(&mut b, 0, 0, &map);
            go(&mut b, keys::JUMP | keys, 0, &map);
            assert!(b.air_jumped, "{b:?}");
            b
        };
        // Straight on, it keeps it all; straight back, half of it over a
        // run; square to it, between.
        let on = fly(keys::FWD);
        assert!(on.v[0] > 12.4, "{on:?}");
        let back = fly(keys::BACK);
        let half = RUN * 0.8 + (12.5 - RUN * 0.8) * AIR_JUMP_KEEP;
        assert!(
            back.v[0] < 0.0 && (flat(&back.v) - half).abs() < 0.5,
            "{back:?}"
        );
        let side = fly(keys::RIGHT);
        assert!(
            flat(&side.v) > half + 1.0 && flat(&side.v) < 12.0,
            "{side:?}"
        );
    }

    #[test]
    fn a_press_over_a_top_you_are_leaving_at_speed_is_still_the_air_jump() {
        use crate::map::Kind;
        use crate::motion::{step, Body, Input};
        let map = Map::new(11);
        let (mut airs, mut late, mut hops) = (0, 0, 0);
        for q in map.props.iter().filter(|q| {
            matches!(q.kind, Kind::Rock | Kind::Pillar)
                && map.near(q.x, q.z, q.r + 2.0).count() == 1
        }) {
            // A top with a drop of a metre and a half or more off its
            // east side.
            let top = q.top().unwrap();
            let below = q.x + q.r + 1.0;
            if !map.land(below, q.z) || top - map.height(below, q.z) < 1.5 {
                continue;
            }
            for k in 0..14 {
                // A little over it, from a metre in from its east edge to
                // just past it, skimming east and falling: the press is
                // a hop if it comes down on the top in time, else the air
                // jump; never lost.
                let mut b = Body {
                    p: [q.x + q.r - 1.0 + k as f32 * 0.1, top + 0.3, q.z],
                    v: [12.0, -4.0, 0.0],
                    ..Body::default()
                };
                let press = Input {
                    keys: keys::FWD | keys::JUMP,
                    ..Input::default()
                };
                step(&mut b, &press, &map);
                let at_once = b.air_jumped;
                let mut hopped = false;
                for _ in 0..JUMP_BUFFER {
                    let was = b.ground;
                    step(
                        &mut b,
                        &Input {
                            keys: keys::FWD,
                            ..press
                        },
                        &map,
                    );
                    hopped |= was && !b.ground && b.v[1] > 1.0;
                }
                let air = b.air_jumped;
                assert!(
                    air != hopped,
                    "{q:?} from {k}: air {air}, hopped {hopped}: {b:?}"
                );
                airs += at_once as i32;
                late += (air && !at_once) as i32;
                hops += hopped as i32;
            }
        }
        // Mostly told apart as it is pressed (the air jump at once), not
        // late, as the press runs out.
        assert!(airs > 20 && hops > 20, "air jumps {airs}, hops {hops}");
        assert!(late * 10 <= airs, "late {late}, at once {airs}");
    }

    #[test]
    fn sprint_held_in_the_air_spends_no_stamina() {
        let map = Map::new(3);
        let mut b = plaza(&map);
        b.p[1] += 40.0;
        b.ground = false;
        for _ in 0..TICK_HZ {
            go(&mut b, keys::FWD | keys::SPRINT, 0, &map);
        }
        assert!(!b.ground && b.spent == 0, "{b:?}");
        // On the ground it spends again.
        let mut b = plaza(&map);
        go(&mut b, keys::FWD | keys::SPRINT, 0, &map);
        assert!(b.spent > 0, "{b:?}");
    }
}
