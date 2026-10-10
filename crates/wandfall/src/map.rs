//! The island, from its seed alone, the same on the server and the page:
//! rolling hills falling to the sea at the shore, its places (`places`:
//! the Spire at the centre, a stone circle, a demon rift, a crystal
//! grove, a basalt causeway), and between them woods and open meadows
//! (trees, giant mushrooms under them), rocks, and ruined rings of
//! pillars. Heights use arithmetic only (no library calls), so both ends
//! agree to the bit.

use engine::rng::{splitmix, Rng};

use crate::laws::*;
use crate::places::{self, Deck, Poi};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Tree,
    Rock,
    Pillar,
    Shroom,
    /// The Spire's tower, the lamps about its plaza, the merlons along
    /// its balcony.
    Tower,
    Lamp,
    Merlon,
    /// The stone circle's stones and altar.
    Stone,
    Altar,
    /// The rift's gate and its obsidian.
    Portal,
    Spike,
    Crystal,
    /// The causeway's basalt and the sea stacks about it.
    Column,
}

impl Kind {
    /// Whether wizards can stand on top of it (and climb onto it).
    pub fn standable(self) -> bool {
        matches!(
            self,
            Kind::Rock
                | Kind::Pillar
                | Kind::Stone
                | Kind::Altar
                | Kind::Merlon
                | Kind::Column
                | Kind::Shroom
        )
    }
}

/// One thing standing on the island. Its trunk, body or shaft blocks
/// wizards and bolts: a cylinder `r` wide and `h` tall from `y`. A giant
/// mushroom's cap, wider than its stem, holds up wizards come down onto
/// it (from below they pass through it) and stops bolts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prop {
    pub kind: Kind,
    pub x: f32,
    pub z: f32,
    pub y: f32,
    pub r: f32,
    pub h: f32,
    /// For the look only: turned, and how big.
    pub yaw: f32,
    pub scale: f32,
}

impl Prop {
    /// Where feet stand on its top (at its highest), if they can: a
    /// boulder's rounded top stands a little over its trunk, a mushroom's
    /// cap over its stem.
    pub fn top(&self) -> Option<f32> {
        let h = match self.kind {
            Kind::Rock => self.h * ROCK_TOP,
            Kind::Shroom => self.h * SHROOM_DOME[0].1,
            _ => self.h,
        };
        self.kind.standable().then_some(self.y + h)
    }

    /// Where feet stand on its top `d` out from its middle, if they can
    /// there: on a mushroom's cap, lower toward its rim.
    pub fn top_at(&self, d: f32) -> Option<f32> {
        let t = self.top().filter(|_| d < self.stand_r())?;
        Some(if self.kind == Kind::Shroom {
            self.y + self.h * dome(d / self.h)
        } else {
            t
        })
    }

    /// How far out from its middle it spreads, as drawn: a tree's crown,
    /// a mushroom's cap to its rim, else its trunk or body.
    pub fn spread(&self) -> f32 {
        match self.kind {
            Kind::Tree => CROWN * self.scale,
            Kind::Shroom => self.h * RIM,
            _ => self.r,
        }
    }

    /// How far out from its middle its top holds you up.
    pub fn stand_r(&self) -> f32 {
        if self.kind == Kind::Shroom {
            self.h * SHROOM_CAP
        } else {
            self.r
        }
    }

    /// How high its trunk, body or shaft stands in the way: a mushroom's
    /// stem up to its cap.
    pub fn solid_top(&self) -> f32 {
        if self.kind == Kind::Shroom {
            self.y + self.h * SHROOM_UNDER
        } else {
            self.top().unwrap_or(self.y + self.h).max(self.y + self.h)
        }
    }

    /// Its cap, if it has one, as rings stacked on its underside, each as
    /// high as the dome is at its edge (so a bolt is stopped close under
    /// the dome): how wide, from what height to what.
    pub fn cap(&self) -> Option<impl Iterator<Item = (f32, f32, f32)> + '_> {
        (self.kind == Kind::Shroom).then(|| {
            let under = self.y + self.h * SHROOM_UNDER;
            RINGS.iter().map(move |k| {
                let r = self.stand_r() * k;
                (r, under, self.y + self.h * dome(r / self.h))
            })
        })
    }
}

