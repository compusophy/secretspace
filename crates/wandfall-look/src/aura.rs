//! What moves at the island's places each frame, and their light: the
//! beacon turning over the Spire and its rune rings, the lamps, the
//! circle's orb, the rift's gate swirling and its embers, the grove's
//! glimmer.

use std::f32::consts::TAU;

use render::geo::{self, hash, mix, rgb, unit, V3};
use render::{m4, Item, Light, Material, Mesh, Pass, Spark};

use crate::fx::Draw;
use crate::land::{CYAN, EMBER, GOLDEN, VIOLET};
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
        for &p in &l.lamps {
            d.lights.push(Light {
                p,
                r: 9.0,
                c: geo::scale(VIOLET, 1.6),
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
                        self.beam,
                        m4::place(top, 0.0, [0.5, 260.0, 0.5]),
                        VIOLET,
                        0.25,
                    );
                    glow(
                        d,
                        self.beam,
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
                    // A void, a red rim about it, runes wheeling in it.
                    d.items.push(
                        Item::new(self.ball, face(2.45))
                            .tint(rgb(255, 50, 20), 0.55 * pulse)
                            .glow(1.0)
                            .material(Material::Rim)
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
                        d.sparks.push(Spark {
                            p: [p.x + a.cos() * r, y, p.z + a.sin() * r],
                            size: 0.06,
                            c: [c[0], c[1], c[2], 0.8 * tw],
                            ..Default::default()
                        });
                    }
                }
            }
        }
    }
}
