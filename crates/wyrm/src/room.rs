//! wyrm as a room the server hosts: one world, a view per browser.

use std::collections::HashMap;

use engine::room::{Outbox, Room, Who};

use crate::bots::NAMES;
use crate::laws::{ARENA, TICK_HZ};
use crate::proto::{self, angle_from_u16, Up};
use crate::view::{self, Viewer};
use crate::world::World;

/// Boards (leaderboard, minimap) go out this often, in ticks.
const BOARD_EVERY: u32 = TICK_HZ / 2;

pub struct Wyrm {
    world: World,
    viewers: HashMap<u32, Viewer>,
    /// Who each connection is.
    whos: HashMap<u32, Who>,
}

impl Wyrm {
    pub fn new(seed: u64) -> Wyrm {
        Wyrm {
            world: World::new(seed),
            viewers: HashMap::new(),
            whos: HashMap::new(),
        }
    }

    fn watchers(&self) -> usize {
        self.whos.values().filter(|w| w.watch).count()
    }
}

impl Room for Wyrm {
    fn id(&self) -> &'static str {
        "wyrm"
    }

    fn hz(&self) -> u32 {
        TICK_HZ
    }

    fn open(&mut self, conn: u32, who: &Who, out: &mut Outbox) {
        out.send(conn, proto::hello(ARENA as u16, TICK_HZ as u8));
        self.viewers.insert(conn, Viewer::default());
        self.whos.insert(conn, who.clone());
    }

    fn who(&mut self, conn: u32, who: &Who) {
        if let Some(w) = self.whos.get_mut(&conn) {
            *w = who.clone();
        }
    }

    fn message(&mut self, conn: u32, bytes: &[u8], _out: &mut Outbox) {
        let (Some(who), Some(up)) = (self.whos.get(&conn), Up::decode(bytes)) else {
            return;
        };
        let Some(v) = self.viewers.get_mut(&conn) else {
            return;
        };
        match up {
            Up::Join { .. } | Up::Steer { .. } if who.watch => {}
            Up::Join { name } => {
                if self.world.find(v.you).is_some() {
                    return;
                }
                // A soul's snake may be waiting for it (after a restart),
                // or playing in another tab: it is theirs.
                if let Some(id) = self.world.claim(who.soul) {
                    v.you = id;
                    for (&c, other) in self.viewers.iter_mut() {
                        if c != conn && other.you == id {
                            other.you = 0;
                        }
                    }
                    return;
                }
                // A soul goes by the name the server gave it; only a page
                // from before souls names itself.
                let name = if who.soul != 0 { &who.name } else { &name };
                v.you = self.world.spawn(name, None);
                let soul = who.soul;
                if let Some(s) = self.world.snakes.iter_mut().find(|s| s.id == v.you) {
                    s.soul = soul;
                }
            }
            Up::Steer { angle, boost } => {
                self.world.steer(v.you, angle_from_u16(angle), boost);
            }
            Up::Screen { w, h } => v.screen = (w as f32, h as f32),
        }
    }

    fn close(&mut self, conn: u32) {
        if let Some(v) = self.viewers.remove(&conn) {
            self.world.remove(v.you);
        }
        self.whos.remove(&conn);
    }

    fn tick(&mut self, out: &mut Outbox) {
        for d in self.world.step() {
            if d.human {
                if let Some((&conn, v)) = self.viewers.iter_mut().find(|(_, v)| v.you == d.id) {
                    let by = d.killer.as_ref().map_or("", |k| k.1.as_str());
                    out.send(conn, proto::died(by, d.score));
                    v.you = 0;
                    v.watching = d.killer.as_ref().map_or(0, |k| k.0);
                }
            }
            if let Some((_, killer)) = &d.killer {
                let line = proto::feed(killer, &d.name, d.score);
                for &conn in self.viewers.keys() {
                    out.send(conn, line.clone());
                }
            }
        }
        let board = self.world.tick.is_multiple_of(BOARD_EVERY);
        let people = self.people().min(u16::MAX as usize) as u16;
        for (&conn, v) in self.viewers.iter_mut() {
            out.send(conn, v.frame(&self.world));
            if board {
                out.send(conn, view::board(&self.world, v.you, people));
            }
        }
    }

    fn people(&self) -> usize {
        self.viewers.len() - self.watchers()
    }

    fn playing(&self) -> usize {
        self.world.snakes.len()
    }

    fn save(&self) -> Option<Vec<u8>> {
        Some(self.world.save())
    }

    fn load(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        // An arena can always start over; a save it cannot read is let be.
        self.world.load(bytes);
        Ok(())
    }

    fn schema(&self) -> u16 {
        1
    }

    fn reserved(&self) -> &'static [&'static str] {
        NAMES
    }

    fn stats(&self) -> Vec<(&'static str, i64)> {
        let w = &self.world;
        let humans = w.humans() as i64;
        let held = w.snakes.iter().filter(|s| s.held_until != 0).count() as i64;
        vec![
            ("people", self.people() as i64),
            ("watchers", self.watchers() as i64),
            ("snakes", humans - held),
            ("waiting", held),
            ("bots", w.snakes.len() as i64 - humans),
        ]
    }
}
