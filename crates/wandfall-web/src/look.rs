//! How Wandfall looks, drawn by the engine: the island (its ground in
//! one mesh, the sea, trees, rocks and ruined pillars as statics), every
//! wizard (a robe and a hat in their colour, a head, a wand), bolts of
//! light, bursts where they strike, the storm's wall, and your own wand.

use render::geo::{self, hash, mix, rgb, unit, Geo, V3};
use render::{m4, Item, Light, Material, Mesh, Pass, Renderer, Spark, Terrain};
use wandfall::laws::{MAP_HALF, SEA};
use wandfall::map::{Kind, Map};

pub const GOLD: V3 = rgb(255, 214, 128);
const STORM: V3 = rgb(150, 70, 230);

pub struct Look {
    pub robe: Mesh,
    pub hat: Mesh,
    pub body: Mesh,
    pub glider: Mesh,
    pub orb: Mesh,
    pub rod: Mesh,
    pub wall: Mesh,
    pub chest: Mesh,
    pub lid: Mesh,
    pub roots: Mesh,
    /// The island's own meshes (ground, trees, rocks, pillars).
    pub held: Vec<Mesh>,
}

/// A wizard's colour from its id.
pub fn hue(id: u16) -> V3 {
    const HUES: [V3; 8] = [
        rgb(70, 110, 230),
        rgb(200, 60, 70),
        rgb(60, 160, 110),
        rgb(150, 80, 200),
        rgb(220, 140, 40),
        rgb(40, 170, 190),
        rgb(220, 90, 160),
        rgb(120, 120, 130),
    ];
    HUES[id as usize % HUES.len()]
}

fn one(r: &mut Renderer, f: impl Fn(&mut Geo)) -> Mesh {
    let mut g = Geo::default();
    f(&mut g);
    r.mesh(&g)
}

/// A mesh whose shared vertices are shaded smooth.
fn smooth(r: &mut Renderer, f: impl Fn(&mut Geo)) -> Mesh {
    let mut g = Geo::default();
    f(&mut g);
    g.smooth();
    r.mesh(&g)
}

/// Metres between the ground's samples.
const TERRAIN_CELL: f32 = 1.25;

