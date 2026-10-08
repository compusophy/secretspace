//! What spells, loot and levels look like: each spell's colour, its bolt
//! in flight, the marks spells leave on a wizard (a ward's bubble, roots
//! at the feet, haste and mending sparks), what a cast or a landing
//! shows (bursts, rings, starfall's mark and pillar, chain leaps, a level
//! gained), and the chests and scrolls on the island.

use render::geo::{self, hash, mix, rgb, unit, V3};
use render::{m4, Item, Light, Pass, Spark};
use wandfall::laws::{spell, COMET_RADIUS, GUST_RADIUS, STARFALL_DELAY, STARFALL_RADIUS, TICK_HZ};
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

/// A spell's colour.
pub fn colour(s: u8) -> V3 {
    match s {
        spell::LANCE => rgb(255, 238, 170),
        spell::COMET => rgb(255, 136, 60),
        spell::CHAIN => rgb(120, 200, 255),
        spell::STARFALL => rgb(214, 170, 255),
        spell::ROOT => rgb(130, 220, 100),
        spell::BLINK => rgb(170, 140, 255),
        spell::WARD => rgb(150, 214, 255),
        spell::MEND => rgb(120, 255, 150),
        spell::GUST => rgb(225, 238, 255),
        spell::HASTE => rgb(255, 228, 90),
        _ => GOLD,
    }
}

fn ring(d: &mut Draw, at: V3, r: f32, n: i32, (c, a): (V3, f32), size: f32, seed: i32) {
    for k in 0..n {
        let ang = k as f32 / n as f32 * std::f32::consts::TAU + unit(hash(seed, k, 9)) * 0.3;
        d.sparks.push(Spark {
            p: [at[0] + ang.cos() * r, at[1], at[2] + ang.sin() * r],
            size,
            c: [c[0], c[1], c[2], a],
        });
    }
}

/// A bolt in flight: the wand's, or a spell's.
pub fn bolt(look: &Look, d: &mut Draw, b: &BoltSeen, mine: bool) {
    let speed = (b.v[0] * b.v[0] + b.v[1] * b.v[1] + b.v[2] * b.v[2])
        .sqrt()
        .max(1.0);
    let dir = [b.v[0] / speed, b.v[1] / speed, b.v[2] / speed];
    let (c, size, reach, tail) = match b.kind {
        WAND => (if mine { GOLD } else { rgb(255, 150, 120) }, 0.16, 7.0, 10),
        spell::LANCE => (colour(b.kind), 0.1, 9.0, 18),
        spell::COMET => (colour(b.kind), 0.5, 12.0, 24),
        k => (colour(k), 0.22, 7.0, 14),
    };
    let m = if b.kind == spell::LANCE {
        let side = geo::norm(geo::cross(dir, [0.0, 1.0, 0.0]));
        let up = geo::cross(side, dir);
        m4::basis(
            b.p,
            geo::scale(dir, 1.8),
            geo::scale(up, size),
            geo::scale(side, size),
        )
    } else {
        m4::place(b.p, 0.0, [size; 3])
    };
    d.items.push(
        Item::new(look.orb, m)
            .tint(c, 1.0)
            .glow(1.0)
            .pass(Pass::Glow),
    );
    d.lights.push(Light {
        p: b.p,
        r: reach,
        c: geo::scale(c, 0.9),
    });
    for k in 0..tail {
        let back = k as f32 * if b.kind == spell::LANCE { 0.4 } else { 0.22 };
        let fade = 1.0 - k as f32 / tail as f32;
        let jit = if b.kind == spell::CHAIN || b.kind == spell::COMET {
            let j = |n| (unit(hash(b.id as i32, k, n)) - 0.5) * 0.4;
            [j(1), j(2), j(3)]
        } else {
            [0.0; 3]
        };
        d.sparks.push(Spark {
            p: geo::add(geo::add(b.p, geo::scale(dir, -back)), jit),
            size: (size * 2.0 + 0.1) * fade + 0.04,
            c: [c[0], c[1], c[2], fade],
        });
    }
}

