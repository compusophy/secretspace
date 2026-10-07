//! What one browser is told: the chunks around it (one keyframe a tick,
//! nearest first; dropped past two chunks), the entities within its sight
//! (entering at the sight radius, leaving a tile past it), each sent whole
//! once and then only what changed, its own Lumen exactly, and what
//! happened. The server sends nothing a browser could not see.

use std::collections::{HashMap, HashSet};

use engine::fixed::{atan2, len, Fx};

use crate::proto::{field, kind, q, Down, Ent, Ev, Frame, Own};
use crate::tiles::{Tiles, CHUNKS};
use crate::world::{Event, Lumen, World};

#[derive(Default)]
pub struct Viewer {
    /// Its Lumen (0: none yet, or it only watches).
    pub you: u16,
    pub watch: bool,
    /// Where it looks from.
    pub centre: (Fx, Fx),
    pub chunks: HashSet<(i32, i32)>,
    known: HashMap<u16, Ent>,
}

/// Glim carried, as steps a glance can read.
pub fn glim_step(g: u32) -> u8 {
    match g {
        0 => 0,
        1..=4 => 1,
        5..=14 => 2,
        15..=49 => 3,
        50..=149 => 4,
        _ => 5,
    }
}

pub fn lumen_ent(l: &Lumen, laws: &crate::laws::Laws) -> Ent {
    let b = &l.me.body;
    Ent {
        id: l.id,
        kind: kind::LUMEN,
        x: q(b.x),
        y: q(b.y),
        facing: b.facing,
        state: b.mv as u8
            | (l.me.act.act.code() & 7) << 3
            | ((l.ghost > 0) as u8) << 6
            | ((l.down > 0) as u8) << 7,
        flame: (l.flame.max(0) / 1000).min(255) as u8,
        glim: glim_step(l.glim),
        hue: l.hue,
        // Flow, and a newcomer's Sparks in the high bit.
        flow: l.flow | (l.spark(laws) as u8) << 7,
        name: l.name.clone(),
    }
}

pub fn own(l: &Lumen) -> Own {
    Own {
        me: l.me,
        flame: l.flame,
        glim: l.glim,
        wood: l.wood,
        stone: l.stone,
        flow: l.flow,
        ghost: l.ghost.min(u16::MAX as u32) as u16,
        down: l.down.min(255) as u8,
        descent: l.descent.min(255) as u8,
        killer: l.killer,
        wheat: l.wheat,
        kindle: l.kindle,
        build: l.build.unwrap_or(0),
        channel: l.channel.map_or(0, |c| c.2.min(255) as u8),
        home: l.home,
        levels: l.xp.map(|x| crate::build::level(x) as u8),
    }
}

/// Every entity in the world as a page would know it, with where it is.
pub fn ents(w: &World) -> Vec<(Ent, Fx, Fx)> {
    let mut out = Vec::new();
    for l in w.lumens.iter().filter(|l| l.alive()) {
        out.push((lumen_ent(l, &w.laws), l.me.body.x, l.me.body.y));
    }
    for m in &w.motes {
        let hue = w.find(m.owner).map_or(0, |o| o.hue);
        let e = Ent {
            id: m.id,
            kind: kind::MOTE,
            x: q(m.x),
            y: q(m.y),
            facing: atan2(m.vy, m.vx),
            hue,
            glim: m.pierce as u8,
            ..Ent::default()
        };
        out.push((e, m.x, m.y));
    }
    for p in &w.pickups {
        let e = Ent {
            id: p.id,
            kind: kind::PICKUP,
            x: q(p.x),
            y: q(p.y),
            glim: p.glim.min(255) as u8,
            ..Ent::default()
        };
        out.push((e, p.x, p.y));
    }
    out
}

fn mask(old: &Ent, new: &Ent) -> u8 {
    let mut m = 0;
    if (old.x, old.y) != (new.x, new.y) {
        m |= field::POS;
    }
    if old.facing != new.facing {
        m |= field::FACING;
    }
    if old.state != new.state {
        m |= field::STATE;
    }
    if old.flame != new.flame {
        m |= field::FLAME;
    }
    if old.glim != new.glim {
        m |= field::GLIM;
    }
    if old.flow != new.flow {
        m |= field::FLOW;
    }
    m
}

fn event(e: &Event) -> Ev {
    let (kind, a, b, n) = match *e {
        Event::Strike { id } => (1, id, 0, 0),
        Event::Hit {
            by,
            on,
            damage,
            big,
        } => (2, by, on, damage | (big as u8) << 7),
        Event::Whiff { id } => (3, id, 0, 0),
        Event::Sling { id, gold } => (4, id, 0, gold as u8),
        Event::Spin { id } => (5, id, 0, 0),
        Event::Dash { id } => (6, id, 0, 0),
        Event::Kick { id } => (7, id, 0, 0),
        Event::Save { id } => (8, id, 0, 0),
        Event::Dodge { id } => (9, id, 0, 0),
        Event::Thorns { id } => (10, id, 0, 0),
        Event::Down { id } => (11, id, 0, 0),
        Event::Gutter { id, by } => (12, id, by, 0),
        Event::Return { id } => (13, id, 0, 0),
        Event::Gather {
            id,
            idx,
            resonant,
            out,
        } => (14, id, idx, resonant as u8 | (out as u8) << 1),
        Event::Banked { id } => (15, id, 0, 0),
        Event::Loop { id, n } => (16, id, n, 0),
        Event::Snuffed { id } => (17, id, 0, 0),
        Event::Kindle { id, on } => (18, id, 0, on as u8),
        Event::Emote { id, what } => (19, id, 0, what),
        Event::Level { id, skill, level } => (20, id, level as u16, skill),
        Event::Placed { id, idx, piece } => (21, id, idx, piece),
        Event::Removed { id, idx } => (22, id, idx, 0),
        Event::Refused { id } => (23, id, 0, 0),
        Event::Dream { id } => (24, id, 0, 0),
    };
    Ev { kind, a, b, n }
}

