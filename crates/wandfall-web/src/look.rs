//! How Wandfall looks, drawn by the engine: the island (`land`), every
//! wizard (jointed, in `rig`), bolts of light, bursts where they strike,
//! the storm's wall, and your own wand.

use render::geo::{self, hash, rgb, unit, Geo, V3};
use render::{m4, Item, Light, Material, Mesh, Pass, Renderer, Spark};

use crate::fx::Draw;
use crate::land::Land;
use crate::rig::Rig;
use wandfall::laws::SEA;
use wandfall::map::Map;

pub const GOLD: V3 = rgb(255, 214, 128);
const STORM: V3 = rgb(150, 70, 230);

pub struct Look {
    /// Every wizard, jointed (`rig`).
    pub rig: Rig,
    /// Your own sleeve in first person, and a plain ball (no glow).
    pub sleeve: Mesh,
    pub ball: Mesh,
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
    /// The island (`land`).
    pub land: Land,
}

pub use crate::rig::hue;

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

impl Look {
    /// Build the island's meshes and set it down as statics.
    pub fn new(r: &mut Renderer, map: &Map) -> Look {
        Look {
            land: Land::new(r, map),
            rig: Rig::new(r),
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
            self.sleeve,
            self.ball,
            self.orb,
            self.rod,
            self.wall,
            self.chest,
            self.lid,
            self.ring,
            self.shard,
            self.beam,
        ];
        for m in all.into_iter().chain(self.land.held) {
            r.free(m);
        }
        self.rig.free(r);
        r.statics(Vec::new());
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
        // Late in a long afternoon: a low gold sun, a lilac horizon.
        sun_dir: geo::norm([0.55, 0.4, 0.3]),
        sun: [3.4, 2.75, 2.1],
        sun_size: 0.035,
        sky: [0.36, 0.40, 0.64],
        low: [0.15, 0.12, 0.10],
        zenith: [0.10, 0.17, 0.50],
        horizon: [0.76, 0.64, 0.78],
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
