//! What spells, loot and levels look like. Every spell has one colour
//! and one shape, the same in its icon, its bolt, its landing and the
//! marks it leaves, so a glance says what is coming:
//!
//! - Fireball (orange): a roaring ball with a trail of flame; a burst of
//!   fire, a shockwave on the ground, embers.
//! - Lance (gold): a beam of light, there and gone.
//! - Frost (ice blue): a fan of shards; crystals about a chilled foot.
//! - Lightning (violet): a ring on the ground closing in, then the bolt.
//! - Blink (pink): gone in sparks, there in sparks.
//! - Ward (blue): a bubble that bursts when it breaks.
//! - Mend (green): motes rising.
//! - Gust (pale): a ring of wind and a dome thrown outward.

use render::geo::{self, hash, mix, rgb, unit, V3};
use render::{m4, Item, Light, Material, Mesh, Pass, Spark};
use wandfall::laws::{
    spell, FIREBALL_RADIUS, GUST_RADIUS, LIGHTNING_DELAY, LIGHTNING_RADIUS, TICK_HZ,
};
use wandfall::proto::{fx, BoltSeen, Ev, Loot, Seen};
use wandfall::world::WAND;

use crate::look::{Look, GOLD};

/// What one frame draws besides the island.
#[derive(Default)]
pub struct Draw {
    pub items: Vec<Item>,
    pub lights: Vec<Light>,
    pub sparks: Vec<Spark>,
}

const WHITE: V3 = [1.0; 3];

/// A spell's colour (the wand's is gold).
pub fn colour(s: u8) -> V3 {
    match s {
        spell::FIREBALL => rgb(255, 120, 40),
        spell::LANCE => rgb(255, 215, 90),
        spell::FROST => rgb(120, 225, 255),
        spell::LIGHTNING => rgb(165, 115, 255),
        spell::BLINK => rgb(255, 100, 200),
        spell::WARD => rgb(80, 140, 255),
        spell::MEND => rgb(120, 235, 110),
        spell::GUST => rgb(215, 235, 245),
        _ => GOLD,
    }
}

/// A random direction, the same for the same seed and n (`up`: only
/// upward).
fn dir(seed: i32, n: i32, up: bool) -> V3 {
    let a = unit(hash(seed, n, 11)) * std::f32::consts::TAU;
    let y = unit(hash(seed, n, 12)) * 2.0 - 1.0;
    let y = if up { y.abs() } else { y };
    let r = (1.0 - y * y).max(0.0).sqrt();
    [r * a.cos(), y, r * a.sin()]
}

fn glow(d: &mut Draw, mesh: Mesh, m: render::M4, c: V3, a: f32) {
    d.items
        .push(Item::new(mesh, m).tint(c, a).glow(1.0).pass(Pass::Glow));
}

/// An energy shell about `at` (bright at its edge).
fn shell(look: &Look, d: &mut Draw, at: V3, r: V3, c: V3, a: f32) {
    d.items.push(
        Item::new(look.orb, m4::place(at, 0.0, r))
            .tint(c, a)
            .glow(0.4)
            .material(Material::Rim)
            .pass(Pass::Glow),
    );
}

/// A ring lying flat at `at`.
fn ring(look: &Look, d: &mut Draw, at: V3, r: f32, c: V3, a: f32, turn: f32) {
    if r > 0.01 && a > 0.0 {
        glow(d, look.ring, m4::place(at, turn, [r, 1.0, r]), c, a);
    }
}

