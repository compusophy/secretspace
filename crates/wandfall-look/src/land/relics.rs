//! What the places and ruins are made of, sculpted (`render::sculpt`)
//! near and far as the flora is: the ruined rings' pillars of worn drums
//! and their capstones; the grove's giant mushrooms and crystals; the
//! circle's standing stones (runes cut in the face toward the altar) and
//! its altar (a ring of runes glowing in its top); the rift's obsidian,
//! faceted, embers glowing in its cracks, and its gate between two
//! horns; and the islets floating over it all (rock in strata under a
//! grassy lip, roots hanging, a crystal under some).

use std::f32::consts::TAU;

use render::geo::{self, hash, mix, rgb, scale, unit, Geo, V3};
use render::sculpt::{both, capsule, carve, cone, ellipsoid, mesh, noise, rbox, smin, torus};
use render::{Mesh, Renderer};
use std::cell::OnceCell;

use super::{crystal, join, lean, one, smooth, CYAN, EMBER};
use crate::flora::FAR;

/// The ruins' stone: a warm grey, grimed toward the ground, lichen where
/// it faces the sky.
pub(super) const STONE: V3 = rgb(150, 142, 128);
pub(super) const GRIME: V3 = rgb(70, 66, 58);
pub(super) const LICHEN: V3 = rgb(128, 134, 74);
const OBSIDIAN: V3 = rgb(24, 10, 14);

/// The heights the pillars are made at (the nearest is stretched a
/// little to fit); a drum's height; a capstone's height (a capped
/// pillar's shaft stops that far short of its top).
pub(super) const PILLARS: [f32; 4] = [2.0, 2.8, 3.8, 4.8];
const DRUM: f32 = 0.8;
pub(super) const CAP: f32 = 0.32;
/// The standing stones are made this tall.
pub(super) const MENHIR: f32 = 4.9;
/// The obsidian is made this wide at its foot and this tall.
pub(super) const SPIKE: (f32, f32) = (0.88, 5.0);

/// A mesh near and far.
pub(super) type NearFar = (Mesh, Mesh);

thread_local! {
    /// What is sculpted is the same on every island: sculpted once.
    static SCULPTED: OnceCell<Vec<Geo>> = const { OnceCell::new() };
}

/// Every sculpted relic, as `Relics::new` takes them: each pillar near
/// and far, the capstone and the standing stone near and far, each
/// obsidian near and far, the altar and the gate, the islets.
fn sculpted() -> Vec<Geo> {
    let mut v = Vec::new();
    for (k, &h) in PILLARS.iter().enumerate() {
        v.extend([pillar(h, k as u32 + 1, 1.0), pillar(h, k as u32 + 1, FAR)]);
    }
    v.extend([capstone(1.0), capstone(FAR), menhir(1.0), menhir(FAR)]);
    for s in [3u32, 11, 19] {
        v.extend([obsidian(s, 1.0), obsidian(s, FAR)]);
    }
    v.extend([altar(), gate()]);
    v.extend([2u32, 7, 13].map(islet));
    v
}

pub(super) struct Relics {
    pub pillars: [NearFar; 4],
    pub cap: NearFar,
    pub shrooms: [Mesh; 3],
    pub menhir: NearFar,
    pub altar: Mesh,
    pub spikes: [NearFar; 3],
    pub gate: Mesh,
    pub crystals: [Mesh; 2],
    pub islets: [Mesh; 3],
}

impl Relics {
    pub(super) fn new(r: &mut Renderer) -> Relics {
        // Uploaded in the order `sculpted` made them.
        let made = SCULPTED.with(|made| {
            let all: Vec<Mesh> = made
                .get_or_init(sculpted)
                .iter()
                .map(|g| r.mesh(g))
                .collect();
            all
        });
        let mut all = made.into_iter();
        let mut next = || all.next().expect("every relic sculpted");
        let mut pair = || (next(), next());
        let pillars = [pair(), pair(), pair(), pair()];
        let (cap, menhir) = (pair(), pair());
        let spikes = [pair(), pair(), pair()];
        let (altar, gate) = pair();
        let islets = [next(), next(), next()];
        let shrooms = [rgb(150, 80, 220), rgb(50, 170, 160), rgb(210, 70, 70)]
            .map(|c| smooth(r, |g| shroom(g, c)));
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
        Relics {
            pillars,
            cap,
            shrooms,
            menhir,
            altar,
            spikes,
            gate,
            crystals,
            islets,
        }
    }

