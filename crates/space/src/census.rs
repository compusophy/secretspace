//! The census nobody runs: how many islands each lineage is alive on, and
//! how many motes it has, across the whole world, with no server counting.
//!
//! Each island keeps HyperLogLog sketches per lineage: one of the islands
//! holding it, one of its motes, and one of all islands. A sketch merges by
//! taking the larger register, so merging is idempotent and commutative:
//! islands swap sketches with their neighbours every few seconds and every
//! island converges on the same estimate however the gossip loops. Counts
//! live in minute-long epochs; an island inserts itself into the current
//! one, and a view merges the current and previous epochs, so a closed tab
//! ages out within two minutes.
//!
//! Small worlds are counted exactly: up to EXACT islands per lineage (and
//! EXACT_WORLD in all) travel as an explicit set, merged by union; past that
//! the sketches take over. Integer only: the estimator runs in fixed point.

use std::collections::BTreeMap;

use crate::rng::splitmix;

/// Registers per sketch (2^6): about 13% standard error past ~160, and
/// close to exact below it, where linear counting takes over.
pub const REGS: usize = 64;
const BITS: u32 = 6;
/// Lineages a gossip message carries, and an island remembers per epoch.
pub const GOSSIP_LINEAGES: usize = 24;
const REMEMBER: usize = 256;
/// Islands counted exactly per lineage, and in the whole world.
pub const EXACT: usize = 16;
pub const EXACT_WORLD: usize = 32;

/// 16 * REGS * ln(REGS / zeros), for zeros = 1..=REGS: linear counting in
/// fixed point.
const LINEAR16: [u32; REGS] = [
    4259, 3549, 3134, 2839, 2611, 2424, 2266, 2129, 2009, 1901, 1803, 1714, 1632, 1556, 1486, 1420,
    1357, 1299, 1244, 1191, 1141, 1093, 1048, 1004, 963, 922, 884, 847, 811, 776, 742, 710, 678,
    648, 618, 589, 561, 534, 507, 481, 456, 431, 407, 384, 361, 338, 316, 295, 273, 253, 233, 213,
    193, 174, 155, 137, 119, 101, 83, 66, 49, 33, 16, 0,
];
/// alpha(64) * 64^2.
const ALPHA_M2: u64 = 2904;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sketch(pub [u8; REGS]);

impl Default for Sketch {
    fn default() -> Self {
        Sketch([0; REGS])
    }
}

impl Sketch {
    /// Add one item, by its 64-bit hash.
    pub fn insert(&mut self, h: u64) {
        let i = (h >> (64 - BITS)) as usize;
        let rank = ((h << BITS) | (1 << (BITS - 1))).leading_zeros() as u8 + 1;
        if rank > self.0[i] {
            self.0[i] = rank;
        }
    }

    pub fn merge(&mut self, other: &Sketch) {
        for (a, b) in self.0.iter_mut().zip(other.0.iter()) {
            *a = (*a).max(*b);
        }
    }

    /// Distinct items inserted, estimated.
    pub fn estimate(&self) -> u32 {
        let zeros = self.0.iter().filter(|&&r| r == 0).count();
        if zeros == REGS {
            return 0;
        }
        // Raw estimate: alpha m^2 / sum(2^-r), with the sum scaled by 2^32.
        let z: u64 = self.0.iter().map(|&r| (1u64 << 32) >> r.min(63)).sum();
        let raw = ALPHA_M2 * (1u64 << 32) / z.max(1);
        if raw <= (REGS as u64 * 5) / 2 && zeros > 0 {
            return (LINEAR16[zeros - 1] + 8) / 16;
        }
        raw.min(u32::MAX as u64) as u32
    }
}

/// Islands and how many motes each holds, while there are few enough to
/// list; None once the list has overflowed.
pub type Exact = Option<Vec<(u64, u32)>>;