/// A mushroom cap's rings, out from its middle (shares of how far its top
/// holds you up): closer near the rim, where the dome falls fastest.
const RINGS: [f32; 6] = [1.0, 0.95, 0.85, 0.7, 0.5, 0.25];

/// How far a mushroom's cap spreads to its rim, for every metre it
/// stands.
const RIM: f32 = SHROOM_DOME[SHROOM_DOME.len() - 1].0;

/// How high a mushroom's dome is `r` out from its middle (both for every
/// metre the mushroom stands), between the points of `SHROOM_DOME`.
fn dome(r: f32) -> f32 {
    let mut was = SHROOM_DOME[0];
    for &(x, y) in &SHROOM_DOME[1..] {
        if r <= x {
            return was.1 + (y - was.1) * (r - was.0) / (x - was.0);
        }
        was = (x, y);
    }
    was.1
}

/// Metres a cell of the props' grid.
const CELL: f32 = 8.0;
const CELLS: i32 = (MAP_HALF * 2.0 / CELL) as i32;

pub struct Map {
    pub seed: u64,
    pub props: Vec<Prop>,
    /// The places, the Spire first; where their cubes wait.
    pub pois: Vec<Poi>,
    pub caches: Vec<[f32; 2]>,
    /// Where to stand above the ground (the Spire's stair and balcony).
    pub decks: Vec<Deck>,
    /// Launch runes: step on one and it throws you up onto your broom
    /// (each its middle, on the ground).
    pub pads: Vec<[f32; 3]>,
    grid: Vec<Vec<u16>>,
    /// How much further than its trunk any prop's top reaches (a cap's).
    reach: f32,
}

