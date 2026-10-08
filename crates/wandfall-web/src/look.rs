//! How Wandfall looks, drawn by the engine: the island (its ground in
//! one mesh, the sea, trees, rocks and ruined pillars as statics), every
//! wizard (a robe and a hat in their colour, a head, a wand), bolts of
//! light, bursts where they strike, the storm's wall, and your own wand.

use render::geo::{self, hash, mix, rgb, unit, Geo, V3};
use render::{m4, Item, Light, Mesh, Pass, Renderer, Spark};
use wandfall::laws::{MAP_HALF, SEA};
use wandfall::map::{Kind, Map};

pub const GOLD: V3 = rgb(255, 214, 128);
const STORM: V3 = rgb(150, 70, 230);

pub struct Look {
    pub sea: Mesh,
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

fn ground(map: &Map) -> Geo {
    let step = 2.0;
    let n = (MAP_HALF * 2.0 / step) as usize;
    let at = |i: usize| -MAP_HALF + i as f32 * step;
    let h: Vec<f32> = (0..=n)
        .flat_map(|j| (0..=n).map(move |i| (i, j)))
        .map(|(i, j)| map.height(at(i), at(j)))
        .collect();
    let hh = |i: usize, j: usize| h[j * (n + 1) + i];
    let mut g = Geo::default();
    for j in 0..n {
        for i in 0..n {
            let (x0, z0, x1, z1) = (at(i), at(j), at(i + 1), at(j + 1));
            let c = [hh(i, j), hh(i, j + 1), hh(i + 1, j + 1), hh(i + 1, j)];
            let top = c.iter().cloned().fold(f32::MIN, f32::max);
            if top < SEA - 3.5 {
                continue;
            }
            let low = c.iter().cloned().fold(f32::MAX, f32::min);
            let k = unit(hash(i as i32, j as i32, 1));
            let grass = mix(rgb(86, 128, 58), rgb(112, 146, 66), k);
            let col = if low < SEA + 0.4 {
                mix(rgb(196, 178, 128), rgb(180, 160, 110), k)
            } else if top - low > 1.4 {
                mix(rgb(120, 116, 108), rgb(140, 134, 122), k)
            } else if low > 6.0 {
                mix(grass, rgb(134, 150, 84), 0.4)
            } else {
                grass
            };
            g.quad(
                [x0, c[0], z0],
                [x0, c[1], z1],
                [x1, c[2], z1],
                [x1, c[3], z0],
                col,
                0.0,
            );
        }
    }
    g
}

fn one(r: &mut Renderer, f: impl Fn(&mut Geo)) -> Mesh {
    let mut g = Geo::default();
    f(&mut g);
    r.mesh(&g)
}

impl Look {
    /// Build the island's meshes and set it down as statics.
    pub fn new(r: &mut Renderer, map: &Map) -> Look {
        let ground = r.mesh(&ground(map));
        let trunk = one(r, |g| {
            g.column(
                [0.0; 3],
                6,
                (0.35, 0.22),
                3.2,
                0.0,
                rgb(96, 72, 50),
                0.0,
                false,
            )
        });
        let crowns = [11u32, 29].map(|s| {
            one(r, |g| {
                let c = mix(
                    rgb(46, 92, 44),
                    rgb(76, 118, 52),
                    unit(hash(s as i32, 1, 7)),
                );
                g.blob([0.0, 3.6, 0.0], [2.0, 1.7, 2.0], s, c, 0.0);
                g.blob(
                    [0.4, 5.0, -0.2],
                    [1.4, 1.3, 1.4],
                    s + 1,
                    mix(c, rgb(120, 150, 70), 0.3),
                    0.0,
                );
            })
        });
        let pines = one(r, |g| {
            for (k, (y, w)) in [(2.0, 1.9), (3.4, 1.5), (4.6, 1.0)].into_iter().enumerate() {
                g.column(
                    [0.0, y, 0.0],
                    7,
                    (w, 0.0),
                    2.0,
                    k as f32,
                    rgb(40, 84, 52),
                    0.0,
                    false,
                );
            }
        });
        let rocks = [5u32, 17, 23].map(|s| {
            one(r, |g| {
                g.blob([0.0, 0.4, 0.0], [1.0, 0.9, 1.0], s, rgb(132, 128, 122), 0.0)
            })
        });
        let pillar = one(r, |g| {
            g.column(
                [0.0; 3],
                8,
                (0.55, 0.48),
                1.0,
                0.0,
                rgb(206, 200, 186),
                0.0,
                true,
            );
        });
        let cap = one(r, |g| {
            g.block(
                [0.0; 3],
                [1.3, 0.3, 1.3],
                0.0,
                rgb(206, 200, 186),
                rgb(170, 164, 152),
                0.0,
            )
        });
        let mut statics = vec![Item::new(ground, m4::ID)];
        for (k, p) in map.props.iter().enumerate() {
            let at = [p.x, p.y, p.z];
            match p.kind {
                Kind::Tree => {
                    let s = p.scale;
                    statics.push(Item::new(trunk, m4::place(at, p.yaw, [s; 3])));
                    let crown = if k % 3 == 0 { pines } else { crowns[k % 2] };
                    statics.push(Item::new(crown, m4::place(at, p.yaw, [s; 3])));
                }
                Kind::Rock => {
                    let s = p.scale;
                    statics.push(Item::new(
                        rocks[k % 3],
                        m4::place(at, p.yaw, [s * 1.2, s * 1.4, s * 1.1]),
                    ));
                }
                Kind::Pillar => {
                    statics.push(Item::new(pillar, m4::place(at, p.yaw, [1.0, p.h, 1.0])));
                    if k % 2 == 0 {
                        let top = [p.x, p.y + p.h, p.z];
                        statics.push(Item::new(cap, m4::place(top, p.yaw, [1.0; 3])));
                    }
                }
            }
        }
        r.statics(statics);
        Look {
            sea: one(r, |g| {
                g.floor((-1.0, -1.0), (1.0, 1.0), 0.0, rgb(60, 120, 160), 0.15)
            }),
            robe: one(r, |g| {
                g.column([0.0; 3], 8, (0.46, 0.2), 1.38, 0.0, [1.0; 3], 0.0, true);
                g.column(
                    [0.0, 1.0, 0.0],
                    8,
                    (0.24, 0.24),
                    0.2,
                    0.0,
                    rgb(230, 220, 200),
                    0.0,
                    false,
                );
            }),
            hat: one(r, |g| {
                g.column(
                    [0.0, 1.66, 0.0],
                    12,
                    (0.42, 0.42),
                    0.04,
                    0.0,
                    [0.8; 3],
                    0.0,
                    true,
                );
                g.column(
                    [0.0, 1.7, 0.0],
                    8,
                    (0.24, 0.0),
                    0.6,
                    0.3,
                    [1.0; 3],
                    0.0,
                    false,
                );
            }),
            body: one(r, |g| {
                g.blob(
                    [0.0, 1.5, 0.0],
                    [0.19, 0.2, 0.19],
                    4,
                    rgb(236, 196, 160),
                    0.0,
                );
                // The wand, held out front and right; its tip alight.
                g.column(
                    [0.12, 0.92, 0.3],
                    5,
                    (0.03, 0.02),
                    0.55,
                    0.0,
                    rgb(110, 76, 50),
                    0.0,
                    true,
                );
                g.blob([0.12, 1.5, 0.3], [0.06, 0.06, 0.06], 2, GOLD, 1.0);
            }),
            glider: one(r, |g| {
                g.blob([0.0; 3], [1.0, 0.12, 1.0], 6, rgb(200, 230, 255), 0.6)
            }),
            orb: one(r, |g| {
                g.blob([0.0; 3], [1.0; 3], 9, rgb(255, 246, 225), 1.0)
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

    pub fn sea(&self, items: &mut Vec<Item>, t: f32) {
        let y = SEA + 0.05 * (t * 0.8).sin();
        items.push(
            Item::new(self.sea, m4::place([0.0, y, 0.0], 0.0, [700.0, 1.0, 700.0]))
                .tint(rgb(120, 180, 220), 0.72)
                .pass(Pass::Faint),
        );
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

/// The sky by day; violet and close when you stand in the storm.
pub fn sky(in_storm: bool) -> render::Look {
    let day = render::Look {
        fog: rgb(178, 198, 222),
        fog_range: (90.0, 460.0),
        sky: rgb(118, 128, 150),
        low: rgb(62, 56, 50),
        sun_dir: geo::norm([0.45, 0.7, 0.3]),
        sun: rgb(225, 210, 186),
        sun_size: 0.045,
        zenith: rgb(66, 118, 205),
        deep: rgb(40, 60, 80),
        stars: 0.0,
    };
    if !in_storm {
        return day;
    }
    render::Look {
        fog: rgb(96, 50, 140),
        fog_range: (4.0, 60.0),
        sky: rgb(120, 90, 160),
        zenith: rgb(60, 30, 100),
        ..day
    }
}
