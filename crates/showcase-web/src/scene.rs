//! The showcase's scene, v1: rolling ground and a pool, a forest, rocks,
//! glowing crystals, a shrine with a golden bell, two hundred fireflies
//! (each a moving light), and a wand in view that casts. Built only from
//! the engine's shapes; everything that never moves is a static.

use render::geo::{self, hash, mix, rgb, unit, Geo, V3};
use render::{m4, Item, Light, Look, Mesh, Pass, Renderer, Spark};

/// Metres from the centre to the edge of the ground.
pub const HALF: f32 = 110.0;
const FLIES: usize = 200;
/// Seconds between casts.
const CAST_EVERY: f32 = 2.2;
const GOLD: V3 = rgb(255, 210, 122);

/// The ground's height at (x, z): rolling hills, a hollow for the pool,
/// a flat round for the shrine.
pub fn height(x: f32, z: f32) -> f32 {
    let r = (x * x + z * z).sqrt();
    let hills = 1.6 * (x * 0.045).sin() * (z * 0.05).cos()
        + 0.8 * (x * 0.11 + 1.3).sin() * (z * 0.09 - 0.7).sin()
        + 3.0 * ((r - 70.0) / 30.0).clamp(0.0, 1.0);
    let pool =
        -2.2 * (1.0 - ((x - 24.0).powi(2) + (z + 18.0).powi(2)).sqrt() / 13.0).clamp(0.0, 1.0);
    let flat = ((r - 9.0) / 8.0).clamp(0.0, 1.0);
    (hills + pool) * flat
}

pub const WATER: f32 = -0.6;

struct Meshes {
    ground: Mesh,
    water: Mesh,
    trunk: Mesh,
    crowns: [Mesh; 2],
    rocks: [Mesh; 3],
    crystal: Mesh,
    shrine: Mesh,
    bell: Mesh,
    rod: Mesh,
    orb: Mesh,
}

pub struct Scene {
    m: Meshes,
    /// Lights that never move: crystals, the bell.
    still: Vec<Light>,
}

fn ground() -> Geo {
    let mut g = Geo::default();
    let step = 2.0;
    let n = (HALF * 2.0 / step) as i32;
    for i in 0..n {
        for j in 0..n {
            let (x0, z0) = (-HALF + i as f32 * step, -HALF + j as f32 * step);
            let (x1, z1) = (x0 + step, z0 + step);
            let p = |x: f32, z: f32| [x, height(x, z), z];
            let k = unit(hash(i, j, 3));
            let h = height(x0, z0);
            let grass = mix(rgb(62, 92, 48), rgb(88, 112, 58), k);
            let col = if h < WATER + 0.3 {
                mix(
                    rgb(120, 110, 86),
                    grass,
                    ((h - WATER) / 0.3).clamp(0.0, 1.0),
                )
            } else if (x0 * x0 + z0 * z0).sqrt() < 10.0 {
                mix(rgb(150, 146, 138), rgb(170, 166, 156), k)
            } else {
                grass
            };
            g.quad(p(x0, z0), p(x0, z1), p(x1, z1), p(x1, z0), col, 0.0);
        }
    }
    g
}

fn shrine() -> Geo {
    let mut g = Geo::default();
    let stone = rgb(200, 196, 186);
    let dark = rgb(150, 146, 140);
    g.block([0.0, 0.0, 0.0], [12.0, 0.5, 12.0], 0.0, stone, dark, 0.0);
    g.block([0.0, 0.5, 0.0], [9.0, 0.4, 9.0], 0.0, stone, dark, 0.0);
    for k in 0..8 {
        let a = k as f32 / 8.0 * std::f32::consts::TAU;
        let at = [a.cos() * 3.8, 0.9, a.sin() * 3.8];
        g.column(at, 8, (0.42, 0.34), 5.2, 0.0, stone, 0.0, true);
        g.block([at[0], 6.1, at[2]], [1.1, 0.3, 1.1], a, stone, dark, 0.0);
    }
    g.column([0.0, 6.4, 0.0], 16, (4.7, 4.7), 0.5, 0.0, stone, 0.0, true);
    g.column([0.0, 6.9, 0.0], 16, (4.4, 0.4), 2.6, 0.0, dark, 0.0, true);
    g
}

fn tree_crown(seed: u32) -> Geo {
    let mut g = Geo::default();
    let c = mix(
        rgb(40, 82, 46),
        rgb(70, 110, 52),
        unit(hash(seed as i32, 1, 7)),
    );
    g.blob([0.0, 3.6, 0.0], [1.9, 1.6, 1.9], seed, c, 0.0);
    g.blob(
        [0.4, 4.9, -0.2],
        [1.3, 1.2, 1.3],
        seed + 1,
        mix(c, rgb(110, 140, 70), 0.3),
        0.0,
    );
    g
}

