//! One island: the patch of the world that lives in one person's tab.
//!
//! An island is a pure function of its inputs: its id, whether it was
//! watched each tick, which portals were open, and which motes arrived.
//! Integer math only, no clocks, no hash-ordered collections; the same
//! inputs replay to the same state and the same hash.
//!
//! Every erg is accounted for. Light and balances together always equal
//! what was minted, granted and imported, minus what was exported and
//! burned (`conserved`).

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::caps::{TickHost, CAPS};
use crate::genome::{mutate, Genome};
use crate::hash::{hash_str, Fnv};
use crate::laws::*;
use crate::mind::{self, Outcome};
use crate::rng::{splitmix, Rng};

#[derive(Clone, Debug)]
pub struct Mote {
    pub id: u64,
    /// The lineage it descends from: the same across every island.
    pub lineage: u64,
    pub name: Rc<str>,
    pub author: Rc<str>,
    pub gen: u32,
    pub hops: u32,
    pub age: u32,
    pub x: u16,
    pub y: u16,
    pub balance: u64,
    pub mem: [i64; MEM_SLOTS],
    pub genome: Rc<Genome>,
    /// Fuel burned in its last thought.
    pub last_used: u32,
    pub(crate) leaving: Option<u8>,
}

/// A mote in the wire: everything it carries from one island to the next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Traveler {
    pub id: u64,
    pub lineage: u64,
    pub name: String,
    pub author: String,
    pub gen: u32,
    pub hops: u32,
    pub age: u32,
    pub balance: u64,
    pub mem: [i64; MEM_SLOTS],
    pub genome: String,
    /// Where along the edge it left, in cells from the edge's start.
    pub offset: u16,
}

/// Where every erg came from and went.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ledger {
    /// Sunlight.
    pub minted: u64,
    /// The island's first light, founders, revived fossils.
    pub granted: u64,
    /// Carried in by arriving motes.
    pub imported: u64,
    /// Carried out by departing motes.
    pub exported: u64,
    /// Thought, tithes, crossings, and light with no room to land.
    pub burned: u64,
}

impl Ledger {
    pub fn expected(&self) -> u64 {
        (self.minted + self.granted + self.imported) - (self.exported + self.burned)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    Arrived(u8),
    Departed(u8),
    Released,
    Starved,
    Old,
    Stillborn,
    Crushed,
}

#[derive(Clone, Debug)]
pub struct Event {
    pub tick: u64,
    pub kind: EventKind,
    pub name: Rc<str>,
    pub lineage: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub births: u64,
    pub miscarriages: u64,
    pub deaths: u64,
    pub arrivals: u64,
    pub departures: u64,
}

/// One lineage's presence on an island.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lineage {
    pub id: u64,
    pub name: Rc<str>,
    pub author: Rc<str>,
    pub count: u32,
    pub ergs: u64,
}

pub struct Island {
    pub id: u64,
    pub tick: u64,
    /// 0..=SUN_MAX: how watched the island is, eased.
    pub sun: u32,
    /// Whether someone is looking at it now.
    pub watched: bool,
    /// Per side: 0 if the portal is shut, else 1 + the sun beyond it.
    pub portals: [u32; 4],
    pub ledger: Ledger,
    pub counts: Counts,
    pub(crate) light: Vec<u32>,
    pub(crate) motes: Vec<Mote>,
    pub(crate) occ: Vec<u32>,
    pub(crate) rng: Rng,
    serial: u64,
    arrivals: Vec<(u8, Traveler)>,
    departures: Vec<(u8, Traveler)>,
    events: Vec<Event>,
}

const NEIGHBORS: [(i64, i64); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];
const MAX_EVENTS: usize = 64;
const MAX_NAME: usize = 32;
/// Light every cell starts with.
const FIRST_LIGHT: u32 = LIGHT_CAP / 4;
/// How far from its entry point an arriving mote looks for room.
const ENTRY_SEARCH: i64 = 6;

/// The lineage id of a founding genome: the same on every island, so a
/// founder's descendants are one lineage across the whole world.
pub fn founder_lineage(name: &str) -> u64 {
    hash_str(&format!("founder:{name}"))
}