/// The marks spells leave on a wizard.
pub fn on_wizard(look: &Look, d: &mut Draw, s: &Seen, t: f32) {
    let at = s.p;
    if s.fx & fx::SHIELD != 0 {
        let c = colour(spell::WARD);
        let m = m4::place([at[0], at[1] + 0.95, at[2]], t, [0.85, 1.1, 0.85]);
        d.items.push(
            Item::new(look.orb, m)
                .tint(c, 0.22)
                .glow(0.6)
                .pass(Pass::Faint),
        );
    }
    if s.fx & fx::ROOTED != 0 {
        d.items
            .push(Item::new(look.roots, m4::place(at, t * 0.2, [1.0; 3])));
    }
    let id = s.id as i32;
    let tick = (t * 20.0) as i32;
    if s.fx & fx::HASTED != 0 {
        let c = colour(spell::HASTE);
        for k in 0..6 {
            let a = unit(hash(id, tick + k, 4)) * std::f32::consts::TAU;
            d.sparks.push(Spark {
                p: [
                    at[0] + a.cos() * 0.4,
                    at[1] + 0.1 + 0.2 * unit(hash(id, k, 5)),
                    at[2] + a.sin() * 0.4,
                ],
                size: 0.12,
                c: [c[0], c[1], c[2], 0.8],
            });
        }
    }
    if s.fx & fx::MENDING != 0 {
        let c = colour(spell::MEND);
        for k in 0..8 {
            let a = k as f32 / 8.0 * std::f32::consts::TAU + t;
            let rise = (t * 0.9 + k as f32 * 0.13).fract();
            d.sparks.push(Spark {
                p: [
                    at[0] + a.cos() * 0.6,
                    at[1] + rise * 2.2,
                    at[2] + a.sin() * 0.6,
                ],
                size: 0.14,
                c: [c[0], c[1], c[2], 1.0 - rise],
            });
        }
        d.lights.push(Light {
            p: [at[0], at[1] + 1.0, at[2]],
            r: 3.5,
            c: geo::scale(c, 0.8),
        });
    }
}

/// Chests (glowing until opened) and scrolls (spinning, bobbing, in
/// their spell's colour, bigger by rank).
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
            d.lights.push(Light {
                p: [p[0], p[1] + 1.0, p[2]],
                r: 4.0,
                c: geo::scale(GOLD, pulse),
            });
        }
    }
    for &(id, s, rank, p) in &l.scrolls {
        if far(p) {
            continue;
        }
        let c = colour(s);
        let bob = 0.7 + 0.12 * (t * 2.2 + id as f32).sin();
        let k = 0.16 + 0.04 * rank as f32;
        let at = [p[0], p[1] + bob, p[2]];
        d.items.push(
            Item::new(
                look.orb,
                m4::place(at, t * 2.0 + id as f32, [k, k * 1.6, k]),
            )
            .tint(c, 1.0)
            .glow(1.0)
            .pass(Pass::Glow),
        );
        d.lights.push(Light {
            p: at,
            r: 3.0 + rank as f32 * 0.5,
            c: geo::scale(c, 1.2),
        });
        ring(
            d,
            [p[0], p[1] + 0.08, p[2]],
            0.5,
            4 + rank as i32 * 2,
            (c, 0.7),
            0.1,
            id as i32,
        );
    }
}