    pub(super) fn meshes(&self) -> impl Iterator<Item = Mesh> + '_ {
        let pairs = self
            .pillars
            .iter()
            .chain(&self.spikes)
            .chain([&self.cap, &self.menhir])
            .flat_map(|&(a, b)| [a, b]);
        pairs
            .chain([self.altar, self.gate])
            .chain(self.shrooms)
            .chain(self.crystals)
            .chain(self.islets)
    }
}

/// A pillar's shaft and where its capstone sits (if it has one), `h`
/// tall: a capped one's shaft stops short by the capstone, so its top is
/// where feet stand.
pub(super) fn pillar_parts(h: f32, capped: bool) -> (f32, Option<f32>) {
    if capped {
        (h - CAP, Some(h - CAP))
    } else {
        (h, None)
    }
}

/// Stone painted: each piece (`piece`) a little lighter or darker,
/// grimed up from the ground (`y` above the foot, which is 0.3 under
/// it), lichen on what faces the sky and in patches.
fn weathered(p: V3, n: V3, piece: i32, seed: u32) -> V3 {
    let tint = 0.88 + 0.22 * unit(hash(piece, seed as i32, 31));
    let low = (1.0 - (p[1] - 0.3) / 1.4).clamp(0.0, 1.0);
    let c = mix(scale(STONE, tint), GRIME, 0.6 * low * low);
    let patch = ((noise(scale(p, 1.3)) - 0.25) / 0.3).clamp(0.0, 1.0);
    let up = ((n[1] - 0.45) / 0.4).clamp(0.0, 1.0);
    mix(c, LICHEN, (0.65 * up + 0.4 * patch).min(0.8))
}

/// A ruined pillar `h` tall: a fluted shaft of drums, a groove at each
/// joint, on a square plinth and a round moulding; its edges chipped,
/// its top broken flat.
fn pillar(h: f32, seed: u32, q: f32) -> Geo {
    let s = seed as f32 * 3.7;
    let f = move |p: V3| {
        let r = (p[0] * p[0] + p[2] * p[2]).sqrt();
        let a = p[2].atan2(p[0]);
        let t = (p[1] / h).clamp(0.0, 1.0);
        let j = p[1] / DRUM;
        let groove = 0.03 * (1.0 - ((j - j.round()).abs() * DRUM / 0.035).min(1.0));
        let shaft =
            r - (0.47 - 0.04 * t + 0.012 * (a * 10.0).cos()) + groove * (p[1] > 0.7) as i32 as f32;
        let plinth = rbox(p, [0.0, 0.2, 0.0], [0.6, 0.32, 0.6], 0.04);
        let moulding = torus(p, [0.0, 0.56, 0.0], 0.47, 0.07);
        let d = smin(shaft, plinth.min(moulding), 0.05);
        let chips = 0.045 * noise([p[0] * 2.3 + s, p[1] * 2.3, p[2] * 2.3]).max(0.0);
        both(d + chips + 0.01 * noise(scale(p, 8.0)), p[1] - h, 0.04)
    };
    mesh(
        &f,
        ([-0.75, -0.15, -0.75], [0.75, h + 0.1, 0.75]),
        0.065 * q,
        &|p, n| (weathered(p, n, (p[1] / DRUM) as i32, seed), 0.0),
        (0.6, 0.15),
    )
}