/// A beam from `a` to `b`, `r` thick.
fn beam(look: &Look, d: &mut Draw, (a, b): (V3, V3), r: f32, (c, alpha): (V3, f32), rim: bool) {
    let along = geo::sub(b, a);
    let len = geo::dot(along, along).sqrt();
    if len < 0.01 {
        return;
    }
    let up = geo::scale(along, 1.0 / len);
    let side = geo::norm(geo::cross(
        up,
        if up[1].abs() < 0.9 {
            [0.0, 1.0, 0.0]
        } else {
            [1.0, 0.0, 0.0]
        },
    ));
    let other = geo::cross(up, side);
    let m = m4::basis(a, geo::scale(side, r), along, geo::scale(other, r));
    // A core burns hot (it blooms); a rim is a halo about it.
    let it = Item::new(look.beam, m).tint(c, alpha).pass(Pass::Glow);
    d.items.push(if rim {
        it.glow(1.0).material(Material::Rim)
    } else {
        it.glow(3.0)
    });
}

/// A shard at `at` pointing along `dir`, `len` long and `w` wide.
fn shard(look: &Look, d: &mut Draw, at: V3, dir: V3, (len, w): (f32, f32), c: V3, a: f32) {
    let side = geo::norm(geo::cross(
        dir,
        if dir[1].abs() < 0.9 {
            [0.0, 1.0, 0.0]
        } else {
            [1.0, 0.0, 0.0]
        },
    ));
    let other = geo::cross(dir, side);
    let m = m4::basis(
        at,
        geo::scale(dir, len),
        geo::scale(side, w),
        geo::scale(other, w),
    );
    glow(d, look.shard, m, c, a);
}

fn light(d: &mut Draw, p: V3, r: f32, c: V3, k: f32) {
    if k > 0.01 {
        d.lights.push(Light {
            p,
            r,
            c: geo::scale(c, k),
        });
    }
}

/// Sparks thrown from `at` at up to `speed` m/s, `t` seconds ago, falling
/// by `fall` m/s², fading out by `life` seconds.
#[allow(clippy::too_many_arguments)]
fn spray(
    d: &mut Draw,
    at: V3,
    n: i32,
    speed: f32,
    (t, life): (f32, f32),
    fall: f32,
    (hot, c): (V3, V3),
    size: f32,
    seed: i32,
) {
    let f = 1.0 - t / life;
    if f <= 0.0 {
        return;
    }
    for k in 0..n {
        let v = geo::scale(
            dir(seed, k, fall > 0.0),
            speed * (0.35 + 0.65 * unit(hash(seed, k, 13))),
        );
        let p = [
            at[0] + v[0] * t,
            at[1] + v[1] * t - 0.5 * fall * t * t,
            at[2] + v[2] * t,
        ];
        let col = mix(hot, c, (1.0 - f).min(1.0));
        d.sparks.push(Spark {
            p,
            size: size * (0.4 + 0.6 * f),
            c: [col[0], col[1], col[2], f],
        });
    }
}

/// A bolt in flight: the wand's, or a spell's.
pub fn bolt(look: &Look, d: &mut Draw, b: &BoltSeen, mine: bool, t: f32) {
    let speed = geo::dot(b.v, b.v).sqrt().max(1.0);
    let fwd = geo::scale(b.v, 1.0 / speed);
    let id = b.id as i32;
    let trail = |d: &mut Draw, n: i32, step: f32, (hot, c): (V3, V3), size: f32, rise: f32| {
        for k in 0..n {
            let u = k as f32 / n as f32;
            let fade = 1.0 - u;
            let j = |q| (unit(hash(id, k + (t * 30.0) as i32, q)) - 0.5) * u * 0.5;
            let col = mix(hot, c, u);
            d.sparks.push(Spark {
                p: [
                    b.p[0] - fwd[0] * k as f32 * step + j(1),
                    b.p[1] - fwd[1] * k as f32 * step + j(2) + rise * u,
                    b.p[2] - fwd[2] * k as f32 * step + j(3),
                ],
                size: size * fade + 0.04,
                c: [col[0], col[1], col[2], fade],
            });
        }
    };
    match b.kind {
        spell::FIREBALL => {
            let c = colour(b.kind);
            let roar = 1.0 + 0.08 * (t * 31.0 + id as f32).sin();
            glow(
                d,
                look.orb,
                m4::place(b.p, 0.0, [0.24; 3]),
                mix(c, WHITE, 0.7),
                1.0,
            );
            shell(look, d, b.p, [0.5 * roar; 3], c, 1.0);
            trail(
                d,
                26,
                0.17,
                (rgb(255, 226, 150), rgb(230, 60, 20)),
                0.5,
                0.8,
            );
            light(d, b.p, 13.0, c, 1.6);
        }
        spell::FROST => {
            let c = colour(b.kind);
            shard(look, d, b.p, fwd, (0.45, 0.07), mix(c, WHITE, 0.3), 1.0);
            trail(d, 6, 0.25, (mix(c, WHITE, 0.6), c), 0.12, 0.0);
            light(d, b.p, 3.5, c, 0.7);
        }
        _ => {
            let c = if b.kind != WAND {
                colour(b.kind)
            } else if mine {
                GOLD
            } else {
                rgb(255, 150, 120)
            };
            glow(
                d,
                look.orb,
                m4::place(b.p, 0.0, [0.13; 3]),
                mix(c, WHITE, 0.5),
                1.0,
            );
            trail(d, 9, 0.2, (mix(c, WHITE, 0.4), c), 0.26, 0.0);
            light(d, b.p, 6.0, c, 0.9);
        }
    }
}

