//! What is underfoot or just ahead: a launch rune (up onto the broom),
//! a ledge to climb.

use super::{flat, keys, Body, Input, Wish};
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
/// steering held till then; the speed you came at is kept for after.
pub(super) fn mantle(b: &mut Body, i: &Input, map: &Map, w: &Wish) {
    if b.mantle > 0 {
        b.mantle -= 1;
        return;
    }
    if b.ground || b.glide || b.tether > 0 || !w.any {
        return;
    }
    let Some((top, mid, gap)) = map.ledge(b.p, (w.x, w.z)) else {
        return;
    };
    let need = (2.0 * GRAVITY * (top + MANTLE_OVER - b.p[1])).sqrt();
    let (dx, dz) = (mid[0] - b.p[0], mid[1] - b.p[2]);
    let d = (dx * dx + dz * dz).sqrt().max(1e-4);
    if b.v[1] >= need || clears(b, i, top, gap, (dx / d, dz / d)) {
        return;
    }
    // Up against its side till the feet clear it, then over in the time
    // they take to rise past its top and come back down to it.
    let over = (2.0 * MANTLE_OVER / GRAVITY).sqrt();
    let push = (d / (2.0 * over)).min(MANTLE_PUSH);
    b.carry = flat(&b.v);
    b.v = [dx / d * push, need, dz / d * push];
    b.mantle = ((need / GRAVITY + over) / DT) as u8 + 2;
}

/// Whether the feet, going as they go, are over its top by the time the
/// body reaches its edge `gap` away along `(nx, nz)` (it flies on over).
fn clears(b: &Body, i: &Input, top: f32, gap: f32, (nx, nz): (f32, f32)) -> bool {
    let toward = b.v[0] * nx + b.v[2] * nz;
    if toward < 1.0 {
        return false;
    }
    let t = gap.max(0.0) / toward;
    let g = if b.v[1] > 0.0 && i.has(keys::JUMP) {
        GRAVITY_UP
    } else {
        GRAVITY
    };
    b.p[1] + b.v[1] * t - 0.5 * g * t * t >= top
}

/// Over the top of a climb: on the way it climbed, with the speed it came
/// at (some of it) if that was more than a run.
pub(super) fn over(b: &mut Body) {
    let sp = flat(&b.v);
    if b.carry > RUN && sp > 1e-4 {
        let k = (b.carry * MANTLE_KEEP / sp).max(1.0);
        b.v[0] *= k;
        b.v[2] *= k;
    }
    b.carry = 0.0;
}

#[cfg(test)]
mod tests {
    use super::super::{flat, keys, step, Body, Input};
    use crate::map::{Kind, Map};

    #[test]
    fn a_rock_climbed_at_a_run_and_more_keeps_the_speed() {
        let map = Map::new(11);
        let mut tried = 0;
        for q in map.props.iter().filter(|q| q.kind == Kind::Rock) {
            // Hopping at it from open ground to its west, too high to
            // hop onto.
            let (top, at) = (q.top().unwrap(), [q.x - q.r - 6.0, q.z]);
            let rise = top - map.height(at[0], at[1]);
            let open = map.near(at[0], at[1], 6.0).count() == 1;
            if !(1.4..2.1).contains(&rise) || !open || !map.land(at[0], at[1]) {
                continue;
            }
            let mut b = Body {
                p: [at[0], map.height(at[0], at[1]), at[1]],
                v: [12.0, 0.0, 0.0],
                ground: true,
                ..Body::default()
            };
            let (mut climbed, mut hopped) = (false, false);
            for _ in 0..40 {
                let jump = if b.ground && !hopped { keys::JUMP } else { 0 };
                hopped |= jump != 0;
                let was = b.mantle;
                step(
                    &mut b,
                    &Input {
                        keys: keys::FWD | jump,
                        ..Input::default()
                    },
                    &map,
                );
                climbed |= was > 0;
                if climbed && b.ground {
                    break;
                }
            }
            if !climbed {
                continue;
            }
            tried += 1;
            assert!(b.ground && (b.p[1] - top).abs() < 0.05, "on top: {b:?}");
            assert!(flat(&b.v) > 8.0, "the speed kept: {b:?}");
        }
        assert!(tried >= 3, "{tried}");
    }
}