/// A capstone a little wider than the pillar, its edges chipped, its
/// top flat (`CAP` up).
fn capstone(q: f32) -> Geo {
    let f = |p: V3| {
        let d = rbox(p, [0.0, CAP / 2.0, 0.0], [0.675, CAP / 2.0, 0.675], 0.05);
        let chips = 0.04 * noise([p[0] * 3.1, p[1] * 3.1 + 5.0, p[2] * 3.1]).max(0.0);
        both(d + chips, p[1] - CAP, 0.02)
    };
    mesh(
        &f,
        ([-0.8, -0.1, -0.8], [0.8, CAP + 0.1, 0.8]),
        0.07 * q,
        &|p, n| (weathered([p[0], p[1] + 2.0, p[2]], n, 0, 5), 0.0),
        (0.5, 0.1),
    )
}

/// A giant mushroom's cap, its profile (out, up) from where it meets the
/// stem, round the rim, to its crown.
const RIM: [(f32, f32); 6] = [
    (0.06, 0.9),
    (0.42, 0.88),
    (0.47, 0.94),
    (0.4, 1.06),
    (0.22, 1.16),
    (0.0, 1.19),
];

/// The top of a mushroom's cap `rr` out from its middle.
fn cap_top(rr: f32) -> f32 {
    RIM[3..]
        .windows(2)
        .find(|w| rr <= w[0].0 && rr >= w[1].0)
        .map_or(RIM[5].1, |w| {
            let k = (w[0].0 - rr) / (w[0].0 - w[1].0);
            w[0].1 + (w[1].1 - w[0].1) * k
        })
}

/// A giant mushroom a metre tall (scaled to each): a pale stem, a cap of
/// colour `c` that glows a little, cream spots on it.
fn shroom(g: &mut Geo, c: V3) {
    let cream = rgb(226, 214, 190);
    g.lathe(
        [0.0; 3],
        &[(0.09, 0.0), (0.075, 0.4), (0.06, 0.8), (0.08, 0.95)],
        10,
        cream,
        0.0,
    );
    g.lathe([0.0; 3], &RIM, 16, c, 0.35);
    for k in 0..7 {
        let a = k as f32 * 2.4;
        let rr = 0.18 + 0.12 * (k % 2) as f32;
        g.sphere(
            [a.cos() * rr, cap_top(rr) + 0.006, a.sin() * rr],
            [0.05, 0.022, 0.05],
            (1, k, 0.0),
            cream,
            0.2,
        );
    }
}

/// Where the surface of `f` is along -x from the middle, at (y, z).
fn face(f: &impl Fn(V3) -> f32, y: f32, z: f32) -> f32 {
    let (mut out, mut inside) = (-1.5f32, 0.0f32);
    for _ in 0..24 {
        let mid = (out + inside) / 2.0;
        if f([mid, y, z]) > 0.0 {
            out = mid;
        } else {
            inside = mid;
        }
    }
    out
}

/// A standing stone `MENHIR` tall: a weathered slab, a little narrower
/// at its top, which is worn flat enough to stand on; streaked by rain,
/// lichen on top; runes cut glowing in its face toward -x (inward once
/// placed).
fn menhir(q: f32) -> Geo {
    let h = MENHIR;
    let f = move |p: V3| {
        let taper = 1.0 - 0.16 * (p[1] / h).clamp(0.0, 1.0);
        let at = [p[0] / taper, p[1] - h / 2.0, p[2] / taper];
        let d = rbox(at, [0.0; 3], [0.5, h / 2.0, 0.84], 0.14) * taper;
        let d = d + 0.07 * noise(scale(p, 0.9)) + 0.02 * noise(scale(p, 3.5));
        both(d, p[1] - h, 0.08)
    };
    let mut g = mesh(
        &f,
        ([-0.8, -0.1, -1.1], [0.8, h + 0.1, 1.1]),
        0.09 * q,
        &|p, n| {
            let streak = 0.5 + 0.5 * noise([p[0] * 3.0, p[1] * 0.25, p[2] * 3.0]);
            let c = mix(rgb(118, 116, 110), rgb(84, 82, 80), 0.5 * streak);
            let low = (1.0 - (p[1] - 0.3) / 1.2).clamp(0.0, 1.0);
            let up = ((n[1] - 0.5) / 0.35).clamp(0.0, 1.0);
            (mix(mix(c, GRIME, 0.5 * low), LICHEN, 0.7 * up), 0.0)
        },
        (0.5, 0.2),
    );
    // Six lines of glyphs, each stroke laid on the face.
    for k in 0..6 {
        let y = 1.0 + k as f32 * 0.55;
        for s in 0..3 {
            let u = |i| unit(hash(k, s * 4 + i, 77)) - 0.5;
            let (a, b) = ([y + u(0) * 0.18, u(1) * 0.6], [y + u(2) * 0.18, u(3) * 0.6]);
            let d = [b[0] - a[0], b[1] - a[1]];
            let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-4);
            let side = [-d[1] / l * 0.013, d[0] / l * 0.013];
            let at = |t: f32, w: f32| {
                let (yy, zz) = (a[0] + d[0] * t + side[0] * w, a[1] + d[1] * t + side[1] * w);
                [face(&f, yy, zz) - 0.012, yy, zz]
            };
            for n in 0..4 {
                let (t0, t1) = (n as f32 / 4.0, (n + 1) as f32 / 4.0);
                g.quad(
                    at(t0, -1.0),
                    at(t0, 1.0),
                    at(t1, 1.0),
                    at(t1, -1.0),
                    CYAN,
                    2.0,
                );
            }
        }
    }
    g
}

