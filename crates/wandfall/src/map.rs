//! The island, from its seed alone, the same on the server and the page:
//! rolling hills falling to the sea at the shore, its places (`places`:
//! the Spire at the centre, a stone circle, a demon rift, a crystal
//! grove), and trees, rocks, giant mushrooms and ruined rings of pillars
//! between them. Heights use arithmetic only (no library calls), so both
//! ends agree to the bit.

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
}

impl Kind {
    /// Whether wizards can stand on top of it (and climb onto it).
    pub fn standable(self) -> bool {
        matches!(
            self,
            Kind::Rock | Kind::Pillar | Kind::Stone | Kind::Altar | Kind::Merlon
        )
    }
}

/// One thing standing on the island. Its trunk, body or shaft blocks
/// wizards and bolts: a cylinder `r` wide and `h` tall from `y`.
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
    /// Where feet stand on its top, if they can: a boulder's rounded top
    /// stands a little over its trunk.
    pub fn top(&self) -> Option<f32> {
        let h = if self.kind == Kind::Rock {
            self.h * ROCK_TOP
        } else {
            self.h
        };
        self.kind.standable().then_some(self.y + h)
    }
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
    grid: Vec<Vec<u16>>,
}

pub fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 0..1 from a lattice point.
fn lattice(seed: u64, x: i32, z: i32) -> f32 {
    let h = splitmix(seed ^ ((x as u32 as u64) << 32 | z as u32 as u64));
    (h >> 40) as f32 / (1u64 << 24) as f32
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
            grid: vec![Vec::new(); (CELLS * CELLS) as usize],
        };
        places::set(&mut m);
        m.scatter();
        m
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
        for q in self.near(x, z, 0.0) {
            if let Some(t) = q.top().filter(|&t| t <= y + STEP) {
                f = f.max(t);
            }
        }
        f
    }

    /// A ledge to climb onto from feet at `p`, pushing along (`wx`, `wz`):
    /// the top of something that can be stood on, close ahead and within
    /// reach above the feet; its top and middle.
    pub fn ledge(&self, p: [f32; 3], (wx, wz): (f32, f32)) -> Option<(f32, [f32; 2])> {
        self.near(p[0], p[2], RADIUS + MANTLE_NEAR)
            .filter_map(|q| {
                let t = q.top()?;
                let (dx, dz) = (q.x - p[0], q.z - p[2]);
                let d = (dx * dx + dz * dz).sqrt().max(1e-4);
                let ahead = (dx * wx + dz * wz) / d;
                (t > p[1] + MANTLE_LOW && t <= p[1] + MANTLE_REACH && ahead >= MANTLE_AHEAD)
                    .then_some((t, [q.x, q.z]))
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
        let c = |v: f32| (((v + MAP_HALF) / CELL).floor() as i32).clamp(0, CELLS - 1);
        for cz in c(p.z - p.r)..=c(p.z + p.r) {
            for cx in c(p.x - p.r)..=c(p.x + p.r) {
                self.grid[(cz * CELLS + cx) as usize].push(i);
            }
        }
    }

    /// Whether a point is on land (not the sea, not past the edge).
    pub fn land(&self, x: f32, z: f32) -> bool {
        x.abs() < MAP_HALF && z.abs() < MAP_HALF && self.height(x, z) > SEA + 0.4
    }

    fn scatter(&mut self) {
        let mut rng = Rng::new(self.seed ^ 0x51ab);
        let mut unit = move || (rng.next_u64() >> 40) as f32 / (1u64 << 24) as f32;
        let put = |m: &mut Map, p: Prop| m.put(p);
        // Ruins first: rings of pillars, the island's landmarks.
        let mut ruins = 0;
        while ruins < RUINS {
            let (x, z) = (
                (unit() * 2.0 - 1.0) * SHORE * 0.8,
                (unit() * 2.0 - 1.0) * SHORE * 0.8,
            );
            if !self.land(x, z) || !self.wild(x, z, 10.0) {
                continue;
            }
            ruins += 1;
            let ring = 5.0 + unit() * 4.0;
            let n = 6 + (unit() * 5.0) as usize;
            for k in 0..n {
                // Some pillars have fallen.
                if unit() < 0.25 {
                    continue;
                }
                let turn = (k * 65536 / n) as u16;
                let (sa, ca) = crate::trig::sin_cos(turn);
                let a = crate::trig::radians(turn);
                let (px, pz) = (x + ca * ring, z + sa * ring);
                if !self.land(px, pz) {
                    continue;
                }
                let h = 2.0 + unit() * 3.5;
                let y = self.height(px, pz) - 0.3;
                put(
                    self,
                    Prop {
                        kind: Kind::Pillar,
                        x: px,
                        z: pz,
                        y,
                        r: 0.55,
                        h,
                        yaw: a,
                        scale: h,
                    },
                );
            }
        }
        let place = |m: &mut Map, kind: Kind, count: usize, unit: &mut dyn FnMut() -> f32| {
            let mut n = 0;
            let mut tries = 0;
            while n < count && tries < count * 20 {
                tries += 1;
                let (x, z) = ((unit() * 2.0 - 1.0) * SHORE, (unit() * 2.0 - 1.0) * SHORE);
                if !m.land(x, z) || m.height(x, z) < SEA + 1.0 || !m.wild(x, z, 3.0) {
                    continue;
                }
                let s = 0.8 + unit() * 0.6;
                let (r, h) = match kind {
                    Kind::Rock => (1.1 * s, 1.3 * s),
                    Kind::Shroom => (0.4 * s, 3.5 * s),
                    _ => (0.35 * s, 6.0 * s),
                };
                // Not inside another.
                if m.near(x, z, r + 1.2).next().is_some() {
                    continue;
                }
                let y = m.height(x, z) - 0.2;
                let yaw = unit() * std::f32::consts::TAU;
                put(
                    m,
                    Prop {
                        kind,
                        x,
                        z,
                        y,
                        r,
                        h,
                        yaw,
                        scale: s,
                    },
                );
                n += 1;
            }
        };
        place(self, Kind::Tree, TREES, &mut unit);
        place(self, Kind::Rock, ROCKS, &mut unit);
        place(self, Kind::Shroom, SHROOMS, &mut unit);
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
    /// standing there.
    pub fn push_out(&self, p: &mut [f32; 3], tall: f32) {
        let hits: Vec<Prop> = self.near(p[0], p[2], RADIUS).copied().collect();
        for q in hits {
            // Over its top (or standing on it), or under it: clear.
            let top = q.top().unwrap_or(q.y + q.h).max(q.y + q.h);
            if p[1] >= top - 0.02 || p[1] + tall < q.y {
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
            .near(mid[0], mid[1], len * 0.5 + BOLT_RADIUS)
            .filter(|q| solid(q))
        {
            // Circle against the segment, on the ground's plane.
            let (fx, fz) = (a[0] - q.x, a[2] - q.z);
            let r = q.r + BOLT_RADIUS;
            let aa = d[0] * d[0] + d[2] * d[2];
            let bb = 2.0 * (fx * d[0] + fz * d[2]);
            let cc = fx * fx + fz * fz - r * r;
            let t = if cc <= 0.0 {
                0.0
            } else {
                let disc = bb * bb - 4.0 * aa * cc;
                if aa < 1e-9 || disc < 0.0 {
                    continue;
                }
                (-bb - disc.sqrt()) / (2.0 * aa)
            };
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            let y = a[1] + d[1] * t;
            if y >= q.y && y <= q.y + q.h && first.is_none_or(|f| t < f) {
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

    /// A random spot on land, from `r` (0..1 numbers).
    pub fn spot(&self, rng: &mut Rng) -> [f32; 2] {
        for _ in 0..200 {
            let u = |rng: &mut Rng| (rng.next_u64() >> 40) as f32 / (1u64 << 24) as f32;
            let (x, z) = (
                (u(rng) * 2.0 - 1.0) * SHORE * 0.85,
                (u(rng) * 2.0 - 1.0) * SHORE * 0.85,
            );
            if self.land(x, z) && self.near(x, z, 1.5).next().is_none() {
                return [x, z];
            }
        }
        [0.0, 0.0]
    }
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
        assert_eq!(Map::new(7).props, m.props, "the same seed, the same island");
    }

    #[test]
    fn the_places_shape_the_island() {
        use crate::places::Place;
        for seed in 0..24 {
            let m = Map::new(seed);
            assert_eq!(m.pois.len(), 4);
            // The Spire: a level plaza, its tower at the centre.
            assert_eq!(m.height(0.0, 0.0), PLATEAU_TOP);
            assert_eq!(m.height(PLATEAU - 1.0, 0.0), PLATEAU_TOP);
            let mut p = [0.5, PLATEAU_TOP, 0.0];
            m.push_out(&mut p, HEIGHT);
            assert!(p[0] >= TOWER_RADIUS + RADIUS - 1e-3, "the tower blocks");
            for q in &m.pois {
                assert!(m.land(q.x, q.z), "seed {seed}: {:?} on land", q.place);
                if q.place == Place::Rift {
                    let rim = m.height(q.x + q.r, q.z);
                    assert!(m.height(q.x, q.z) < rim - 2.0, "seed {seed}: a bowl");
                    assert!(m.height(q.x + 4.0, q.z) > SEA + 1.0, "not flooded");
                }
            }
            let kinds = [Kind::Tower, Kind::Stone, Kind::Spike, Kind::Crystal];
            for k in kinds {
                assert!(m.props.iter().any(|p| p.kind == k), "seed {seed}: {k:?}");
            }
            assert!(m.caches.len() >= 8);
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
        m.push_out(&mut p, HEIGHT);
        let d = ((p[0] - q.x).powi(2) + (p[2] - q.z).powi(2)).sqrt();
        assert!(d >= q.r + RADIUS - 1e-4);
        let a = [q.x - 3.0, q.y + q.h * 0.5, q.z];
        let b = [q.x + 3.0, q.y + q.h * 0.5, q.z];
        let t = m.strikes(a, b).expect("the rock stops it");
        assert!(t > 0.2 && t < 0.6, "{t}");
        let up = [q.x - 3.0, q.y + q.h + 20.0, q.z];
        assert!(m.strikes(up, [q.x + 3.0, up[1], q.z]).is_none(), "over it");
    }
}
