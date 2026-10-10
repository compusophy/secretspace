//! What each cast and landing shows, as the events come: a circle of runes
//! at the caster's wand, a beam of light (Lance), frost's fan and its
//! shatter, a ward going up and breaking, mending, a level gained; the
//! big ones are in `blasts`. What stays with its caster (a ward, mending,
//! a gust, the flash at its wand) is drawn where the caster is drawn; what
//! it flung (frost's fan) goes the way it looked as it cast.

use render::geo::{self, mix, V3};
use render::{Shape, Spark};
use wandfall::laws::spell;
use wandfall::proto::Ev;

use super::blasts::{blink, explosion, gust, mark, strike};
use super::{
    colour, crystal, dir, glint, light, lone, puffs, ring, rnd, seed_of, shell, sigil, spray,
    twinkle, Casters, Clock, Draw, Ray, Spray, UP, WHITE,
};
use crate::look::{Look, GOLD};

/// What casts, landings, marks, beams and levels show, by `clock`, of
/// the casters `who` says.
pub fn shows(look: &Look, d: &mut Draw, list: &[(f64, Ev)], clock: Clock, who: &Casters) {
    for (k, &(when, e)) in list.iter().enumerate() {
        let age = clock.age(when) as f32;
        let secs = age / 1000.0;
        let seed = seed_of(when, &e);
        match e {
            Ev::Cast {
                by,
                spell: s,
                stage,
                at,
            } => {
                // The caster as drawn now, and as it stood casting; else
                // what the cast says.
                let drawn = who.drawn(by);
                let stood = who.stood(when, by);
                let feet = |up: f32| {
                    drawn.map(|w| w.feet).or(stood.map(|s| s.1)).unwrap_or([
                        at[0],
                        at[1] - up,
                        at[2],
                    ])
                };
                let wand = drawn.map_or(at, |w| w.tip);
                let fwd = stood.map_or([1.0, 0.0, 0.0], |s| s.0);
                match (s, stage) {
                    (spell::FIREBALL, 1) => explosion(look, d, at, secs, seed),
                    (spell::LIGHTNING, 2) => mark(look, d, at, age, seed),
                    (spell::LIGHTNING, 1) => strike(look, d, at, secs, seed),
                    // Gone from where it stood (its drawn self leaps
                    // away a moment later).
                    (spell::BLINK, 0) => {
                        let from = stood.map_or([at[0], at[1] - 1.6, at[2]], |s| s.1);
                        blink(look, d, from, secs, seed, true)
                    }
                    (spell::BLINK, 1) => {
                        blink(look, d, [at[0], at[1] - 1.0, at[2]], secs, seed, false)
                    }
                    (spell::GUST, 0) => gust(look, d, feet(1.5), (secs, seed)),
                    (spell::FROST, 0) => {
                        fan(d, wand, fwd, secs, seed);
                        flash(look, d, wand, fwd, (s, secs));
                    }
                    (spell::FROST, 1) => {
                        // Its shards land together: one light where they
                        // land on one another.
                        let lit = !list[..k].iter().any(|&(w, o)| {
                            w == when
                                && matches!(o, Ev::Cast {
                                    spell: spell::FROST,
                                    stage: 1,
                                    at: p,
                                    ..
                                } if geo::dot(geo::sub(p, at), geo::sub(p, at)) < 4.0)
                        });
                        shatter(look, d, at, (secs, seed), lit)
                    }
                    (spell::TETHER, 1) => super::rope::catch(look, d, at, secs, seed),
                    (spell::WARD, 0) => ward(look, d, feet(1.5), secs),
                    (spell::WARD, 2) => {
                        let mid = drawn.map_or(at, |w| [w.feet[0], w.feet[1] + 1.0, w.feet[2]]);
                        ward_breaks(look, d, mid, secs, seed)
                    }
                    (spell::MEND, 0) => mend(look, d, feet(1.55), secs, seed),
                    (_, 0) => flash(look, d, wand, fwd, (s, secs)),
                    _ => {}
                }
            }
            // From the caster's wand as drawn.
            Ev::Beam {
                by,
                spell: s,
                from,
                to,
            } => {
                let from = who.drawn(by).map_or(from, |w| w.tip);
                lance(look, d, (from, to), (s, secs, seed));
            }
            Ev::Level { who: id, .. } => {
                if let Some(w) = who.drawn(id) {
                    level(look, d, w.feet, age, seed);
                }
            }
            _ => {}
        }
    }
}

/// A ward going up about a wizard standing at `feet`: a bubble rising
/// over it, a circle of runes spreading under it.
fn ward(look: &Look, d: &mut Draw, feet: V3, secs: f32) {
    let c = colour(spell::WARD);
    let f = 1.0 - secs / 0.4;
    if f <= 0.0 {
        return;
    }
    let r = 0.4 + (secs / 0.25).min(1.0) * 1.0;
    let mid = [feet[0], feet[1] + 0.95, feet[2]];
    shell(look, d, mid, [r, r * 1.2, r], c, f);
    let floor = [feet[0], feet[1] + 0.06, feet[2]];
    sigil(look, d, floor, UP, 0.6 + secs * 2.0, (c, f), secs * 2.0);
    light(d, mid, 5.0, c, 2.0 * f);
}

