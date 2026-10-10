//! What moves at the island's places each frame, and their light: the
//! launch runes (a turning circle, a shaft of light, rising motes), the
//! beacon turning over the Spire and its rune rings, the lamps' violet
//! flames, the circle's orb in its shell of light, the rift's gate
//! burning and swirling, embers streaking up off its lava into smoke, the
//! grove's glimmer and glints, the rune turning over the causeway's crown
//! and the spray blowing across its columns.

use std::f32::consts::TAU;

use render::geo::{self, hash, mix, rgb, unit, V3};
use render::{m4, Item, Light, Material, Mesh, Pass, Shape, Spark};

use crate::fx::Draw;
use crate::land::{CYAN, EMBER, GOLDEN, VIOLET};

/// A launch rune's colour: a warm sky-gold.
const PAD: V3 = rgb(255, 200, 110);
/// The causeway's: sea-green.
const SPRAY: V3 = rgb(150, 255, 214);
use crate::look::Look;
use wandfall::laws::RIFT_DEPTH;
use wandfall::places::Place;

impl Look {
    /// What moves at the places, and their light, `t` seconds in.
    pub fn places(&self, d: &mut Draw, t: f32) {
        let l = &self.land;
        let glow = |d: &mut Draw, mesh: Mesh, m: render::M4, c: V3, a: f32| {
            d.items
                .push(Item::new(mesh, m).tint(c, a).glow(1.0).pass(Pass::Glow));
        };
        for (k, &p) in l.lamps.iter().enumerate() {
            // A violet flame, flickering, and its light with it.
            let k = k as i32;
            let flick =
                0.85 + 0.1 * (t * 9.0 + k as f32).sin() + 0.05 * (t * 23.0 + k as f32 * 2.0).sin();
            let c = mix(VIOLET, [1.0; 3], 0.25);
            d.sparks.push(Spark {
                p: [p[0], p[1] + 0.1, p[2]],
                size: 0.5 * flick,
                c: [c[0], c[1], c[2], 0.9],
                shape: Shape::Flame,
                seed: unit(hash(k, 1, 51)),
                ..Default::default()
            });
            d.lights.push(Light {
                p,
                r: 9.0,
                c: geo::scale(VIOLET, 1.6 * flick),
            });
        }
        // Launch runes: a circle of runes turning on the ground, a shaft
        // of light up out of it, motes rising; its light.
        for (k, &p) in l.pads.iter().enumerate() {
            let floor = [p[0], p[1] + 0.25, p[2]];
            let pulse = 0.75 + 0.25 * (t * 2.6 + k as f32).sin();
            let r = wandfall::laws::PAD_R;
            glow(
                d,
                self.sigil,
                m4::place(floor, t * 0.7, [r * 1.15, 1.0, r * 1.15]),
                PAD,
                0.9 * pulse,
            );
            glow(
                d,
                self.ring,
                m4::place(floor, -t, [r * 1.5, 1.0, r * 1.5]),
                PAD,
                0.5,
            );
            // Its shaft of light, soft at its edge, fading as it rises.
            d.items.push(
                Item::new(self.shaft, m4::place(floor, 0.0, [r * 0.4, 9.0, r * 0.4]))
                    .tint(PAD, 0.35 * pulse)
                    .glow(1.0)
                    .material(Material::Rim)
                    .pass(Pass::Glow),
            );
            for n in 0..10 {
                let u = |i: u32| unit(hash(k as i32, n, 61 + i));
                let f = (t * (0.5 + 0.3 * u(0)) + u(1)).fract();
                let a = u(2) * TAU + t * 1.5;
                let rr = r * (0.3 + 0.6 * u(3));
                d.sparks.push(Spark {
                    p: [p[0] + a.cos() * rr, floor[1] + f * 6.0, p[2] + a.sin() * rr],
                    size: 0.1,
                    c: [PAD[0], PAD[1], PAD[2], 1.0 - f],
                    v: [0.0, 0.5, 0.0],
                    ..Default::default()
                });
            }
            d.lights.push(Light {
                p: [p[0], p[1] + 1.0, p[2]],
                r: 7.0,
                c: geo::scale(PAD, 1.4 * pulse),
            });
        }
        for &(p, c) in &l.crystals {
            d.lights.push(Light {
                p,
                r: 8.0,
                c: geo::scale(c, 1.2),
            });
        }
        for p in &l.pois {
            let base = [p.x, p.level, p.z];
            match p.place {
                Place::Spire => {
                    // The beacon: a crystal turning over the hat, its
                    // light up into the sky, runes wheeling about it.
                    let foot = base[1] - 0.3;
                    let top = [p.x, foot + 45.0 + (t * 1.3).sin() * 0.4, p.z];
                    let m = m4::place(top, t * 0.8, [1.3, 2.0, 1.3]);
                    d.items
                        .push(Item::new(l.gem, m).tint(VIOLET, 1.0).glow(2.2));
                    glow(
                        d,
                        self.shaft,
                        m4::place(top, 0.0, [0.5, 260.0, 0.5]),
                        VIOLET,
                        0.25,
                    );
                    glow(
                        d,
                        self.shaft,
                        m4::place(top, 0.0, [1.6, 260.0, 1.6]),
                        VIOLET,
                        0.06,
                    );
                    d.lights.push(Light {
                        p: top,
                        r: 40.0,
                        c: geo::scale(VIOLET, 4.0),
                    });
                    for (y, r, w) in [
                        (foot + 31.4, 6.2, 0.35),
                        (foot + 35.4, 4.4, -0.5),
                        (top[1], 2.4, 0.9),
                    ] {
                        glow(
                            d,
                            l.runes,
                            m4::place([p.x, y, p.z], t * w, [r; 3]),
                            GOLDEN,
                            0.8,
                        );
                    }
                    let floor = [p.x, base[1] + 0.04, p.z];
                    glow(
                        d,
                        l.runes,
                        m4::place(floor, -t * 0.05, [9.0; 3]),
                        GOLDEN,
                        0.35,
                    );
                }
                Place::Circle => {
                    let at = [p.x, base[1] + 0.5 + 1.9 + (t * 1.7).sin() * 0.12, p.z];
                    d.items.push(
                        Item::new(self.orb, m4::place(at, 0.0, [0.28; 3]))
                            .tint(CYAN, 1.0)
                            .glow(2.0),
                    );
                    d.items.push(
                        Item::new(self.orb, m4::place(at, 0.0, [0.55; 3]))
                            .tint(CYAN, 0.5)
                            .glow(0.3)
                            .detail(3.0)
                            .rough(1.0)
                            .material(Material::Energy)
                            .pass(Pass::Glow),
                    );
                    glow(d, l.runes, m4::place(at, t * 0.6, [1.4; 3]), CYAN, 0.9);
                    let floor = [p.x, base[1] + 0.56, p.z];
                    glow(
                        d,
                        l.runes,
                        m4::place(floor, -t * 0.1, [p.r * 0.4; 3]),
                        CYAN,
                        0.25,
                    );
                    d.lights.push(Light {
                        p: at,
                        r: 14.0,
                        c: geo::scale(CYAN, 2.5),
                    });
                }
                Place::Rift => {
                    // The gate's swirl, and embers rising off the lava.
                    let floor = base[1] - RIFT_DEPTH;
                    let (foot, yaw) = l.gate;
                    let (s, c) = yaw.sin_cos();
                    let mid = [foot[0], foot[1] + 3.6, foot[2]];
                    let face = |r: f32| {
                        m4::basis(
                            mid,
                            [c * 0.06, 0.0, s * 0.06],
                            [0.0, r, 0.0],
                            [-s * r, 0.0, c * r],
                        )
                    };
                    let pulse = 0.8 + 0.2 * (t * 3.0).sin();
                    // A void, fire burning over it and round its rim,
                    // runes wheeling in it.
                    d.items.push(
                        Item::new(self.ball, face(2.45))
                            .tint(rgb(255, 50, 20), 0.55 * pulse)
                            .glow(1.0)
                            .material(Material::Rim)
                            .pass(Pass::Glow),
                    );
                    d.items.push(
                        Item::new(self.ball, face(2.5))
                            .tint(rgb(255, 90, 30), 0.7)
                            .glow(0.4)
                            .detail(1.1)
                            .rough(0.6)
                            .material(Material::Energy)
                            .pass(Pass::Glow),
                    );
                    d.items.push(
                        Item::new(self.ball, face(2.25))
                            .tint(rgb(14, 2, 8), 1.0)
                            .rough(0.3),
                    );
                    for (r, w) in [(2.3, 0.7), (1.5, -1.1), (0.8, 1.6)] {
                        let (sa, ca) = (t * w).sin_cos();
                        let up = [0.0, ca * r, 0.0];
                        let across = [-s * sa * r, 0.0, c * sa * r];
                        let u = geo::add(up, across);
                        let v = geo::sub([-s * ca * r, 0.0, c * ca * r], [0.0, sa * r, 0.0]);
                        let m = m4::basis(
                            geo::add(mid, [c * 0.08, 0.0, s * 0.08]),
                            u,
                            [c * 0.02, 0.0, s * 0.02],
                            v,
                        );
                        let m2 = m4::basis(
                            geo::sub(mid, [c * 0.08, 0.0, s * 0.08]),
                            u,
                            [c * 0.02, 0.0, s * 0.02],
                            v,
                        );
                        glow(d, l.runes, m, EMBER, 0.8 * pulse);
                        glow(d, l.runes, m2, EMBER, 0.8 * pulse);
                    }
                    d.lights.push(Light {
                        p: mid,
                        r: 18.0,
                        c: geo::scale(EMBER, 3.5 * pulse),
                    });
                    d.lights.push(Light {
                        p: [p.x, floor + 0.6, p.z],
                        r: 12.0,
                        c: geo::scale(EMBER, 2.0),
                    });
                    for k in 0..48 {
                        let u = |i| unit(hash(k, i, 31));
                        let f = (t / (3.0 + 2.0 * u(0)) + u(1)).fract();
                        let a = u(2) * TAU;
                        let r = 1.0 + 9.0 * u(3) + f * 1.5;
                        d.sparks.push(Spark {
                            p: [
                                p.x + a.cos() * r,
                                floor + 0.3 + f * (6.0 + 6.0 * u(4)),
                                p.z + a.sin() * r,
                            ],
                            size: 0.07 + 0.06 * u(5),
                            c: [
                                EMBER[0],
                                EMBER[1] * (1.0 - f * 0.5),
                                EMBER[2],
                                (1.0 - f) * 0.9,
                            ],
                            v: [0.0, 0.35 + 0.3 * u(6), 0.0],
                            ..Default::default()
                        });
                    }
                    // Smoke rolling up off the lava, lit red from below.
                    for k in 0..10 {
                        let u = |i| unit(hash(k, i, 33));
                        let f = (t / (7.0 + 3.0 * u(0)) + u(1)).fract();
                        let a = u(2) * TAU + f * 0.6;
                        let r = 2.0 + 7.0 * u(3) + f * 2.0;
                        let warm = mix(rgb(120, 40, 20), rgb(40, 34, 36), f);
                        d.sparks.push(Spark {
                            p: [p.x + a.cos() * r, floor + 1.0 + f * 14.0, p.z + a.sin() * r],
                            size: 2.0 + 4.0 * f,
                            c: [
                                warm[0],
                                warm[1],
                                warm[2],
                                0.35 * (f * 6.0).min(1.0) * (1.0 - f),
                            ],
                            shape: Shape::Smoke,
                            seed: u(4),
                            ..Default::default()
                        });
                    }
                }
                Place::Grove => {
                    for k in 0..36 {
                        let u = |i| unit(hash(k, i, 41));
                        let a = u(0) * TAU + t * 0.05 * (u(1) - 0.5);
                        let r = p.r * u(2).sqrt();
                        let y = base[1]
                            + 0.8
                            + 3.5 * u(3)
                            + (t * (0.5 + u(4)) + u(5) * TAU).sin() * 0.4;
                        let tw = 0.5 + 0.5 * (t * 2.0 + u(6) * TAU).sin();
                        let c = mix(VIOLET, CYAN, u(7));
                        // One in four a glint, now and then.
                        let star = k % 4 == 0;
                        d.sparks.push(Spark {
                            p: [p.x + a.cos() * r, y, p.z + a.sin() * r],
                            size: if star { 0.35 * tw * tw } else { 0.06 },
                            c: [c[0], c[1], c[2], 0.8 * tw],
                            shape: if star { Shape::Star } else { Shape::Glow },
                            ..Default::default()
                        });
                    }
                }
                Place::Causeway => {
                    // A rune ring turning over the crown, the cubes' mark.
                    let c = l.crown;
                    let at = [c[0], c[1] + 1.6 + (t * 1.3).sin() * 0.1, c[2]];
                    glow(d, l.runes, m4::place(at, t * 0.5, [1.1; 3]), SPRAY, 0.8);
                    glow(
                        d,
                        l.runes,
                        m4::place([c[0], c[1] + 0.08, c[2]], -t * 0.2, [1.4; 3]),
                        SPRAY,
                        0.3,
                    );
                    d.lights.push(Light {
                        p: at,
                        r: 10.0,
                        c: geo::scale(SPRAY, 1.6),
                    });
                    // Spray blowing over the columns, low and drifting.
                    for k in 0..28 {
                        let u = |i| unit(hash(k, i, 43));
                        let life = 4.0 + 3.0 * u(0);
                        let age = (t / life + u(1)).fract();
                        let a = u(2) * TAU;
                        let r = p.r * 0.8 * u(3).sqrt();
                        let x = p.x + a.cos() * r + (age - 0.5) * 6.0;
                        let z = p.z + a.sin() * r + (age - 0.5) * 2.0;
                        let y = base[1] + 1.0 + 7.0 * u(4) + age * 1.5;
                        let fade = (age * (1.0 - age) * 4.0).min(1.0);
                        d.sparks.push(Spark {
                            p: [x, y, z],
                            size: 0.05 + 0.04 * u(5),
                            c: [SPRAY[0], SPRAY[1], SPRAY[2], 0.55 * fade],
                            shape: Shape::Glow,
                            ..Default::default()
                        });
                    }
                }
            }
        }
    }
}