/// Union of two exact lists (an island's larger count wins), or None if
/// either has overflowed or the union would pass `cap`.
fn union(a: &Exact, b: &Exact, cap: usize) -> Exact {
    let (a, b) = (a.as_ref()?, b.as_ref()?);
    let mut out: BTreeMap<u64, u32> = a.iter().copied().collect();
    for &(island, n) in b {
        let e = out.entry(island).or_insert(0);
        *e = (*e).max(n);
    }
    (out.len() <= cap).then(|| out.into_iter().collect())
}

/// One lineage's count: exact while small, sketched always.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tally {
    pub name: String,
    pub author: String,
    pub islands: Sketch,
    pub motes: Sketch,
    pub exact: Exact,
}

impl Tally {
    fn merge(&mut self, other: &Tally) {
        self.islands.merge(&other.islands);
        self.motes.merge(&other.motes);
        self.exact = union(&self.exact, &other.exact, EXACT);
    }

    pub fn islands(&self) -> u32 {
        match &self.exact {
            Some(e) => e.len() as u32,
            None => self.islands.estimate(),
        }
    }

    pub fn motes(&self) -> u32 {
        match &self.exact {
            Some(e) => e.iter().map(|&(_, n)| n).sum(),
            None => self.motes.estimate(),
        }
    }
}

/// What one island tells its neighbours.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gossip {
    pub epoch: u32,
    pub world: Sketch,
    /// The islands in the world, while there are at most EXACT_WORLD.
    pub world_exact: Option<Vec<u64>>,
    pub lineages: Vec<(u64, Tally)>,
}

#[derive(Clone, Debug)]
struct Epoch {
    n: u32,
    world: Sketch,
    world_exact: Exact,
    lineages: BTreeMap<u64, Tally>,
}

impl Default for Epoch {
    fn default() -> Self {
        Epoch {
            n: 0,
            world: Sketch::default(),
            world_exact: Some(Vec::new()),
            lineages: BTreeMap::new(),
        }
    }
}

fn as_exact(ids: &Option<Vec<u64>>) -> Exact {
    ids.as_ref().map(|v| v.iter().map(|&id| (id, 1)).collect())
}

impl Epoch {
    fn merge_tally(&mut self, lineage: u64, t: &Tally) {
        match self.lineages.get_mut(&lineage) {
            Some(mine) => mine.merge(t),
            None => {
                if self.lineages.len() >= REMEMBER {
                    // Forget the least widespread to make room.
                    let least = self
                        .lineages
                        .iter()
                        .min_by_key(|(&id, t)| (t.islands(), id))
                        .map(|(&id, _)| id);
                    if let Some(id) = least {
                        if t.islands() <= self.lineages[&id].islands() {
                            return;
                        }
                        self.lineages.remove(&id);
                    }
                }
                self.lineages.insert(lineage, t.clone());
            }
        }
    }
}

/// One lineage as the world knows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Known {
    pub lineage: u64,
    pub name: String,
    pub author: String,
    pub islands: u32,
    pub motes: u32,
    /// Counted exactly rather than estimated.
    pub exact: bool,
}

/// An island's view of the world census.
#[derive(Clone, Debug, Default)]
pub struct Census {
    current: Epoch,
    previous: Epoch,
}

fn island_hash(island: u64) -> u64 {
    splitmix(island ^ 0x5EC2_E75A_ACE0_0001)
}

fn mote_hash(island: u64, i: u32) -> u64 {
    splitmix(island ^ splitmix(i as u64 + 1))
}

impl Census {
    /// Move to `epoch` if time has passed: the current epoch becomes the
    /// previous one. A clock that runs backwards changes nothing.
    fn advance(&mut self, epoch: u32) {
        if epoch == self.current.n {
            return;
        }
        if epoch == self.current.n + 1 {
            self.previous = std::mem::take(&mut self.current);
        } else if epoch > self.current.n {
            self.previous = Epoch::default();
            self.current = Epoch::default();
        } else {
            return;
        }
        self.current.n = epoch;
        self.previous.n = epoch.saturating_sub(1);
    }

