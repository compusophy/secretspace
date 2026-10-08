//! How Wandfall looks, drawn by the engine: the island (its ground in
//! one mesh, the sea, trees, rocks and ruined pillars as statics), every
//! wizard (a robe and a hat in their colour, a head, a wand), bolts of
//! light, bursts where they strike, the storm's wall, and your own wand.

use render::geo::{self, hash, mix, rgb, unit, Geo, V3};
use render::{m4, Item, Light, Material, Mesh, Pass, Renderer, Spark, Terrain};

use crate::fx::Draw;
use wandfall::laws::{MAP_HALF, SEA};
use wandfall::map::{Kind, Map};

pub const GOLD: V3 = rgb(255, 214, 128);
const STORM: V3 = rgb(150, 70, 230);

/// How a wizard stands this frame: bobbing as it walks, flashing when
/// hit, its arm raised
/// to cast (0 at rest, 1 along its aim, `aim` radians up), its wand's tip
/// alight (a colour, and how bright: 1 just cast), gliding down.
pub struct Pose {
    pub bob: f32,
    /// Just hit: 1 flashes it white.
    pub flash: f32,
    pub arm: f32,
    pub aim: f32,
    pub tip: (V3, f32),
    pub glide: bool,
}

pub struct Look {
    /// A wizard: its robe (and left sleeve) in its colour, hat and mantle
    /// darker, the rest as it is (a face, a beard or not, a belt, a
    /// hand); its right arm apart, to raise: the sleeve, and the hand
    /// with the wand.
    pub robe: Mesh,
    pub hat: Mesh,
    pub body: [Mesh; 2],
    pub sleeve: Mesh,
    pub hand: Mesh,
    /// A plain ball (no glow).
    pub ball: Mesh,
    pub glider: Mesh,
    pub orb: Mesh,
    pub rod: Mesh,
    pub wall: Mesh,
    pub chest: Mesh,
    pub lid: Mesh,
    /// A flat ring a metre across (marks, shockwaves); an ice shard a
    /// metre each way along x; a beam a metre up y.
    pub ring: Mesh,
    pub shard: Mesh,
    pub beam: Mesh,
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
                    (0.47, 0.12),
                    (0.36, 0.6),
                    (0.28, 0.98),
                    (0.27, 1.12),
                    (0.2, 1.34),
                    (0.0, 1.4),
                ];
                g.lathe([0.0; 3], &profile, 18, [1.0; 3], 0.0);
                // The left sleeve, hanging a little forward.
                g.lathe(
                    [0.06, 0.84, -0.3],
                    &[(0.11, 0.0), (0.09, 0.2), (0.07, 0.46)],
                    10,
                    [1.0; 3],
                    0.0,
                );
            }),
            hat: smooth(r, |g| {
                let profile = [
                    (0.47, 0.0),
                    (0.47, 0.035),
                    (0.27, 0.065),
                    (0.2, 0.3),
                    (0.11, 0.62),
                    (0.0, 0.86),
                ];
                g.lathe([0.0, 1.66, 0.0], &profile, 18, [1.0; 3], 0.0);
                // A mantle over the shoulders.
                g.lathe(
                    [0.0, 1.1, 0.0],
                    &[
                        (0.33, 0.0),
                        (0.35, 0.1),
                        (0.3, 0.2),
                        (0.17, 0.3),
                        (0.0, 0.33),
                    ],
                    18,
                    [1.0; 3],
                    0.0,
                );
            }),
            body: [true, false].map(|beard| {
                smooth(r, |g| {
                    let skin = rgb(214, 160, 120);
                    let gold = rgb(214, 170, 90);
                    let white = rgb(248, 248, 252);
                    let head = ([0.0, 1.52, 0.0], [0.19, 0.21, 0.19]);
                    g.sphere(head.0, head.1, (2, 4, 0.02), skin, 0.0);
                    // Eyes (so you see which way it faces), brows, a nose.
                    for z in [-0.06f32, 0.06] {
                        let ball = [0.172, 1.565, z];
                        g.sphere(ball, [0.024; 3], (1, 5, 0.0), white, 0.0);
                        let dot = [0.19, 1.565, z];
                        g.sphere(dot, [0.012; 3], (1, 5, 0.0), rgb(30, 40, 60), 0.0);
                        let (a, b) = ([0.17, 1.61, z - 0.03], [0.17, 1.615, z + 0.03]);
                        g.spike(a, b, 0.012, 4, white, 0.0);
                    }
                    let nose = ([0.2, 1.5, 0.0], [0.04, 0.035, 0.035]);
                    g.sphere(nose.0, nose.1, (1, 6, 0.0), skin, 0.0);
                    if beard {
                        let (a, b) = ([0.11, 1.45, 0.0], [0.22, 1.12, 0.0]);
                        g.spike(a, b, 0.12, 10, white, 0.0);
                    }
                    // Belt, hem and hat band.
                    let belt = rgb(90, 62, 40);
                    g.lathe(
                        [0.0, 0.94, 0.0],
                        &[(0.29, 0.0), (0.295, 0.07)],
                        18,
                        belt,
                        0.0,
                    );
                    g.lathe([0.0; 3], &[(0.51, 0.0), (0.49, 0.07)], 18, gold, 0.1);
                    let band = [(0.26, 0.0), (0.245, 0.06)];
                    g.lathe([0.0, 1.725, 0.0], &band, 18, gold, 0.1);
                    // The left hand.
                    g.sphere([0.06, 0.8, -0.3], [0.065; 3], (1, 7, 0.0), skin, 0.0);
                })
            }),
            ball: smooth(r, |g| {
                g.sphere([0.0; 3], [1.0; 3], (2, 3, 0.0), [1.0; 3], 0.0)
            }),
            // The right arm hangs from its shoulder down -y; it is turned
            // up to cast.
            sleeve: smooth(r, |g| {
                g.lathe(
                    [0.0, -0.5, 0.0],
                    &[(0.11, 0.0), (0.09, 0.22), (0.075, 0.5)],
                    10,
                    [1.0; 3],
                    0.0,
                );
            }),
            hand: smooth(r, |g| {
                g.sphere(
                    [0.0, -0.55, 0.0],
                    [0.065; 3],
                    (1, 8, 0.0),
                    rgb(214, 160, 120),
                    0.0,
                );
                g.lathe(
                    [0.0, -1.0, 0.0],
                    &[(0.016, 0.0), (0.022, 0.25), (0.028, 0.47)],
                    8,
                    rgb(110, 76, 50),
                    0.0,
                );
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
            ring: one(r, |g| {
                let n = 48;
                for k in 0..n {
                    let at = |k: i32, r: f32| {
                        let a = k as f32 / n as f32 * std::f32::consts::TAU;
                        [a.cos() * r, 0.0, a.sin() * r]
                    };
                    let (a, b, c, d) = (at(k, 0.8), at(k + 1, 0.8), at(k, 1.0), at(k + 1, 1.0));
                    g.tri(a, b, c, [1.0; 3], 1.0);
                    g.tri(b, d, c, [1.0; 3], 1.0);
                }
            }),
            shard: one(r, |g| {
                let base = [-0.2, 0.0, 0.0];
                g.spike(base, [1.0, 0.0, 0.0], 1.0, 4, [1.0; 3], 1.0);
                g.spike(base, [-1.0, 0.0, 0.0], 1.0, 4, [1.0; 3], 1.0);
            }),
            beam: smooth(r, |g| {
                g.lathe([0.0; 3], &[(1.0, 0.0), (1.0, 1.0)], 10, [1.0; 3], 1.0)
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
            self.body[0],
            self.body[1],
            self.sleeve,
            self.hand,
            self.ball,
            self.glider,
            self.orb,
            self.rod,
            self.wall,
            self.chest,
            self.lid,
            self.ring,
            self.shard,
            self.beam,
        ];
        for m in all.into_iter().chain(self.held) {
            r.free(m);
        }
        r.statics(Vec::new());
    }

    /// A wizard standing at `at`, facing `yaw` (radians), posed.
    pub fn wizard(&self, d: &mut Draw, id: u16, at: V3, yaw: f32, pose: &Pose) {
        let at = [at[0], at[1] + pose.bob, at[2]];
        let m = m4::place(at, yaw, [1.0; 3]);
        let c = hue(id);
        let flash = geo::mix(c, [1.0; 3], 0.7 * pose.flash);
        d.items.push(
            Item::new(self.robe, m)
                .tint(flash, 1.0)
                .glow(0.6 * pose.flash)
                .rough(0.85),
        );
        d.items.push(
            Item::new(self.hat, m)
                .tint(geo::scale(c, 0.62), 1.0)
                .rough(0.8),
        );
        d.items
            .push(Item::new(self.body[id as usize % 2], m).rough(0.7));
        // The right arm: hanging forward at rest, along the aim to cast.
        let rest = 0.55;
        let up = std::f32::consts::FRAC_PI_2 + pose.aim.clamp(-1.0, 1.0);
        let th = rest + (up - rest) * pose.arm.clamp(0.0, 1.0);
        let (s, co) = yaw.sin_cos();
        let turn = |v: V3| [co * v[0] - s * v[2], v[1], s * v[0] + co * v[2]];
        let shoulder = geo::add(at, turn([0.04, 1.3, 0.27]));
        let (st, ct) = th.sin_cos();
        let along = turn([st, -ct, 0.0]);
        let x = turn([ct, st, 0.0]);
        let z = turn([0.0, 0.0, 1.0]);
        let arm = m4::basis(shoulder, x, geo::scale(along, -1.0), z);
        d.items
            .push(Item::new(self.sleeve, arm).tint(c, 1.0).rough(0.85));
        d.items.push(Item::new(self.hand, arm).rough(0.6));
        let tip = geo::add(shoulder, geo::scale(along, 1.02));
        let (col, flare) = pose.tip;
        let k = 0.055 + 0.07 * flare;
        d.items.push(
            Item::new(self.orb, m4::place(tip, 0.0, [k; 3]))
                .tint(col, 1.0)
                .glow(1.0)
                .pass(Pass::Glow),
        );
        if flare > 0.05 {
            d.lights.push(Light {
                p: tip,
                r: 3.0 + 5.0 * flare,
                c: geo::scale(col, 2.5 * flare),
            });
        }
        if pose.glide {
            let top = [at[0], at[1] + 2.7, at[2]];
            d.items.push(
                Item::new(self.glider, m4::place(top, yaw, [1.4, 1.0, 1.4]))
                    .tint(rgb(200, 230, 255), 0.5)
                    .glow(0.5)
                    .pass(Pass::Faint),
            );
        }
    }

    /// Light bursting where something struck, `age` ms ago, in the
    /// colour of what struck.
    pub fn burst(
        &self,
        lights: &mut Vec<Light>,
        sparks: &mut Vec<Spark>,
        at: V3,
        (age, seed): (f32, u32),
        c: V3,
    ) {
        let k = 1.0 - age / 600.0;
        if k <= 0.0 {
            return;
        }
        lights.push(Light {
            p: at,
            r: 6.0,
            c: geo::scale(c, 2.0 * k),
        });
        for n in 0..14 {
            let a = unit(hash(seed as i32, n, 3)) * std::f32::consts::TAU;
            let up = unit(hash(seed as i32, n, 4));
            let d = (1.0 - k) * 2.2;
            sparks.push(Spark {
                p: [at[0] + a.cos() * d, at[1] + up * d, at[2] + a.sin() * d],
                size: 0.15,
                c: [0.5 + c[0] * 0.5, 0.5 + c[1] * 0.5, 0.5 + c[2] * 0.5, k],
            });
        }
    }

    /// The storm's wall: a circle of violet light from the sea to the sky.
    /// Its edge glows where you see it side on; where it meets the ground
    /// it burns; nearest you, it crackles (`eye`, `t` seconds).
    pub fn storm(&self, d: &mut Draw, centre: [f32; 2], r: f32, eye: V3, t: f32) {
        if r <= 0.5 {
            return;
        }
        let at = [centre[0], SEA - 6.0, centre[1]];
        d.items.push(
            Item::new(self.wall, m4::place(at, 0.0, [r, 75.0, r]))
                .tint([1.0; 3], 0.34)
                .glow(0.7)
                .pass(Pass::Faint),
        );
        d.items.push(
            Item::new(self.wall, m4::place(at, t * 0.05, [r + 0.4, 75.0, r + 0.4]))
                .tint(STORM, 0.7)
                .glow(0.5)
                .material(Material::Rim)
                .pass(Pass::Glow),
        );
        let foot = [centre[0], SEA - 0.5, centre[1]];
        d.items.push(
            Item::new(self.wall, m4::place(foot, 0.0, [r + 0.2, 3.0, r + 0.2]))
                .tint(rgb(220, 160, 255), 0.5)
                .glow(1.0)
                .pass(Pass::Glow),
        );
        // Crackling, along the stretch of wall nearest you.
        let (dx, dz) = (eye[0] - centre[0], eye[2] - centre[1]);
        if (dx * dx + dz * dz).sqrt() > r + 60.0 {
            return;
        }
        let a0 = dz.atan2(dx);
        let flick = (t * 14.0) as i32;
        for n in 0..36 {
            let a = a0 + (unit(hash(flick, n, 71)) - 0.5) * (40.0 / r.max(10.0)).min(3.0);
            let y = eye[1] - 4.0 + 16.0 * unit(hash(flick, n, 72));
            d.sparks.push(Spark {
                p: [centre[0] + a.cos() * r, y, centre[1] + a.sin() * r],
                size: 0.2 + 0.25 * unit(hash(flick, n, 73)),
                c: [0.85, 0.6, 1.0, 0.9],
            });
        }
    }

    /// Your wand before you: where its tip is, given the camera, and the
    /// kick of a cast (1 just cast).
    pub fn wand(
        &self,
        items: &mut Vec<Item>,
        lights: &mut Vec<Light>,
        cam: &render::Camera,
        kick: f32,
        (c, robe): (V3, V3),
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
        // Your sleeve, from out of sight to the hand on the grip.
        let shoulder = at(0.02, 0.34, -0.55);
        let arm = geo::sub(grip, shoulder);
        let side = geo::norm(geo::cross(arm, up));
        let other = geo::norm(geo::cross(side, arm));
        items.push(
            Item::new(
                self.sleeve,
                m4::basis(
                    shoulder,
                    geo::scale(side, 0.55),
                    geo::scale(arm, -2.0),
                    geo::scale(other, 0.55),
                ),
            )
            .tint(robe, 1.0)
            .rough(0.85)
            .pass(Pass::View),
        );
        items.push(
            Item::new(self.ball, m4::place(grip, 0.0, [0.04; 3]))
                .tint(rgb(214, 160, 120), 1.0)
                .pass(Pass::View),
        );
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
            .tint(c, 1.0)
            .glow(1.0)
            .pass(Pass::View),
        );
        lights.push(Light {
            p: tip,
            r: 2.5 + 4.0 * kick,
            c: geo::scale(c, 0.6 + 1.6 * kick),
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
        fog: 0.0011,
        fog_falloff: 0.015,
        clouds: 0.5,
        stars: 0.0,
        exposure: 1.0,
        bloom: 0.05,
        vignette: 0.28,
        sea: Some(SEA),
        water: rgb(16, 76, 106),
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
