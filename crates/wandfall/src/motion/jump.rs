//! Jumps. A jump is a press: holding it does not hop again. Pressed a
//! moment early it waits for the ground; a moment after running off an
//! edge it still goes. Timed to a landing, a hop keeps its speed and
//! gains a little; against a wall it kicks off it (`wall`); in the air,
//! once, it is the air jump.

use super::{flat, keys, Body, Input, Wish};
use crate::laws::*;

pub(super) fn jump(b: &mut Body, i: &Input, w: &Wish) {
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
    {
        air_jump(b, w);
    }
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

/// The air jump: up again, turned the way you steer.
fn air_jump(b: &mut Body, w: &Wish) {
    b.air_jumped = true;
    b.spent += AIR_JUMP_STAMINA;
    b.breath = 0;
    b.v[1] = AIR_JUMP;
    if w.any {
        let sp = flat(&b.v).max(RUN * 0.8);
        let (mut dx, mut dz) = if sp > 1e-4 {
            (
                b.v[0] / sp * 0.35 + w.x * 0.65,
                b.v[2] / sp * 0.35 + w.z * 0.65,
            )
        } else {
            (w.x, w.z)
        };
        let d = (dx * dx + dz * dz).sqrt().max(1e-6);
        (dx, dz) = (dx / d, dz / d);
        b.v[0] = dx * sp;
        b.v[2] = dz * sp;
    }
    b.buffer = 0;
}