/// The marks spells leave on a wizard.
/// `me`: it is you (seen from inside: no bubble about your eyes).
pub fn on_wizard(look: &Look, d: &mut Draw, s: &Seen, t: f32, me: bool) {
    let at = s.p;
    let id = s.id as i32;
    if s.fx & fx::SHIELD != 0 && me {
        let c = colour(spell::WARD);
        ring(look, d, [at[0], at[1] + 0.05, at[2]], 1.0, c, 0.7, t);
    } else if s.fx & fx::SHIELD != 0 {
        let c = colour(spell::WARD);
        let wob = 1.0 + 0.03 * (t * 6.0 + id as f32).sin();
        shell(
            look,
            d,
            [at[0], at[1] + 0.95, at[2]],
            [0.95 * wob, 1.2 * wob, 0.95 * wob],
            c,
            0.9,
        );
        ring(look, d, [at[0], at[1] + 0.05, at[2]], 1.0, c, 0.6, t);
        light(d, [at[0], at[1] + 1.0, at[2]], 3.5, c, 0.8);
    }
    if s.fx & fx::CHILLED != 0 {
        let c = colour(spell::FROST);
        for k in 0..6 {
            let a = k as f32 / 6.0 * std::f32::consts::TAU + id as f32;
            let up = geo::norm([a.cos() * 0.5, 1.0, a.sin() * 0.5]);
            let base = [at[0] + a.cos() * 0.35, at[1] + 0.12, at[2] + a.sin() * 0.35];
            let len = 0.22 + 0.1 * unit(hash(id, k, 3));
            shard(look, d, base, up, (len, 0.06), mix(c, WHITE, 0.4), 0.8);
        }
        for k in 0..6 {
            let a = unit(hash(id, (t * 8.0) as i32 + k, 4)) * std::f32::consts::TAU;
            d.sparks.push(Spark {
                p: [
                    at[0] + a.cos() * 0.45,
                    at[1] + 0.2 + 1.4 * unit(hash(id, k, 5)),
                    at[2] + a.sin() * 0.45,
                ],
                size: 0.09,
                c: [c[0], c[1], c[2], 0.9],
            });
        }
        light(d, [at[0], at[1] + 0.4, at[2]], 2.5, c, 0.7);
    }
    if s.fx & fx::MENDING != 0 {
        let c = colour(spell::MEND);
        for k in 0..10 {
            let a = k as f32 / 10.0 * std::f32::consts::TAU + t * 2.0;
            let rise = (t * 0.9 + k as f32 * 0.1).fract();
            let r = 0.65 - rise * 0.3;
            d.sparks.push(Spark {
                p: [at[0] + a.cos() * r, at[1] + rise * 2.3, at[2] + a.sin() * r],
                size: 0.15,
                c: [c[0], c[1], c[2], 1.0 - rise],
            });
        }
        let pulse = 0.5 + 0.5 * (t * 5.0).sin();
        ring(
            look,
            d,
            [at[0], at[1] + 0.05, at[2]],
            0.8,
            c,
            0.4 + 0.3 * pulse,
            0.0,
        );
        light(d, [at[0], at[1] + 1.0, at[2]], 4.0, c, 0.9);
    }
}