impl Look {
    /// Build the island's meshes and set it down as statics.
    pub fn new(r: &mut Renderer, map: &Map) -> Look {
        let t = Terrain::sample(
            [-MAP_HALF, -MAP_HALF],
            MAP_HALF * 2.0,
            TERRAIN_CELL,
            |x, z| map.height(x, z),
        );
        r.terrain(&t);
        let ground = r.mesh(&t.mesh(SEA - 4.0, |_, _, _| [1.0; 3]));
        let bark = rgb(96, 74, 56);
        let trunk = smooth(r, |g| {
            let profile = [
                (0.46, -0.3),
                (0.36, 0.4),
                (0.27, 1.6),
                (0.2, 3.2),
                (0.1, 4.4),
            ];
            g.lathe([0.0; 3], &profile, 9, bark, 0.0);
        });
        let crowns = [11u32, 29, 47].map(|s| {
            smooth(r, |g| {
                let c = mix(
                    rgb(60, 104, 42),
                    rgb(98, 132, 54),
                    unit(hash(s as i32, 1, 7)),
                );
                let dark = mix(c, rgb(38, 76, 34), 0.45);
                g.sphere([0.0, 3.9, 0.0], [2.1, 1.8, 2.1], (2, s, 0.3), c, 0.0);
                g.sphere(
                    [0.6, 5.1, -0.3],
                    [1.5, 1.3, 1.5],
                    (2, s + 1, 0.3),
                    mix(c, rgb(136, 160, 72), 0.3),
                    0.0,
                );
                g.sphere([-0.8, 4.6, 0.6], [1.3, 1.1, 1.3], (2, s + 2, 0.3), c, 0.0);
                g.sphere([0.3, 3.3, 1.1], [1.2, 1.0, 1.2], (2, s + 3, 0.3), dark, 0.0);
            })
        });
        let pines = smooth(r, |g| {
            for (y, w, h) in [
                (1.5, 2.3, 2.5),
                (2.9, 1.85, 2.3),
                (4.1, 1.4, 2.1),
                (5.2, 0.95, 1.9),
            ] {
                g.lathe(
                    [0.0, y, 0.0],
                    &[(w, 0.0), (w * 0.5, h * 0.45), (0.0, h)],
                    12,
                    rgb(36, 80, 52),
                    0.0,
                );
            }
        });
        let rocks = [5u32, 17, 23].map(|s| {
            smooth(r, |g| {
                g.sphere(
                    [0.0, 0.35, 0.0],
                    [1.0, 0.85, 1.0],
                    (3, s, 0.34),
                    rgb(130, 126, 120),
                    0.0,
                )
            })
        });
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
        let mut statics = vec![Item::new(ground, m4::ID).material(Material::Terrain)];
        for (k, p) in map.props.iter().enumerate() {
            let at = [p.x, p.y, p.z];
            match p.kind {
                Kind::Tree => {
                    let s = p.scale;
                    statics.push(
                        Item::new(trunk, m4::place(at, p.yaw, [s; 3]))
                            .rough(0.9)
                            .detail(0.4),
                    );
                    let crown = if k % 3 == 0 { pines } else { crowns[k % 3] };
                    statics.push(
                        Item::new(crown, m4::place(at, p.yaw, [s; 3]))
                            .material(Material::Foliage)
                            .rough(0.7),
                    );
                }
                Kind::Rock => {
                    let s = p.scale;
                    let m = m4::place(at, p.yaw, [s * 1.2, s * 1.4, s * 1.1]);
                    statics.push(Item::new(rocks[k % 3], m).rough(0.85).detail(0.6));
                }
                Kind::Pillar => {
                    let m = m4::place(at, p.yaw, [1.0, p.h, 1.0]);
                    statics.push(Item::new(pillar, m).rough(0.7).detail(0.3));
                    if k % 2 == 0 {
                        let top = [p.x, p.y + p.h, p.z];
                        statics.push(
                            Item::new(cap, m4::place(top, p.yaw, [1.0; 3]))
                                .rough(0.75)
                                .detail(0.3),
                        );
                    }
                }
            }
        }
        r.statics(statics);
        let mut held = vec![ground, trunk, pines, pillar, cap];
        held.extend(crowns);
        held.extend(rocks);
        Look {
            held,
            robe: smooth(r, |g| {
                let profile = [
                    (0.5, 0.0),
                    (0.45, 0.25),
                    (0.33, 0.85),
                    (0.25, 1.2),
                    (0.2, 1.38),
                    (0.0, 1.42),
                ];
                g.lathe([0.0; 3], &profile, 16, [1.0; 3], 0.0);
                g.lathe(
                    [0.0, 1.0, 0.0],
                    &[(0.27, 0.0), (0.29, 0.1), (0.25, 0.2)],
                    16,
                    rgb(230, 218, 196),
                    0.0,
                );
            }),
            hat: smooth(r, |g| {
                let profile = [
                    (0.46, 0.0),
                    (0.46, 0.04),
                    (0.26, 0.07),
                    (0.18, 0.3),
                    (0.08, 0.58),
                    (0.0, 0.74),
                ];
                g.lathe([0.0, 1.64, 0.0], &profile, 16, [1.0; 3], 0.0);
            }),
            body: smooth(r, |g| {
                g.sphere(
                    [0.0, 1.5, 0.0],
                    [0.19, 0.21, 0.19],
                    (2, 4, 0.02),
                    rgb(236, 196, 160),
                    0.0,
                );
                // The wand, held out front and right; its tip alight.
                g.lathe(
                    [0.12, 0.92, 0.3],
                    &[(0.03, 0.0), (0.024, 0.3), (0.018, 0.56)],
                    8,
                    rgb(110, 76, 50),
                    0.0,
                );
                g.sphere([0.12, 1.5, 0.3], [0.06; 3], (1, 2, 0.0), GOLD, 1.0);
            }),
            glider: smooth(r, |g| {
                g.sphere(
                    [0.0; 3],
                    [1.0, 0.12, 1.0],
                    (2, 6, 0.05),
                    rgb(200, 230, 255),
                    0.6,
                )
            }),
            orb: smooth(r, |g| {
                g.sphere([0.0; 3], [1.0; 3], (2, 9, 0.0), rgb(255, 246, 225), 1.0)
            }),
            rod: smooth(r, |g| {
                g.lathe(
                    [0.0; 3],
                    &[(1.0, 0.0), (0.9, 0.5), (0.8, 1.0)],
                    8,
                    rgb(110, 78, 52),
                    0.0,
                )
            }),
            chest: one(r, |g| {
                let wood = rgb(120, 82, 50);
                g.block([0.0; 3], [1.1, 0.6, 0.75], 0.0, wood, rgb(100, 66, 40), 0.0);
                for x in [-0.4, 0.4] {
                    g.block([x, 0.0, 0.0], [0.08, 0.62, 0.78], 0.0, GOLD, GOLD, 0.25);
                }
            }),
            lid: one(r, |g| {
                g.block(
                    [0.0, 0.6, 0.0],
                    [1.14, 0.22, 0.79],
                    0.0,
                    rgb(130, 90, 56),
                    rgb(110, 74, 46),
                    0.0,
                );
                g.block([0.0, 0.6, 0.0], [0.1, 0.24, 0.81], 0.0, GOLD, GOLD, 0.3);
            }),
            roots: one(r, |g| {
                for k in 0..7 {
                    let a = k as f32 / 7.0 * std::f32::consts::TAU;
                    let (c, s) = (a.cos(), a.sin());
                    let base = [c * 0.55, 0.0, s * 0.55];
                    let tip = [c * 0.2, 1.1 + 0.2 * (k % 2) as f32, s * 0.2];
                    g.spike(base, tip, 0.09, 5, rgb(86, 120, 50), 0.15);
                }
            }),
            wall: one(r, |g| {
                g.column([0.0; 3], 64, (1.0, 1.0), 1.0, 0.0, STORM, 0.5, false);
            }),
        }
    }