/// What casts, landings, leaps and levels show, `now` ms.
pub fn shows(
    look: &Look,
    d: &mut Draw,
    list: &[(f64, Ev)],
    now: f64,
    at_of: impl Fn(u16) -> Option<V3>,
) {
    for (k, &(when, e)) in list.iter().enumerate() {
        let age = (now - when) as f32;
        let seed = when as i32 ^ k as i32;
        match e {
            Ev::Cast {
                spell: s,
                stage,
                at,
                ..
            } => {
                let c = colour(s);
                match (s, stage) {
                    (spell::COMET, 1) | (spell::STARFALL, 1) => {
                        let life = 700.0;
                        let f = (1.0 - age / life).max(0.0);
                        if f <= 0.0 {
                            continue;
                        }
                        let r = if s == spell::COMET {
                            COMET_RADIUS
                        } else {
                            STARFALL_RADIUS
                        };
                        let grow = (age / 250.0).min(1.0);
                        ring(
                            d,
                            [at[0], at[1] + 0.3, at[2]],
                            r * grow,
                            28,
                            (c, f),
                            0.35,
                            seed,
                        );
                        ring(
                            d,
                            [at[0], at[1] + 1.2, at[2]],
                            r * 0.6 * grow,
                            16,
                            (mix(c, [1.0; 3], 0.5), f),
                            0.3,
                            seed + 1,
                        );
                        d.lights.push(Light {
                            p: [at[0], at[1] + 1.5, at[2]],
                            r: r * 2.5,
                            c: geo::scale(c, 3.0 * f),
                        });
                        if s == spell::STARFALL && age < 300.0 {
                            for n in 0..20 {
                                let y = at[1] + 30.0 * (1.0 - age / 300.0) * (n as f32 / 20.0);
                                d.sparks.push(Spark {
                                    p: [at[0], y, at[2]],
                                    size: 0.6,
                                    c: [c[0], c[1], c[2], 1.0],
                                });
                            }
                        }
                    }
                    (spell::STARFALL, 0) => {
                        let wait = STARFALL_DELAY as f32 * 1000.0 / TICK_HZ as f32;
                        if age > wait {
                            continue;
                        }
                        let pulse = 0.5 + 0.5 * (age / 60.0).sin();
                        let m = m4::place(
                            [at[0], at[1] - 0.2, at[2]],
                            0.0,
                            [STARFALL_RADIUS, 0.6, STARFALL_RADIUS],
                        );
                        d.items.push(
                            Item::new(look.wall, m)
                                .tint(c, 0.25 + 0.2 * pulse)
                                .glow(1.0)
                                .pass(Pass::Faint),
                        );
                    }
                    (spell::GUST, 0) => {
                        let f = (1.0 - age / 450.0).max(0.0);
                        if f > 0.0 {
                            let feet = [at[0], at[1] - 1.2, at[2]];
                            ring(
                                d,
                                feet,
                                GUST_RADIUS * (age / 300.0).min(1.0),
                                32,
                                (c, f),
                                0.3,
                                seed,
                            );
                        }
                    }
                    (spell::BLINK, _) => {
                        let f = (1.0 - age / 500.0).max(0.0);
                        for n in 0..16 {
                            let j = |q| (unit(hash(seed, n, q)) - 0.5) * 1.6 * (1.0 + age / 300.0);
                            d.sparks.push(Spark {
                                p: [at[0] + j(1), at[1] - 0.6 + j(2), at[2] + j(3)],
                                size: 0.18,
                                c: [c[0], c[1], c[2], f],
                            });
                        }
                    }
                    _ => {
                        // A flash at the caster's wand.
                        let f = (1.0 - age / 200.0).max(0.0);
                        if f > 0.0 {
                            d.lights.push(Light {
                                p: at,
                                r: 5.0,
                                c: geo::scale(c, 2.0 * f),
                            });
                        }
                    }
                }
            }
            Ev::Link { from, to } => {
                let f = (1.0 - age / 300.0).max(0.0);
                let (Some(a), Some(b)) = (at_of(from), at_of(to)) else {
                    continue;
                };
                if f <= 0.0 {
                    continue;
                }
                let c = colour(spell::CHAIN);
                for n in 0..=16 {
                    let u = n as f32 / 16.0;
                    let j = |q| (unit(hash(seed + (now / 40.0) as i32, n, q)) - 0.5) * 0.6;
                    d.sparks.push(Spark {
                        p: [
                            a[0] + (b[0] - a[0]) * u + j(1),
                            a[1] + 1.2 + (b[1] - a[1]) * u + j(2),
                            a[2] + (b[2] - a[2]) * u + j(3),
                        ],
                        size: 0.22,
                        c: [c[0], c[1], c[2], f],
                    });
                }
                d.lights.push(Light {
                    p: [b[0], b[1] + 1.2, b[2]],
                    r: 6.0,
                    c: geo::scale(c, 2.0 * f),
                });
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
                d.lights.push(Light {
                    p: [p[0], p[1] + 1.5, p[2]],
                    r: 6.0,
                    c: geo::scale(GOLD, 1.5 * f),
                });
            }
            _ => {}
        }
    }
}
