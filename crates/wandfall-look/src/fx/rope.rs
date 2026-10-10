//! The Tether: its rope of light from a wizard's hand to where it caught,
//! thrown out slack and wavering, then taut as it hauls them, motes
//! running up it; and the catch, sparks where it bit.

use std::f32::consts::PI;

use render::geo::{self, mix, V3};
use render::{Shape, Spark};
use wandfall::laws::spell;
use wandfall::proto::{fx, Ev, Seen};

use super::{
    across, beam, colour, glint, light, ring, rnd, spray, Casters, Clock, Draw, Ray, Spray, WHITE,
};
use crate::look::Look;

/// How long the rope takes to fly out to where it caught (ms), and how
/// long it wavers before it is taut.
const THROW: f32 = 110.0;
const SETTLE: f32 = 350.0;

/// Every rope hauling someone now (`fx::TETHER`): from their wand (as
/// `who` has it drawn) to where their last Tether caught.
pub fn ropes(
    look: &Look,
    d: &mut Draw,
    wizards: &[Seen],
    shows: &[(f64, Ev)],
    clock: Clock,
    who: &Casters,
) {
    for s in wizards.iter().filter(|s| s.fx & fx::TETHER != 0) {
        let caught = shows.iter().rev().find_map(|&(when, e)| match e {
            Ev::Cast {
                by,
                spell: spell::TETHER,
                stage: 1,
                at,
            } if by == s.id => Some((when, at)),
            _ => None,
        });
        if let Some((when, to)) = caught {
            let hand = who
                .drawn(s.id)
                .map_or([s.p[0], s.p[1] + 1.25, s.p[2]], |w| w.tip);
            rope(look, d, (hand, to), clock.age(when) as f32, s.id as i32);
        }
    }
}

/// A rope from `a` to `b`, `age` ms after it was thrown.
fn rope(look: &Look, d: &mut Draw, (a, b): (V3, V3), age: f32, seed: i32) {
    let c = colour(spell::TETHER);
    let along = geo::sub(b, a);
    let len = geo::dot(along, along).sqrt();
    if len < 0.05 {
        return;
    }
    let out = (age / THROW).clamp(0.0, 1.0);
    let way = geo::scale(along, 1.0 / len);
    let (side, other) = across(way);
    let t = age / 1000.0;
    // Slack as it flies, a wavering that fades once it bites.
    let wob = (0.45 * (1.0 - (age / SETTLE).min(1.0)) + 0.03) * (len / 6.0).min(1.0);
    const N: usize = 14;
    let at = |k: usize| {
        let f = k as f32 / N as f32;
        let bow = (f * PI).sin() * wob;
        let s1 = (f * PI * 3.0 - t * 28.0 + seed as f32).sin() * bow;
        let s2 = (f * PI * 2.0 + t * 21.0).cos() * bow * 0.6;
        geo::add(
            geo::add(a, geo::scale(along, f * out)),
            geo::add(geo::scale(side, s1), geo::scale(other, s2)),
        )
    };
    let core = mix(c, WHITE, 0.55);
    for k in 0..N {
        let (p, q) = (at(k), at(k + 1));
        beam(look, d, (p, q), 0.03, (core, 1.0), Ray::Core);
        beam(look, d, (p, q), 0.07, (c, 0.45), Ray::Halo);
    }
    let head = at(N);
    if out < 1.0 {
        glint(d, head, 0.5, WHITE, 1.0);
    } else {
        // Motes running up it toward where it bit, on it as it wavers.
        for n in 0..7 {
            let f = (t * 2.2 + n as f32 / 7.0 + rnd(seed, n, 9)).fract();
            let u = f * N as f32;
            let k = (u as usize).min(N - 1);
            let p = geo::add(at(k), geo::scale(geo::sub(at(k + 1), at(k)), u - k as f32));
            d.sparks.push(Spark {
                p,
                size: 0.1,
                c: [c[0], c[1], c[2], 0.9 * (1.0 - f * 0.5)],
                v: geo::scale(way, 4.0),
                ..Default::default()
            });
        }
        d.sparks.push(Spark {
            p: head,
            size: 0.35,
            c: [core[0], core[1], core[2], 0.8],
            shape: Shape::Star,
            ..Default::default()
        });
    }
    light(d, a, 3.0, c, 0.9);
}

/// Where a Tether bit, `secs` after: a flash, sparks, a ring.
pub(super) fn catch(look: &Look, d: &mut Draw, at: V3, secs: f32, seed: i32) {
    let f = 1.0 - secs / 0.45;
    if f <= 0.0 {
        return;
    }
    let c = colour(spell::TETHER);
    if secs < 0.12 {
        glint(d, at, 1.6 * (1.0 - secs / 0.12), WHITE, 1.0);
    }
    spray(
        d,
        at,
        secs,
        seed,
        Spray {
            fall: 9.0,
            streak: 0.03,
            ..Spray::new(14, 6.0, 0.4, (WHITE, c), 0.07)
        },
    );
    ring(look, d, at, 0.2 + secs * 2.5, c, f * 0.7, secs * 3.0);
    light(d, at, 5.0, c, 2.0 * f);
}