    /// Let every mesh go (another island takes this one's place).
    pub fn free(self, r: &mut Renderer) {
        let all = [
            self.robe,
            self.hat,
            self.body,
            self.glider,
            self.orb,
            self.rod,
            self.wall,
            self.chest,
            self.lid,
            self.roots,
        ];
        for m in all.into_iter().chain(self.held) {
            r.free(m);
        }
        r.statics(Vec::new());
    }

    /// A wizard standing at `at`, facing `yaw` (radians).
    pub fn wizard(&self, items: &mut Vec<Item>, id: u16, at: V3, yaw: f32, glide: bool) {
        let m = m4::place(at, yaw, [1.0; 3]);
        let c = hue(id);
        items.push(Item::new(self.robe, m).tint(c, 1.0));
        items.push(Item::new(self.hat, m).tint(geo::scale(c, 0.7), 1.0));
        items.push(Item::new(self.body, m));
        if glide {
            let top = [at[0], at[1] + 2.6, at[2]];
            items.push(
                Item::new(self.glider, m4::place(top, yaw, [1.4, 1.0, 1.4]))
                    .tint(rgb(200, 230, 255), 0.5)
                    .glow(0.5)
                    .pass(Pass::Faint),
            );
        }
    }

    /// Light bursting where a bolt struck, `age` ms ago.
    pub fn burst(
        &self,
        lights: &mut Vec<Light>,
        sparks: &mut Vec<Spark>,
        at: V3,
        age: f32,
        seed: u32,
    ) {
        let k = 1.0 - age / 600.0;
        if k <= 0.0 {
            return;
        }
        lights.push(Light {
            p: at,
            r: 6.0,
            c: geo::scale(GOLD, 2.0 * k),
        });
        for n in 0..14 {
            let a = unit(hash(seed as i32, n, 3)) * std::f32::consts::TAU;
            let up = unit(hash(seed as i32, n, 4));
            let d = (1.0 - k) * 2.2;
            sparks.push(Spark {
                p: [at[0] + a.cos() * d, at[1] + up * d, at[2] + a.sin() * d],
                size: 0.15,
                c: [1.0, 0.85, 0.5, k],
            });
        }
    }

    /// The storm's wall: a circle of violet light from the sea to the sky.
    pub fn storm(&self, items: &mut Vec<Item>, centre: [f32; 2], r: f32) {
        if r <= 0.5 {
            return;
        }
        items.push(
            Item::new(
                self.wall,
                m4::place([centre[0], SEA - 6.0, centre[1]], 0.0, [r, 75.0, r]),
            )
            .tint([1.0; 3], 0.34)
            .glow(0.7)
            .pass(Pass::Faint),
        );
    }

    /// Your wand before you: where its tip is, given the camera, and the
    /// kick of a cast (1 just cast).
    pub fn wand(
        &self,
        items: &mut Vec<Item>,
        lights: &mut Vec<Light>,
        cam: &render::Camera,
        kick: f32,
    ) -> V3 {
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
        let grip = at(0.36 - 0.06 * kick, 0.2, -0.27);
        let tip = at(0.7 - 0.06 * kick, 0.16, -0.21 + 0.05 * kick);
        let along = geo::sub(tip, grip);
        items.push(
            Item::new(
                self.rod,
                m4::basis(grip, along, geo::scale(up, 0.014), geo::scale(right, 0.014)),
            )
            .pass(Pass::View),
        );
        let s = 0.018 + 0.016 * kick;
        items.push(
            Item::new(
                self.orb,
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
            r: 2.5 + 4.0 * kick,
            c: geo::scale(GOLD, 0.6 + 1.6 * kick),
        });
        tip
    }
}

/// The sky by day, a little after noon; violet and close when you stand
/// in the storm.
pub fn sky(in_storm: bool) -> render::Look {
    let day = render::Look {
        sun_dir: geo::norm([0.5, 0.62, 0.32]),
        sun: [3.3, 3.0, 2.6],
        sun_size: 0.035,
        sky: [0.36, 0.46, 0.66],
        low: [0.14, 0.13, 0.10],
        zenith: [0.09, 0.22, 0.58],
        horizon: [0.58, 0.68, 0.82],
        deep: [0.08, 0.12, 0.18],
        fog: 0.0016,
        fog_falloff: 0.015,
        clouds: 0.5,
        stars: 0.0,
        exposure: 1.0,
        bloom: 0.05,
        vignette: 0.28,
        sea: Some(SEA),
        water: rgb(10, 52, 74),
        waves: 0.4,
        wind: [0.9, 0.35],
    };
    if !in_storm {
        return day;
    }
    render::Look {
        sky: [0.30, 0.18, 0.44],
        zenith: [0.14, 0.05, 0.26],
        horizon: [0.42, 0.20, 0.58],
        fog: 0.05,
        fog_falloff: 0.002,
        clouds: 0.9,
        ..day
    }
}