/// The colour and brightness of a wizard's wand tip: the spell it last
/// cast, flaring and fading; gold at rest.
pub fn tip(list: &[(f64, Ev)], who: u16, now: f64) -> (V3, f32) {
    list.iter()
        .rev()
        .find_map(|&(when, e)| match e {
            Ev::Cast {
                by,
                spell,
                stage: 0,
                ..
            } if by == who && now - when < 450.0 => {
                Some((colour(spell), (1.0 - (now - when) / 450.0) as f32))
            }
            _ => None,
        })
        .unwrap_or((GOLD, 0.0))
}

/// Chests (glowing until opened) and scrolls (a crystal of their spell
/// under a pillar of its light, taller by rank).
pub fn loot(look: &Look, d: &mut Draw, l: &Loot, t: f32, eye: V3) {
    let far = |p: V3| (p[0] - eye[0]).powi(2) + (p[2] - eye[2]).powi(2) > 140.0 * 140.0;
    for &(id, p, open) in &l.chests {
        if far(p) {
            continue;
        }
        let m = m4::place(p, id as f32 * 0.7, [1.0; 3]);
        d.items
            .push(Item::new(look.chest, m).glow(if open { 0.0 } else { 0.1 }));
        if !open {
            d.items.push(Item::new(look.lid, m));
            let pulse = 0.7 + 0.3 * (t * 2.0 + id as f32).sin();
            light(d, [p[0], p[1] + 1.0, p[2]], 4.0, GOLD, pulse);
            beam(
                look,
                d,
                ([p[0], p[1] + 0.9, p[2]], [p[0], p[1] + 6.0, p[2]]),
                0.07,
                (GOLD, 0.6 * pulse),
                true,
            );
        }
    }
    for &(id, s, rank, p) in &l.scrolls {
        if far(p) {
            continue;
        }
        let c = colour(s);
        let bob = 0.8 + 0.12 * (t * 2.2 + id as f32).sin();
        let at = [p[0], p[1] + bob, p[2]];
        let a = t * 1.6 + id as f32;
        let side = [a.cos(), 0.0, a.sin()];
        let k = 0.15 + 0.03 * rank as f32;
        let m = m4::basis(
            at,
            [0.0, k * 2.2, 0.0],
            geo::scale(side, k),
            geo::scale([-side[2], 0.0, side[0]], k),
        );
        glow(d, look.shard, m, c, 1.0);
        glow(
            d,
            look.orb,
            m4::place(at, 0.0, [k * 0.45; 3]),
            mix(c, WHITE, 0.8),
            1.0,
        );
        let top = 2.5 + 1.5 * rank as f32;
        beam(
            look,
            d,
            ([p[0], p[1], p[2]], [p[0], p[1] + top, p[2]]),
            0.09 + 0.03 * rank as f32,
            (c, 0.8),
            true,
        );
        ring(look, d, [p[0], p[1] + 0.06, p[2]], 0.55, c, 0.7, t);
        if rank > 1 {
            ring(look, d, [p[0], p[1] + 0.07, p[2]], 0.75, GOLD, 0.6, -t);
        }
        light(d, at, 3.0 + rank as f32, c, 1.2);
    }
}

