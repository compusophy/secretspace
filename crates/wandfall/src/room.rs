//! Wandfall as a room the server hosts: one island, one match at a time,
//! a frame a tick for every page.

use std::collections::HashMap;

use engine::room::{Outbox, Room, Who};

use crate::laws::{BOT_NAMES, TICK_HZ};
use crate::proto::{self, Ev, Up, PROTO};
use crate::view;
use crate::world::{Event, Phase, World};

pub struct Wandfall {
    world: World,
    /// Each connection's wizard (0: not joined).
    you: HashMap<u32, u16>,
    whos: HashMap<u32, Who>,
}

impl Wandfall {
    pub fn new(seed: u64) -> Wandfall {
        Wandfall {
            world: World::new(seed),
            you: HashMap::new(),
            whos: HashMap::new(),
        }
    }

    fn roster(&self) -> Vec<u8> {
        let list: Vec<(u16, bool, String)> = self
            .world
            .players
            .iter()
            .map(|p| (p.id, p.bot, p.name.clone()))
            .collect();
        proto::roster(&list)
    }
}

fn wire(e: &Event) -> Ev {
    match *e {
        Event::Hit { by, to, amount } => Ev::Hit { by, to, amount },
        Event::Out { who, by, place } => Ev::Out { who, by, place },
        Event::Win { who } => Ev::Win { who },
        Event::Begin => Ev::Begin,
        Event::Lobby => Ev::Lobby,
    }
}

impl Room for Wandfall {
    fn id(&self) -> &'static str {
        "wandfall"
    }

    fn hz(&self) -> u32 {
        TICK_HZ
    }

    fn open(&mut self, conn: u32, who: &Who, out: &mut Outbox) {
        self.whos.insert(conn, who.clone());
        self.you.insert(conn, 0);
        // Watchers see the island and the match, never a wizard of their own.
        out.send(conn, proto::welcome(0, self.world.seed(), TICK_HZ as u8));
        out.send(conn, self.roster());
    }

    fn who(&mut self, conn: u32, who: &Who) {
        if let Some(w) = self.whos.get_mut(&conn) {
            *w = who.clone();
        }
        let id = self.you.get(&conn).copied().unwrap_or(0);
        if let Some(p) = self.world.players.iter_mut().find(|p| p.id == id) {
            if !who.name.is_empty() {
                p.name = who.name.clone();
                self.world.roster_dirty = true;
            }
        }
    }

    fn message(&mut self, conn: u32, bytes: &[u8], out: &mut Outbox) {
        let (Some(who), Some(up)) = (self.whos.get(&conn), Up::decode(bytes)) else {
            return;
        };
        if who.watch {
            return;
        }
        let you = self.you.get(&conn).copied().unwrap_or(0);
        match up {
            Up::Join { proto } => {
                if proto != PROTO {
                    // An older page: it reloads on a Welcome without a wizard.
                    out.send(conn, proto::welcome(0, self.world.seed(), TICK_HZ as u8));
                    return;
                }
                if you != 0 && self.world.find(you).is_some() {
                    out.send(conn, proto::welcome(you, self.world.seed(), TICK_HZ as u8));
                    return;
                }
                let name = if who.name.is_empty() {
                    format!("wizard {}", conn % 1000)
                } else {
                    who.name.clone()
                };
                let id = self.world.join(&name, who.soul);
                self.you.insert(conn, id);
                out.send(conn, proto::welcome(id, self.world.seed(), TICK_HZ as u8));
            }
            Up::Inputs(list) => {
                for i in list {
                    self.world.input(you, i);
                }
            }
        }
    }

    fn close(&mut self, conn: u32) {
        if let Some(id) = self.you.remove(&conn) {
            if id != 0 {
                let mut ev = Vec::new();
                self.world.leave(id, &mut ev);
            }
        }
        self.whos.remove(&conn);
    }

    fn tick(&mut self, out: &mut Outbox) {
        let ev = self.world.step();
        if self.world.roster_dirty {
            self.world.roster_dirty = false;
            let r = self.roster();
            for &conn in self.you.keys() {
                out.send(conn, r.clone());
            }
        }
        if !ev.is_empty() {
            let msg = proto::events(&ev.iter().map(wire).collect::<Vec<_>>());
            for &conn in self.you.keys() {
                out.send(conn, msg.clone());
            }
        }
        // Nobody here: nothing to say.
        if self.you.is_empty() {
            return;
        }
        for (&conn, &id) in &self.you {
            out.send(conn, view::frame(&self.world, id).encode());
        }
    }

    fn people(&self) -> usize {
        self.whos.values().filter(|w| !w.watch).count()
    }

    fn playing(&self) -> usize {
        self.world.players.len()
    }

    fn reserved(&self) -> &'static [&'static str] {
        BOT_NAMES
    }

    fn stats(&self) -> Vec<(&'static str, i64)> {
        let w = &self.world;
        vec![
            ("people", self.people() as i64),
            ("bots", w.players.iter().filter(|p| p.bot).count() as i64),
            ("alive", w.alive() as i64),
            ("fighting", (w.phase == Phase::Fight) as i64),
            ("matches", w.matches as i64),
        ]
    }
}