impl Scene {
    pub fn new(r: &mut Renderer) -> Scene {
        let m = Meshes {
            ground: r.mesh(&ground()),
            water: one(r, |g| {
                g.floor((-14.0, -14.0), (14.0, 14.0), 0.0, rgb(70, 120, 150), 0.15)
            }),
            trunk: one(r, |g| {
                g.column(
                    [0.0; 3],
                    6,
                    (0.32, 0.2),
                    3.4,
                    0.0,
                    rgb(92, 70, 52),
                    0.0,
                    false,
                )
            }),
            crowns: [r.mesh(&tree_crown(11)), r.mesh(&tree_crown(29))],
            rocks: [5u32, 17, 23].map(|s| {
                let mut g = Geo::default();
                g.blob([0.0, 0.3, 0.0], [1.0, 0.7, 0.9], s, rgb(128, 124, 120), 0.0);
                r.mesh(&g)
            }),
            crystal: one(r, |g| {
                for (k, (dx, dz, h)) in [
                    (0.0, 0.0, 2.2),
                    (0.5, 0.2, 1.4),
                    (-0.4, 0.3, 1.1),
                    (0.1, -0.5, 1.6),
                ]
                .into_iter()
                .enumerate()
                {
                    let lean = 0.25 * k as f32;
                    g.spike(
                        [dx, 0.0, dz],
                        [dx * (1.0 + lean), h, dz * (1.0 + lean)],
                        0.28,
                        5,
                        rgb(150, 210, 255),
                        0.7,
                    );
                }
            }),
            shrine: r.mesh(&shrine()),
            bell: one(r, |g| {
                g.column([0.0, 0.0, 0.0], 12, (0.9, 0.45), 1.3, 0.0, GOLD, 0.6, true);
                g.blob([0.0, 1.35, 0.0], [0.35, 0.25, 0.35], 3, GOLD, 0.6);
            }),
            rod: one(r, |g| {
                g.column(
                    [0.0; 3],
                    6,
                    (1.0, 0.8),
                    1.0,
                    0.0,
                    rgb(110, 78, 52),
                    0.0,
                    true,
                )
            }),
            orb: one(r, |g| {
                g.blob([0.0; 3], [1.0, 1.0, 1.0], 9, rgb(255, 244, 220), 1.0)
            }),
        };
        let mut statics = vec![
            Item::new(m.ground, m4::ID),
            Item::new(m.shrine, m4::ID),
            Item::new(m.bell, m4::place([0.0, 4.6, 0.0], 0.0, [1.0; 3])).glow(0.3),
        ];
        let mut still = vec![Light {
            p: [0.0, 4.4, 0.0],
            r: 14.0,
            c: geo::scale(GOLD, 1.6),
        }];
        // A forest about the clearing, rocks among it.
        for k in 0..700 {
            let a = unit(hash(k, 0, 1)) * std::f32::consts::TAU;
            let d = 16.0 + unit(hash(k, 1, 1)).sqrt() * (HALF - 20.0);
            let (x, z) = (a.cos() * d, a.sin() * d);
            if height(x, z) < WATER + 0.4 {
                continue;
            }
            let at = [x, height(x, z) - 0.1, z];
            let yaw = unit(hash(k, 2, 1)) * 6.3;
            let s = 0.8 + 0.6 * unit(hash(k, 3, 1));
            if k % 5 == 0 {
                let rock = m.rocks[(k as usize / 5) % 3];
                statics.push(Item::new(rock, m4::place(at, yaw, [s * 1.3, s, s * 1.2])));
                continue;
            }
            statics.push(Item::new(m.trunk, m4::place(at, yaw, [s; 3])));
            statics.push(Item::new(
                m.crowns[k as usize % 2],
                m4::place(at, yaw, [s; 3]),
            ));
        }
        // Crystals in a ring, each a light.
        for k in 0..40 {
            let a = k as f32 / 40.0 * std::f32::consts::TAU + 0.1;
            let d = 13.0 + 9.0 * unit(hash(k, 5, 2));
            let (x, z) = (a.cos() * d, a.sin() * d);
            let at = [x, height(x, z), z];
            let hue = mix(rgb(120, 200, 255), rgb(200, 140, 255), unit(hash(k, 6, 2)));
            statics.push(Item::new(m.crystal, m4::place(at, a, [1.0; 3])).tint(hue, 1.0));
            still.push(Light {
                p: [x, at[1] + 1.2, z],
                r: 7.0,
                c: geo::scale(hue, 1.1),
            });
        }
        r.statics(statics);
        Scene { m, still }
    }

