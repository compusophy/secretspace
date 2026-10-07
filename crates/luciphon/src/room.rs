//! Luciphon as a room the server hosts: one island at 30 Hz, a view per
//! browser, Lumens keyed by soul. v0.1 keeps only Lumens across a restart
//! (schema 0): each waits 30 s, still and untouchable, for its soul.

use std::collections::HashMap;

use engine::fixed::Fx;
use engine::room::{Outbox, Room, Who};
use engine::snap::{sections, Sections};
use engine::wire::{Reader, Writer};

use crate::bots::{Brain, NAMES, RESIDENTS};
use crate::laws::{Laws, HZ, LAWS};
use crate::motion::Move;
use crate::proto::{Down, Up, PROTO};
use crate::view::Viewer;
use crate::world::World;

/// The oldest page protocol this server still speaks.
const OLDEST: u16 = 1;
/// Boards go out this often, in ticks.
const BOARD_EVERY: u32 = 2 * HZ;
/// A Lumen loaded from a save waits this long for its soul.
const HOLD: u32 = 30 * HZ;
const LUMENS: u16 = 1;

pub struct Luciphon {
    world: World,
    viewers: HashMap<u32, Viewer>,
    whos: HashMap<u32, Who>,
    /// Lumens waiting for their souls, until a tick.
    held: HashMap<u16, u32>,
    build: u32,
}

impl Luciphon {
    pub fn new(seed: u64) -> Luciphon {
        Luciphon::with(LAWS, seed)
    }

    pub fn with(laws: Laws, seed: u64) -> Luciphon {
        let world_seed = laws.world_seed as u64;
        let mut world = World::new(laws, world_seed);
        world.rng = engine::rng::Rng::new(seed);
        for k in 0..RESIDENTS {
            let brain = Brain::new(&mut world.rng);
            world.spawn(NAMES[k % NAMES.len()], 0, Some(brain));
        }
        Luciphon {
            world,
            viewers: HashMap::new(),
            whos: HashMap::new(),
            held: HashMap::new(),
            build: 0,
        }
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    fn welcome(&self, you: u16) -> Vec<u8> {
        Down::Welcome {
            proto: PROTO,
            oldest: OLDEST,
            build: self.build,
            laws: self.world.laws.encode(),
            tick: self.world.tick,
            hz: HZ as u8,
            seed: self.world.seed,
            you,
        }
        .encode()
    }

    fn join(&mut self, conn: u32, out: &mut Outbox) {
        let Some(who) = self.whos.get(&conn).cloned() else {
            return;
        };
        if who.watch {
            return;
        }
        let you = self.viewers.get(&conn).map_or(0, |v| v.you);
        if self.world.find(you).is_some() {
            return;
        }
        // A soul's Lumen: waiting after a restart, or in another tab.
        let mine = (who.soul != 0)
            .then(|| {
                self.world
                    .lumens
                    .iter()
                    .find(|l| l.soul == who.soul && l.bot.is_none())
            })
            .flatten()
            .map(|l| l.id);
        let id = match mine {
            Some(id) => {
                if self.held.remove(&id).is_some() {
                    if let Some(i) = self.world.index(id) {
                        self.world.lumens[i].ghost = 2 * HZ;
                    }
                }
                for (&c, v) in self.viewers.iter_mut() {
                    if c != conn && v.you == id {
                        v.you = 0;
                        v.watch = true;
                        out.send(c, Down::Elsewhere.encode());
                    }
                }
                id
            }
            None => {
                let name = if who.name.is_empty() {
                    "lumen"
                } else {
                    &who.name
                };
                self.world.spawn(name, who.soul, None)
            }
        };
        if let Some(v) = self.viewers.get_mut(&conn) {
            v.you = id;
            v.watch = false;
        }
        out.send(conn, self.welcome(id));
    }
}

impl Room for Luciphon {
    fn id(&self) -> &'static str {
        "luciphon"
    }

    fn hz(&self) -> u32 {
        HZ
    }

    fn open(&mut self, conn: u32, who: &Who, out: &mut Outbox) {
        out.send(conn, self.welcome(0));
        let mut v = Viewer::default();
        v.watch = who.watch;
        self.viewers.insert(conn, v);
        self.whos.insert(conn, who.clone());
    }

    fn who(&mut self, conn: u32, who: &Who) {
        if let Some(w) = self.whos.get_mut(&conn) {
            *w = who.clone();
        }
        let you = self.viewers.get(&conn).map_or(0, |v| v.you);
        if let Some(i) = self.world.index(you) {
            if !who.name.is_empty() {
                self.world.lumens[i].name = who.name.clone();
            }
        }
    }

    fn message(&mut self, conn: u32, bytes: &[u8], out: &mut Outbox) {
        let Some(up) = Up::decode(bytes) else {
            return;
        };
        let you = self.viewers.get(&conn).map_or(0, |v| v.you);
        match up {
            Up::Join { .. } => self.join(conn, out),
            Up::Input { seq, it } => {
                if you != 0 {
                    self.world.intend(you, seq, it);
                }
            }
            Up::Ping { t, rtt } => {
                out.send(conn, Down::Pong { t }.encode());
                if let Some(i) = self.world.index(you) {
                    self.world.lumens[i].rtt = rtt as u32 * 4;
                }
            }
            Up::Heart { .. } | Up::Device { .. } => {}
        }
    }