/// The circle's altar: a plinth, a slab over it with a ring of runes
/// cut glowing in its top.
fn altar() -> Geo {
    let ring = |p: V3| torus(p, [0.0, 1.12, 0.0], 0.42, 0.045);
    let f = move |p: V3| {
        let plinth = rbox(p, [0.0, 0.45, 0.0], [0.85, 0.45, 0.55], 0.06);
        let slab = rbox(p, [0.0, 1.01, 0.0], [1.05, 0.11, 0.75], 0.05);
        let d = smin(plinth, slab, 0.05) + 0.01 * noise(scale(p, 5.0));
        carve(d, ring(p), 0.01)
    };
    mesh(
        &f,
        ([-1.2, -0.1, -0.9], [1.2, 1.25, 0.9]),
        0.05,
        &|p, n| {
            if ring(p) < 0.03 {
                return (CYAN, 1.8);
            }
            let top = p[1] > 0.88;
            let c = if top {
                rgb(130, 126, 134)
            } else {
                rgb(96, 92, 100)
            };
            (
                mix(c, LICHEN, 0.4 * ((n[1] - 0.6) / 0.3).clamp(0.0, 1.0)),
                0.0,
            )
        },
        (0.55, 0.12),
    )
}

/// Obsidian thrust up, `SPIKE` wide and tall: a leaning blade cut in
/// glassy facets, two shards at its foot, embers glowing in the cracks
/// near the ground.
fn obsidian(seed: u32, q: f32) -> Geo {
    let u = |k: i32| unit(hash(seed as i32, k, 0x0b5));
    let (r0, h) = SPIKE;
    let tip = [0.4 * (u(1) - 0.5), h, 0.4 * (u(2) - 0.5)];
    let cuts: Vec<(V3, f32)> = (0..8)
        .map(|k| {
            let a = (k as f32 + 0.6 * u(10 + k)) / 8.0 * TAU;
            let n = geo::norm([a.cos(), r0 / h * (0.7 + 0.6 * u(20 + k)), a.sin()]);
            (n, r0 * n[0].hypot(n[2]) * (0.78 + 0.14 * u(30 + k)))
        })
        .collect();
    let shards: Vec<(V3, V3, f32)> = (0..2)
        .map(|k| {
            let a = (k as f32 * 0.45 + u(40 + k)) * TAU;
            let (s, c) = a.sin_cos();
            (
                [c * 0.55, -0.2, s * 0.55],
                [c * 1.15, 0.7 + 0.5 * u(50 + k), s * 1.15],
                0.22 + 0.1 * u(60 + k),
            )
        })
        .collect();
    let f = move |p: V3| {
        let mut d = cone(p, [0.0, -0.3, 0.0], tip, r0, 0.02);
        for (n, off) in &cuts {
            d = both(d, geo::dot(*n, p) - off, 0.01);
        }
        for &(a, b, r) in &shards {
            d = smin(d, cone(p, a, b, r, 0.01), 0.05);
        }
        d + 0.006 * noise(scale(p, 9.0))
    };
    mesh(
        &f,
        ([-1.5, -0.35, -1.5], [1.5, h + 0.2, 1.5]),
        0.075 * q,
        &|p, _| {
            let w = noise([
                p[0] * 1.8 + 0.6 * noise(scale(p, 0.9)),
                p[1] * 0.9,
                p[2] * 1.8 + seed as f32,
            ]);
            let crack = (1.0 - w.abs() / 0.08).max(0.0) * (1.0 - p[1] / 2.2).max(0.0);
            (mix(OBSIDIAN, EMBER, crack), 1.8 * crack)
        },
        (0.4, 0.1),
    )
}

