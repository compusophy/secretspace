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
use wandfall::places::{Place, Poi};

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
    /// The launch runes: their middles, on the ground, and which way is
    /// up off the ground about them.
    pub(crate) pads: Vec<(V3, V3)>,
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

/// One geometry onto another.
pub(crate) fn join(g: &mut Geo, h: Geo) {
    let base = g.len() as u32;
    g.v.extend(h.v);
    g.i.extend(h.i.into_iter().map(|k| k + base));
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

/// Of sizes made, the index of the one nearest `want` (by how many times
/// over or under it is).
fn nearest(made: &[f32], want: f32) -> usize {
    (0..made.len())
        .min_by(|&a, &b| {
            let off = |k: usize| (made[k] / want).ln().abs();
            off(a).total_cmp(&off(b))
        })
        .unwrap_or(0)
}

/// Which way is up off the ground about `at` (its slope across a launch
/// rune's breadth).
pub(crate) fn slope(map: &Map, at: V3) -> V3 {
    let d = 1.5;
    let h = |dx: f32, dz: f32| map.height(at[0] + dx, at[2] + dz);
    let (sx, sz) = (
        (h(d, 0.0) - h(-d, 0.0)) / (2.0 * d),
        (h(0.0, d) - h(0.0, -d)) / (2.0 * d),
    );
    geo::norm([-sx, 1.0, -sz])
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
        let crowns = [
            (11u32, (rgb(42, 78, 30), rgb(118, 150, 60))),
            (29, (rgb(22, 70, 62), rgb(70, 136, 108))),
            (47, (rgb(70, 44, 118), rgb(156, 108, 206))),
        ]
        .map(|(s, colours)| near_far(r, &|q| flora::crown(s, colours, q)));
        let pine_trunk = near_far(r, &|q| flora::pine_trunk(q));
        let pines = [5u32, 12, 21].map(|s| near_far(r, &|q| flora::pine(s, q)));
        let rocks = [5u32, 17, 23].map(|s| near_far(r, &|q| flora::boulder(s, q)));
        let spire = Spire::new(r, map);
        let relics = Relics::new(r);

        let mut statics: Vec<Item> = ground
            .iter()
            .map(|&(m, at)| Item::new(m, m4::place(at, 0.0, [1.0; 3])).material(Material::Terrain))
            .collect();
        statics.push(Item::new(spire.decks, m4::ID).rough(0.85).detail(0.15));
        // What lies on the ground finer than its samples: the paving, the
        // lava.
        let mut laid = Vec::new();
        for p in &map.pois {
            let (g, rough) = match p.place {
                Place::Spire => (ground::paving(p), 0.8),
                Place::Rift => (ground::lava(p, map.seed as u32, &t), 0.5),
                _ => continue,
            };
            let m = r.mesh(&g);
            statics.push(Item::new(m, m4::ID).rough(rough).detail(0.25));
            laid.push(m);
        }
        let mut lamps = Vec::new();
        let mut gems = Vec::new();
        let mut gate_at = ([0.0; 3], 0.0);
        let basalt = crate::basalt::Basalt::new(r);
        // The causeway's crown: its tallest column.
        let mut crown = ([0.0; 3], f32::MIN);
        let far = |it: Item, near_far: (Mesh, Mesh)| it.far(near_far.1, FAR_AWAY);
        for (k, p) in map.props.iter().enumerate() {
            let at = [p.x, p.y, p.z];
            let h = hash(k as i32, 3, 5);
            match p.kind {
                Kind::Tree => {
                    let s = p.scale;
                    let m = m4::place(at, p.yaw, [s; 3]);
                    let pine = k % 7 == 0 || k % 7 == 4;
                    let (trunk, crown) = if pine {
                        (pine_trunk, pines[k % 3])
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
                    // Each its own shade: lighter or darker, warmer or
                    // cooler.
                    let u = |i| unit(hash(k as i32, i, 17)) - 0.5;
                    let shade = 0.95 + 0.1 * u(3);
                    let tint = [
                        (1.0 + 0.05 * u(0)) * shade,
                        (1.0 + 0.04 * u(1)) * shade,
                        (1.0 + 0.05 * u(2)) * shade,
                    ];
                    statics.push(far(Item::new(trunk.0, m), trunk).rough(0.9).detail(0.25));
                    statics.push(
                        far(Item::new(crown.0, m), crown)
                            .tint(tint, 1.0)
                            .material(Material::Foliage)
                            .rough(0.75),
                    );
                }
                Kind::Rock => {
                    let m = m4::place(at, p.yaw, geo::scale(flora::ROCK, p.scale));
                    let rock = rocks[k % 3];
                    statics.push(far(Item::new(rock.0, m), rock).rough(0.85).detail(0.35));
                }
                Kind::Pillar => {
                    let (shaft, cap) = relics::pillar_parts(p.h, k % 2 == 0);
                    let v = nearest(&relics::PILLARS, shaft);
                    let m = m4::place(at, p.yaw, [1.0, shaft / relics::PILLARS[v], 1.0]);
                    let pillar = relics.pillars[v];
                    statics.push(far(Item::new(pillar.0, m), pillar).rough(0.8).detail(0.3));
                    if let Some(y) = cap {
                        let m = m4::place([p.x, p.y + y, p.z], p.yaw, [1.0; 3]);
                        let cap = relics.cap;
                        statics.push(far(Item::new(cap.0, m), cap).rough(0.8).detail(0.3));
                    }
                }
                Kind::Shroom => {
                    let m = m4::place(at, p.yaw, [p.h; 3]);
                    statics.push(Item::new(relics.shrooms[k % 3], m).rough(0.6).detail(0.2));
                }
                Kind::Tower => {
                    let m = m4::place(at, 0.0, [1.0; 3]);
                    statics.push(Item::new(spire.tower, m).rough(0.8).detail(0.12));
                    statics.push(Item::new(spire.trim, m).rough(0.85).detail(0.25));
                }
                Kind::Merlon => {
                    let m = m4::place(at, p.yaw, [1.0; 3]);
                    let merlon = spire.merlon;
                    statics.push(far(Item::new(merlon.0, m), merlon).rough(0.85).detail(0.2));
                }
                Kind::Lamp => {
                    statics.push(Item::new(spire.lamp, m4::place(at, 0.0, [1.0; 3])).rough(0.5));
                    lamps.push([p.x, p.y + 3.15, p.z]);
                }
                Kind::Stone => {
                    // Taller or shorter: stretched upright only.
                    let m = m4::place(at, p.yaw, [1.0, p.h / relics::MENHIR, 1.0]);
                    let stone = relics.menhir;
                    statics.push(far(Item::new(stone.0, m), stone).rough(0.9).detail(0.25));
                }
                Kind::Altar => {
                    statics.push(
                        Item::new(relics.altar, m4::place(at, 0.4, [1.0; 3]))
                            .rough(0.85)
                            .detail(0.3),
                    );
                }
                Kind::Spike => {
                    let (wide, tall) = relics::SPIKE;
                    let w = p.r * 1.1 / wide;
                    let m = m4::place(at, p.yaw, [w, p.h / tall, w]);
                    let spike = relics.spikes[k % 3];
                    statics.push(far(Item::new(spike.0, m), spike).rough(0.12));
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
                    if p.h > crown.1 {
                        crown = ([p.x, p.y + p.h, p.z], p.h);
                    }
                }
            }
        }
        // Shards of obsidian about the rift's lip (small: underfoot).
        for p in map.pois.iter().filter(|p| p.place == Place::Rift) {
            for k in 0..12 {
                let u = |i| unit(hash(k, i, 0x5a4d));
                let a = u(0) * TAU;
                let d = p.r * (0.82 + 0.25 * u(1));
                let (x, z) = (p.x + a.cos() * d, p.z + a.sin() * d);
                let w = 0.25 + 0.15 * u(2);
                let m = m4::place(
                    [x, map.height(x, z) - 0.08, z],
                    a,
                    [w, 0.05 + 0.04 * u(3), w],
                );
                statics.push(Item::new(relics.spikes[k as usize % 3].1, m).rough(0.12));
            }
        }
        for k in 0..7 {
            let a = (k as f32 + unit(hash(k, 1, map.seed as u32))) / 7.0 * TAU;
            let d = 36.0 + 34.0 * unit(hash(k, 2, map.seed as u32));
            let (x, z) = (a.cos() * d, a.sin() * d);
            let y = map.height(x, z).max(SEA) + 26.0 + 16.0 * unit(hash(k, 3, map.seed as u32));
            let s = 0.6 + 0.6 * unit(hash(k, 4, map.seed as u32));
            statics.push(
                Item::new(
                    relics.islets[k as usize % 3],
                    m4::place([x, y, z], a, [s; 3]),
                )
                .rough(0.9)
                .detail(0.4),
            );
            if k % 2 == 0 {
                let m = m4::place([x, y + 0.4 * s, z], a, [s * 0.8; 3]);
                // Always far off: the coarser tree.
                statics.push(Item::new(trunks[0].1, m).rough(0.9));
                statics.push(Item::new(crowns[k as usize % 3].1, m).material(Material::Foliage));
            } else {
                // A crystal hanging point down from its underside.
                let c = mix(AMETHYST, AQUA, (k % 3) as f32 / 2.0);
                let (m, light) = relics::hanging([x, y, z], s);
                statics.push(Item::new(relics.crystals[0], m).tint(c, 1.0).rough(0.25));
                gems.push((light, c));
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
        held.extend(laid);
        for (a, b) in trunks
            .into_iter()
            .chain(crowns)
            .chain(rocks)
            .chain(pines)
            .chain([pine_trunk])
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
            pads: map.pads.iter().map(|&q| (q, slope(map, q))).collect(),
            crown: crown.0,
        }
    }
}
