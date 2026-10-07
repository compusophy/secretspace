//! The arena as a room the server hosts: one world, a view per browser.

use std::collections::HashMap;

use engine::room::{Outbox, Room};

use crate::laws::{ARENA, TICK_HZ};
use crate::proto::{self, angle_from_u16, Up};
use crate::view::{self, Viewer};
use crate::world::World;

/// Boards (leaderboard, minimap) go out this often, in ticks.
const BOARD_EVERY: u32 = TICK_HZ / 2;

pub struct Arena {
    world: World,
    viewers: HashMap<u32, Viewer>,
}

impl Arena {
    pub fn new(seed: u64) -> Arena {
        Arena {
            world: World::new(seed),
            viewers: HashMap::new(),
        }
    }
}

impl Room for Arena {
    fn id(&self) -> &'static str {
        "arena"
    }

    fn hz(&self) -> u32 {
        TICK_HZ
    }

    fn open(&mut self, conn: u32, out: &mut Outbox) {
        out.send(conn, proto::hello(ARENA as u16, TICK_HZ as u8));
        self.viewers.insert(conn, Viewer::default());
    }

    fn message(&mut self, conn: u32, bytes: &[u8], _out: &mut Outbox) {
        let (Some(v), Some(up)) = (self.viewers.get_mut(&conn), Up::decode(bytes)) else {
            return;
        };
        match up {
            Up::Join { name } => {
                if self.world.find(v.you).is_none() {
                    v.you = self.world.spawn(&name, None);
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
        let people = self.viewers.len().min(u16::MAX as usize) as u16;
        for (&conn, v) in self.viewers.iter_mut() {
            out.send(conn, v.frame(&self.world));
            if board {
                out.send(conn, view::board(&self.world, v.you, people));
            }
        }
    }

    fn people(&self) -> usize {
        self.viewers.len()
    }
}