    fn close(&mut self, conn: u32) {
        if let Some(v) = self.viewers.remove(&conn) {
            if !self.held.contains_key(&v.you) {
                self.world.remove(v.you);
            }
        }
        self.whos.remove(&conn);
    }

    fn tick(&mut self, out: &mut Outbox) {
        // Lumens waiting for souls that did not come back are let go.
        let now = self.world.tick;
        let gone: Vec<u16> = self
            .held
            .iter()
            .filter(|&(_, &until)| until <= now)
            .map(|(&id, _)| id)
            .collect();
        for id in gone {
            self.held.remove(&id);
            self.world.remove(id);
        }
        for &id in self.held.keys() {
            if let Some(i) = self.world.index(id) {
                self.world.lumens[i].ghost = self.world.lumens[i].ghost.max(2);
                self.world.lumens[i].me.body.vx = Fx::ZERO;
                self.world.lumens[i].me.body.vy = Fx::ZERO;
            }
        }
        self.world.step();
        let board = self.world.tick.is_multiple_of(BOARD_EVERY);
        let people = self.people().min(u16::MAX as usize) as u16;
        let awake = self.world.lumens.len().min(u16::MAX as usize) as u16;
        let mut msgs = Vec::new();
        for (&conn, v) in self.viewers.iter_mut() {
            msgs.clear();
            v.frame(&self.world, &mut msgs);
            for m in msgs.drain(..) {
                out.send(conn, m);
            }
            if board {
                out.send(conn, Down::Board { people, awake }.encode());
            }
        }
    }

    fn people(&self) -> usize {
        self.whos.values().filter(|w| !w.watch).count()
    }

    fn playing(&self) -> usize {
        self.world.lumens.len()
    }

    fn backlog(&self) -> usize {
        120
    }

    fn reserved(&self) -> &'static [&'static str] {
        NAMES
    }

    fn save(&self) -> Option<Vec<u8>> {
        let people: Vec<_> = self
            .world
            .lumens
            .iter()
            .filter(|l| l.bot.is_none() && l.soul != 0 && l.alive())
            .collect();
        let mut w = Writer::default();
        w.u16(people.len() as u16);
        for l in people {
            let b = &l.me.body;
            w.u64(l.soul).str(&l.name).u8(l.hue).i32(b.x.0).i32(b.y.0);
            w.i32(l.flame).u32(l.glim).u32(l.wood).u32(l.stone);
        }
        let mut s = Sections::default();
        s.add(LUMENS, &w.0);
        Some(s.finish())
    }

    fn load(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        // v0.1 can always start over: a save it cannot read is let be.
        let Ok(secs) = sections(bytes) else {
            return Ok(());
        };
        let Some(&(_, b)) = secs.iter().find(|s| s.0 == LUMENS) else {
            return Ok(());
        };
        let mut r = Reader::new(b);
        let Some(n) = r.u16() else {
            return Ok(());
        };
        for _ in 0..n.min(512) {
            let (Some(soul), Some(name), Some(hue), Some(x), Some(y)) =
                (r.u64(), r.str(), r.u8(), r.i32(), r.i32())
            else {
                break;
            };
            let (Some(flame), Some(glim), Some(wood), Some(stone)) =
                (r.i32(), r.u32(), r.u32(), r.u32())
            else {
                break;
            };
            let (x, y) = (Fx(x), Fx(y));
            let t = self.world.tiles.under(x, y);
            let id = self.world.spawn(&name, soul, None);
            if let Some(i) = self.world.index(id) {
                let l = &mut self.world.lumens[i];
                l.hue = hue;
                if !t.void() && !t.solid() {
                    l.me.body = crate::motion::Body::at(x, y, &self.world.laws);
                }
                l.flame = flame.clamp(1, self.world.laws.flame);
                (l.glim, l.wood, l.stone) =
                    (glim.min(1 << 20), wood.min(1 << 20), stone.min(1 << 20));
                l.me.body.mv = Move::Free;
                self.held.insert(id, self.world.tick + HOLD);
            }
        }
        Ok(())
    }

    fn still(&mut self, _out: &mut Outbox) {
        // Everyone holds where they are; the save that follows keeps them.
        for l in &mut self.world.lumens {
            l.me.body.vx = Fx::ZERO;
            l.me.body.vy = Fx::ZERO;
        }
    }

    fn stats(&self) -> Vec<(&'static str, i64)> {
        let w = &self.world;
        let residents = w.lumens.iter().filter(|l| l.bot.is_some()).count() as i64;
        let mut out = vec![
            ("people", self.people() as i64),
            ("lumens", w.lumens.len() as i64),
            ("residents", residents),
            ("waiting", self.held.len() as i64),
            ("motes", w.motes.len() as i64),
            ("pickups", w.pickups.len() as i64),
        ];
        for k in [
            "landed strike",
            "knocked down",
            "fell to the Dark",
            "fell to a strike",
            "fell to thorns",
            "fell to a wall",
        ] {
            out.push((k, w.tally.get(k).copied().unwrap_or(0) as i64));
        }
        out
    }
}
