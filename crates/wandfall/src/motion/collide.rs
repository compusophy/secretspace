//! Gravity, and the move itself: out of anything standing in the way,
//! onto the ground, a deck or a top underfoot.

use super::{flat, keys, Body, Input};
use crate::laws::*;
use crate::map::Map;

/// Gravity: lighter rising with jump held (not thrown up by a launch
/// rune: that is the rune's height); a Tether bears most of your weight;
/// the broom falls slowly.
pub(super) fn fall(b: &mut Body, i: &Input) {
    let pull = if b.v[1] > 0.0 && i.has(keys::JUMP) && b.mantle == 0 && !b.glide {
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
    let mut p = [b.p[0], b.p[1] + b.v[1] * DT, b.p[2]];
    // Across in steps short enough that nothing thin is passed through.
    let n = (flat(&b.v) * DT / (RADIUS * 0.75)).ceil().clamp(1.0, 6.0);
    let lim = MAP_HALF - 1.0;
    // Coming down onto a top from over it, it holds you rather than
    // pushing you off it; on the ground, you step up onto a low one.
    let y = p[1].max(b.p[1]);
    let rise = if was { STEP } else { 0.0 };
    let mut pushed = [0.0, 0.0];
    for _ in 0..n as i32 {
        let at = [
            (p[0] + b.v[0] * DT / n).clamp(-lim, lim),
            y,
            (p[2] + b.v[2] * DT / n).clamp(-lim, lim),
        ];
        // Out of anything standing there, no longer moving into it
        // (climbing a ledge, you push against its side on purpose).
        let mut out = at;
        map.push_out(&mut out, b.tall(), rise);
        let by = [out[0] - at[0], out[2] - at[2]];
        let d = (by[0] * by[0] + by[1] * by[1]).sqrt();
        if d > 1e-4 && b.mantle == 0 {
            let (nx, nz) = (by[0] / d, by[1] / d);
            let into = b.v[0] * nx + b.v[2] * nz;
            if into < 0.0 {
                b.v[0] -= nx * into;
                b.v[2] -= nz * into;
            }
        }
        pushed = [pushed[0] + by[0], pushed[1] + by[1]];
        (p[0], p[2]) = (out[0], out[2]);
    }
    b.wall = b.wall.saturating_sub(1);
    if !was {
        crate::wall::touch(b, pushed);
    }
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
    use super::super::tests::{east, on, pillar};
    use super::super::{flat, keys, step, Body, Input};
    use crate::laws::*;
    use crate::map::{Kind, Map};
    use crate::trig;

    #[test]
    fn against_a_pillar_you_stop_and_hops_in_place_gain_nothing() {
        let map = Map::new(11);
        let (q, at) = pillar(&map, (2.0, 4.0));
        // Running into it fast, holding forward: stopped, not still
        // running into it.
        let mut b = on(&map, at, [12.5, 0.0]);
        for _ in 0..60 {
            step(&mut b, &east(keys::FWD), &map);
        }
        assert!(flat(&b.v) < 0.5, "stopped against it: {b:?}");
        assert!(b.p[0] < q.x - q.r - RADIUS + 0.01, "{b:?}");
        // Hopping in place against it, each on its landing: no speed.
        let start = b.p;
        let mut most: f32 = 0.0;
        for _ in 0..90 {
            let k = keys::FWD | if b.ground { keys::JUMP } else { 0 };
            step(&mut b, &east(k), &map);
            most = most.max(flat(&b.v));
        }
        assert!(most < 1.0, "hops gain nothing: {most}");
        assert!((b.p[0] - start[0]).abs() < 0.1, "{b:?}");
    }

    #[test]
    fn at_tether_speed_you_go_round_a_lamp_not_through_it() {
        let map = Map::new(11);
        for q in map.props.iter().filter(|q| q.kind == Kind::Lamp) {
            for off in [0.0, 0.1, -0.2] {
                let y = q.y + 0.5;
                let mut b = Body {
                    p: [q.x - 6.0, y, q.z + off],
                    v: [TETHER_SPEED, 0.0, 0.0],
                    tether: TETHER_TICKS,
                    anchor: [q.x + 30.0, y + 0.5, q.z + off],
                    ..Body::default()
                };
                for _ in 0..12 {
                    let was = b.p;
                    step(&mut b, &Input::default(), &map);
                    // From in front of it to behind it in a tick, never
                    // round its side: through it.
                    let front = was[0] < q.x && (was[2] - q.z).abs() < q.r + RADIUS * 0.5;
                    assert!(!(front && b.p[0] > q.x), "through {q:?}: {was:?} -> {b:?}");
                }
            }
        }
    }

    #[test]
    fn across_the_causeway_you_step_up_onto_a_column_a_little_higher() {
        let map = Map::new(2);
        let cols: Vec<_> = map
            .props
            .iter()
            .filter(|q| q.kind == Kind::Column)
            .collect();
        let mut tried = 0;
        for a in &cols {
            for c in &cols {
                let (ta, tc) = (a.top().unwrap(), c.top().unwrap());
                let d = (c.x - a.x).hypot(c.z - a.z);
                if !(0.5..2.6).contains(&d) || !(0.05..STEP - 0.05).contains(&(tc - ta)) {
                    continue;
                }
                tried += 1;
                let mut b = Body {
                    p: [a.x, ta, a.z],
                    ground: true,
                    ..Body::default()
                };
                let yaw = trig::heading(trig::atan2(c.z - a.z, c.x - a.x));
                let i = Input {
                    keys: keys::FWD,
                    yaw,
                    ..Input::default()
                };
                for _ in 0..20 {
                    if (b.p[0] - c.x).hypot(b.p[2] - c.z) < 0.4 {
                        break;
                    }
                    step(&mut b, &i, &map);
                }
                assert!(
                    b.ground && b.p[1] >= tc - 0.01,
                    "onto {c:?} from {a:?}: {b:?}"
                );
            }
        }
        assert!(tried > 20, "{tried}");
    }

    #[test]
    fn a_mushroom_cap_holds_you_and_you_walk_under_it() {
        let map = Map::new(11);
        let q = *map
            .props
            .iter()
            .find(|q| q.kind == Kind::Shroom && map.near(q.x, q.z, 4.0).count() == 1)
            .expect("a mushroom on its own");
        let (r, _, top) = q.cap().unwrap();
        // Coming down onto its cap, well out from the stem: held there.
        let mut b = Body {
            p: [q.x + r * 0.7, top + 2.0, q.z],
            ..Body::default()
        };
        for _ in 0..40 {
            step(&mut b, &Input::default(), &map);
        }
        assert!(b.ground && (b.p[1] - top).abs() < 0.01, "on the cap: {b:?}");
        // On the ground, walking under it past the stem: not stopped.
        let x = q.x - r - 1.0;
        let mut b = on(&map, [x, q.z + q.r + RADIUS + 0.3], [0.0, 0.0]);
        for _ in 0..30 {
            step(&mut b, &east(keys::FWD), &map);
        }
        assert!(
            b.p[0] > q.x + r * 0.5 && b.p[1] < top - 1.0,
            "under it: {b:?}"
        );
        // A bolt coming down onto it strikes the cap.
        let (a, c) = ([q.x + r * 0.5, top + 3.0, q.z], [q.x + r * 0.5, q.y, q.z]);
        let t = map.strikes(a, c).expect("the cap stops it");
        assert!((a[1] + (c[1] - a[1]) * t - top).abs() < 0.05, "{t}");
    }

    #[test]
    fn a_launch_rune_throws_you_as_high_with_jump_held_or_not() {
        let map = Map::new(11);
        let q = map.pads[0];
        let peak = |keys: u16| {
            let mut b = on(&map, [q[0], q[2]], [0.0, 0.0]);
            let mut top: f32 = b.p[1];
            for _ in 0..200 {
                step(&mut b, &east(keys), &map);
                top = top.max(b.p[1]);
            }
            top - q[1]
        };
        let (held, not) = (peak(keys::JUMP), peak(0));
        assert!((held - not).abs() < 0.5, "held {held}, not {not}");
    }
}