/// sin of whole degrees, scaled by 1000 (Bhaskara I's approximation):
/// integer only, good to about 0.2%.
pub fn sin1000(deg: i64) -> i64 {
    let d = deg.rem_euclid(360);
    let (d, sign) = if d < 180 { (d, 1) } else { (d - 180, -1) };
    let p = d * (180 - d);
    sign * 4000 * p / (40500 - p)
}

/// Where the warm spot is at `tick`: a figure eight across the island.
pub fn spot(tick: u64) -> (i64, i64) {
    let deg = ((tick % SPOT_PERIOD) * 360 / SPOT_PERIOD) as i64;
    let (hw, hh) = (W as i64 / 2, H as i64 / 2);
    (
        hw + (hw - 7) * sin1000(deg) / 1000,
        hh + (hh - 6) * sin1000(2 * deg + 90) / 1000,
    )
}

fn clip(s: &str) -> Rc<str> {
    let mut end = s.len().min(MAX_NAME);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    Rc::from(&s[..end])
}

impl Island {
    pub const EMPTY: u32 = u32::MAX;

    pub fn new(id: u64) -> Island {
        let mut isl = Island {
            id,
            tick: 0,
            sun: 0,
            watched: false,
            portals: [0; 4],
            ledger: Ledger::default(),
            counts: Counts::default(),
            light: vec![FIRST_LIGHT; CELLS],
            motes: Vec::new(),
            occ: vec![Self::EMPTY; CELLS],
            rng: Rng::new(id),
            serial: 0,
            arrivals: Vec::new(),
            departures: Vec::new(),
            events: Vec::new(),
        };
        isl.ledger.granted = FIRST_LIGHT as u64 * CELLS as u64;
        isl
    }

    pub fn cell(x: i64, y: i64) -> Option<usize> {
        if (0..W as i64).contains(&x) && (0..H as i64).contains(&y) {
            Some(y as usize * W + x as usize)
        } else {
            None
        }
    }

    pub fn motes(&self) -> &[Mote] {
        &self.motes
    }

    pub fn light(&self) -> &[u32] {
        &self.light
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }

    pub fn mote_at(&self, c: usize) -> Option<usize> {
        match self.occ[c] {
            Self::EMPTY => None,
            i => Some(i as usize),
        }
    }

    fn next_id(&mut self) -> u64 {
        self.serial += 1;
        splitmix(self.id ^ splitmix(self.serial))
    }

    fn event(&mut self, kind: EventKind, name: Rc<str>, lineage: u64) {
        if self.events.len() == MAX_EVENTS {
            self.events.remove(0);
        }
        self.events.push(Event {
            tick: self.tick,
            kind,
            name,
            lineage,
        });
    }

    /// Light (or a body's ergs) laid down in a cell; what has no room burns.
    fn deposit(&mut self, c: usize, amount: u64) {
        let room = (LIGHT_CAP - self.light[c]) as u64;
        let put = amount.min(room);
        self.light[c] += put as u32;
        self.ledger.burned += amount - put;
    }

    fn free_cell(&mut self) -> Option<usize> {
        for _ in 0..64 {
            let c = self.rng.below(CELLS as u64) as usize;
            if self.occ[c] == Self::EMPTY {
                return Some(c);
            }
        }
        (0..CELLS).find(|&c| self.occ[c] == Self::EMPTY)
    }

    #[allow(clippy::too_many_arguments)]
    fn place(
        &mut self,
        c: usize,
        id: u64,
        lineage: u64,
        name: Rc<str>,
        author: Rc<str>,
        genome: Rc<Genome>,
        balance: u64,
    ) -> usize {
        let i = self.motes.len();
        self.motes.push(Mote {
            id,
            lineage,
            name,
            author,
            gen: 0,
            hops: 0,
            age: 0,
            x: (c % W) as u16,
            y: (c / W) as u16,
            balance,
            mem: [0; MEM_SLOTS],
            genome,
            last_used: 0,
            leaving: None,
        });
        self.occ[c] = i as u32;
        i
    }

    /// Founders: `count` motes of a genome, each granted `endow`.
    pub fn seed(&mut self, name: &str, src: &str, count: usize, endow: u64) {
        let genome = Genome::new(src).expect("founding genomes compile");
        let lineage = founder_lineage(name);
        let (name, author) = (clip(name), clip("genesis"));
        for _ in 0..count {
            let Some(c) = self.free_cell() else { return };
            let id = self.next_id();
            self.place(
                c,
                id,
                lineage,
                name.clone(),
                author.clone(),
                genome.clone(),
                endow,
            );
            self.ledger.granted += endow;
        }
    }