/// Where a cast came from and which way it went (the caster's eyes to
/// its wand), if the caster is known.
fn aim(by: u16, at: V3, at_of: &impl Fn(u16) -> Option<V3>) -> V3 {
    at_of(by)
        .map(|p| geo::norm(geo::sub(at, [p[0], p[1] + wandfall::laws::EYE, p[2]])))
        .filter(|v| v.iter().all(|x| x.is_finite()))
        .unwrap_or([1.0, 0.0, 0.0])
}

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
                match (s, stage) {
                    (spell::FIREBALL, 1) => explosion(look, d, at, secs, seed),
                    (spell::FROST, 1) => {
                        spray(d, at, 8, 4.0, (secs, 0.3), 6.0, (WHITE, c), 0.1, seed);
                        light(d, at, 2.5, c, 1.5 * (1.0 - secs / 0.2));
                    }
                    (spell::LIGHTNING, 2) => mark(look, d, at, age, seed),
                    (spell::LIGHTNING, 1) => strike(look, d, at, secs, seed),
                    (spell::BLINK, 0) => blink(d, [at[0], at[1] - 1.6, at[2]], secs, seed, true),
                    (spell::BLINK, 1) => {
                        let feet = [at[0], at[1] - 1.0, at[2]];
                        blink(d, feet, secs, seed, false);
                        let f = 1.0 - secs / 0.5;
                        ring(
                            look,
                            d,
                            [feet[0], feet[1] + 0.05, feet[2]],
                            0.4 + secs * 3.0,
                            c,
                            f,
                            0.0,
                        );
                    }
                    (spell::GUST, 0) => {
                        let feet = [at[0], at[1] - 1.5, at[2]];
                        gust(look, d, feet, (secs, seed), by == me.0);
                    }
                    (spell::WARD, 0) => {
                        let f = 1.0 - secs / 0.3;
                        if f > 0.0 {
                            let r = 0.4 + secs * 2.5;
                            let mid = [at[0], at[1] - 0.6, at[2]];
                            // A shell about your own eyes would blind you.
                            if by != me.0 {
                                shell(look, d, mid, [r, r * 1.2, r], c, f);
                            }
                            light(d, mid, 5.0, c, 2.0 * f);
                        }
                    }
                    (spell::WARD, 2) => {
                        let f = 1.0 - secs / 0.35;
                        if f > 0.0 {
                            let r = 1.0 + secs * 3.0;
                            if by != me.0 {
                                shell(look, d, at, [r, r * 1.2, r], c, f);
                            }
                            spray(d, at, 18, 7.0, (secs, 0.45), 9.0, (WHITE, c), 0.13, seed);
                            light(d, at, 6.0, c, 3.0 * f);
                        }
                    }
                    (spell::MEND, 0) => {
                        let f = 1.0 - secs / 0.6;
                        if f > 0.0 {
                            let feet = [at[0], at[1] - 1.55 + secs * 3.0, at[2]];
                            ring(look, d, feet, 0.8, c, f, 0.0);
                            light(d, at, 5.0, c, 1.5 * f);
                        }
                    }
                    (spell::FROST, 0) => {
                        let fwd = aim(by, at, &at_of);
                        let f = 1.0 - secs / 0.25;
                        for n in 0..14 {
                            if f <= 0.0 {
                                break;
                            }
                            let j = dir(seed, n, false);
                            let v = geo::add(geo::scale(fwd, 9.0), geo::scale(j, 2.5));
                            let p = geo::add(at, geo::scale(v, secs));
                            d.sparks.push(Spark {
                                p,
                                size: 0.16 * f + 0.04,
                                c: [c[0], c[1], c[2], f],
                            });
                        }
                        light(d, at, 5.0, c, 2.0 * f.max(0.0));
                    }
                    (_, 0) => {
                        // A flash at the caster's wand.
                        let f = 1.0 - secs / 0.18;
                        light(d, at, 6.0, c, 3.0 * f.max(0.0));
                        spray(d, at, 8, 3.0, (secs, 0.25), 0.0, (WHITE, c), 0.12, seed);
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
                let f = 1.0 - secs / 0.35;
                if f <= 0.0 {
                    continue;
                }
                let from = match me {
                    (id, Some(tip)) if id == by => tip,
                    _ => from,
                };
                let c = colour(s);
                // Yours starts at your wand, by your eye: its halo starts
                // a little way out.
                let core = 0.05 * (0.5 + 0.5 * f);
                beam(look, d, (from, to), core, (mix(c, WHITE, 0.7), 1.0), false);
                let way = geo::norm(geo::sub(to, from));
                let skip = if by == me.0 { 1.2 } else { 0.0 };
                let out = geo::add(from, geo::scale(way, skip));
                let halo = 0.28 * (0.4 + 0.6 * f);
                beam(look, d, (out, to), halo, (c, f), true);
                // Motes drifting off it (not by your own eyes).
                let along = geo::sub(to, from);
                let len = geo::dot(along, along).sqrt().max(0.01);
                let start = (skip * 4.0 / len).min(1.0);
                for n in 0..32 {
                    let u = start + (1.0 - start) * unit(hash(seed, n, 21));
                    let drift = geo::scale(dir(seed, n, false), secs * 1.6);
                    let p = geo::add(geo::add(from, geo::scale(along, u)), drift);
                    d.sparks.push(Spark {
                        p,
                        size: 0.08,
                        c: [c[0], c[1], c[2], f],
                    });
                }
                spray(
                    d,
                    to,
                    12,
                    6.0,
                    (secs, 0.35),
                    4.0,
                    (WHITE, c),
                    0.12,
                    seed + 1,
                );
                light(d, to, 7.0, c, 2.5 * f);
                light(d, from, 4.0, c, 1.2 * f);
            }
            Ev::Level { who, .. } => {
                let f = (1.0 - age / 1200.0).max(0.0);
                let Some(p) = at_of(who) else {
                    continue;
                };
                for n in 0..24 {
                    let a = n as f32 / 24.0 * std::f32::consts::TAU + age / 300.0;
                    let rise = ((age / 1200.0) + n as f32 / 24.0).fract() * 3.0;
                    d.sparks.push(Spark {
                        p: [p[0] + a.cos() * 0.8, p[1] + rise, p[2] + a.sin() * 0.8],
                        size: 0.16,
                        c: [GOLD[0], GOLD[1], GOLD[2], f],
                    });
                }
                ring(
                    look,
                    d,
                    [p[0], p[1] + 0.05, p[2]],
                    0.6 + age / 600.0,
                    GOLD,
                    f,
                    0.0,
                );
                light(d, [p[0], p[1] + 1.5, p[2]], 6.0, GOLD, 1.5 * f);
            }
            _ => {}
        }
    }
}

