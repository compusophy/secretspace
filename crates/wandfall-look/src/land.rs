//! The island as it looks: its ground (`ground`: painted where the places
//! are, grass held back from the plaza and the rift) and everything
//! standing on it, set down as statics: trees and boulders (`flora`), the
//! Spire's tower, stair and lamps (`spire`), the places' stones, obsidian,
//! crystals and ruins and the islets floating over it all (`relics`), the
//! causeway's basalt (`basalt`). What moves at the places each frame (the
//! beacon, rune rings, the gate's swirl, embers, glimmer and their light)
//! is `aura`'s.

use std::f32::consts::TAU;

use render::geo::{self, hash, mix, rgb, unit, Geo, STRIDE, V3};
use render::{m4, Item, Material, Mesh, Renderer, Terrain};

use wandfall::laws::{MAP_HALF, SEA};
use wandfall::map::{Kind, Map};
use wandfall::places::Poi;

use crate::flora;

mod ground;
mod relics;
mod spire;

use relics::Relics;
use spire::Spire;

/// Metres between the ground's samples.
const TERRAIN_CELL: f32 = 1.25;
/// Trees and boulders farther than this are drawn coarser.
const FAR_AWAY: f32 = 30.0;
/// The ground in this many pieces a side, so a shadow's cascade draws
/// only those near it.
const GROUND_CHUNKS: usize = 4;

pub(crate) const VIOLET: V3 = rgb(178, 132, 255);
pub(crate) const CYAN: V3 = rgb(110, 225, 255);
const AMETHYST: V3 = rgb(140, 80, 255);
const AQUA: V3 = rgb(60, 190, 255);
pub(crate) const EMBER: V3 = rgb(255, 92, 30);
pub(crate) const GOLDEN: V3 = rgb(255, 206, 120);

/// The island's meshes and where its lights and moving things are.
pub struct Land {
    pub held: Vec<Mesh>,
    pub(crate) pois: Vec<Poi>,
    pub(crate) lamps: Vec<V3>,
    pub(crate) crystals: Vec<(V3, V3)>,
    /// The rift's gate: its foot and its turn.
    pub(crate) gate: (V3, f32),
    /// The beacon's crystal, and a ring of runes a metre across.
    pub(crate) gem: Mesh,
    pub(crate) runes: Mesh,
    /// The launch runes (their middles, on the ground).
    pub(crate) pads: Vec<V3>,
    /// The top of the causeway's crown.
    pub(crate) crown: V3,
}

/// A mesh, flat-shaded.
fn one(r: &mut Renderer, f: impl Fn(&mut Geo)) -> Mesh {
    let mut g = Geo::default();
    f(&mut g);
    r.mesh(&g)
}

/// A mesh whose shared vertices are shaded smooth.
pub(crate) fn smooth(r: &mut Renderer, f: impl Fn(&mut Geo)) -> Mesh {
    let mut g = Geo::default();
    f(&mut g);
    g.smooth();
    r.mesh(&g)
}

/// Lean the vertices from `from` on `lean` radians from upright, toward
/// `yaw`, then move them by `at`.
fn lean(g: &mut Geo, from: usize, lean: f32, yaw: f32, at: V3) {
    let (sl, cl) = lean.sin_cos();
    let (sy, cy) = yaw.sin_cos();
    let rot = |v: V3| {
        let (x, y, z) = (v[0] * cl + v[1] * sl, -v[0] * sl + v[1] * cl, v[2]);
        [x * cy - z * sy, y, x * sy + z * cy]
    };
    for k in from..g.len() {
        let o = k * STRIDE;
        let p = rot([g.v[o], g.v[o + 1], g.v[o + 2]]);
        let n = rot([g.v[o + 3], g.v[o + 4], g.v[o + 5]]);
        g.v[o..o + 3].copy_from_slice(&geo::add(p, at));
        g.v[o + 3..o + 6].copy_from_slice(&n);
    }
}

/// A six-sided crystal, `h` tall and `r` wide, pointed.
fn crystal(g: &mut Geo, r: f32, h: f32, col: V3, glow: f32) {
    g.column([0.0; 3], 6, (r, r * 0.86), h * 0.76, 0.3, col, glow, false);
    g.column(
        [0.0, h * 0.76, 0.0],
        6,
        (r * 0.86, 0.0),
        h * 0.24,
        0.3,
        col,
        glow,
        false,
    );
}

