//! Jumps. A jump is a press: holding it does not hop again. Pressed a
//! moment early it waits for the ground (it is not the air jump, if the
//! ground is that near); a moment after running off an edge it still
//! goes. Timed to a landing, a hop keeps its speed and gains a little;
//! against a wall it kicks off it (`wall`); in the air, once, it is the
//! air jump.

use super::{flat, keys, Body, Input, Wish};
use crate::laws::*;
use crate::map::Map;

pub(super) fn jump(b: &mut Body, i: &Input, w: &Wish, map: &Map) {
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
        hop(b);
    } else if press
        && b.coyote == 0
        && crate::wall::can(b)
        && (b.air_jumped || !crate::wall::into(b, w))
    {
        // Off a wall (pushing into it with the air jump to spend, that
        // is the climb's: the air jump).
        crate::wall::kick(b);
        b.buffer = 0;
    } else if press
        && !b.ground
        && b.coyote == 0
        && !b.glide
        && !b.air_jumped
        && b.mantle == 0
        && !b.winded
        && b.spent + AIR_JUMP_STAMINA <= STAMINA
        && !landing(b, map)
    {
        air_jump(b, w);
    }
}

/// Whether it comes down onto what is under it before an early press
/// runs out (so the press is a hop then, not the air jump).
fn landing(b: &Body, map: &Map) -> bool {
    if b.v[1] > 0.0 {
        return false;
    }
    let t = (JUMP_BUFFER - 1) as f32 * DT;
    let floor = map.floor(b.p[0], b.p[2], b.p[1]).max(SEA - 0.9);
    b.p[1] - floor <= -b.v[1] * t + 0.5 * GRAVITY * t * t
}

/// Off the ground. Out of a slide, the slide's speed goes with it. Timed
/// to the landing, a hop keeps its speed and gains a little.
fn hop(b: &mut Body) {
    let sp = flat(&b.v);
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
