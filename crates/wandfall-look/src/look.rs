//! How Wandfall looks, drawn by the engine: the island (`land`), every
//! wizard (jointed, in `rig`), the meshes spells are drawn with, and the
//! storm's wall.

use render::geo::{self, hash, rgb, unit, Geo, V3};
use render::{m4, Item, Light, Material, Mesh, Pass, Renderer, Shape, Spark};

use crate::fx::{self, Draw, Ray};
use crate::land::Land;
use crate::rig::Rig;
use wandfall::laws::{SEA, SPELLS};
use wandfall::map::Map;

pub const GOLD: V3 = rgb(255, 214, 128);
const STORM: V3 = rgb(150, 70, 230);
/// The storm's lightning, and where its wall burns along the ground.
const BOLT: V3 = rgb(215, 170, 255);
const SEAM: V3 = rgb(225, 150, 255);
/// How tall the storm's wall stands (m): over the drop, so its top is
/// never seen from a broom.
const WALL: f32 = 300.0;

pub struct Look {
    /// Every wizard, jointed (`rig`).
    pub rig: Rig,
    /// A plain ball (no glow).
    pub ball: Mesh,
    pub orb: Mesh,
    pub wall: Mesh,
    /// A soft flat ring a metre across (shockwaves); an arcane circle;
    /// an ice crystal a metre along x; a beam, a ray drawn to a point at
    /// each end, a shaft fading up, a pillar fading at its top, each a
    /// metre up y (`fx::meshes`).
    pub ring: Mesh,
    pub sigil: Mesh,
    pub crystal: Mesh,
    pub beam: Mesh,
    pub ray: Mesh,
    pub shaft: Mesh,
    pub pillar: Mesh,
    /// A spell cube a unit across for each spell, its icon on every face.
    pub cubes: Vec<Mesh>,
    /// The island (`land`), and its ground for what lies on it.
    pub land: Land,
    pub ground: fx::Ground,
    /// Whether the renderer lays decals (it reads the scene's depth only
    /// with ambient occlusion or the sun's shafts on; a new tier builds
    /// a new look): if not, what lies on the ground is drawn over it.
    pub marks: bool,
}

pub use crate::rig::hue;

fn one(r: &mut Renderer, f: impl Fn(&mut Geo)) -> Mesh {
    let mut g = Geo::default();
    f(&mut g);
    r.mesh(&g)
}