    /// A new mind, released by a person around (x, y) as a clutch. Its
    /// endowment is drawn from the island's brightest cells, so releasing
    /// costs the land and mints nothing. Returns the new lineage's id.
    pub fn release(
        &mut self,
        src: &str,
        name: &str,
        author: &str,
        x: i64,
        y: i64,
    ) -> Result<u64, String> {
        self.release_clutch(src, name, author, x, y, CLUTCH, RELEASE_ENDOW, CLEARING)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn release_clutch(
        &mut self,
        src: &str,
        name: &str,
        author: &str,
        x: i64,
        y: i64,
        count: usize,
        endow: u64,
        clearing: i64,
    ) -> Result<u64, String> {
        let genome = Genome::new(src).map_err(|d| d.to_string())?;
        let (x, y) = (x.clamp(0, W as i64 - 1), y.clamp(0, H as i64 - 1));
        // Clear the ground: every mote within the radius returns to the
        // light of its cell.
        let mut cleared = 0;
        for m in self.motes.iter_mut() {
            if (m.x as i64 - x).abs() <= clearing
                && (m.y as i64 - y).abs() <= clearing
                && m.leaving.is_none()
            {
                m.age = MAX_AGE;
                cleared += 1;
            }
        }
        if cleared > 0 {
            self.settle();
        }
        let cost = endow * count as u64;
        let total: u64 = self.light.iter().map(|&l| l as u64).sum();
        if total < cost {
            return Err(format!(
                "not enough light on the island ({total} of {cost})"
            ));
        }
        let mut cells = Vec::new();
        'search: for r in 0..=ENTRY_SEARCH {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dy.abs()) != r {
                        continue;
                    }
                    if let Some(k) = Island::cell(x + dx, y + dy) {
                        if self.occ[k] == Self::EMPTY {
                            cells.push(k);
                            if cells.len() == count {
                                break 'search;
                            }
                        }
                    }
                }
            }
        }
        if cells.is_empty() {
            return Err("no room here".into());
        }
        let cost = endow * cells.len() as u64;
        let near = |k: usize| {
            ((k % W) as i64 - x).abs() <= clearing && ((k / W) as i64 - y).abs() <= clearing
        };
        let mut order: Vec<usize> = (0..CELLS).collect();
        order.sort_by(|&a, &b| {
            near(b)
                .cmp(&near(a))
                .then(self.light[b].cmp(&self.light[a]))
                .then(a.cmp(&b))
        });
        let mut owed = cost;
        for k in order {
            let take = (self.light[k] as u64).min(owed).min(LIGHT_CAP as u64 / 2);
            self.light[k] -= take as u32;
            owed -= take;
            if owed == 0 {
                break;
            }
        }
        let lineage = Fnv::default()
            .u64(genome.hash)
            .bytes(author.as_bytes())
            .u64(self.id)
            .u64(self.tick)
            .finish();
        let (name, author) = (clip(name), clip(author));
        for c in cells {
            let id = self.next_id();
            self.place(
                c,
                id,
                lineage,
                name.clone(),
                author.clone(),
                genome.clone(),
                endow,
            );
        }
        self.event(EventKind::Released, name, lineage);
        Ok(lineage)
    }

    /// Queue a mote that came through the portal on `side`; it lands at
    /// the start of the next tick.
    pub fn arrive(&mut self, side: u8, t: Traveler) {
        self.arrivals.push((side % 4, t));
    }

    /// Motes that crossed a portal since the last call.
    pub fn take_departures(&mut self) -> Vec<(u8, Traveler)> {
        std::mem::take(&mut self.departures)
    }

    fn entry(side: u8, offset: u16) -> (i64, i64) {
        let (o_x, o_y) = (
            (offset as i64).min(W as i64 - 1),
            (offset as i64).min(H as i64 - 1),
        );
        match side {
            0 => (o_x, 0),
            1 => (W as i64 - 1, o_y),
            2 => (o_x, H as i64 - 1),
            _ => (0, o_y),
        }
    }

    fn admit(&mut self, side: u8, t: Traveler) {
        self.ledger.imported += t.balance;
        self.counts.arrivals += 1;
        let (ex, ey) = Island::entry(side, t.offset);
        let entry = Island::cell(ex, ey).expect("entry is on the island");
        let name = clip(&t.name);
        let Ok(genome) = Genome::new(&t.genome) else {
            self.deposit(entry, t.balance);
            self.event(EventKind::Stillborn, name, t.lineage);
            return;
        };
        let mut spot = None;
        'search: for r in 0..=ENTRY_SEARCH {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dy.abs()) != r {
                        continue;
                    }
                    if let Some(c) = Island::cell(ex + dx, ey + dy) {
                        if self.occ[c] == Self::EMPTY {
                            spot = Some(c);
                            break 'search;
                        }
                    }
                }
            }
        }
        let Some(c) = spot else {
            self.deposit(entry, t.balance);
            self.event(EventKind::Crushed, name, t.lineage);
            return;
        };
        let i = self.place(
            c,
            t.id,
            t.lineage,
            name.clone(),
            clip(&t.author),
            genome,
            t.balance,
        );
        let m = &mut self.motes[i];
        m.gen = t.gen;
        m.hops = t.hops;
        m.age = t.age;
        m.mem = t.mem;
        self.event(EventKind::Arrived(side), name, t.lineage);
    }

    /// Fossils: the richest `max` motes, to keep when the tab closes.
    pub fn fossils(&self, max: usize) -> Vec<Traveler> {
        let mut order: Vec<&Mote> = self.motes.iter().collect();
        order.sort_by(|a, b| b.balance.cmp(&a.balance).then(a.id.cmp(&b.id)));
        order
            .into_iter()
            .take(max)
            .map(|m| Island::traveler(m, m.balance, 0))
            .collect()
    }

    /// Fossils wake: each is placed anywhere free, its ergs granted.
    pub fn revive(&mut self, fossils: Vec<Traveler>) {
        for t in fossils {
            let Some(c) = self.free_cell() else { return };
            let Ok(genome) = Genome::new(&t.genome) else {
                continue;
            };
            let balance = t.balance.min(MAX_CARRY);
            self.ledger.granted += balance;
            let i = self.place(
                c,
                t.id,
                t.lineage,
                clip(&t.name),
                clip(&t.author),
                genome,
                balance,
            );
            let m = &mut self.motes[i];
            m.gen = t.gen;
            m.hops = t.hops;
            m.age = t.age;
            m.mem = t.mem;
        }
    }

    fn traveler(m: &Mote, balance: u64, offset: u16) -> Traveler {
        Traveler {
            id: m.id,
            lineage: m.lineage,
            name: m.name.to_string(),
            author: m.author.to_string(),
            gen: m.gen,
            hops: m.hops,
            age: m.age,
            balance,
            mem: m.mem,
            genome: m.genome.src.clone(),
            offset,
        }
    }

    // ---- the tick ----------------------------------------------------------

    pub fn step(&mut self) {
        self.tick += 1;
        let target = if self.watched { SUN_MAX } else { 0 };
        if self.sun < target {
            self.sun = (self.sun + SUN_RAMP).min(target);
        } else if self.sun > target {
            self.sun = self.sun.saturating_sub(SUN_RAMP).max(target);
        }
        for (side, t) in std::mem::take(&mut self.arrivals) {
            self.admit(side, t);
        }
        self.shine();
        // Newborns do not think in their birth tick: the bound is taken
        // before the loop.
        let n = self.motes.len();
        for i in 0..n {
            if self.motes[i].leaving.is_none() {
                self.think(i);
            }
        }
        self.settle();
    }

    fn shine(&mut self) {
        let s = self.sun as i64;
        if s == 0 {
            return;
        }
        let (cx, cy) = spot(self.tick);
        let r2 = SPOT_RADIUS as i64 * SPOT_RADIUS as i64;
        let t = self.tick as i64;
        for y in 0..H as i64 {
            for x in 0..W as i64 {
                let d2 = (x - cx) * (x - cx) + (y - cy) * (y - cy);
                let bonus = if d2 < r2 {
                    SPOT_PEAK as i64 * (r2 - d2) / r2
                } else {
                    0
                };
                // Hundredths of light this tick; the remainder is dithered
                // across cells and ticks so the average is exact.
                let h = (SUN_BASE as i64 + bonus) * s;
                let mut add = h / 100;
                if (x * 37 + y * 101 + t * 13).rem_euclid(100) < h % 100 {
                    add += 1;
                }
                let c = y as usize * W + x as usize;
                let add = (add as u32).min(LIGHT_CAP - self.light[c]);
                self.light[c] += add;
                self.ledger.minted += add as u64;
            }
        }
    }

    fn think(&mut self, i: usize) {
        let genome = Rc::clone(&self.motes[i].genome);
        let tank = self.motes[i].balance.min(TANK_MAX);
        self.motes[i].balance -= tank;
        let run = {
            let mut host = TickHost {
                island: self,
                me: i,
                harvests: 0,
            };
            mind::run(&genome.prog, CAPS, &mut host, tank)
        };
        // A mind that ran dry mid-thought forfeits its whole tank.
        let used = if run.outcome == Outcome::OutOfFuel {
            tank
        } else {
            run.used
        };
        self.ledger.burned += used;
        let m = &mut self.motes[i];
        m.balance += tank - used;
        m.age += 1;
        m.last_used = used as u32;
    }

    fn settle(&mut self) {
        let motes = std::mem::take(&mut self.motes);
        let mut keep = Vec::with_capacity(motes.len());
        for m in motes {
            let c = m.y as usize * W + m.x as usize;
            if let Some(side) = m.leaving {
                let carry = m.balance.min(MAX_CARRY);
                self.deposit(c, m.balance - carry);
                self.ledger.exported += carry;
                self.counts.departures += 1;
                let offset = if side % 2 == 0 { m.x } else { m.y };
                let mut t = Island::traveler(&m, carry, offset);
                t.hops += 1;
                self.departures.push((side, t));
                self.event(EventKind::Departed(side), m.name.clone(), m.lineage);
                continue;
            }
            if m.balance == 0 {
                self.counts.deaths += 1;
                self.event(EventKind::Starved, m.name.clone(), m.lineage);
                continue;
            }
            if m.age >= MAX_AGE {
                self.counts.deaths += 1;
                self.deposit(c, m.balance);
                self.event(EventKind::Old, m.name.clone(), m.lineage);
                continue;
            }
            keep.push(m);
        }
        self.motes = keep;
        self.occ.fill(Self::EMPTY);
        for (i, m) in self.motes.iter().enumerate() {
            self.occ[m.y as usize * W + m.x as usize] = i as u32;
        }
    }

    // ---- acts, called from the capability table ------------------------------

    pub(crate) fn step_mote(&mut self, me: usize, dx: i64, dy: i64) -> i64 {
        if dx == 0 && dy == 0 {
            return 0;
        }
        let (x, y) = (self.motes[me].x as i64, self.motes[me].y as i64);
        let (nx, ny) = (x + dx, y + dy);
        let from = y as usize * W + x as usize;
        match Island::cell(nx, ny) {
            Some(c) => {
                if self.occ[c] != Self::EMPTY {
                    return 0;
                }
                self.occ[from] = Self::EMPTY;
                self.occ[c] = me as u32;
                self.motes[me].x = nx as u16;
                self.motes[me].y = ny as u16;
                1
            }
            None => {
                let side = if nx < 0 {
                    3
                } else if nx >= W as i64 {
                    1
                } else if ny < 0 {
                    0
                } else {
                    2
                };
                if self.portals[side as usize] == 0 || self.motes[me].balance < COST_CROSS {
                    return 0;
                }
                self.motes[me].balance -= COST_CROSS;
                self.ledger.burned += COST_CROSS;
                self.occ[from] = Self::EMPTY;
                self.motes[me].leaving = Some(side);
                2
            }
        }
    }

    pub(crate) fn harvest(&mut self, me: usize, nth: u32) -> i64 {
        let c = self.motes[me].y as usize * W + self.motes[me].x as usize;
        let take = self.light[c].min(HARVEST_MAX >> nth.min(31));
        self.light[c] -= take;
        self.motes[me].balance += take as u64;
        take as i64
    }

    /// The adjacent mote at (dx, dy), if any.
    fn neighbor(&self, me: usize, dx: i64, dy: i64) -> Option<usize> {
        if dx.abs() > 1 || dy.abs() > 1 || (dx == 0 && dy == 0) {
            return None;
        }
        let m = &self.motes[me];
        let c = Island::cell(m.x as i64 + dx, m.y as i64 + dy)?;
        self.mote_at(c).filter(|&o| o != me)
    }

    pub(crate) fn bite(&mut self, me: usize, dx: i64, dy: i64) -> i64 {
        let Some(o) = self.neighbor(me, dx, dy) else {
            return -1;
        };
        let take = self.motes[o].balance.min(BITE_MAX);
        let t = tithe(take);
        self.motes[o].balance -= take;
        self.motes[me].balance += take - t;
        self.ledger.burned += t;
        (take - t) as i64
    }

    pub(crate) fn give(&mut self, me: usize, dx: i64, dy: i64, amt: i64) -> i64 {
        let Some(o) = self.neighbor(me, dx, dy) else {
            return -1;
        };
        let amt = (amt.max(0) as u64).min(self.motes[me].balance);
        let t = tithe(amt);
        self.motes[me].balance -= amt;
        self.motes[o].balance += amt - t;
        self.ledger.burned += t;
        (amt - t) as i64
    }

    pub(crate) fn spawn(&mut self, me: usize) -> i64 {
        if self.motes[me].balance < ENDOW + SPAWN_RESERVE {
            return 0;
        }
        let (x, y) = (self.motes[me].x as i64, self.motes[me].y as i64);
        let start = self.rng.below(8) as usize;
        let free = (0..8).find_map(|k| {
            let (dx, dy) = NEIGHBORS[(start + k) % 8];
            Island::cell(x + dx, y + dy).filter(|&c| self.occ[c] == Self::EMPTY)
        });
        let Some(c) = free else { return 0 };
        let genome = if self.rng.chance(MUTATE_NUM, MUTATE_DEN) {
            match mutate(&self.motes[me].genome.src, &mut self.rng) {
                Some(g) => g,
                None => {
                    self.counts.miscarriages += 1;
                    return 0;
                }
            }
        } else {
            self.motes[me].genome.clone()
        };
        let id = self.next_id();
        let p = &self.motes[me];
        let (lineage, name, author, gen) = (p.lineage, p.name.clone(), p.author.clone(), p.gen + 1);
        self.motes[me].balance -= ENDOW;
        let i = self.place(c, id, lineage, name, author, genome, ENDOW);
        self.motes[i].gen = gen;
        self.counts.births += 1;
        1
    }

    // ---- reading -----------------------------------------------------------

    /// Light plus balances: every erg the island holds.
    pub fn held(&self) -> u64 {
        self.light.iter().map(|&l| l as u64).sum::<u64>()
            + self.motes.iter().map(|m| m.balance).sum::<u64>()
    }

    pub fn conserved(&self) -> bool {
        self.held() == self.ledger.expected()
    }

    /// Lineages present, most numerous first.
    pub fn census(&self) -> Vec<Lineage> {
        let mut by: BTreeMap<u64, Lineage> = BTreeMap::new();
        for m in &self.motes {
            let e = by.entry(m.lineage).or_insert_with(|| Lineage {
                id: m.lineage,
                name: m.name.clone(),
                author: m.author.clone(),
                count: 0,
                ergs: 0,
            });
            e.count += 1;
            e.ergs += m.balance;
        }
        let mut v: Vec<Lineage> = by.into_values().collect();
        v.sort_by(|a, b| b.count.cmp(&a.count).then(a.id.cmp(&b.id)));
        v
    }

    /// The receipt: a hash of the whole island.
    pub fn hash(&self) -> u64 {
        let mut h = Fnv::default();
        h.u64(self.id).u64(self.tick).u64(self.sun as u64);
        for p in self.portals {
            h.u64(p as u64);
        }
        for &l in &self.light {
            h.u64(l as u64);
        }
        for m in &self.motes {
            h.u64(m.id)
                .u64(m.lineage)
                .u64(m.balance)
                .u64(m.x as u64)
                .u64(m.y as u64)
                .u64(m.age as u64)
                .u64(m.genome.hash);
            for v in m.mem {
                h.i64(v);
            }
        }
        let l = self.ledger;
        h.u64(l.minted)
            .u64(l.granted)
            .u64(l.imported)
            .u64(l.exported)
            .u64(l.burned);
        h.finish()
    }
}
