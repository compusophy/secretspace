//! The world kept (§12), schema 1: sections META, TILES, NODES, PIECES
//! (crops), HEARTHS (claims and their vaults), LUMENS (every soul's Lumen,
//! asleep or awake at the save), DROPS, OUTLINES. Unknown sections are
//! skipped and missing ones are empty; a known section that will not read
//! fails the whole load. Schema 1 is frozen: `world-s1.snap` must always
//! load (`every_old_world_still_loads`); later schemas migrate from it.

use engine::fixed::Fx;
use engine::snap::{sections, Sections};
use engine::wire::{Reader, Writer};

use crate::gather::{Crop, Node};
use crate::land::{Bag, Claim};
use crate::life::Dreamer;
use crate::tiles::{rle, unrle, CHUNKS};
use crate::world::{Pickup, World};

pub const SCHEMA: u16 = 1;

const META: u16 = 10;
const TILES: u16 = 11;
const NODES: u16 = 12;
const PIECES: u16 = 13;
const HEARTHS: u16 = 14;
const LUMENS: u16 = 15;
const DROPS: u16 = 16;
const OUTLINES: u16 = 17;
/// Far more than a world holds; a count past it is not ours.
const MOST: usize = 1 << 16;

fn bag(w: &mut Writer, b: &Bag) {
    w.u32(b.glim).u32(b.wood).u32(b.stone).u32(b.wheat);
}

fn get_bag(r: &mut Reader) -> Option<Bag> {
    Some(Bag {
        glim: r.u32()?,
        wood: r.u32()?,
        stone: r.u32()?,
        wheat: r.u32()?,
    })
}

fn dreamer(w: &mut Writer, soul: u64, d: &Dreamer, awake: bool) {
    w.u64(soul)
        .u8(awake as u8)
        .str(&d.name)
        .u8(d.hue)
        .i32(d.x.0)
        .i32(d.y.0)
        .i32(d.flame);
    bag(
        w,
        &Bag {
            glim: d.glim,
            wood: d.wood,
            stone: d.stone,
            wheat: d.wheat,
        },
    );
    w.u16(d.claim);
    for x in d.xp {
        w.u32(x);
    }
    w.u32(d.played).u8(d.sparks_off as u8).u8(d.home as u8);
}

fn get_dreamer(r: &mut Reader) -> Option<(u64, bool, Dreamer)> {
    let (soul, awake) = (r.u64()?, r.u8()? != 0);
    let (name, hue) = (engine::who::clean_name(&r.str()?), r.u8()?);
    let (x, y, flame) = (Fx(r.i32()?), Fx(r.i32()?), r.i32()?);
    let b = get_bag(r)?;
    let claim = r.u16()?;
    let mut xp = [0u32; 7];
    for v in &mut xp {
        *v = r.u32()?;
    }
    let (played, sparks_off, home) = (r.u32()?, r.u8()? != 0, r.u8()? != 0);
    Some((
        soul,
        awake,
        Dreamer {
            name,
            hue,
            x,
            y,
            flame,
            glim: b.glim,
            wood: b.wood,
            stone: b.stone,
            wheat: b.wheat,
            claim,
            xp,
            played,
            sparks_off,
            home,
            deaths: Vec::new(),
        },
    ))
}