/// Wizards knocked out: a burst of their colour going up where they fell.
pub fn falls(look: &Look, d: &mut Draw, list: &[(f64, V3, u16)], now: f64) {
    for &(when, at, who) in list {
        let t = ((now - when) / 1000.0) as f32;
        let f = 1.0 - t / 1.2;
        if f <= 0.0 {
            continue;
        }
        let c = mix(crate::look::hue(who), WHITE, 0.35);
        let seed = who as i32 ^ when as i32;
        for n in 0..36 {
            let a = unit(hash(seed, n, 61)) * std::f32::consts::TAU + t * 3.0;
            let r = 0.3 + t * (0.6 + unit(hash(seed, n, 62)));
            let y =
                at[1] + 0.2 + t * 3.5 * unit(hash(seed, n, 63)) + 1.6 * unit(hash(seed, n, 64)) * f;
            d.sparks.push(Spark {
                p: [at[0] + a.cos() * r, y, at[2] + a.sin() * r],
                size: 0.16 * f + 0.04,
                c: [c[0], c[1], c[2], f],
            });
        }
        ring(
            look,
            d,
            [at[0], at[1] + 0.06, at[2]],
            0.5 + t * 4.0,
            c,
            f,
            0.0,
        );
        spray(
            d,
            [at[0], at[1] + 1.0, at[2]],
            16,
            6.0,
            (t, 0.7),
            9.0,
            (WHITE, GOLD),
            0.14,
            seed,
        );
        light(d, [at[0], at[1] + 1.2, at[2]], 8.0, c, 3.0 * f * f);
    }
}

