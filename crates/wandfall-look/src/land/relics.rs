//! What the places and ruins are made of, as meshes: the ruined rings'
//! pillars and capstones, the grove's giant mushrooms and crystals, the
//! circle's standing stones and altar, the rift's obsidian and gate, and
//! the islets floating over it all.

use std::f32::consts::TAU;

use render::geo::{self, hash, rgb, unit};
use render::{Mesh, Renderer};

use super::{crystal, lean, one, smooth, CYAN, EMBER};

pub(super) struct Relics {
    pub pillar: Mesh,
    pub cap: Mesh,
    pub shrooms: [Mesh; 3],
    pub menhir: Mesh,
    pub altar: Mesh,
    pub spike: Mesh,
    pub gate: Mesh,
    pub crystals: [Mesh; 2],
    pub islet: Mesh,
}

impl Relics {
    pub(super) fn new(r: &mut Renderer) -> Relics {
        let stone = rgb(208, 200, 184);
        let pillar = smooth(r, |g| {
            let profile = [
                (0.66, 0.0),
                (0.66, 0.06),
                (0.54, 0.12),
                (0.5, 0.5),
                (0.47, 0.94),
                (0.53, 0.97),
                (0.53, 1.0),
            ];
            g.lathe([0.0; 3], &profile, 16, stone, 0.0);
        });
        let cap = one(r, |g| {
            g.block(
                [0.0; 3],
                [1.35, 0.32, 1.35],
                0.0,
                stone,
                rgb(176, 168, 154),
                0.0,
            )
        });
        // Giant mushrooms, a metre tall (scaled to each): a pale stem, a
        // cap that glows a little, spots.
        let shrooms = [rgb(150, 80, 220), rgb(50, 170, 160), rgb(210, 70, 70)].map(|c| {
            smooth(r, |g| {
                let cream = rgb(226, 214, 190);
                g.lathe(
                    [0.0; 3],
                    &[(0.09, 0.0), (0.075, 0.4), (0.06, 0.8), (0.08, 0.95)],
                    10,
                    cream,
                    0.0,
                );
                let rim = [
                    (0.06, 0.9),
                    (0.42, 0.88),
                    (0.47, 0.94),
                    (0.4, 1.06),
                    (0.22, 1.16),
                    (0.0, 1.19),
                ];
                g.lathe([0.0; 3], &rim, 16, c, 0.35);
                for k in 0..7 {
                    let a = k as f32 * 2.4;
                    let rr = 0.18 + 0.12 * (k % 2) as f32;
                    let y = 1.16 - rr * 0.55;
                    g.sphere(
                        [a.cos() * rr, y, a.sin() * rr],
                        [0.045, 0.02, 0.045],
                        (1, k, 0.0),
                        cream,
                        0.2,
                    );
                }
            })
        });
        // A standing stone a unit tall, its runes on the side facing in.
        let menhir = one(r, |g| {
            // Five-sided, a face toward -x (inward once placed).
            let grey = rgb(112, 110, 106);
            let rot = 144f32.to_radians();
            g.column([0.0; 3], 5, (0.5, 0.36), 0.86, rot, grey, 0.0, false);
            g.column(
                [0.0, 0.86, 0.0],
                5,
                (0.36, 0.14),
                0.14,
                rot,
                grey,
                0.0,
                true,
            );
            for k in 0..5 {
                let y = 0.22 + k as f32 * 0.12;
                let x = -(0.809 * (0.5 - 0.14 * y / 0.86) + 0.008);
                for s in 0..3 {
                    let h = |i| unit(hash(k, s * 4 + i, 77)) - 0.5;
                    let a = [x, y + h(0) * 0.08, h(1) * 0.3];
                    let b = [x, y + h(2) * 0.08, h(3) * 0.3];
                    let d = geo::norm(geo::sub(b, a));
                    let side = [0.0, -d[2] * 0.012, d[1] * 0.012];
                    g.quad(
                        geo::sub(a, side),
                        geo::add(a, side),
                        geo::add(b, side),
                        geo::sub(b, side),
                        CYAN,
                        2.0,
                    );
                }
            }
        });
        let altar = one(r, |g| {
            let dark = rgb(96, 92, 100);
            g.block([0.0; 3], [1.7, 0.9, 1.1], 0.0, dark, dark, 0.0);
            g.block(
                [0.0, 0.9, 0.0],
                [2.1, 0.22, 1.5],
                0.0,
                rgb(130, 126, 134),
                dark,
                0.0,
            );
        });
        // Obsidian a unit tall, shards about its foot; the gate.
        let obsidian = rgb(24, 10, 14);
        let spike = one(r, |g| {
            g.spike([0.0; 3], [0.12, 1.0, 0.05], 1.0, 5, obsidian, 0.0);
            g.spike([0.7, 0.0, 0.3], [1.1, 0.4, 0.5], 0.4, 4, obsidian, 0.0);
            g.spike([-0.4, 0.0, -0.7], [-0.7, 0.3, -1.0], 0.35, 4, obsidian, 0.0);
            g.spike([0.0, 0.0, 0.0], [0.08, 0.6, 0.03], 0.3, 4, EMBER, 0.6);
        });
        let gate = one(r, |g| {
            g.block(
                [0.0; 3],
                [2.2, 0.6, 7.6],
                0.0,
                rgb(40, 30, 40),
                obsidian,
                0.0,
            );
            for z in [-3.1f32, 3.1] {
                g.spike([0.0, 0.5, z], [0.0, 7.6, z * 1.25], 0.7, 6, obsidian, 0.0);
            }
            // The ring the gate opens in.
            let (n, m, big, tube) = (32, 6, 2.7, 0.28);
            let at = |i: usize, j: usize| {
                let (a, b) = (i as f32 / n as f32 * TAU, j as f32 / m as f32 * TAU);
                let rr = big + tube * b.cos();
                [tube * b.sin(), 3.6 + rr * a.sin(), rr * a.cos()]
            };
            for i in 0..n {
                for j in 0..m {
                    let glow = if j == m / 2 { 1.4 } else { 0.0 };
                    let c = if glow > 0.0 { EMBER } else { obsidian };
                    g.quad(
                        at(i, j),
                        at(i + 1, j),
                        at(i + 1, j + 1),
                        at(i, j + 1),
                        c,
                        glow,
                    );
                }
            }
        });
        let crystals = [0u32, 1].map(|s| {
            one(r, |g| {
                crystal(g, 0.42, 1.0, [1.0; 3], 0.3);
                for k in 0..4 {
                    let from = g.len();
                    let h = 0.35 + 0.3 * unit(hash(k, 1, s));
                    crystal(g, 0.2, h, [1.0; 3], 0.3);
                    let a = k as f32 * 1.7 + s as f32;
                    lean(
                        g,
                        from,
                        0.5 + 0.4 * unit(hash(k, 2, s)),
                        a,
                        [a.cos() * 0.5, 0.0, a.sin() * 0.5],
                    );
                }
            })
        });
        // Islets floating over the island: earth hanging under a green
        // top, a crystal under some.
        let islet = smooth(r, |g| {
            let earth = [
                (0.0, -7.0),
                (1.4, -5.2),
                (3.0, -2.8),
                (4.2, -0.8),
                (4.5, 0.0),
            ];
            g.lathe([0.0; 3], &earth, 14, rgb(112, 92, 80), 0.0);
            g.lathe(
                [0.0; 3],
                &[(4.6, 0.0), (4.4, 0.35), (3.0, 0.6), (0.0, 0.7)],
                14,
                rgb(70, 112, 50),
                0.0,
            );
        });
        Relics {
            pillar,
            cap,
            shrooms,
            menhir,
            altar,
            spike,
            gate,
            crystals,
            islet,
        }
    }

    pub(super) fn meshes(&self) -> impl Iterator<Item = Mesh> + '_ {
        [
            self.pillar,
            self.cap,
            self.menhir,
            self.altar,
            self.spike,
            self.gate,
            self.islet,
        ]
        .into_iter()
        .chain(self.shrooms)
        .chain(self.crystals)
    }
}