impl World {
    /// The world as a snapshot payload.
    pub fn save(&self) -> Vec<u8> {
        let mut s = Sections::default();
        let mut w = Writer::default();
        w.u16(SCHEMA).u32(self.tick).u64(self.seed);
        s.add(META, &w.0);

        let mut w = Writer::default();
        for cy in 0..CHUNKS {
            for cx in 0..CHUNKS {
                rle(&self.tiles.chunk(cx, cy), &mut w);
            }
        }
        s.add(TILES, &w.0);

        let mut w = Writer::default();
        w.u32(self.nodes.len() as u32);
        let mut nodes: Vec<_> = self.nodes.iter().collect();
        nodes.sort_by_key(|n| *n.0);
        for (&i, n) in nodes {
            w.u16(i).u32(n.left).u32(n.regrow);
        }
        s.add(NODES, &w.0);

        let mut w = Writer::default();
        w.u32(self.crops.len() as u32);
        let mut crops: Vec<_> = self.crops.iter().collect();
        crops.sort_by_key(|c| *c.0);
        for (&i, c) in crops {
            w.u16(i).u32(c.planted).u16(c.claim);
        }
        s.add(PIECES, &w.0);

        let mut w = Writer::default();
        w.u32(self.claims.len() as u32);
        for c in &self.claims {
            w.u16(c.id).u64(c.soul).str(&c.name).u8(c.hue);
            let (has, (hx, hy)) = (c.hearth.is_some(), c.hearth.unwrap_or((0, 0)));
            w.u8(has as u8).i32(hx).i32(hy);
            bag(&mut w, &c.vault);
            w.u32(c.tiles)
                .u64(c.seen)
                .u8(c.cold as u8)
                .u64(c.acc as u64)
                .u32(c.fade_at);
        }
        s.add(HEARTHS, &w.0);

        // Every soul's Lumen: the sleepers, and the awake as they stand.
        let mut w = Writer::default();
        let awake: Vec<_> = self
            .lumens
            .iter()
            .filter(|l| l.soul != 0 && l.bot.is_none())
            .collect();
        w.u32((self.dreamers.len() + awake.len()) as u32);
        let mut sleepers: Vec<_> = self.dreamers.iter().collect();
        sleepers.sort_by_key(|d| *d.0);
        for (&soul, d) in sleepers {
            dreamer(&mut w, soul, d, false);
        }
        for l in awake {
            dreamer(&mut w, l.soul, &Dreamer::of(l), true);
        }
        s.add(LUMENS, &w.0);

        let mut w = Writer::default();
        w.u32(self.pickups.len() as u32);
        for p in &self.pickups {
            w.i32(p.x.0).i32(p.y.0);
            bag(
                &mut w,
                &Bag {
                    glim: p.glim,
                    wood: p.wood,
                    stone: p.stone,
                    wheat: p.wheat,
                },
            );
        }
        s.add(DROPS, &w.0);

        let mut w = Writer::default();
        w.u32(self.outlines.len() as u32);
        let mut outlines: Vec<_> = self.outlines.iter().collect();
        outlines.sort_by_key(|o| *o.0);
        for (&i, &(c, until)) in outlines {
            w.u16(i).u16(c).u64(until);
        }
        s.add(OUTLINES, &w.0);
        s.finish()
    }

    /// Load a snapshot payload into this (fresh) world. The souls awake at
    /// the save, to hold for their pages. Err for bytes that should have
    /// loaded; Ok(None) for a save from before schema 1 (a world that can
    /// start over).
    pub fn load(&mut self, payload: &[u8]) -> Result<Option<Vec<u64>>, &'static str> {
        let secs = sections(payload)?;
        let get = |tag: u16| secs.iter().find(|s| s.0 == tag).map(|s| s.1);
        let Some(meta) = get(META) else {
            return Ok(None);
        };
        let mut r = Reader::new(meta);
        let schema = r.u16().ok_or("short meta")?;
        if schema != SCHEMA {
            return Err("unknown schema");
        }
        let tick = r.u32().ok_or("short meta")?;
        let seed = r.u64().ok_or("short meta")?;