/// The rift's gate: a slab of obsidian, a horn curving up either side,
/// and the ring between them that the gate opens in (its middle aglow).
fn gate() -> Geo {
    let horn = |p: V3, s: f32| {
        let at = |y: f32, z: f32| [0.0, y, z * s];
        let a = cone(p, at(0.3, 3.1), at(3.4, 3.75), 0.7, 0.45);
        let b = cone(p, at(3.4, 3.75), at(6.1, 3.5), 0.45, 0.22);
        let c = cone(p, at(6.1, 3.5), at(7.9, 2.8), 0.22, 0.03);
        smin(smin(a, b, 0.2), c, 0.15)
    };
    let f = move |p: V3| {
        let slab = rbox(p, [0.0, 0.3, 0.0], [1.1, 0.3, 3.8], 0.08);
        let d = smin(slab, horn(p, 1.0).min(horn(p, -1.0)), 0.3);
        d + 0.02 * noise(scale(p, 2.5))
    };
    let mut g = mesh(
        &f,
        ([-1.3, -0.1, -4.8], [1.3, 8.2, 4.8]),
        0.14,
        &|p, n| {
            let sheen = 0.5 + 0.5 * n[1].max(0.0);
            (
                mix(rgb(40, 30, 40), OBSIDIAN, (p[1] / 0.8).min(1.0) * sheen),
                0.0,
            )
        },
        (0.4, 0.2),
    );
    // The ring the gate opens in.
    let mut ring = Geo::default();
    let (n, m, big, tube) = (32, 6, 2.7, 0.28);
    let at = |i: usize, j: usize| {
        let (a, b) = (i as f32 / n as f32 * TAU, j as f32 / m as f32 * TAU);
        let rr = big + tube * b.cos();
        [tube * b.sin(), 3.6 + rr * a.sin(), rr * a.cos()]
    };
    for i in 0..n {
        for j in 0..m {
            let glow = if j == m / 2 { 1.4 } else { 0.0 };
            let c = if glow > 0.0 { EMBER } else { OBSIDIAN };
            ring.quad(
                at(i, j),
                at(i + 1, j),
                at(i + 1, j + 1),
                at(i, j + 1),
                c,
                glow,
            );
        }
    }
    join(&mut g, ring);
    g
}

/// The deepest point of an islet (where a crystal hangs from), as made.
pub(super) const ISLET_TIP: V3 = [0.0, -6.6, 0.0];

/// A crystal hanging point down from the tip of an islet at `at`, `s`
/// times its size (turned over, not mirrored, so it is not drawn inside
/// out), and where its light is.
pub(super) fn hanging(at: V3, s: f32) -> (render::M4, V3) {
    let tip = at[1] + ISLET_TIP[1] * s;
    let m = render::m4::basis(
        [at[0], tip + 0.3 * s, at[2]],
        [s * 1.4, 0.0, 0.0],
        [0.0, -3.0 * s, 0.0],
        [0.0, 0.0, -s * 1.4],
    );
    (m, [at[0], tip - 1.6 * s, at[2]])
}

