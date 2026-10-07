//! The page's copy of what it was told: the tiles of its known chunks,
//! every entity in sight as the server quantized it, and its own Lumen
//! exactly. `the_browser_rebuilds_every_tile_and_body_exactly` holds it to
//! the server's view, point for point.

use std::collections::{HashMap, HashSet};

use crate::proto::{Down, Ent, Ev, Own};
use crate::tiles::{Tile, Tiles, CHUNK};

#[derive(Clone, Debug, Default)]
pub struct Mirror {
    pub tiles: Tiles,
    pub chunks: HashSet<(i32, i32)>,
    pub ents: HashMap<u16, Ent>,
    pub own: Option<Own>,
    pub tick: u32,
    pub ack: u16,
    pub you: u16,
    /// What happened in the last frame.
    pub events: Vec<Ev>,
}

impl Mirror {
    /// Take in one message. Whether it was a frame.
    pub fn apply(&mut self, d: &Down) -> bool {
        match d {
            Down::Welcome { you, .. } => {
                if *you != 0 {
                    self.you = *you;
                }
                false
            }
            Down::Chunk { cx, cy, tiles } => {
                self.tiles.set_chunk(*cx as i32, *cy as i32, tiles);
                self.chunks.insert((*cx as i32, *cy as i32));
                false
            }
            Down::ChunkGone { cx, cy } => {
                let blank = vec![Tile::default(); (CHUNK * CHUNK) as usize];
                self.tiles.set_chunk(*cx as i32, *cy as i32, &blank);
                self.chunks.remove(&(*cx as i32, *cy as i32));
                false
            }
            Down::Frame(f) => {
                self.tick = f.tick;
                self.ack = f.ack;
                for id in &f.gone {
                    self.ents.remove(id);
                }
                for e in &f.new {
                    self.ents.insert(e.id, e.clone());
                }
                for (mask, e) in &f.moved {
                    if let Some(known) = self.ents.get_mut(&e.id) {
                        use crate::proto::field::*;
                        if mask & POS != 0 {
                            known.x = e.x;
                            known.y = e.y;
                        }
                        if mask & FACING != 0 {
                            known.facing = e.facing;
                        }
                        if mask & STATE != 0 {
                            known.state = e.state;
                        }
                        if mask & FLAME != 0 {
                            known.flame = e.flame;
                        }
                        if mask & GLIM != 0 {
                            known.glim = e.glim;
                        }
                        if mask & FLOW != 0 {
                            known.flow = e.flow;
                        }
                    }
                }
                for &(i, t) in &f.tiles {
                    if (i as usize) < self.tiles.t.len() {
                        self.tiles.t[i as usize] = t;
                    }
                }
                self.own = f.own;
                self.events = f.events.clone();
                true
            }
            _ => false,
        }
    }
}
