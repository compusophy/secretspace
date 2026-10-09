//! The Tether's pull: a rope of light caught somewhere (`Body::anchor`)
//! hauls its wizard there, fast, gravity all but gone. Near it, it lets
//! go with a hop up (onto the ledge it caught); a jump lets go sooner,
//! keeping every bit of the speed (a slingshot). Part of `motion::step`,
//! so the page predicts it to the bit.

use crate::laws::*;
use crate::motion::Body;

/// One tick of the pull.
pub fn pull(b: &mut Body) {
    let d = [
        b.anchor[0] - b.p[0],
        b.anchor[1] - b.p[1],
        b.anchor[2] - b.p[2],
    ];
    let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if dist < TETHER_ARRIVE {
        // There: most of the speed spent on the catch, a hop up.
        b.v[0] *= 0.5;
        b.v[2] *= 0.5;
        let_go(b);
        return;
    }
    b.tether -= 1;
    let speed = if b.chill > 0 {
        TETHER_SPEED * CHILL_SLOW
    } else {
        TETHER_SPEED
    };
    let k = (TETHER_GRIP * DT).min(1.0);
    for (v, d) in b.v.iter_mut().zip(d) {
        *v += (d / dist * speed - *v) * k;
    }
    b.glide = false;
    b.slide = false;
}

/// Let go: a hop up, the speed kept.
pub fn let_go(b: &mut Body) {
    b.tether = 0;
    b.v[1] = b.v[1].max(TETHER_POP);
}

#[cfg(test)]
mod tests {
    use crate::laws::*;
    use crate::map::Map;
    use crate::motion::{keys, step, Body, Input};

    fn stand(map: &Map) -> Body {
        let [x, z] = map.spot(&mut engine::rng::Rng::new(5));
        Body {
            p: [x, map.height(x, z), z],
            ground: true,
            ..Body::default()
        }
    }

    #[test]
    fn it_hauls_you_up_to_where_it_caught_and_lets_go() {
        let map = Map::new(11);
        let mut b = stand(&map);
        let to = [b.p[0] + 20.0, b.p[1] + 12.0, b.p[2]];
        b.anchor = to;
        b.tether = TETHER_TICKS;
        let idle = Input::default();
        let mut closest = f32::MAX;
        for _ in 0..TETHER_TICKS {
            step(&mut b, &idle, &map);
            let d = ((b.p[0] - to[0]).powi(2) + (b.p[1] - to[1]).powi(2)).sqrt();
            closest = closest.min(d);
            if b.tether == 0 {
                break;
            }
        }
        assert_eq!(b.tether, 0, "it got there and let go");
        assert!(closest < TETHER_ARRIVE + 0.5, "{closest}");
        assert!(b.v[1] > 0.0, "a hop up off it");
    }

    #[test]
    fn a_jump_lets_go_with_the_speed_kept() {
        let map = Map::new(11);
        let mut b = stand(&map);
        b.anchor = [b.p[0] + 40.0, b.p[1] + 5.0, b.p[2]];
        b.tether = TETHER_TICKS;
        let idle = Input::default();
        for _ in 0..20 {
            step(&mut b, &idle, &map);
        }
        assert!(b.tether > 0 && b.v[0] > TETHER_SPEED * 0.8, "{b:?}");
        let jump = Input {
            keys: keys::JUMP,
            ..Input::default()
        };
        let before = b.v[0];
        step(&mut b, &jump, &map);
        assert_eq!(b.tether, 0);
        assert!(!b.air_jumped, "the press let go; no air jump spent");
        assert!(b.v[0] > before * 0.95 && b.v[1] > 0.0, "{b:?}");
    }
}
