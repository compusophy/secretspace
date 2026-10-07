//! Luciphon as a room the server hosts: one island at 30 Hz, a view per
//! browser, Lumens keyed by soul. The whole world is kept (schema 1,
//! `persist`); a page that leaves Dreams (or Lingers, in a fight), and
//! after a restart every Lumen that was awake waits 30 s, still and
//! untouchable, for its soul.

use std::collections::HashMap;

use engine::fixed::Fx;
use engine::room::{Outbox, Room, Who};

use crate::bots::{Brain, NAMES, RESIDENTS};
use crate::laws::{Laws, HZ, LAWS};
use crate::motion::Move;
use crate::proto::{ClaimInfo, Down, Up, PROTO};
use crate::view::Viewer;
use crate::world::World;

/// The oldest page protocol this server still speaks (3: the wand).
const OLDEST: u16 = 3;
/// Boards go out this often, in ticks.
const BOARD_EVERY: u32 = 2 * HZ;
/// A Lumen loaded from a save waits this long for its soul.
const HOLD: u32 = 30 * HZ;

pub struct Luciphon {
    world: World,
    viewers: HashMap<u32, Viewer>,
    whos: HashMap<u32, Who>,
    /// Lumens waiting for their souls, until a tick.
    held: HashMap<u16, u32>,
    build: u32,
    /// The claims as pages last heard of them.
    told: Vec<ClaimInfo>,
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
            told: Vec::new(),
        }
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    fn claims(&self) -> Vec<ClaimInfo> {
        self.world
            .claims
            .iter()
            .map(|c| ClaimInfo {
                id: c.id,
                hue: c.hue,
                name: c.name.clone(),
                hearth: c.hearth.map(|(x, y)| (x as i16, y as i16)),
            })
            .collect()
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
                if let Some(i) = self.world.index(id) {
                    self.world.lumens[i].linger = 0;
                    self.world.lumens[i].idle = 0;
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
                if who.soul == 0 {
                    self.world.spawn(name, 0, None)
                } else {
                    self.world.wake(who.soul, name)
                }
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
        out.send(conn, Down::Claims(self.claims()).encode());
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
            // Too old a page to play: a Welcome that says so, and no Lumen.
            Up::Join { proto } if proto < OLDEST => out.send(conn, self.welcome(0)),
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
            Up::Heart { act, arg } => {
                if you != 0 {
                    self.world.heart(you, act, arg);
                }
            }
            Up::Device { .. } => {}
        }
    }

    fn close(&mut self, conn: u32) {
        if let Some(v) = self.viewers.remove(&conn) {
            let taken = self.viewers.values().any(|o| o.you == v.you);
            if !self.held.contains_key(&v.you) && !taken {
                self.world.leave(v.you);
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
            self.world.dream(id);
        }
        for &id in self.held.keys() {
            if let Some(i) = self.world.index(id) {
                let l = &mut self.world.lumens[i];
                l.ghost = l.ghost.max(2);
                l.idle = 0;
                l.me.body.vx = Fx::ZERO;
                l.me.body.vy = Fx::ZERO;
                // Its timers stand still while it waits.
                l.wick_since = l.wick_since.wrapping_add(1);
            }
        }
        self.world.unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        self.world.step();
        let claims = self.claims();
        if claims != self.told {
            let msg = Down::Claims(claims.clone()).encode();
            for &conn in self.viewers.keys() {
                out.send(conn, msg.clone());
            }
            self.told = claims;
        }
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
        Some(self.world.save())
    }

    fn load(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        // A save from before the world was kept (v0.1) starts over.
        let Some(awake) = self.world.load(bytes)? else {
            return Ok(());
        };
        // Who was awake waits for their soul, still and untouchable.
        for soul in awake {
            let name = self
                .world
                .dreamers
                .get(&soul)
                .map(|d| d.name.clone())
                .unwrap_or_default();
            let id = self.world.wake(soul, &name);
            if let Some(i) = self.world.index(id) {
                self.world.lumens[i].me.body.mv = Move::Free;
            }
            self.held.insert(id, self.world.tick + HOLD);
        }
        Ok(())
    }

    fn schema(&self) -> u16 {
        crate::persist::SCHEMA
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
            ("dreaming", w.dreamers.len() as i64),
            (
                "hearths",
                w.claims.iter().filter(|c| c.hearth.is_some()).count() as i64,
            ),
            ("kindled", w.claims.iter().map(|c| c.tiles as i64).sum()),
            ("vaults", w.claims.iter().map(|c| c.vault.glim as i64).sum()),
            ("residents", residents),
            ("waiting", self.held.len() as i64),
            ("motes", w.motes.len() as i64),
            ("pickups", w.pickups.len() as i64),
        ];
        for k in [
            "gather",
            "resonant strike",
            "loop closed",
            "harvest",
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