    /// The look at dusk.
    pub fn look() -> Look {
        Look {
            fog: rgb(64, 58, 92),
            fog_range: (35.0, 160.0),
            sky: rgb(70, 76, 120),
            low: rgb(34, 30, 38),
            sun_dir: geo::norm([-0.6, 0.12, -0.5]),
            sun: rgb(150, 100, 80),
            sun_size: 0.05,
            zenith: rgb(18, 22, 52),
            deep: rgb(20, 18, 30),
            stars: 0.6,
        }
    }

    /// What moves this frame (at `t` seconds), seen by this camera: items,
    /// lights and sparks.
    pub fn frame(&self, t: f32, cam: &render::Camera) -> (Vec<Item>, Vec<Light>, Vec<Spark>) {
        let mut items = vec![Item::new(
            self.m.water,
            m4::place([24.0, WATER + 0.02 * (t * 1.3).sin(), -18.0], 0.0, [1.0; 3]),
        )
        .tint(rgb(150, 190, 220), 0.55)
        .pass(Pass::Faint)];
        let mut lights = self.still.clone();
        let mut sparks = Vec::with_capacity(FLIES + 64);
        for k in 0..FLIES as i32 {
            let a = unit(hash(k, 7, 3)) * std::f32::consts::TAU
                + t * (0.05 + 0.1 * unit(hash(k, 8, 3)));
            let d = 12.0 + 50.0 * unit(hash(k, 9, 3));
            let (x, z) = (a.cos() * d + (t * 0.7 + k as f32).sin() * 1.5, a.sin() * d);
            let y =
                height(x, z) + 1.0 + 1.2 * unit(hash(k, 10, 3)) + 0.4 * (t * 1.7 + k as f32).sin();
            let c = mix(rgb(255, 220, 120), rgb(160, 255, 170), unit(hash(k, 11, 3)));
            let pulse = 0.6 + 0.4 * (t * 2.3 + k as f32 * 1.7).sin();
            lights.push(Light {
                p: [x, y, z],
                r: 4.0,
                c: geo::scale(c, 0.9 * pulse),
            });
            sparks.push(Spark {
                p: [x, y, z],
                size: 0.18,
                c: [c[0], c[1], c[2], pulse],
            });
        }
        // The wand in view, and its cast.
        let fwd = cam.forward();
        let (_, right, up) = cam.matrices(cam.fov);
        let at = |ahead: f32, side: f32, lift: f32| {
            geo::add(
                cam.eye,
                geo::add(
                    geo::scale(fwd, ahead),
                    geo::add(geo::scale(right, side), geo::scale(up, lift)),
                ),
            )
        };
        let phase = (t % CAST_EVERY) / CAST_EVERY;
        let kick = (1.0 - phase * 6.0).max(0.0);
        let grip = at(0.36 - 0.06 * kick, 0.2, -0.27);
        let tip = at(0.7 - 0.06 * kick, 0.16, -0.21 + 0.05 * kick);
        let along = geo::sub(tip, grip);
        items.push(
            Item::new(
                self.m.rod,
                m4::basis(grip, along, geo::scale(up, 0.014), geo::scale(right, 0.014)),
            )
            .pass(Pass::View),
        );
        let s = 0.03 + 0.02 * kick;
        items.push(
            Item::new(
                self.m.orb,
                m4::basis(
                    tip,
                    geo::scale(fwd, s),
                    geo::scale(up, s),
                    geo::scale(right, s),
                ),
            )
            .tint(GOLD, 1.0)
            .glow(1.0)
            .pass(Pass::View),
        );
        lights.push(Light {
            p: tip,
            r: 3.0 + 4.0 * kick,
            c: geo::scale(GOLD, 0.8 + 1.5 * kick),
        });
        // The bolt flies out from the tip.
        if phase < 0.5 {
            let bolt = geo::add(tip, geo::scale(fwd, phase * 60.0));
            lights.push(Light {
                p: bolt,
                r: 8.0,
                c: geo::scale(GOLD, 2.0),
            });
            for k in 0..24 {
                let back = k as f32 * 0.12;
                let p = geo::sub(bolt, geo::scale(fwd, back));
                let fade = 1.0 - k as f32 / 24.0;
                sparks.push(Spark {
                    p,
                    size: 0.35 * fade + 0.05,
                    c: [1.0, 0.85, 0.55, fade],
                });
            }
        }
        (items, lights, sparks)
    }
}

fn one(r: &mut Renderer, f: impl Fn(&mut Geo)) -> Mesh {
    let mut g = Geo::default();
    f(&mut g);
    r.mesh(&g)
}