    /// Count this island: itself in the world, and each lineage it holds
    /// (lineage, name, author, motes here).
    pub fn note(&mut self, epoch: u32, island: u64, here: &[(u64, &str, &str, u32)]) {
        self.advance(epoch);
        if epoch != self.current.n {
            return;
        }
        let e = &mut self.current;
        e.world.insert(island_hash(island));
        e.world_exact = union(&e.world_exact, &Some(vec![(island, 1)]), EXACT_WORLD);
        for &(lineage, name, author, count) in here.iter().filter(|h| h.3 > 0) {
            let count = count.min(crate::laws::CELLS as u32);
            let mut t = Tally {
                name: name.to_string(),
                author: author.to_string(),
                islands: Sketch::default(),
                motes: Sketch::default(),
                exact: Some(vec![(island, count)]),
            };
            t.islands.insert(island_hash(island));
            for i in 0..count {
                t.motes.insert(mote_hash(island, i));
            }
            e.merge_tally(lineage, &t);
        }
    }

    /// Fold in a neighbour's gossip, if it is for an epoch we keep.
    pub fn merge(&mut self, epoch_now: u32, g: &Gossip) {
        self.advance(epoch_now);
        let e = if g.epoch == self.current.n {
            &mut self.current
        } else if g.epoch == self.previous.n && self.previous.n != self.current.n {
            &mut self.previous
        } else {
            return;
        };
        e.world.merge(&g.world);
        e.world_exact = union(&e.world_exact, &as_exact(&g.world_exact), EXACT_WORLD);
        for (lineage, t) in g.lineages.iter().take(GOSSIP_LINEAGES) {
            e.merge_tally(*lineage, t);
        }
    }

    /// What to tell the neighbours: this epoch's world, and its most
    /// widespread lineages (plus any in `always`, such as this island's own).
    pub fn gossip(&self, always: &[u64]) -> Gossip {
        let e = &self.current;
        let mut ids: Vec<u64> = always
            .iter()
            .copied()
            .filter(|id| e.lineages.contains_key(id))
            .collect();
        ids.truncate(GOSSIP_LINEAGES / 2);
        let mut rest: Vec<(&u64, &Tally)> = e
            .lineages
            .iter()
            .filter(|(id, _)| !ids.contains(id))
            .collect();
        rest.sort_by_key(|(&id, t)| (std::cmp::Reverse(t.islands()), id));
        ids.extend(
            rest.into_iter()
                .map(|(&id, _)| id)
                .take(GOSSIP_LINEAGES - ids.len()),
        );
        Gossip {
            epoch: e.n,
            world: e.world,
            world_exact: e
                .world_exact
                .as_ref()
                .map(|v| v.iter().map(|&(id, _)| id).collect()),
            lineages: ids
                .into_iter()
                .map(|id| (id, e.lineages[&id].clone()))
                .collect(),
        }
    }

    /// Islands in the world, estimated over the last two epochs.
    pub fn islands(&self) -> u32 {
        match union(
            &self.current.world_exact,
            &self.previous.world_exact,
            EXACT_WORLD,
        ) {
            Some(ids) => ids.len() as u32,
            None => {
                let mut w = self.current.world;
                w.merge(&self.previous.world);
                w.estimate()
            }
        }
    }

    /// Every lineage heard of, most widespread first.
    pub fn view(&self) -> Vec<Known> {
        let mut by: BTreeMap<u64, Tally> = self.previous.lineages.clone();
        for (id, t) in &self.current.lineages {
            match by.get_mut(id) {
                Some(m) => m.merge(t),
                None => {
                    by.insert(*id, t.clone());
                }
            }
        }
        let mut v: Vec<Known> = by
            .into_iter()
            .map(|(lineage, t)| Known {
                lineage,
                islands: t.islands(),
                motes: t.motes(),
                exact: t.exact.is_some(),
                name: t.name,
                author: t.author,
            })
            .collect();
        v.sort_by(|a, b| {
            b.islands
                .cmp(&a.islands)
                .then(b.motes.cmp(&a.motes))
                .then(a.lineage.cmp(&b.lineage))
        });
        v
    }
}