/// A spell cube a unit across, centred: its icon (as the bar draws it) on
/// every face, a quad for each run of one colour in a row, its bright
/// parts glowing, about a dark core.
fn cube(r: &mut Renderer, sp: u8) -> Mesh {
    const N: i32 = 20;
    let mut c = pixels::Canvas::new(N, N);
    crate::icon::icon(&mut c, sp, pixels::Rect::new(0.0, 0.0, N as f32, N as f32));
    let core = rgb(16, 14, 26);
    let px = |i: i32, j: i32| {
        let o = ((j * N + i) * 4) as usize;
        let a = c.data[o + 3] as f32 / 255.0;
        let col = rgb(c.data[o], c.data[o + 1], c.data[o + 2]);
        let col = geo::mix(core, col, a);
        let lum = 0.3 * col[0] + 0.55 * col[1] + 0.15 * col[2];
        // Quantised a little, so a row runs into few quads.
        let q = |v: f32| (v * 24.0).round() / 24.0;
        (
            [q(col[0]), q(col[1]), q(col[2])],
            ((lum - 0.45) * 3.0).clamp(0.0, 1.6),
        )
    };
    let mut g = Geo::default();
    g.block([0.0, -0.49, 0.0], [0.98; 3], 0.0, core, core, 0.0);
    // Each face: right, down (as the icon's rows run), out.
    let faces: [(V3, V3, V3); 6] = [
        ([1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]),
        ([-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, 0.0, -1.0], [0.0, -1.0, 0.0], [1.0, 0.0, 0.0]),
        ([0.0, 0.0, 1.0], [0.0, -1.0, 0.0], [-1.0, 0.0, 0.0]),
        ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, -1.0, 0.0]),
    ];
    for (u, v, n) in faces {
        let at = |i: i32, j: i32| {
            let (a, b) = (i as f32 / N as f32 - 0.5, j as f32 / N as f32 - 0.5);
            geo::add(
                geo::scale(n, 0.502),
                geo::add(geo::scale(u, a), geo::scale(v, b)),
            )
        };
        for j in 0..N {
            let mut i = 0;
            while i < N {
                let (col, glow) = px(i, j);
                let mut e = i + 1;
                while e < N && px(e, j) == (col, glow) {
                    e += 1;
                }
                g.quad(at(i, j), at(i, j + 1), at(e, j + 1), at(e, j), col, glow);
                i = e;
            }
        }
    }
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
        let q = r.quality();
        Look {
            land: Land::new(r, map),
            ground: fx::Ground::new(map),
            marks: q.ao > 0 || q.shafts,
            rig: Rig::new(r),
            ball: smooth(r, |g| {
                g.sphere([0.0; 3], [1.0; 3], (2, 3, 0.0), [1.0; 3], 0.0)
            }),
            // The right arm hangs from its shoulder down -y; it is turned
            // up to cast.
            orb: smooth(r, |g| {
                g.sphere([0.0; 3], [1.0; 3], (2, 9, 0.0), rgb(255, 246, 225), 1.0)
            }),
            ring: r.mesh(&fx::meshes::soft_ring()),
            sigil: r.mesh(&fx::meshes::sigil()),
            crystal: r.mesh(&fx::meshes::crystal()),
            beam: r.mesh(&fx::meshes::beam()),
            ray: r.mesh(&fx::meshes::ray()),
            shaft: r.mesh(&fx::meshes::shaft()),
            pillar: r.mesh(&fx::meshes::pillar()),
            wall: one(r, |g| {
                g.column([0.0; 3], 64, (1.0, 1.0), 1.0, 0.0, STORM, 0.5, false);
            }),
            cubes: (0..SPELLS.len() as u8).map(|sp| cube(r, sp)).collect(),
        }
    }

    /// Let every mesh go (another island takes this one's place).
    pub fn free(self, r: &mut Renderer) {
        let all = [
            self.ball,
            self.orb,
            self.wall,
            self.ring,
            self.sigil,
            self.crystal,
            self.beam,
            self.ray,
            self.shaft,
            self.pillar,
        ];
        for m in all.into_iter().chain(self.land.held).chain(self.cubes) {
            r.free(m);
        }
        self.rig.free(r);
        r.statics(Vec::new());
    }

    /// The storm's wall: a dark violet veil from the sea to the sky (what
    /// lies beyond it dimmed), storm cloud flowing up it. Its edge glows
    /// where you see it side on; where it meets the sea, and the ground
    /// along the stretch nearest you, it burns; nearest you, it crackles,
    /// and now and then lightning crawls down it to the ground, lighting
    /// it (`eye`, `t` seconds).
    pub fn storm(&self, d: &mut Draw, centre: [f32; 2], r: f32, eye: V3, t: f32) {
        if r <= 0.5 {
            return;
        }
        let at = [centre[0], SEA - 6.0, centre[1]];
        d.items.push(
            Item::new(self.wall, m4::place(at, 0.0, [r, WALL, r]))
                .tint(rgb(90, 50, 140), 0.5)
                .glow(0.12)
                .pass(Pass::Faint),
        );
        d.items.push(
            Item::new(self.wall, m4::place(at, t * 0.05, [r + 0.4, WALL, r + 0.4]))
                .tint(STORM, 0.7)
                .glow(0.5)
                .material(Material::Rim)
                .pass(Pass::Glow),
        );
        // Storm cloud flowing up the wall (one layer: it can fill the
        // screen, and phones draw it too).
        d.items.push(
            Item::new(
                self.wall,
                m4::place(at, -t * 0.02, [r + 0.9, WALL, r + 0.9]),
            )
            .tint(rgb(115, 65, 205), 0.55)
            .glow(0.0)
            .detail(0.18)
            .rough(0.55)
            .material(Material::Energy)
            .pass(Pass::Glow),
        );
        let foot = [centre[0], SEA - 0.5, centre[1]];
        d.items.push(
            Item::new(self.wall, m4::place(foot, 0.0, [r + 0.2, 3.0, r + 0.2]))
                .tint(SEAM, 0.5)
                .glow(1.0)
                .pass(Pass::Glow),
        );
        // Crackling, along the stretch of wall nearest you.
        let (dx, dz) = (eye[0] - centre[0], eye[2] - centre[1]);
        if (dx * dx + dz * dz).sqrt() > r + 60.0 {
            return;
        }
        let a0 = dz.atan2(dx);
        // The wall at `a` round it, `up` over the ground (or the sea).
        let on = |a: f32, up: f32| {
            let (x, z) = (centre[0] + a.cos() * r, centre[1] + a.sin() * r);
            [x, self.ground.at(x, z) + up, z]
        };
        // Where it meets the ground, it burns: a seam of fire along it,
        // over hills and down to the shore, flames licking up from it.
        let span = (80.0 / r.max(10.0)).min(3.0);
        const LINKS: i32 = 40;
        for n in 0..LINKS {
            let a = |k: i32| a0 + (k as f32 / LINKS as f32 - 0.5) * span;
            let (p, q) = (on(a(n), 0.2), on(a(n + 1), 0.2));
            fx::beam(self, d, (p, q), 0.35, (SEAM, 0.6), Ray::Halo);
            fx::beam(self, d, (p, q), 0.06, (SEAM, 0.8), Ray::Core);
        }
        let lick = (t * 6.0) as i32;
        for n in 0..14 {
            let a = a0 + (unit(hash(lick, n, 75)) - 0.5) * span;
            let u = unit(hash(lick, n, 76));
            d.sparks.push(Spark {
                p: on(a, 0.4 + 0.5 * u),
                size: 0.9 + 0.8 * u,
                c: [0.9, 0.55, 1.0, 0.55],
                shape: Shape::Flame,
                seed: unit(hash(lick, n, 77)),
                ..Default::default()
            });
        }
        // Lightning: in some of each 0.4 s, a bolt down the wall within
        // 35 m either side of you, flickering for a quarter second.
        let slot = (t / 0.4).floor() as i32;
        for s in [slot - 1, slot] {
            let start = s as f32 * 0.4 + 0.4 * unit(hash(s, 2, 74));
            let age = t - start;
            if unit(hash(s, 1, 74)) > 0.3 || !(0.0..0.25).contains(&age) {
                continue;
            }
            let a = a0 + (unit(hash(s, 3, 74)) - 0.5) * (70.0 / r.max(10.0)).min(3.0);
            // Down to the ground where it strikes, lighting it there.
            let end = on(a, 0.0);
            let top = [
                end[0],
                end[1].max(eye[1]) + 22.0 + 18.0 * unit(hash(s, 4, 74)),
                end[2],
            ];
            let f = 1.0 - age / 0.25;
            let flick = s.wrapping_mul(7) + (age / 0.05) as i32;
            fx::forks(self, d, (top, end), 10, 7.0, flick, (0.1, BOLT, f));
            d.lights.push(Light {
                p: on(a, 3.0),
                r: 45.0,
                c: geo::scale(BOLT, 5.0 * f),
            });
        }
        let flick = (t * 14.0) as i32;
        for n in 0..36 {
            let a = a0 + (unit(hash(flick, n, 71)) - 0.5) * (40.0 / r.max(10.0)).min(3.0);
            let (x, z) = (centre[0] + a.cos() * r, centre[1] + a.sin() * r);
            let y = (eye[1] - 4.0 + 16.0 * unit(hash(flick, n, 72))).max(self.ground.at(x, z));
            d.sparks.push(Spark {
                p: [x, y, z],
                size: 0.2 + 0.25 * unit(hash(flick, n, 73)),
                c: [0.85, 0.6, 1.0, 0.9],
                ..Default::default()
            });
        }
    }
}