/// A fireball bursting at `at`, `t` seconds ago.
fn explosion(look: &Look, d: &mut Draw, at: V3, t: f32, seed: i32) {
    let c = colour(spell::FIREBALL);
    let r = FIREBALL_RADIUS;
    let f = 1.0 - t / 0.55;
    if f <= 0.0 {
        return;
    }
    let grow = (t / 0.16).min(1.0);
    shell(
        look,
        d,
        at,
        [r * (0.3 + 0.7 * grow); 3],
        rgb(255, 80, 24),
        f * 0.9,
    );
    shell(
        look,
        d,
        at,
        [r * (0.2 + 0.5 * grow); 3],
        rgb(255, 170, 60),
        f * 0.6,
    );
    if t < 0.12 {
        let k = 1.0 - t / 0.12;
        let hot = mix(c, rgb(255, 220, 120), 0.5);
        glow(
            d,
            look.orb,
            m4::place(at, 0.0, [r * (0.2 + 0.25 * grow); 3]),
            hot,
            0.5 * k,
        );
    }
    ring(
        look,
        d,
        [at[0], at[1] - 0.3, at[2]],
        r * 1.5 * (t / 0.4).min(1.0),
        c,
        f,
        0.0,
    );
    spray(
        d,
        at,
        30,
        11.0,
        (t, 0.8),
        12.0,
        (rgb(255, 230, 150), rgb(200, 40, 10)),
        0.22,
        seed,
    );
    spray(
        d,
        at,
        12,
        4.0,
        (t, 0.5),
        -2.0,
        (rgb(255, 200, 120), c),
        0.5,
        seed + 3,
    );
    light(d, at, r * 3.5, c, 3.0 * f * f);
}

/// Lightning's warning: a ring where it will strike, a second ring
/// closing to the middle as the moment comes.
fn mark(look: &Look, d: &mut Draw, at: V3, age: f32, seed: i32) {
    let wait = LIGHTNING_DELAY as f32 * 1000.0 / TICK_HZ as f32;
    if age > wait {
        return;
    }
    let c = colour(spell::LIGHTNING);
    let k = age / wait;
    let r = LIGHTNING_RADIUS;
    let floor = [at[0], at[1] + 0.08, at[2]];
    let pulse = 0.6 + 0.4 * (age / 50.0).sin();
    ring(look, d, floor, r, c, 0.9 * pulse, 0.0);
    ring(
        look,
        d,
        floor,
        r * (1.0 - k).max(0.08),
        mix(c, WHITE, 0.4),
        0.9,
        0.0,
    );
    let flick = (age / 60.0) as i32;
    for n in 0..18 {
        let a = unit(hash(seed + flick, n, 31)) * std::f32::consts::TAU;
        d.sparks.push(Spark {
            p: [
                at[0] + a.cos() * r,
                at[1] + 0.1 + 0.4 * unit(hash(seed + flick, n, 32)),
                at[2] + a.sin() * r,
            ],
            size: 0.14,
            c: [c[0], c[1], c[2], 1.0],
        });
    }
    beam(
        look,
        d,
        (at, [at[0], at[1] + 30.0, at[2]]),
        0.3,
        (c, 0.15 + 0.3 * k),
        true,
    );
    light(d, [at[0], at[1] + 1.0, at[2]], r * 1.6, c, 0.6 + 1.4 * k);
}