/// A ward breaking about `mid`: its bubble flung out and gone, shards of
/// it falling.
fn ward_breaks(look: &Look, d: &mut Draw, mid: V3, secs: f32, seed: i32) {
    let c = colour(spell::WARD);
    let f = 1.0 - secs / 0.5;
    if f <= 0.0 {
        return;
    }
    let r = 1.0 + secs * 3.0;
    shell(look, d, mid, [r, r * 1.2, r], c, f * 0.7);
    for n in 0..12 {
        let w = dir(seed, n, false);
        let go = 3.5 * secs * (0.6 + 0.4 * rnd(seed, n, 3));
        let p = [
            mid[0] + w[0] * (1.0 + go),
            mid[1] + w[1] * (1.2 + go) - 4.0 * secs * secs,
            mid[2] + w[2] * (1.0 + go),
        ];
        crystal(look, d, p, w, (0.22, 0.05), mix(c, WHITE, 0.4), f);
    }
    spray(
        d,
        mid,
        secs,
        seed,
        Spray {
            fall: 6.0,
            shape: Shape::Star,
            ..Spray::new(14, 7.0, 0.5, (WHITE, c), 0.35)
        },
    );
    light(d, mid, 6.0, c, 3.0 * f);
}

/// Mending a wizard standing at `feet`: a circle of runes under it, a
/// ring rising, motes winding up.
fn mend(look: &Look, d: &mut Draw, feet: V3, secs: f32, seed: i32) {
    let c = colour(spell::MEND);
    let f = 1.0 - secs / 0.8;
    if f <= 0.0 {
        return;
    }
    let floor = [feet[0], feet[1] + 0.06, feet[2]];
    sigil(look, d, floor, UP, 1.1, (c, f), secs);
    let rise = [feet[0], feet[1] + secs * 3.0, feet[2]];
    ring(look, d, rise, 0.9, c, f, 0.0);
    spray(
        d,
        [feet[0], feet[1] + 0.4, feet[2]],
        secs,
        seed,
        Spray {
            fall: -3.0,
            up: true,
            ..Spray::new(16, 1.6, 0.8, (WHITE, c), 0.14)
        },
    );
    light(d, [feet[0], feet[1] + 1.5, feet[2]], 5.0, c, 1.5 * f);
}

/// A cast leaving a wand at `at` toward `fwd`: a small circle of runes
/// facing the way it went, turning fast and gone, a glint and a flash.
fn flash(look: &Look, d: &mut Draw, at: V3, fwd: V3, (s, secs): (u8, f32)) {
    let c = colour(s);
    let f = 1.0 - secs / 0.3;
    if f <= 0.0 {
        return;
    }
    let out = geo::add(at, geo::scale(fwd, 0.25));
    sigil(look, d, out, fwd, 0.28 + secs * 0.8, (c, f), secs * 9.0);
    glint(d, at, 0.9 * f, mix(c, WHITE, 0.4), f);
    light(d, at, 6.0, c, 3.0 * f);
}

/// Frost's fan: a spray of ice streaking out the way it was cast.
fn fan(d: &mut Draw, at: V3, fwd: V3, secs: f32, seed: i32) {
    let c = colour(spell::FROST);
    let f = 1.0 - secs / 0.3;
    if f <= 0.0 {
        return;
    }
    for n in 0..16 {
        let j = dir(seed, n, false);
        let v = geo::add(geo::scale(fwd, 10.0), geo::scale(j, 2.8));
        d.sparks.push(Spark {
            p: geo::add(at, geo::scale(v, secs)),
            size: 0.1 * f + 0.03,
            c: [c[0], c[1], c[2], f],
            v: geo::scale(v, 0.03),
            ..Default::default()
        });
    }
    puffs(
        d,
        at,
        (secs, seed),
        (5, 0.6),
        ([0.8, 0.92, 1.0], 0.35),
        (0.3, 2.5),
        (0.8, 0.2),
    );
}

