//! The Tether's pull: a rope of light caught somewhere (`Body::anchor`)
//! hauls its wizard there, fast, gravity all but gone; steering square
//! to the rope swings you about it (round a trunk in the way, or wide of
//! a foe). Near it, it lets go with a hop up (onto the ledge it caught);
//! a jump lets go sooner, keeping every bit of the speed (a slingshot).
//! Part of `motion::step`, so the page predicts it to the bit.

use crate::laws::*;
use crate::motion::{Body, Wish};

/// One tick of the pull: along the rope, toward its pace; across it, the
/// swing across the ground kept (dying slowly away) and steered, and any
/// sag up or down taken up as the pull takes hold (so neither gravity
/// nor a fall at the cast bows the rope much).
pub fn pull(b: &mut Body, w: &Wish) {
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
    let rope = d.map(|x| x / dist);
    let along = dot(b.v, rope);
    let grip = (TETHER_GRIP * DT).min(1.0);
    let sway = 1.0 - TETHER_SWAY * DT;
    let keep = [sway, 1.0 - grip, sway];
    let mut side = [0, 1, 2].map(|k| (b.v[k] - rope[k] * along) * keep[k]);
    if w.any {
        let s = [w.x, 0.0, w.z];
        let on = dot(s, rope);
        for (k, x) in side.iter_mut().enumerate() {
            *x += (s[k] - rope[k] * on) * TETHER_STEER * DT;
        }
    }
    let along = along + (speed - along) * grip;
    b.v = [0, 1, 2].map(|k| rope[k] * along + side[k]);
    b.glide = false;
    b.slide = false;
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
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

    /// On the Spire's plaza (seed 3), west of the tower and clear of it:
    /// the way east is open.
    fn stand(map: &Map) -> Body {
        let (x, z) = (-17.0, -9.0);
        Body {
            p: [x, map.height(x, z), z],
            ground: true,
            ..Body::default()
        }
    }

    #[test]
    fn it_hauls_you_up_to_where_it_caught_and_lets_go() {
        let map = Map::new(3);
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
        let map = Map::new(3);
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

    #[test]
    fn cast_falling_it_hauls_you_on_level_and_as_soon() {
        let map = Map::new(3);
        // Six metres over the plaza, falling fast, a rope caught level
        // with you thirty metres east.
        let haul = |fall: f32| {
            let mut b = stand(&map);
            b.p[1] += 6.0;
            b.ground = false;
            b.v = [0.0, -fall, 0.0];
            let y = b.p[1];
            b.anchor = [b.p[0] + 30.0, y, b.p[2]];
            b.tether = TETHER_TICKS;
            let (mut low, mut ticks) = (y, 0);
            while b.tether > 0 {
                step(&mut b, &Input::default(), &map);
                low = low.min(b.p[1]);
                ticks += 1;
            }
            (y - low, ticks)
        };
        // Its own weight bows the rope a little; a fall at the cast, not
        // much more (it is taken up as the pull takes hold), and it gets
        // there as soon.
        let (dip, ticks) = haul(0.0);
        assert!(
            dip < 0.6 && ticks <= 42,
            "level: {dip} m down, {ticks} ticks"
        );
        let (fell, late) = haul(12.0);
        assert!(
            fell < 1.5 && late <= ticks + 1,
            "falling: {fell} m, {late} ticks"
        );
    }

    #[test]
    fn steering_square_to_the_rope_swings_you_off_its_line() {
        let map = Map::new(3);
        let swing = |keys: u16| {
            let mut b = stand(&map);
            b.p[1] += 3.0;
            b.ground = false;
            let z = b.p[2];
            b.anchor = [b.p[0] + 40.0, b.p[1] + 4.0, z];
            b.tether = TETHER_TICKS;
            let mut most: f32 = 0.0;
            for _ in 0..TETHER_TICKS {
                step(
                    &mut b,
                    &Input {
                        keys,
                        ..Input::default()
                    },
                    &map,
                );
                most = most.max(z - b.p[2]);
                if b.tether == 0 {
                    break;
                }
            }
            most
        };
        assert!(swing(0) < 0.1, "straight along it, let be");
        assert!(swing(keys::LEFT) > 1.5, "swung left: {}", swing(keys::LEFT));
    }
}
