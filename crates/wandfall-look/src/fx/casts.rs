//! What each cast and landing shows, as the events come: a circle of runes
//! at the caster's wand, a beam of light (Lance), frost's fan and its
//! shatter, a ward going up and breaking, mending, a level gained; the
//! big ones are in `blasts`.

use render::geo::{self, mix, V3};
use render::{Shape, Spark};
use wandfall::laws::spell;
use wandfall::proto::Ev;

use super::blasts::{blink, explosion, gust, mark, strike};
use super::{
    aim, beam, colour, crystal, dir, glint, light, puffs, ring, rnd, shell, sigil, spray, Draw,
    Ray, Spray, WHITE,
};
use crate::look::{Look, GOLD};

const UP: V3 = [0.0, 1.0, 0.0];

/// What casts, landings, marks, beams and levels show, `now` ms. `me`:
/// your id and your wand's tip (your beams start there).
pub fn shows(
    look: &Look,
    d: &mut Draw,
    list: &[(f64, Ev)],
    now: f64,
    me: (u16, Option<V3>),
    at_of: impl Fn(u16) -> Option<V3>,
) {
    for (k, &(when, e)) in list.iter().enumerate() {
        let age = (now - when) as f32;
        let secs = age / 1000.0;
        let seed = when as i32 ^ (k as i32).wrapping_mul(7919);
        match e {
            Ev::Cast {
                by,
                spell: s,
                stage,
                at,
            } => {
                let c = colour(s);
                let mine = by == me.0;
                match (s, stage) {
                    (spell::FIREBALL, 1) => explosion(look, d, at, secs, seed),
                    (spell::LIGHTNING, 2) => mark(look, d, at, age, seed),
                    (spell::LIGHTNING, 1) => strike(look, d, at, secs, seed),
                    (spell::BLINK, 0) => {
                        blink(look, d, [at[0], at[1] - 1.6, at[2]], secs, seed, true)
                    }
                    (spell::BLINK, 1) => {
                        blink(look, d, [at[0], at[1] - 1.0, at[2]], secs, seed, false)
                    }
                    (spell::GUST, 0) => gust(look, d, [at[0], at[1] - 1.5, at[2]], (secs, seed)),
                    (spell::FROST, 0) => {
                        let fwd = aim(by, at, &at_of);
                        fan(d, at, fwd, secs, seed);
                        flash(look, d, at, fwd, (s, secs), mine);
                    }
                    (spell::FROST, 1) => shatter(look, d, at, secs, seed),
                    (spell::TETHER, 1) => super::rope::catch(look, d, at, secs, seed),
                    (spell::WARD, 0) => {
                        let f = 1.0 - secs / 0.4;
                        if f > 0.0 {
                            let r = 0.4 + (secs / 0.25).min(1.0) * 1.0;
                            let mid = [at[0], at[1] - 0.6, at[2]];
                            // A shell about your own eyes would blind you.
                            if !mine {
                                shell(look, d, mid, [r, r * 1.2, r], c, f);
                            }
                            let feet = [at[0], at[1] - 1.5, at[2]];
                            sigil(look, d, feet, UP, 0.6 + secs * 2.0, (c, f), secs * 2.0);
                            light(d, mid, 5.0, c, 2.0 * f);
                        }
                    }
                    (spell::WARD, 2) => {
                        let f = 1.0 - secs / 0.5;
                        if f > 0.0 {
                            let r = 1.0 + secs * 3.0;
                            if !mine {
                                shell(look, d, at, [r, r * 1.2, r], c, f * 0.7);
                            }
                            // Shards of it, flung out and falling.
                            for n in 0..12 {
                                let w = dir(seed, n, false);
                                let go = 3.5 * secs * (0.6 + 0.4 * rnd(seed, n, 3));
                                let p = [
                                    at[0] + w[0] * (1.0 + go),
                                    at[1] + w[1] * (1.2 + go) - 4.0 * secs * secs,
                                    at[2] + w[2] * (1.0 + go),
                                ];
                                crystal(look, d, p, w, (0.22, 0.05), mix(c, WHITE, 0.4), f);
                            }
                            spray(
                                d,
                                at,
                                secs,
                                seed,
                                Spray {
                                    fall: 6.0,
                                    shape: Shape::Star,
                                    ..Spray::new(14, 7.0, 0.5, (WHITE, c), 0.35)
                                },
                            );
                            light(d, at, 6.0, c, 3.0 * f);
                        }
                    }
                    (spell::MEND, 0) => {
                        let f = 1.0 - secs / 0.8;
                        if f > 0.0 {
                            let feet = [at[0], at[1] - 1.55, at[2]];
                            sigil(
                                look,
                                d,
                                [feet[0], feet[1] + 0.06, feet[2]],
                                UP,
                                1.1,
                                (c, f),
                                secs,
                            );
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
                            light(d, at, 5.0, c, 1.5 * f);
                        }
                    }
                    (_, 0) => {
                        let fwd = aim(by, at, &at_of);
                        flash(look, d, at, fwd, (s, secs), mine);
                    }
                    _ => {}
                }
            }
            Ev::Beam {
                by,
                spell: s,
                from,
                to,
            } => {
                let from = match me {
                    (id, Some(tip)) if id == by => tip,
                    _ => from,
                };
                lance(look, d, (from, to), (s, secs, seed), by == me.0);
            }
            Ev::Level { who, .. } => {
                if let Some(p) = at_of(who) {
                    level(look, d, p, age, seed);
                }
            }
            _ => {}
        }
    }
}