/// Frost landing: ice bursting, glints and a puff of cold mist; its own
/// light if `lit` (not when its fellow shards light it).
fn shatter(look: &Look, d: &mut Draw, at: V3, (secs, seed): (f32, i32), lit: bool) {
    let c = colour(spell::FROST);
    let f = 1.0 - secs / 0.5;
    if f <= 0.0 {
        return;
    }
    for n in 0..9 {
        let w = dir(seed, n, true);
        let go = 3.0 * secs;
        let p = geo::add(at, geo::scale(w, 0.2 + go));
        crystal(look, d, p, w, (0.45 * f + 0.1, 0.1), mix(c, WHITE, 0.4), f);
    }
    glint(d, at, 1.6 * f, WHITE, f);
    spray(
        d,
        at,
        secs,
        seed,
        Spray {
            fall: 6.0,
            streak: 0.04,
            ..Spray::new(12, 5.0, 0.4, (WHITE, c), 0.1)
        },
    );
    spray(
        d,
        at,
        secs,
        seed + 1,
        Spray {
            shape: Shape::Star,
            ..Spray::new(7, 2.0, 0.5, (WHITE, c), 0.6)
        },
    );
    puffs(
        d,
        at,
        (secs, seed),
        (6, 1.0),
        ([0.8, 0.92, 1.0], 0.5),
        (0.7, 2.2),
        (1.0, 0.3),
    );
    if lit {
        light(d, at, 3.0, c, 1.6 * f);
    }
}

/// Lance: a beam of light from `from` (the caster's wand) to `to`, a
/// white-hot core in a flowing halo, motes winding round it, a flare
/// where it strikes.
fn lance(look: &Look, d: &mut Draw, (from, to): (V3, V3), (s, secs, seed): (u8, f32, i32)) {
    let f = 1.0 - secs / 0.4;
    if f <= 0.0 {
        return;
    }
    let c = colour(s);
    let core = 0.045 * (0.5 + 0.5 * f);
    lone(
        look,
        d,
        (from, to),
        core,
        (mix(c, WHITE, 0.75), 1.0),
        Ray::Core,
    );
    let way = geo::norm(geo::sub(to, from));
    let halo = 0.13 * (0.4 + 0.6 * f);
    lone(look, d, (from, to), halo, (c, f * 0.5), Ray::Flow(5.0));
    lone(look, d, (from, to), halo * 1.8, (c, f * 0.45), Ray::Halo);
    // Motes winding round it, and drifting off.
    let along = geo::sub(to, from);
    let len = geo::dot(along, along).sqrt().max(0.01);
    let (sx, sy) = super::across(way);
    for n in 0..40 {
        let u = (n as f32 + 0.5) / 40.0;
        let a = u * len * 2.4 - secs * 14.0 + n as f32 * 0.3;
        let r = 0.16 + secs * 0.8;
        let off = geo::add(geo::scale(sx, a.cos() * r), geo::scale(sy, a.sin() * r));
        let drift = geo::scale(dir(seed, n, false), secs * 1.2);
        d.sparks.push(Spark {
            p: geo::add(geo::add(geo::add(from, geo::scale(along, u)), off), drift),
            size: 0.07,
            c: [c[0], c[1], c[2], f],
            ..Default::default()
        });
    }
    glint(d, to, 2.2 * f, mix(c, WHITE, 0.5), f);
    spray(
        d,
        to,
        secs,
        seed + 1,
        Spray {
            fall: 6.0,
            streak: 0.05,
            ..Spray::new(14, 7.0, 0.4, (WHITE, c), 0.1)
        },
    );
    sigil(
        look,
        d,
        to,
        geo::scale(way, -1.0),
        0.3 + secs * 2.5,
        (c, f * 0.8),
        secs * 6.0,
    );
    light(d, to, 7.0, c, 2.5 * f);
    light(d, from, 4.0, c, 1.2 * f);
}

/// A level gained: a golden circle of runes rising about the wizard,
/// sparks winding up, glints.
fn level(look: &Look, d: &mut Draw, p: V3, age: f32, seed: i32) {
    let f = (1.0 - age / 1400.0).max(0.0);
    if f <= 0.0 {
        return;
    }
    let k = age / 1400.0;
    sigil(
        look,
        d,
        [p[0], p[1] + 0.06 + k * 2.2, p[2]],
        UP,
        0.9 + k * 0.3,
        (GOLD, f),
        age / 500.0,
    );
    for n in 0..28 {
        let a = n as f32 / 28.0 * std::f32::consts::TAU + age / 260.0;
        let rise = ((age / 1200.0) + n as f32 / 28.0).fract() * 3.0;
        d.sparks.push(Spark {
            p: [p[0] + a.cos() * 0.8, p[1] + rise, p[2] + a.sin() * 0.8],
            size: 0.14,
            c: [GOLD[0], GOLD[1], GOLD[2], f],
            v: [-a.sin() * 0.25, 0.4, a.cos() * 0.25],
            ..Default::default()
        });
    }
    for n in 0..4 {
        let u = ((age / 600.0) + rnd(seed, n, 2)).fract();
        let a = rnd(seed, n, 3) * std::f32::consts::TAU;
        glint(
            d,
            [
                p[0] + a.cos() * 0.7,
                p[1] + 0.4 + u * 2.4,
                p[2] + a.sin() * 0.7,
            ],
            0.6,
            WHITE,
            twinkle(u) * f,
        );
    }
    light(d, [p[0], p[1] + 1.5, p[2]], 6.0, GOLD, 1.5 * f);
}