pub fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 0..1 from 64 random bits (their top 24).
fn unit_of(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// 0..1, the next from `rng`.
pub fn unit(rng: &mut Rng) -> f32 {
    unit_of(rng.next_u64())
}

/// 0..1 from a lattice point.
fn lattice(seed: u64, x: i32, z: i32) -> f32 {
    unit_of(splitmix(seed ^ ((x as u32 as u64) << 32 | z as u32 as u64)))
}

/// Smooth value noise, 0..1.
fn noise(seed: u64, x: f32, z: f32) -> f32 {
    let (fx, fz) = (x.floor(), z.floor());
    let (ix, iz) = (fx as i32, fz as i32);
    let (tx, tz) = (smooth(x - fx), smooth(z - fz));
    let a = lattice(seed, ix, iz);
    let b = lattice(seed, ix + 1, iz);
    let c = lattice(seed, ix, iz + 1);
    let d = lattice(seed, ix + 1, iz + 1);
    let top = a + (b - a) * tx;
    let bottom = c + (d - c) * tx;
    top + (bottom - top) * tz
}

/// The bare hills at (x, z), falling to the sea at the shore.
fn hills(s: u64, x: f32, z: f32) -> f32 {
    let k = 1.0 / HILL_SIZE;
    let n = noise(s, x * k, z * k)
        + 0.45 * noise(s ^ 1, x * k * 2.3, z * k * 2.3)
        + 0.12 * noise(s ^ 2, x * k * 6.1, z * k * 6.1);
    let hills = 1.5 + (n - 0.55) * HILLS;
    let r = (x * x + z * z).sqrt();
    let shore = smooth((r - (SHORE - 30.0)) / 30.0);
    hills * (1.0 - shore) + (SEA - 4.0) * shore
}

impl Map {
    pub fn new(seed: u64) -> Map {
        let mut m = Map {
            seed,
            props: Vec::new(),
            pois: places::find(seed, |x, z| hills(seed, x, z)),
            caches: Vec::new(),
            decks: Vec::new(),
            pads: Vec::new(),
            grid: vec![Vec::new(); (CELLS * CELLS) as usize],
            reach: 0.0,
        };
        places::set(&mut m);
        m.scatter();
        m.launchers();
        m
    }

    /// The launch runes: one by each place, the rest out in the wild, far
    /// apart; from a stream of their own, so nothing else moves.
    fn launchers(&mut self) {
        let mut rng = Rng::new(self.seed ^ 0x1a0c_4e55);
        let fits = |m: &Map, x: f32, z: f32| {
            m.land(x, z)
                && m.height(x, z) > SEA + 1.0
                && m.near(x, z, PAD_R + 1.5).next().is_none()
                && m.pads
                    .iter()
                    .all(|q| (q[0] - x).powi(2) + (q[2] - z).powi(2) > PAD_APART * PAD_APART)
        };
        for k in 0..self.pois.len() {
            let p = self.pois[k];
            for _ in 0..16 {
                let (s, c) = crate::trig::sin_cos((unit(&mut rng) * 65536.0) as u16);
                let (x, z) = (p.x + c * p.r * 1.25, p.z + s * p.r * 1.25);
                if fits(self, x, z) {
                    self.pads.push([x, self.height(x, z), z]);
                    break;
                }
            }
        }
        let (mut n, mut tries) = (0, 0);
        while n < PADS_WILD && tries < 600 {
            tries += 1;
            let (x, z) = (
                (unit(&mut rng) * 2.0 - 1.0) * SHORE * 0.8,
                (unit(&mut rng) * 2.0 - 1.0) * SHORE * 0.8,
            );
            if fits(self, x, z) && self.wild(x, z, 6.0) {
                self.pads.push([x, self.height(x, z), z]);
                n += 1;
            }
        }
    }

    /// The launch rune feet at `p` stand on, if any.
    pub fn pad_under(&self, p: [f32; 3]) -> Option<[f32; 3]> {
        self.pads.iter().copied().find(|q| {
            (q[0] - p[0]).powi(2) + (q[2] - p[2]).powi(2) < PAD_R * PAD_R
                && (p[1] - q[1]).abs() < 1.0
        })
    }

    /// The ground's height at (x, z): the hills, as the places shape them.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        let mut h = hills(self.seed, x, z);
        for p in &self.pois {
            h = p.shape(h, x, z);
        }
        h
    }

    /// What feet at `y` stand on at (x, z): the highest of the ground, any
    /// deck and the top of anything standing there that can be stood on,
    /// no more than a step above them.
    pub fn floor(&self, x: f32, z: f32, y: f32) -> f32 {
        let mut f = self.height(x, z);
        for d in &self.decks {
            if let Some(h) = d.level(x, z, y + STEP) {
                f = f.max(h);
            }
        }
        for q in self.near(x, z, self.reach) {
            let (dx, dz) = (q.x - x, q.z - z);
            let d = (dx * dx + dz * dz).sqrt();
            if let Some(t) = q.top_at(d).filter(|&t| t <= y + STEP) {
                f = f.max(t);
            }
        }
        f
    }

    /// A ledge to climb onto from feet at `p`, pushing along (`wx`, `wz`):
    /// the top of something that can be stood on, close ahead and within
    /// reach above the feet (not a cap over the head); its top, its
    /// middle, and how far its edge is from the body's side.
    pub fn ledge(&self, p: [f32; 3], (wx, wz): (f32, f32)) -> Option<(f32, [f32; 2], f32)> {
        self.near(p[0], p[2], RADIUS + MANTLE_NEAR + self.reach)
            .filter_map(|q| {
                let t = q.top()?;
                let (dx, dz) = (q.x - p[0], q.z - p[2]);
                let d = (dx * dx + dz * dz).sqrt().max(1e-4);
                let over = q.cap().is_some() && d < q.stand_r();
                let gap = d - q.stand_r() - RADIUS;
                if over || gap >= MANTLE_NEAR {
                    return None;
                }
                let ahead = (dx * wx + dz * wz) / d;
                (t > p[1] + MANTLE_LOW && t <= p[1] + MANTLE_REACH && ahead >= MANTLE_AHEAD)
                    .then_some((t, [q.x, q.z], gap))
            })
            .next()
    }

    /// Whether a deck's surface lies between heights `lo` and `hi` at (x, z).
    fn deck_between(&self, x: f32, z: f32, lo: f32, hi: f32) -> bool {
        self.decks
            .iter()
            .any(|d| d.level(x, z, hi).is_some_and(|h| h >= lo))
    }

    /// Whether (x, z) is clear of the places, `pad` metres to spare.
    pub fn wild(&self, x: f32, z: f32, pad: f32) -> bool {
        self.pois.iter().all(|p| p.dist(x, z) > p.r * 1.15 + pad)
    }

    /// Set a prop down, in the grid of every cell it touches.
    pub fn put(&mut self, p: Prop) {
        let i = self.props.len() as u16;
        self.props.push(p);
        self.reach = self.reach.max(p.stand_r() - p.r);
        let c = |v: f32| (((v + MAP_HALF) / CELL).floor() as i32).clamp(0, CELLS - 1);
        for cz in c(p.z - p.r)..=c(p.z + p.r) {
            for cx in c(p.x - p.r)..=c(p.x + p.r) {
                self.grid[(cz * CELLS + cx) as usize].push(i);
            }
        }
    }

    /// How wooded (x, z) is, 0..1: woods where it is high, meadows low.
    pub fn wood(&self, x: f32, z: f32) -> f32 {
        let (x, z) = (x / WOODS, z / WOODS);
        noise(self.seed ^ 0x3d, x, z) * 0.7 + noise(self.seed ^ 0x3e, x * 2.3, z * 2.3) * 0.3
    }

    /// How far something at (x, z) spreading `spread` out (a crown, a
    /// cap) is from the spread of the nearest of the things `of` picks
    /// (m; under nought, into it).
    fn gap(&self, x: f32, z: f32, spread: f32, of: impl Fn(&Prop) -> bool) -> f32 {
        // Nothing spreads further than the biggest tree's crown (its
        // scale 1.4); and looking for a wood, this much further.
        self.near(x, z, spread + CROWN * 1.4 + SHROOM_WOOD)
            .filter(|q| of(q))
            .map(|q| {
                let (dx, dz) = (q.x - x, q.z - z);
                (dx * dx + dz * dz).sqrt() - spread - q.spread()
            })
            .fold(f32::MAX, f32::min)
    }

    /// Whether a point is on land (not the sea, not past the edge).
    pub fn land(&self, x: f32, z: f32) -> bool {
        x.abs() < MAP_HALF && z.abs() < MAP_HALF && self.height(x, z) > SEA + 0.4
    }

    fn scatter(&mut self) {
        let mut rng = Rng::new(self.seed ^ 0x51ab);
        // Ruins first: rings of pillars, the island's landmarks.
        let (mut ruins, mut tries) = (0, 0);
        while ruins < RUINS && tries < RUINS * 200 {
            tries += 1;
            let (x, z) = (
                (unit(&mut rng) * 2.0 - 1.0) * SHORE * 0.8,
                (unit(&mut rng) * 2.0 - 1.0) * SHORE * 0.8,
            );
            if !self.land(x, z) || !self.wild(x, z, 10.0) {
                continue;
            }
            ruins += 1;
            let ring = 5.0 + unit(&mut rng) * 4.0;
            let n = 6 + (unit(&mut rng) * 5.0) as usize;
            for k in 0..n {
                // Some pillars have fallen.
                if unit(&mut rng) < 0.25 {
                    continue;
                }
                let turn = (k * 65536 / n) as u16;
                let (sa, ca) = crate::trig::sin_cos(turn);
                let a = crate::trig::radians(turn);
                let (px, pz) = (x + ca * ring, z + sa * ring);
                if !self.land(px, pz) {
                    continue;
                }
                let h = 2.0 + unit(&mut rng) * 3.5;
                let y = self.height(px, pz) - 0.3;
                self.put(Prop {
                    kind: Kind::Pillar,
                    x: px,
                    z: pz,
                    y,
                    r: 0.55,
                    h,
                    yaw: a,
                    scale: h,
                });
            }
        }
        let place = |m: &mut Map, kind: Kind, count: usize, rng: &mut Rng| {
            let mut n = 0;
            let mut tries = 0;
            while n < count && tries < count * 40 {
                tries += 1;
                let (x, z) = (
                    (unit(rng) * 2.0 - 1.0) * SHORE,
                    (unit(rng) * 2.0 - 1.0) * SHORE,
                );
                if !m.land(x, z) || m.height(x, z) < SEA + 1.0 || !m.wild(x, z, 3.0) {
                    continue;
                }
                // Trees in the woods (thinning at their edges), now and
                // then one alone in a meadow.
                if kind == Kind::Tree {
                    let thick = smooth((m.wood(x, z) - WOOD_EDGE) / WOOD_SOFT);
                    if unit(rng) >= thick.max(WOOD_LONE) {
                        continue;
                    }
                }
                let s = 0.8 + unit(rng) * 0.6;
                let (r, h) = match kind {
                    Kind::Rock => (1.1 * s, 1.3 * s),
                    Kind::Shroom => (0.4 * s, 3.5 * s),
                    _ => (0.35 * s, 6.0 * s),
                };
                // Not inside another. A tree's crown not over a ruin's
                // pillar or a mushroom's cap (the grove's): their tops
                // stand up into it. A mushroom by a wood under open sky,
                // its cap clear of every crown and of all else.
                let fits = match kind {
                    Kind::Tree => {
                        let tall = |q: &Prop| matches!(q.kind, Kind::Pillar | Kind::Shroom);
                        m.gap(x, z, CROWN * s, tall) >= 0.0
                    }
                    Kind::Shroom => {
                        let cap = h * RIM;
                        m.gap(x, z, cap, |_| true) >= 0.0
                            && m.gap(x, z, cap, |q| q.kind == Kind::Tree) < SHROOM_WOOD
                    }
                    _ => true,
                };
                if !fits || m.near(x, z, r + 1.2).next().is_some() {
                    continue;
                }
                let y = m.height(x, z) - 0.2;
                let yaw = unit(rng) * std::f32::consts::TAU;
                m.put(Prop {
                    kind,
                    x,
                    z,
                    y,
                    r,
                    h,
                    yaw,
                    scale: s,
                });
                n += 1;
            }
        };
        place(self, Kind::Tree, TREES, &mut rng);
        place(self, Kind::Rock, ROCKS, &mut rng);
        place(self, Kind::Shroom, SHROOMS, &mut rng);
    }

    /// Props whose cylinder comes within `d` of (x, z) on the ground.
    pub fn near(&self, x: f32, z: f32, d: f32) -> impl Iterator<Item = &Prop> + '_ {
        let c = |v: f32| (((v + MAP_HALF) / CELL).floor() as i32).clamp(0, CELLS - 1);
        let (x0, x1, z0, z1) = (c(x - d), c(x + d), c(z - d), c(z + d));
        let mut seen: Vec<u16> = Vec::new();
        for cz in z0..=z1 {
            for cx in x0..=x1 {
                for &i in &self.grid[(cz * CELLS + cx) as usize] {
                    if !seen.contains(&i) {
                        seen.push(i);
                    }
                }
            }
        }
        seen.sort_unstable();
        seen.into_iter()
            .map(move |i| &self.props[i as usize])
            .filter(move |p| {
                let (dx, dz) = (p.x - x, p.z - z);
                let reach = p.r + d;
                dx * dx + dz * dz < reach * reach
            })
    }

    /// Move a wizard at `p` (feet), `tall` metres tall, out of anything
    /// standing there; not out of a top it can step up onto (`rise`
    /// above its feet at most).
    pub fn push_out(&self, p: &mut [f32; 3], tall: f32, rise: f32) {
        let hits: Vec<Prop> = self.near(p[0], p[2], RADIUS).copied().collect();
        for q in hits {
            // Over its top (or standing on it), a step below the feet, or
            // over the head: clear.
            let step = q.top().is_some_and(|t| t <= p[1] + rise);
            if p[1] >= q.solid_top() - 0.02 || step || p[1] + tall < q.y {
                continue;
            }
            let (dx, dz) = (p[0] - q.x, p[2] - q.z);
            let d = (dx * dx + dz * dz).sqrt();
            let want = q.r + RADIUS;
            if d >= want {
                continue;
            }
            let (nx, nz) = if d > 1e-4 {
                (dx / d, dz / d)
            } else {
                (1.0, 0.0)
            };
            p[0] = q.x + nx * want;
            p[2] = q.z + nz * want;
        }
    }

    /// Where along a bolt's path from `a` to `b` (0..1) it strikes the
    /// ground or something standing, if it does.
    pub fn strikes(&self, a: [f32; 3], b: [f32; 3]) -> Option<f32> {
        self.strikes_if(a, b, |_| true)
    }

    /// As `strikes`, passing through the things `solid` says are not.
    pub fn strikes_if(
        &self,
        a: [f32; 3],
        b: [f32; 3],
        solid: impl Fn(&Prop) -> bool,
    ) -> Option<f32> {
        let mut first: Option<f32> = None;
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let len = (d[0] * d[0] + d[2] * d[2]).sqrt();
        let mid = [a[0] + d[0] * 0.5, a[2] + d[2] * 0.5];
        for q in self
            .near(mid[0], mid[1], len * 0.5 + BOLT_RADIUS + self.reach)
            .filter(|q| solid(q))
        {
            let mut hit = through(a, d, [q.x, q.z], q.r + BOLT_RADIUS, (q.y, q.solid_top()));
            for (r, lo, hi) in q.cap().into_iter().flatten() {
                let ring = through(a, d, [q.x, q.z], r + BOLT_RADIUS, (lo, hi));
                hit = match (hit, ring) {
                    (Some(s), Some(c)) => Some(s.min(c)),
                    (s, c) => s.or(c),
                };
            }
            if let Some(t) = hit.filter(|&t| first.is_none_or(|f| t < f)) {
                first = Some(t);
            }
        }
        // The ground, and the decks, a metre at a time.
        let steps = (len.max(d[1].abs()).ceil() as usize).max(1);
        let mut was = a[1];
        for k in 1..=steps {
            let t = k as f32 / steps as f32;
            if first.is_some_and(|f| t >= f) {
                break;
            }
            let p = [a[0] + d[0] * t, a[1] + d[1] * t, a[2] + d[2] * t];
            if p[1] < self.height(p[0], p[2]).max(SEA)
                || (!self.decks.is_empty()
                    && self.deck_between(p[0], p[2], p[1].min(was), p[1].max(was)))
            {
                return Some(t);
            }
            was = p[1];
        }
        first
    }

    /// A random spot on land with nothing standing near, from `rng`.
    pub fn spot(&self, rng: &mut Rng) -> [f32; 2] {
        for _ in 0..200 {
            let (x, z) = (
                (unit(rng) * 2.0 - 1.0) * SHORE * 0.85,
                (unit(rng) * 2.0 - 1.0) * SHORE * 0.85,
            );
            if self.land(x, z) && self.near(x, z, 1.5).next().is_none() {
                return [x, z];
            }
        }
        [0.0, 0.0]
    }
}

