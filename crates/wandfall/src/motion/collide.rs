//! Gravity, and the move itself: out of anything standing in the way,
//! onto the ground, a deck or a top underfoot.

use super::{keys, Body, Input};
use crate::laws::*;
use crate::map::Map;

/// Gravity: lighter rising with jump held; a Tether bears most of your
/// weight; the broom falls slowly.
pub(super) fn fall(b: &mut Body, i: &Input) {
    let pull = if b.v[1] > 0.0 && i.has(keys::JUMP) && b.mantle == 0 {
        GRAVITY_UP
    } else {
        GRAVITY
    };
    let bear = if b.tether > 0 { TETHER_GRAVITY } else { 1.0 };
    b.v[1] -= pull * bear * DT;
    if b.glide && b.v[1] < -GLIDE_FALL {
        b.v[1] = -GLIDE_FALL;
    }
}

/// The tick's move.
pub(super) fn advance(b: &mut Body, map: &Map) {
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
    b.wall = b.wall.saturating_sub(1);
    if !was {
        crate::wall::touch(b, [out[0] - p[0], out[2] - p[2]]);
    }
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