        let mut tiles = crate::tiles::Tiles::default();
        if let Some(b) = get(TILES) {
            let mut r = Reader::new(b);
            for cy in 0..CHUNKS {
                for cx in 0..CHUNKS {
                    let chunk = unrle(&mut r).ok_or("bad tiles")?;
                    tiles.set_chunk(cx, cy, &chunk);
                }
            }
        }
        let count = |r: &mut Reader, size: usize| -> Result<usize, &'static str> {
            let n = r.u32().ok_or("short count")? as usize;
            if n > MOST || r.room(n, size).is_none() {
                return Err("bad count");
            }
            Ok(n)
        };
        let mut nodes = std::collections::HashMap::new();
        if let Some(b) = get(NODES) {
            let mut r = Reader::new(b);
            for _ in 0..count(&mut r, 10)? {
                let (i, left, regrow) = (r.u16(), r.u32(), r.u32());
                let (Some(i), Some(left), Some(regrow)) = (i, left, regrow) else {
                    return Err("bad node");
                };
                nodes.insert(i, Node { left, regrow });
            }
        }
        let mut crops = std::collections::HashMap::new();
        if let Some(b) = get(PIECES) {
            let mut r = Reader::new(b);
            for _ in 0..count(&mut r, 8)? {
                let (Some(i), Some(planted), Some(claim)) = (r.u16(), r.u32(), r.u16()) else {
                    return Err("bad crop");
                };
                crops.insert(i, Crop { planted, claim });
            }
        }
        let mut claims = Vec::new();
        if let Some(b) = get(HEARTHS) {
            let mut r = Reader::new(b);
            for _ in 0..count(&mut r, 40)? {
                let c = (|| {
                    let (id, soul, name, hue) = (r.u16()?, r.u64()?, r.str()?, r.u8()?);
                    let (has, hx, hy) = (r.u8()? != 0, r.i32()?, r.i32()?);
                    let vault = get_bag(&mut r)?;
                    Some(Claim {
                        id,
                        soul,
                        name: engine::who::clean_name(&name),
                        hue,
                        hearth: has.then_some((hx, hy)),
                        vault,
                        tiles: r.u32()?,
                        seen: r.u64()?,
                        cold: r.u8()? != 0,
                        acc: r.u64()? as i64,
                        fade_at: r.u32()?,
                    })
                })()
                .ok_or("bad claim")?;
                claims.push(c);
            }
        }
        let mut dreamers = std::collections::HashMap::new();
        let mut awake = Vec::new();
        if let Some(b) = get(LUMENS) {
            let mut r = Reader::new(b);
            for _ in 0..count(&mut r, 50)? {
                let (soul, was_awake, d) = get_dreamer(&mut r).ok_or("bad lumen")?;
                if was_awake {
                    awake.push(soul);
                }
                dreamers.insert(soul, d);
            }
        }
        let mut pickups = Vec::new();
        if let Some(b) = get(DROPS) {
            let mut r = Reader::new(b);
            for _ in 0..count(&mut r, 24)? {
                let (Some(x), Some(y)) = (r.i32(), r.i32()) else {
                    return Err("bad drop");
                };
                let b = get_bag(&mut r).ok_or("bad drop")?;
                pickups.push((Fx(x), Fx(y), b));
            }
        }
        let mut outlines = std::collections::HashMap::new();
        if let Some(b) = get(OUTLINES) {
            let mut r = Reader::new(b);
            for _ in 0..count(&mut r, 12)? {
                let (Some(i), Some(c), Some(until)) = (r.u16(), r.u16(), r.u64()) else {
                    return Err("bad outline");
                };
                outlines.insert(i, (c, until));
            }
        }
        // All read: now it is this world.
        self.tick = tick;
        self.seed = seed;
        if get(TILES).is_some() {
            self.tiles = tiles;
        }
        self.nodes = nodes;
        self.crops = crops;
        self.claims = claims;
        self.dreamers = dreamers;
        self.outlines = outlines;
        self.pickups.clear();
        for (x, y, b) in pickups {
            let id = self.new_id();
            self.pickups.push(Pickup {
                id,
                x,
                y,
                glim: b.glim,
                wood: b.wood,
                stone: b.stone,
                wheat: b.wheat,
            });
        }
        Ok(Some(awake))
    }
}