/// An islet floating over the island, about 9 across: a grassy top with
/// a lip that overhangs, under it earth, then rock in strata narrowing to
/// a point (`ISLET_TIP`), lumps hanging off it, roots hanging from under
/// the lip.
fn islet(seed: u32) -> Geo {
    let u = |k: i32| unit(hash(seed as i32, k, 0x151e7));
    let lumps: Vec<(V3, V3)> = (0..4)
        .map(|k| {
            let a = (k as f32 + u(k)) / 4.0 * TAU;
            let rr = 1.0 + 0.8 * u(10 + k);
            let y = -1.8 - 2.0 * u(20 + k);
            let s = 0.8 + 0.5 * u(30 + k);
            ([a.cos() * rr, y, a.sin() * rr], [s, s * 1.4, s])
        })
        .collect();
    let roots: Vec<(V3, V3)> = (0..9)
        .map(|k| {
            let a = (k as f32 + 0.7 * u(40 + k)) / 9.0 * TAU;
            let rr = 3.0 + 1.1 * u(50 + k);
            let top = [a.cos() * rr, -0.5, a.sin() * rr];
            let len = 1.2 + 2.0 * u(60 + k);
            (top, [top[0] * 0.92, -0.5 - len, top[2] * 0.92])
        })
        .collect();
    let grass = |p: V3| ellipsoid(p, [0.0, 0.2, 0.0], [4.8, 0.45, 4.6]);
    let hanging = roots.clone();
    let f = move |p: V3| {
        let top = ellipsoid(p, [0.0, -0.2, 0.0], [4.5, 1.0, 4.3]);
        let body = cone(p, [0.0, -0.9, 0.0], ISLET_TIP, 3.0, 0.3);
        let mut d = smin(top, body, 0.6);
        for &(c, r) in &lumps {
            d = smin(d, ellipsoid(p, c, r), 0.5);
        }
        // Rock in strata: ledges round it every so often.
        let strata = 0.1 * (p[1] * 3.2 + 0.6 * noise(scale(p, 0.5))).sin();
        let d = d
            + 0.25 * noise(scale(p, 0.5))
            + 0.08 * noise(scale(p, 1.6))
            + strata * (p[1] < -0.9) as i32 as f32;
        // Flat on top, the grass's lip over it.
        let d = both(d, p[1] - 0.35 - 0.1 * noise(scale(p, 0.8)), 0.25);
        let mut d = smin(d, both(grass(p), p[1] - 0.55, 0.1), 0.15);
        for &(a, b) in &hanging {
            d = smin(d, cone(p, a, b, 0.22, 0.08), 0.12);
        }
        d
    };
    mesh(
        &f,
        ([-5.6, -7.2, -5.6], [5.6, 1.0, 5.6]),
        0.28,
        &move |p, n| {
            let c = if p[1] > 0.1 && n[1] > 0.25 {
                mix(
                    rgb(62, 104, 44),
                    rgb(96, 132, 58),
                    0.5 + 0.5 * noise(scale(p, 0.7)),
                )
            } else if p[1] > -0.9 {
                mix(
                    rgb(82, 62, 46),
                    rgb(66, 50, 38),
                    0.5 + 0.3 * noise(scale(p, 1.2)),
                )
            } else {
                let band = 0.5 + 0.5 * (p[1] * 3.2 + 0.6 * noise(scale(p, 0.5))).sin();
                let rock = mix(rgb(98, 92, 86), rgb(76, 70, 68), band);
                mix(rock, rgb(44, 40, 42), ((-p[1] - 3.0) / 3.0).clamp(0.0, 1.0))
            };
            // The roots, dark.
            let root = roots
                .iter()
                .map(|&(a, b)| capsule(p, a, b, 0.0))
                .fold(f32::MAX, f32::min);
            (
                if root < 0.25 && p[1] < -0.4 {
                    rgb(58, 42, 30)
                } else {
                    c
                },
                0.0,
            )
        },
        (0.6, 0.8),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_capped_pillar_stands_flush_with_its_top() {
        // Feet stand on a pillar at its height: the capstone's top there.
        for h in [2.06f32, 3.4, 5.28] {
            let (shaft, cap) = pillar_parts(h, true);
            let cap = cap.expect("capped");
            assert!(
                (cap + CAP - h).abs() < 1e-5,
                "{h}: the cap's top at {}",
                cap + CAP
            );
            assert!((shaft - cap).abs() < 1e-5, "the cap sits on the shaft");
            assert_eq!(pillar_parts(h, false), (h, None));
        }
    }

    #[test]
    fn the_relics_stand_where_feet_and_bolts_meet_them() {
        // The standing stone's top is flat enough to stand on (within a
        // few centimetres of its height across the middle of it).
        let g = menhir(1.0);
        let st = geo::STRIDE;
        let tops: Vec<f32> =
            g.v.chunks(st)
                .filter(|v| v[0].abs() < 0.3 && v[2].abs() < 0.5 && v[1] > MENHIR - 0.5)
                .map(|v| v[1])
                .collect();
        assert!(!tops.is_empty());
        let low = tops.iter().fold(f32::MAX, |a, &b| a.min(b));
        assert!(low > MENHIR - 0.12, "the top dips to {low}");
        // A pillar's top is at its height; its shaft as wide as it blocks.
        for (k, &h) in PILLARS.iter().enumerate() {
            let g = pillar(h, k as u32, 1.0);
            let top = g.v.chunks(st).map(|v| v[1]).fold(f32::MIN, f32::max);
            assert!((top - h).abs() < 0.03, "{h}: its top at {top}");
            let wide =
                g.v.chunks(st)
                    .filter(|v| v[1] > 1.2 && v[1] < h - 0.3)
                    .map(|v| v[0].hypot(v[2]))
                    .fold(0.0f32, f32::max);
            assert!(wide < 0.55 && wide > 0.4, "{h}: {wide} across");
        }
        // The obsidian's embers show: some of its surface glows.
        let g = obsidian(3, 1.0);
        let lit = g.v.chunks(st).filter(|v| v[9] > 0.3).count();
        assert!(lit * 50 > g.len() / 10, "{lit} of {} glow", g.len());
    }

    #[test]
    fn a_crystal_hangs_from_an_islet_not_inside_it() {
        // The islet comes to its tip; the crystal hangs below it, wound
        // the right way out (its turn not a mirror).
        let g = islet(7);
        let low =
            g.v.chunks(geo::STRIDE)
                .map(|v| v[1])
                .fold(f32::MAX, f32::min);
        assert!((low - ISLET_TIP[1]).abs() < 0.5, "its tip at {low}");
        let (m, light) = hanging([0.0, 40.0, 0.0], 0.8);
        let det = m[0] * (m[5] * m[10] - m[6] * m[9]) - m[4] * (m[1] * m[10] - m[2] * m[9])
            + m[8] * (m[1] * m[6] - m[2] * m[5]);
        assert!(det > 0.0, "mirrored: {det}");
        let base = m[13];
        let point = m[5] + m[13];
        assert!(
            point < base && base < 40.0 + low * 0.8 + 1.0,
            "{base} to {point}"
        );
        assert!(
            light[1] < 40.0 + low * 0.8,
            "its light under the earth: {light:?}"
        );
    }

    #[test]
    fn a_mushroom_wears_its_spots_on_its_cap() {
        // Most of each spot stands proud of the cap, not inside it.
        let mut g = Geo::default();
        shroom(&mut g, [1.0; 3]);
        let cream = rgb(226, 214, 190)[0];
        let spots: Vec<&[f32]> =
            g.v.chunks(geo::STRIDE)
                .filter(|v| (v[6] - cream).abs() < 1e-4 && v[1] > 1.0)
                .collect();
        let out = spots
            .iter()
            .filter(|v| v[1] > cap_top(v[0].hypot(v[2])))
            .count();
        assert!(
            !spots.is_empty() && out * 2 > spots.len(),
            "{out} of {} show",
            spots.len()
        );
    }
}