/// Where along the segment from `a` (along `d`, 0..1) it first is inside
/// an upright cylinder about `at`, `r` wide, from height `lo` to `hi`: in
/// through its side, or down (or up) through its top (or bottom).
fn through(a: [f32; 3], d: [f32; 3], at: [f32; 2], r: f32, (lo, hi): (f32, f32)) -> Option<f32> {
    // Inside its circle, on the ground's plane...
    let (fx, fz) = (a[0] - at[0], a[2] - at[1]);
    let aa = d[0] * d[0] + d[2] * d[2];
    let cc = fx * fx + fz * fz - r * r;
    let (t0, t1) = if aa < 1e-9 {
        if cc > 0.0 {
            return None;
        }
        (0.0, 1.0)
    } else {
        let bb = 2.0 * (fx * d[0] + fz * d[2]);
        let disc = bb * bb - 4.0 * aa * cc;
        if disc < 0.0 {
            return None;
        }
        let s = disc.sqrt();
        ((-bb - s) / (2.0 * aa), (-bb + s) / (2.0 * aa))
    };
    // ...and inside its height, at once.
    let (y0, y1) = if d[1].abs() < 1e-9 {
        if a[1] < lo || a[1] > hi {
            return None;
        }
        (0.0, 1.0)
    } else {
        let (u, w) = ((lo - a[1]) / d[1], (hi - a[1]) / d[1]);
        (u.min(w), u.max(w))
    };
    let (enter, exit) = (t0.max(y0).max(0.0), t1.min(y1).min(1.0));
    (enter <= exit).then_some(enter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_island_in_the_sea() {
        let m = Map::new(7);
        assert!(m.height(0.0, 0.0) > SEA, "land at the centre");
        assert!(m.height(MAP_HALF - 1.0, 0.0) < SEA, "sea at the edge");
        let trees = m.props.iter().filter(|p| p.kind == Kind::Tree).count();
        assert!(trees > TREES / 2, "trees: {trees}");
        assert!(m.props.iter().any(|p| p.kind == Kind::Pillar));
        // Launch runes: by the places and out in the wild, on land, clear.
        assert!(m.pads.len() >= PADS_WILD, "{}", m.pads.len());
        for q in &m.pads {
            assert!(m.land(q[0], q[2]) && m.near(q[0], q[2], PAD_R).next().is_none());
        }
        assert_eq!(Map::new(7).props, m.props, "the same seed, the same island");
    }

    #[test]
    fn trees_grow_in_woods_with_meadows_between() {
        for seed in 0..8 {
            let m = Map::new(seed);
            let trees = m.props.iter().filter(|q| q.kind == Kind::Tree).count();
            assert_eq!(trees, TREES, "seed {seed}: every tree finds a place");
            // Land in the wild near a tree (in the woods), and not.
            let (mut wild, mut wooded) = (0, 0);
            for k in 0..900 {
                let (x, z) = ((k % 30) as f32 * 9.0 - 135.0, (k / 30) as f32 * 9.0 - 135.0);
                if m.land(x, z) && m.wild(x, z, 0.0) {
                    wild += 1;
                    wooded += m.near(x, z, 8.0).any(|q| q.kind == Kind::Tree) as i32;
                }
            }
            let share = wooded as f32 / wild as f32;
            assert!((0.45..0.85).contains(&share), "seed {seed}: {share} wooded");
        }
    }

    #[test]
    fn mushrooms_grow_by_the_woods_and_no_crown_hides_a_cap_or_a_pillar() {
        for seed in 0..16 {
            let m = Map::new(seed);
            let gap = |a: &Prop, b: &Prop| (a.x - b.x).hypot(a.z - b.z) - a.spread() - b.spread();
            let mut wild = 0;
            for s in m.props.iter().filter(|q| q.kind == Kind::Pillar) {
                // No crown over a ruin's pillar either.
                for t in m.props.iter().filter(|q| q.kind == Kind::Tree) {
                    assert!(gap(s, t) >= 0.0, "seed {seed}: {s:?} under {t:?}");
                }
            }
            for s in m.props.iter().filter(|q| q.kind == Kind::Shroom) {
                // No crown over any cap (the grove's, too).
                for t in m.props.iter().filter(|q| q.kind == Kind::Tree) {
                    assert!(gap(s, t) >= 0.0, "seed {seed}: {s:?} under {t:?}");
                }
                if !m.wild(s.x, s.z, 0.0) {
                    continue;
                }
                // Out in the wild, by a wood, nothing else under its cap.
                wild += 1;
                let others = m.props.iter().filter(|q| !std::ptr::eq(*q, s));
                assert!(
                    others.clone().all(|q| gap(s, q) >= 0.0),
                    "seed {seed}: {s:?}"
                );
                let by = others
                    .filter(|q| q.kind == Kind::Tree)
                    .any(|t| gap(s, t) < SHROOM_WOOD);
                assert!(by, "seed {seed}: {s:?} by a wood");
            }
            assert!(wild >= SHROOMS * 9 / 10, "seed {seed}: {wild} mushrooms");
        }
    }

    #[test]
    fn the_places_shape_the_island() {
        use crate::places::Place;
        for seed in 0..24 {
            let m = Map::new(seed);
            assert_eq!(m.pois.len(), 5);
            // The Spire: a level plaza, its tower at the centre.
            assert_eq!(m.height(0.0, 0.0), PLATEAU_TOP);
            assert_eq!(m.height(PLATEAU - 1.0, 0.0), PLATEAU_TOP);
            let mut p = [0.5, PLATEAU_TOP, 0.0];
            m.push_out(&mut p, HEIGHT, 0.0);
            assert!(p[0] >= TOWER_RADIUS + RADIUS - 1e-3, "the tower blocks");
            for q in &m.pois {
                assert!(m.land(q.x, q.z), "seed {seed}: {:?} on land", q.place);
                if q.place == Place::Rift {
                    let rim = m.height(q.x + q.r, q.z);
                    assert!(m.height(q.x, q.z) < rim - 2.0, "seed {seed}: a bowl");
                    assert!(m.height(q.x + 4.0, q.z) > SEA + 1.0, "not flooded");
                }
            }
            let kinds = [
                Kind::Tower,
                Kind::Stone,
                Kind::Spike,
                Kind::Crystal,
                Kind::Column,
            ];
            for k in kinds {
                assert!(m.props.iter().any(|p| p.kind == k), "seed {seed}: {k:?}");
            }
            assert!(m.caches.len() >= 8);
            // Each cache lies clear, or on something to stand on.
            for c in &m.caches {
                let mut under = m.near(c[0], c[1], RADIUS);
                assert!(under.all(|q| q.top().is_some()), "seed {seed}: {c:?}");
            }
        }
    }

    #[test]
    fn the_stair_climbs_the_tower_to_its_balcony() {
        let m = Map::new(9);
        let foot = m.height(0.0, 0.0) - 0.3;
        let r = TOWER_RADIUS + STAIR_WIDTH / 2.0;
        let at = |turns: f32| {
            let a = STAIR_FROM + turns * std::f32::consts::TAU;
            (a.cos() * r, a.sin() * r)
        };
        // A quarter turn up: a step above where it started, not the ground.
        let (x, z) = at(0.25);
        let up = foot + 0.3 + (BALCONY - 0.3) / STAIR_TURNS * 0.25;
        assert!((m.floor(x, z, up) - up).abs() < 0.01);
        assert!(
            m.floor(x, z, PLATEAU_TOP) < up - 1.0,
            "from below, the plaza"
        );
        // The second turn stands over the first.
        let high = up + (BALCONY - 0.3) / STAIR_TURNS;
        assert!((m.floor(x, z, high) - high).abs() < 0.01);
        assert!((m.floor(x, z, high - 3.0) - up).abs() < 0.01);
        // The balcony, half way round it.
        let (x, z) = at(0.4);
        assert!((m.floor(x, z, foot + BALCONY) - (foot + BALCONY)).abs() < 1e-4);
        // A bolt dropping onto the balcony strikes it.
        let y = foot + BALCONY;
        assert!(m.strikes([x, y + 2.0, z], [x + 0.01, y - 2.0, z]).is_some());
    }

    #[test]
    fn props_block_wizards_and_bolts() {
        let m = Map::new(3);
        let q = *m.props.iter().find(|p| p.kind == Kind::Rock).unwrap();
        let mut p = [q.x + 0.1, q.y + 0.2, q.z];
        m.push_out(&mut p, HEIGHT, 0.0);
        let d = ((p[0] - q.x).powi(2) + (p[2] - q.z).powi(2)).sqrt();
        assert!(d >= q.r + RADIUS - 1e-4);
        let a = [q.x - 3.0, q.y + q.h * 0.5, q.z];
        let b = [q.x + 3.0, q.y + q.h * 0.5, q.z];
        let t = m.strikes(a, b).expect("the rock stops it");
        assert!(t > 0.2 && t < 0.6, "{t}");
        let up = [q.x - 3.0, q.y + q.h + 20.0, q.z];
        assert!(m.strikes(up, [q.x + 3.0, up[1], q.z]).is_none(), "over it");
    }

    #[test]
    fn a_bolt_coming_down_onto_a_top_strikes_the_top() {
        let m = Map::new(11);
        let tops = m.props.iter().filter(|q| {
            matches!(
                q.kind,
                Kind::Pillar | Kind::Rock | Kind::Column | Kind::Stone
            )
        });
        let mut n = 0;
        for q in tops {
            let top = q.top().unwrap();
            // Past its side over it and down in through its top; and
            // straight down onto its middle.
            let rays = [
                (
                    [q.x - q.r - 1.0, top + 1.0, q.z],
                    [q.x + q.r * 0.5, top - 1.0, q.z],
                ),
                ([q.x, top + 4.0, q.z], [q.x, q.y, q.z]),
            ];
            for (a, b) in rays {
                let t = m.strikes(a, b).expect("it strikes");
                let y = a[1] + (b[1] - a[1]) * t;
                // Unless something else stood in the way first.
                if (y - top).abs() > 0.05 {
                    assert!(y > top, "{q:?}: struck at {y}, under its top {top}");
                }
                n += 1;
            }
        }
        assert!(n > 100, "{n}");
    }
}
