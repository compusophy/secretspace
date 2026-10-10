//! What is underfoot or just ahead: a launch rune (up onto the broom),
//! a ledge to climb.

use super::{Body, Wish};
use crate::laws::*;
use crate::map::Map;

/// A launch rune underfoot: up into the sky, onto your broom.
pub(super) fn rune(b: &mut Body, map: &Map) {
    if b.ground && !b.glide && map.pad_under(b.p).is_some() {
        b.tether = 0;
        b.v[1] = PAD_UP;
        b.glide = true;
        b.ground = false;
        b.slide = false;
        b.coyote = 0;
        b.buffer = 0;
    }
}

/// A ledge within reach as you push toward it in the air (and would
/// fall short of it): up and over, timed to come down on its middle,
/// steering held till then.
pub(super) fn mantle(b: &mut Body, map: &Map, w: &Wish) {
    if b.mantle > 0 {
        b.mantle -= 1;
        return;
    }
    if b.ground || b.glide || b.tether > 0 || !w.any {
        return;
    }
    let Some((top, mid)) = map.ledge(b.p, (w.x, w.z)) else {
        return;
    };
    let need = (2.0 * GRAVITY * (top + MANTLE_OVER - b.p[1])).sqrt();
    if b.v[1] < need {
        let (dx, dz) = (mid[0] - b.p[0], mid[1] - b.p[2]);
        let d = (dx * dx + dz * dz).sqrt().max(1e-4);
        // Up against its side till the feet clear it, then over in the
        // time they take to rise past its top and come back down to it.
        let over = (2.0 * MANTLE_OVER / GRAVITY).sqrt();
        let push = (d / (2.0 * over)).min(MANTLE_PUSH);
        b.v = [dx / d * push, need, dz / d * push];
        b.mantle = ((need / GRAVITY + over) / DT) as u8 + 2;
    }
}