/// A cast leaving a wand at `at` toward `fwd`: a small circle of runes
/// facing the way it went, turning fast and gone, a glint and a flash
/// (not a circle in your own face).
fn flash(look: &Look, d: &mut Draw, at: V3, fwd: V3, (s, secs): (u8, f32), mine: bool) {
    let c = colour(s);
    let f = 1.0 - secs / 0.3;
    if f <= 0.0 {
        return;
    }
    if !mine {
        let out = geo::add(at, geo::scale(fwd, 0.25));
        sigil(look, d, out, fwd, 0.28 + secs * 0.8, (c, f), secs * 9.0);
    }
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

/// Frost landing: ice bursting, glints and a puff of cold mist.
fn shatter(look: &Look, d: &mut Draw, at: V3, secs: f32, seed: i32) {
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
    light(d, at, 3.0, c, 1.6 * f);
}

/// Lance: a beam of light from `from` to `to`, a white-hot core in a
/// flowing halo, motes winding round it, a flare where it strikes. Yours
/// starts at your wand, by your eye: its halo and motes start a little
/// way out.
fn lance(
    look: &Look,
    d: &mut Draw,
    (from, to): (V3, V3),
    (s, secs, seed): (u8, f32, i32),
    mine: bool,
) {
    let f = 1.0 - secs / 0.4;
    if f <= 0.0 {
        return;
    }
    let c = colour(s);
    let core = 0.045 * (0.5 + 0.5 * f);
    beam(
        look,
        d,
        (from, to),
        core,
        (mix(c, WHITE, 0.75), 1.0),
        Ray::Core,
    );
    let way = geo::norm(geo::sub(to, from));
    let skip = if mine { 1.2 } else { 0.0 };
    let out = geo::add(from, geo::scale(way, skip));
    let halo = 0.24 * (0.4 + 0.6 * f);
    beam(look, d, (out, to), halo, (c, f * 0.8), Ray::Flow(5.0));
    beam(look, d, (out, to), halo * 1.5, (c, f * 0.5), Ray::Halo);
    // Motes winding round it, and drifting off.
    let along = geo::sub(to, from);
    let len = geo::dot(along, along).sqrt().max(0.01);
    let (sx, sy) = super::across(way);
    let start = (skip / len).min(1.0);
    for n in 0..40 {
        let u = start + (1.0 - start) * (n as f32 + 0.5) / 40.0;
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
        let twinkle = (1.0 - (u * 2.0 - 1.0).abs()).powi(2);
        glint(
            d,
            [
                p[0] + a.cos() * 0.7,
                p[1] + 0.4 + u * 2.4,
                p[2] + a.sin() * 0.7,
            ],
            0.6,
            WHITE,
            twinkle * f,
        );
    }
    light(d, [p[0], p[1] + 1.5, p[2]], 6.0, GOLD, 1.5 * f);
}