/// Lightning striking at `at`, `t` seconds ago: a jagged bolt from the
/// sky, a flash, a shockwave.
fn strike(look: &Look, d: &mut Draw, at: V3, t: f32, seed: i32) {
    let c = colour(spell::LIGHTNING);
    let f = 1.0 - t / 0.5;
    if f <= 0.0 {
        return;
    }
    if t < 0.28 {
        let flick = seed + (t / 0.05) as i32;
        let n = 10;
        let mut prev = [at[0], at[1] + 45.0, at[2]];
        for k in 1..=n {
            let u = k as f32 / n as f32;
            let j = |q| (unit(hash(flick, k, q)) - 0.5) * 3.5 * (1.0 - u * 0.8);
            let p = if k == n {
                at
            } else {
                [at[0] + j(1), at[1] + 45.0 * (1.0 - u), at[2] + j(2)]
            };
            beam(look, d, (prev, p), 0.09, (mix(c, WHITE, 0.8), 1.0), false);
            beam(look, d, (prev, p), 0.5, (c, f), true);
            prev = p;
        }
    }
    let floor = [at[0], at[1] + 0.08, at[2]];
    ring(
        look,
        d,
        floor,
        LIGHTNING_RADIUS * (0.4 + t * 3.0),
        mix(c, WHITE, 0.3),
        f,
        0.0,
    );
    spray(d, floor, 26, 9.0, (t, 0.6), 14.0, (WHITE, c), 0.18, seed);
    light(d, [at[0], at[1] + 3.0, at[2]], 20.0, c, 7.0 * f * f);
}

/// A wizard going (`out`) or arriving in a column of sparks at `feet`.
fn blink(d: &mut Draw, feet: V3, t: f32, seed: i32, out: bool) {
    let c = colour(spell::BLINK);
    let f = 1.0 - t / 0.45;
    if f <= 0.0 {
        return;
    }
    for n in 0..30 {
        let a = unit(hash(seed, n, 41)) * std::f32::consts::TAU;
        let y = unit(hash(seed, n, 42)) * 1.9;
        let r = if out { 0.45 * f } else { 0.45 + t * 2.5 };
        let lift = if out { t * 2.0 } else { 0.0 };
        d.sparks.push(Spark {
            p: [
                feet[0] + a.cos() * r,
                feet[1] + y + lift,
                feet[2] + a.sin() * r,
            ],
            size: 0.14,
            c: [c[0], c[1], c[2], f],
        });
    }
    light(d, [feet[0], feet[1] + 1.0, feet[2]], 6.0, c, 2.5 * f);
}

/// Gust: a ring of wind and a dome thrown outward from `feet`.
/// `mine`: you cast it (you stand inside it: no dome).
fn gust(look: &Look, d: &mut Draw, feet: V3, (t, seed): (f32, i32), mine: bool) {
    let c = colour(spell::GUST);
    let f = 1.0 - t / 0.5;
    if f <= 0.0 {
        return;
    }
    let grow = (t / 0.3).min(1.0);
    let r = GUST_RADIUS * grow;
    ring(look, d, [feet[0], feet[1] + 0.1, feet[2]], r, c, f, t * 3.0);
    ring(
        look,
        d,
        [feet[0], feet[1] + 0.9, feet[2]],
        r * 0.8,
        c,
        f * 0.6,
        -t * 3.0,
    );
    if !mine {
        shell(look, d, feet, [r, r * 0.45, r], c, f * 0.7);
    }
    // Streaks of wind: short arcs flung outward, turning as they go.
    for n in 0..24 {
        let a0 = n as f32 / 24.0 * std::f32::consts::TAU + unit(hash(seed, n, 53));
        let rr = r * (0.5 + 0.5 * unit(hash(seed, n, 51)));
        let y = feet[1] + 0.3 + 1.6 * unit(hash(seed, n, 52));
        for k in 0..5 {
            let a = a0 + t * 4.0 - k as f32 * 0.07;
            let tail = 1.0 - k as f32 / 5.0;
            d.sparks.push(Spark {
                p: [feet[0] + a.cos() * rr, y, feet[2] + a.sin() * rr],
                size: 0.1 + 0.06 * tail,
                c: [c[0], c[1], c[2], f * tail],
            });
        }
    }
    light(d, [feet[0], feet[1] + 1.0, feet[2]], r + 2.0, c, 1.5 * f);
}