/// Glyphs about a ring a metre across, and the ring.
fn rune_ring(g: &mut Geo, glyphs: usize, seed: u32) {
    let at = |a: f32, r: f32| [a.cos() * r, 0.0, a.sin() * r];
    let n = 64;
    for k in 0..n {
        let (a0, a1) = (k as f32 / n as f32 * TAU, (k + 1) as f32 / n as f32 * TAU);
        g.quad(
            at(a0, 0.95),
            at(a0, 1.0),
            at(a1, 1.0),
            at(a1, 0.95),
            [1.0; 3],
            1.0,
        );
        g.quad(
            at(a0, 0.7),
            at(a0, 0.73),
            at(a1, 0.73),
            at(a1, 0.7),
            [1.0; 3],
            1.0,
        );
    }
    // Each glyph: a few strokes in a little box between the rings.
    for k in 0..glyphs {
        let mid = k as f32 / glyphs as f32 * TAU;
        let w = TAU / glyphs as f32 * 0.32;
        let p = |u: f32, v: f32| at(mid + (u - 0.5) * w, 0.76 + v * 0.15);
        for s in 0..3 {
            let h = |i| unit(hash(k as i32, s * 4 + i, seed));
            let (a, b) = (p(h(0), h(1)), p(h(2), h(3)));
            let d = geo::norm(geo::sub(b, a));
            let side = geo::scale([-d[2], 0.0, d[0]], 0.008);
            g.quad(
                geo::sub(a, side),
                geo::add(a, side),
                geo::add(b, side),
                geo::sub(b, side),
                [1.0; 3],
                1.0,
            );
        }
    }
}