impl Viewer {
    /// Everything this browser should be told this tick, in order: at most
    /// one chunk keyframe, chunks it no longer needs, then the frame.
    pub fn frame(&mut self, w: &World, out: &mut Vec<Vec<u8>>) {
        let me = w.find(self.you);
        if let Some(l) = me.filter(|l| l.alive()) {
            self.centre = l.pos();
        } else if self.watch || me.is_none() {
            self.centre = (Fx::ZERO, Fx::ZERO);
        }
        let sight = w.laws.sight;
        let (cx, cy) = (self.centre.0.floor(), self.centre.1.floor());

        // Chunks: wanted within sight + 4; kept within 2 chunks.
        let half = sight + 4;
        let (c0x, c0y) = Tiles::chunk_of(cx - half, cy - half);
        let (c1x, c1y) = Tiles::chunk_of(cx + half, cy + half);
        let (hx, hy) = Tiles::chunk_of(cx, cy);
        let mut want: Vec<(i32, i32)> = Vec::new();
        for y in c0y.max(0)..=c1y.min(CHUNKS - 1) {
            for x in c0x.max(0)..=c1x.min(CHUNKS - 1) {
                if !self.chunks.contains(&(x, y)) {
                    want.push((x, y));
                }
            }
        }
        want.sort_by_key(|&(x, y)| (x - hx).pow(2) + (y - hy).pow(2));
        if let Some(&(x, y)) = want.first() {
            self.chunks.insert((x, y));
            let tiles = w.tiles.chunk(x, y);
            out.push(
                Down::Chunk {
                    cx: x as u8,
                    cy: y as u8,
                    tiles,
                }
                .encode(),
            );
        }
        let far: Vec<(i32, i32)> = self
            .chunks
            .iter()
            .copied()
            .filter(|&(x, y)| (x - hx).abs() > 2 || (y - hy).abs() > 2)
            .collect();
        for (x, y) in far {
            self.chunks.remove(&(x, y));
            out.push(
                Down::ChunkGone {
                    cx: x as u8,
                    cy: y as u8,
                }
                .encode(),
            );
        }

        // Entities in sight.
        let mut f = Frame {
            tick: w.tick,
            ..Frame::default()
        };
        if let Some(l) = me {
            f.ack = l.ack;
            f.repeats = l.repeats;
            f.queue = l.queue.len().min(127) as u8 | (l.dropped as u8) << 7;
            f.own = Some(own(l));
        }
        let (enter, leave) = (Fx::int(sight), Fx::int(sight + 1));
        let mut seen = HashSet::new();
        for (e, x, y) in ents(w) {
            let d = len(x.sub(self.centre.0), y.sub(self.centre.1));
            let known = self.known.contains_key(&e.id);
            let visible = d <= enter || (known && d <= leave) || e.id == self.you;
            if !visible {
                continue;
            }
            seen.insert(e.id);
            match self.known.get_mut(&e.id) {
                Some(old) if old.kind == e.kind => {
                    let m = mask(old, &e);
                    if m != 0 {
                        f.moved.push((m, e.clone()));
                        *old = e;
                    }
                }
                _ => {
                    if self.known.contains_key(&e.id) {
                        f.gone.push(e.id);
                    }
                    f.new.push(e.clone());
                    self.known.insert(e.id, e);
                }
            }
        }
        let gone: Vec<u16> = self
            .known
            .keys()
            .copied()
            .filter(|id| !seen.contains(id))
            .collect();
        for id in gone {
            self.known.remove(&id);
            f.gone.push(id);
        }
        // Tiles that changed, in chunks this browser knows.
        f.tiles = w
            .dirty
            .iter()
            .filter(|&&i| {
                let (x, y) = Tiles::at_index(i as usize);
                self.chunks.contains(&Tiles::chunk_of(x, y))
            })
            .map(|&i| (i, w.tiles.t[i as usize]))
            .collect();
        f.events = w
            .events
            .iter()
            .map(event)
            .filter(|e| self.known.contains_key(&e.a) || e.a == self.you)
            .collect();
        out.push(Down::Frame(Box::new(f)).encode());
    }

    /// What this viewer knows of an entity (for tests).
    pub fn knows(&self, id: u16) -> Option<&Ent> {
        self.known.get(&id)
    }
}