impl Land {
    /// Build the island's meshes and set it down as statics.
    pub fn new(r: &mut Renderer, map: &Map) -> Land {
        let mut t = Terrain::sample(
            [-MAP_HALF, -MAP_HALF],
            MAP_HALF * 2.0,
            TERRAIN_CELL,
            |x, z| map.height(x, z),
        );
        t.grow(|x, z| ground::lush(map, x, z));
        r.terrain(&t);
        let ground: Vec<(Mesh, V3)> = t
            .chunks(GROUND_CHUNKS, SEA - 4.0, |x, z, _| ground::paint(map, x, z))
            .into_iter()
            .map(|(g, mid)| (r.mesh(&g), mid))
            .collect();
        // Trees and boulders, sculpted, near and far (`flora`).
        let near_far =
            |r: &mut Renderer, f: &dyn Fn(f32) -> Geo| (r.mesh(&f(1.0)), r.mesh(&f(flora::FAR)));
        let trunks = [3u32, 8].map(|s| near_far(r, &|q| flora::trunk(s, q)));
        // Crowns: green, a deep teal, and the wizard-wood's violet.
        let leaves = [
            (rgb(42, 78, 30), rgb(118, 150, 60)),
            (rgb(22, 70, 62), rgb(70, 136, 108)),
            (rgb(70, 44, 118), rgb(156, 108, 206)),
        ];
        let crowns = [11u32, 29, 47].map(|s| {
            let colours = leaves[(s as usize / 18) % 3];
            near_far(r, &|q| flora::crown(s, colours, q))
        });
        let pine_trunk = near_far(r, &|q| flora::pine_trunk(q));
        let pines = near_far(r, &|q| flora::pine(5, q));
        let rocks = [5u32, 17, 23].map(|s| near_far(r, &|q| flora::boulder(s, q)));
        let spire = Spire::new(r, map);
        let relics = Relics::new(r);

        let mut statics: Vec<Item> = ground
            .iter()
            .map(|&(m, at)| Item::new(m, m4::place(at, 0.0, [1.0; 3])).material(Material::Terrain))
            .collect();
        statics.push(Item::new(spire.decks, m4::ID).rough(0.85).detail(0.15));
        let mut lamps = Vec::new();
        let mut gems = Vec::new();
        let mut gate_at = ([0.0; 3], 0.0);
        let basalt = crate::basalt::Basalt::new(r);
        let mut crown = [0.0, f32::MIN, 0.0];
        for (k, p) in map.props.iter().enumerate() {
            let at = [p.x, p.y, p.z];
            let h = hash(k as i32, 3, 5);
            match p.kind {
                Kind::Tree => {
                    let s = p.scale;
                    let m = m4::place(at, p.yaw, [s; 3]);
                    let pine = k % 7 == 0 || k % 7 == 4;
                    let (trunk, crown) = if pine {
                        (pine_trunk, pines)
                    } else {
                        (
                            trunks[k % 2],
                            match k % 7 {
                                6 => crowns[2],
                                3 => crowns[1],
                                _ => crowns[0],
                            },
                        )
                    };
                    statics.push(
                        Item::new(trunk.0, m)
                            .far(trunk.1, FAR_AWAY)
                            .rough(0.9)
                            .detail(0.25),
                    );
                    statics.push(
                        Item::new(crown.0, m)
                            .far(crown.1, FAR_AWAY)
                            .material(Material::Foliage)
                            .rough(0.75),
                    );
                }
                Kind::Rock => {
                    let s = p.scale;
                    let m = m4::place(at, p.yaw, [s * 1.2, s * 1.4, s * 1.1]);
                    let (near, far) = rocks[k % 3];
                    statics.push(
                        Item::new(near, m)
                            .far(far, FAR_AWAY)
                            .rough(0.85)
                            .detail(0.35),
                    );
                }
                Kind::Pillar => {
                    let m = m4::place(at, p.yaw, [1.0, p.h, 1.0]);
                    statics.push(Item::new(relics.pillar, m).rough(0.7).detail(0.3));
                    if k % 2 == 0 {
                        let top = [p.x, p.y + p.h, p.z];
                        statics.push(
                            Item::new(relics.cap, m4::place(top, p.yaw, [1.0; 3]))
                                .rough(0.75)
                                .detail(0.3),
                        );
                    }
                }
                Kind::Shroom => {
                    let m = m4::place(at, p.yaw, [p.h; 3]);
                    statics.push(Item::new(relics.shrooms[k % 3], m).rough(0.6).detail(0.2));
                }
                Kind::Tower => {
                    let m = m4::place(at, 0.0, [1.0; 3]);
                    statics.push(Item::new(spire.tower, m).rough(0.8).detail(0.12));
                    statics.push(Item::new(spire.trim, m).rough(0.85).detail(0.1));
                }
                Kind::Merlon => {
                    let m = m4::place(at, p.yaw, [1.0; 3]);
                    statics.push(Item::new(spire.merlon, m).rough(0.85).detail(0.15));
                }
                Kind::Lamp => {
                    statics.push(Item::new(spire.lamp, m4::place(at, 0.0, [1.0; 3])).rough(0.5));
                    lamps.push([p.x, p.y + 3.15, p.z]);
                }
                Kind::Stone => {
                    let m = m4::place(at, p.yaw, [1.1, p.h, 1.9]);
                    statics.push(Item::new(relics.menhir, m).rough(0.9).detail(0.2));
                }
                Kind::Altar => {
                    statics.push(
                        Item::new(relics.altar, m4::place(at, 0.4, [1.0; 3]))
                            .rough(0.85)
                            .detail(0.5),
                    );
                }
                Kind::Spike => {
                    let m = m4::place(at, p.yaw, [p.r * 1.1, p.h, p.r * 1.1]);
                    statics.push(Item::new(relics.spike, m).rough(0.55));
                }
                Kind::Portal => {
                    statics
                        .push(Item::new(relics.gate, m4::place(at, p.yaw, [1.0; 3])).rough(0.15));
                    gate_at = (at, p.yaw);
                }
                Kind::Crystal => {
                    let c = mix(AMETHYST, AQUA, unit(h));
                    let m = m4::place(at, p.yaw, [p.r * 1.3, p.h, p.r * 1.3]);
                    statics.push(
                        Item::new(relics.crystals[k % 2], m)
                            .tint(c, 1.0)
                            .rough(0.25),
                    );
                    gems.push(([p.x, p.y + p.h * 0.6, p.z], c));
                }
                Kind::Column => {
                    basalt.put(&mut statics, p, k);
                    if p.y + p.h > crown[1] {
                        crown = [p.x, p.y + p.h, p.z];
                    }
                }
            }
        }
        for k in 0..7 {
            let a = (k as f32 + unit(hash(k, 1, map.seed as u32))) / 7.0 * TAU;
            let d = 36.0 + 34.0 * unit(hash(k, 2, map.seed as u32));
            let (x, z) = (a.cos() * d, a.sin() * d);
            let y = map.height(x, z).max(SEA) + 26.0 + 16.0 * unit(hash(k, 3, map.seed as u32));
            let s = 0.6 + 0.6 * unit(hash(k, 4, map.seed as u32));
            statics.push(
                Item::new(relics.islet, m4::place([x, y, z], a, [s; 3]))
                    .rough(0.9)
                    .detail(0.6),
            );
            if k % 2 == 0 {
                let m = m4::place([x, y + 0.6 * s, z], a, [s * 0.8; 3]);
                // Always far off: the coarser tree.
                statics.push(Item::new(trunks[0].1, m).rough(0.9));
                statics.push(Item::new(crowns[k as usize % 3].1, m).material(Material::Foliage));
            } else {
                let c = mix(AMETHYST, AQUA, (k % 3) as f32 / 2.0);
                let m = m4::basis(
                    [x, y - 4.0 * s, z],
                    [s * 1.4, 0.0, 0.0],
                    [0.0, -3.0 * s, 0.0],
                    [0.0, 0.0, s * 1.4],
                );
                statics.push(Item::new(relics.crystals[0], m).tint(c, 1.0).rough(0.25));
                gems.push(([x, y - 6.0 * s, z], c));
            }
        }
        r.statics(statics);
        let gem = smooth(r, |g| {
            g.spike([0.0; 3], [0.0, 1.0, 0.0], 0.55, 4, [1.0; 3], 1.0);
            g.spike([0.0; 3], [0.0, -1.0, 0.0], 0.55, 4, [1.0; 3], 1.0);
        });
        let runes = one(r, |g| rune_ring(g, 18, 5));
        let mut held: Vec<Mesh> = spire.meshes().into_iter().chain(relics.meshes()).collect();
        held.extend(ground.iter().map(|g| g.0));
        for (a, b) in trunks
            .into_iter()
            .chain(crowns)
            .chain(rocks)
            .chain([pine_trunk, pines])
        {
            held.extend([a, b]);
        }
        held.extend([gem, runes]);
        held.extend(basalt.meshes());
        Land {
            held,
            pois: map.pois.clone(),
            lamps,
            crystals: gems,
            gate: gate_at,
            gem,
            runes,
            pads: map.pads.clone(),
            crown,
        }
    }
}
