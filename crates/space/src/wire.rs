//! What crosses between tabs. A small binary format, versioned, bounded
//! everywhere: every byte comes from a stranger, so decoding never panics
//! and never allocates past the limits below.

use crate::census::{Exact, Gossip, Sketch, Tally, EXACT, EXACT_WORLD, GOSSIP_LINEAGES, REGS};
use crate::island::Traveler;
use crate::laws::{MAX_SOURCE, MEM_SLOTS};

pub const MAGIC: [u8; 2] = *b"SS";
pub const VERSION: u8 = 1;
const MAX_PLACE: usize = 48;
const MAX_NAME: usize = 32;
pub const MAX_CENSUS: usize = 24;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hello {
    /// Where the island is, as its person's time zone names it.
    pub place: String,
    /// Minutes east of UTC.
    pub tz_min: i16,
    pub sun: u8,
    pub asleep: bool,
    /// How many of its portals are shut.
    pub free: u8,
    pub pop: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CensusEntry {
    pub lineage: u64,
    pub count: u32,
    pub name: String,
    pub author: String,
}

/// One lineage across the whole world, as the relay counts it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldEntry {
    pub lineage: u64,
    /// Islands it is alive on.
    pub tabs: u32,
    pub count: u32,
    pub name: String,
    pub author: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    Hello(Hello),
    /// "Link my side `side` to you."
    LinkReq {
        side: u8,
    },
    /// "Linked: my side `side` to your side `yours`."
    LinkAck {
        side: u8,
        yours: u8,
    },
    LinkNo,
    /// "My side `side` is no longer linked to you."
    Unlink {
        side: u8,
    },
    /// A mote crossing from the sender's side `side`.
    Mote {
        seq: u64,
        side: u8,
        traveler: Traveler,
    },
    MoteAck {
        seq: u64,
    },
    Census(Vec<CensusEntry>),
    Bye,
    /// The relay's count of the whole world: islands online, and the
    /// lineages alive on the most of them.
    World {
        islands: u32,
        entries: Vec<WorldEntry>,
    },
    /// An island's census sketches, for its neighbours to merge.
    Gossip(Gossip),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Envelope {
    pub from: u64,
    /// 0: everyone who can hear.
    pub to: u64,
    pub msg: Msg,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireError {
    Short,
    Magic,
    Version,
    Kind,
    TooLong,
    Utf8,
}

struct W(Vec<u8>);

impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn str(&mut self, s: &str, max: usize) {
        let mut end = s.len().min(max);
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        self.u16(end as u16);
        self.0.extend_from_slice(&s.as_bytes()[..end]);
    }
}

struct R<'a> {
    b: &'a [u8],
    at: usize,
}

impl R<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], WireError> {
        let end = self.at.checked_add(n).ok_or(WireError::Short)?;
        let s = self.b.get(self.at..end).ok_or(WireError::Short)?;
        self.at = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, WireError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, WireError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, WireError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, WireError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn str(&mut self, max: usize) -> Result<String, WireError> {
        let n = self.u16()? as usize;
        if n > max {
            return Err(WireError::TooLong);
        }
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| WireError::Utf8)
    }
}

fn put_traveler(w: &mut W, t: &Traveler) {
    w.u64(t.id);
    w.u64(t.lineage);
    w.str(&t.name, MAX_NAME);
    w.str(&t.author, MAX_NAME);
    w.u32(t.gen);
    w.u32(t.hops);
    w.u32(t.age);
    w.u64(t.balance);
    for v in t.mem {
        w.u64(v as u64);
    }
    w.str(&t.genome, MAX_SOURCE);
    w.u16(t.offset);
}

fn get_traveler(r: &mut R) -> Result<Traveler, WireError> {
    let id = r.u64()?;
    let lineage = r.u64()?;
    let name = r.str(MAX_NAME)?;
    let author = r.str(MAX_NAME)?;
    let gen = r.u32()?;
    let hops = r.u32()?;
    let age = r.u32()?;
    let balance = r.u64()?;
    let mut mem = [0i64; MEM_SLOTS];
    for v in mem.iter_mut() {
        *v = r.u64()? as i64;
    }
    let genome = r.str(MAX_SOURCE)?;
    let offset = r.u16()?;
    Ok(Traveler {
        id,
        lineage,
        name,
        author,
        gen,
        hops,
        age,
        balance,
        mem,
        genome,
        offset,
    })
}

pub fn encode(env: &Envelope) -> Vec<u8> {
    let mut w = W(Vec::with_capacity(64));
    w.0.extend_from_slice(&MAGIC);
    w.u8(VERSION);
    let kind = match &env.msg {
        Msg::Hello(_) => 1,
        Msg::LinkReq { .. } => 2,
        Msg::LinkAck { .. } => 3,
        Msg::LinkNo => 4,
        Msg::Unlink { .. } => 5,
        Msg::Mote { .. } => 6,
        Msg::MoteAck { .. } => 7,
        Msg::Census(_) => 8,
        Msg::Bye => 9,
        Msg::World { .. } => 10,
        Msg::Gossip(_) => 11,
    };
    w.u8(kind);
    w.u64(env.from);
    w.u64(env.to);
    match &env.msg {
        Msg::Hello(h) => {
            w.str(&h.place, MAX_PLACE);
            w.u16(h.tz_min as u16);
            w.u8(h.sun);
            w.u8(h.asleep as u8);
            w.u8(h.free);
            w.u16(h.pop);
        }
        Msg::LinkReq { side } | Msg::Unlink { side } => w.u8(*side),
        Msg::LinkAck { side, yours } => {
            w.u8(*side);
            w.u8(*yours);
        }
        Msg::LinkNo | Msg::Bye => {}
        Msg::Mote {
            seq,
            side,
            traveler,
        } => {
            w.u64(*seq);
            w.u8(*side);
            put_traveler(&mut w, traveler);
        }
        Msg::MoteAck { seq } => w.u64(*seq),
        Msg::Census(entries) => {
            let n = entries.len().min(MAX_CENSUS);
            w.u8(n as u8);
            for e in &entries[..n] {
                w.u64(e.lineage);
                w.u32(e.count);
                w.str(&e.name, MAX_NAME);
                w.str(&e.author, MAX_NAME);
            }
        }
        Msg::Gossip(g) => {
            w.u32(g.epoch);
            w.0.extend_from_slice(&g.world.0);
            match &g.world_exact {
                Some(ids) if ids.len() <= EXACT_WORLD => {
                    w.u8(ids.len() as u8);
                    for id in ids {
                        w.u64(*id);
                    }
                }
                _ => w.u8(0xff),
            }
            let n = g.lineages.len().min(GOSSIP_LINEAGES);
            w.u8(n as u8);
            for (lineage, t) in &g.lineages[..n] {
                w.u64(*lineage);
                w.str(&t.name, MAX_NAME);
                w.str(&t.author, MAX_NAME);
                w.0.extend_from_slice(&t.islands.0);
                w.0.extend_from_slice(&t.motes.0);
                match &t.exact {
                    Some(list) if list.len() <= EXACT => {
                        w.u8(list.len() as u8);
                        for (island, n) in list {
                            w.u64(*island);
                            w.u32(*n);
                        }
                    }
                    _ => w.u8(0xff),
                }
            }
        }
        Msg::World { islands, entries } => {
            w.u32(*islands);
            let n = entries.len().min(MAX_CENSUS);
            w.u8(n as u8);
            for e in &entries[..n] {
                w.u64(e.lineage);
                w.u32(e.tabs);
                w.u32(e.count);
                w.str(&e.name, MAX_NAME);
                w.str(&e.author, MAX_NAME);
            }
        }
    }
    w.0
}

/// A sketch's registers; no honest register exceeds 64.
fn sketch(r: &mut R) -> Result<Sketch, WireError> {
    let mut regs = [0u8; REGS];
    for (reg, b) in regs.iter_mut().zip(r.take(REGS)?) {
        *reg = (*b).min(64);
    }
    Ok(Sketch(regs))
}

/// An exact list of (island, motes), capped at one mote a cell.
fn exact(r: &mut R) -> Result<Exact, WireError> {
    match r.u8()? {
        0xff => Ok(None),
        k if k as usize <= EXACT => {
            let mut list = Vec::with_capacity(k as usize);
            for _ in 0..k {
                list.push((r.u64()?, r.u32()?.min(crate::laws::CELLS as u32)));
            }
            Ok(Some(list))
        }
        _ => Err(WireError::TooLong),
    }
}

pub fn decode(b: &[u8]) -> Result<Envelope, WireError> {
    let mut r = R { b, at: 0 };
    if r.take(2)? != MAGIC {
        return Err(WireError::Magic);
    }
    if r.u8()? != VERSION {
        return Err(WireError::Version);
    }
    let kind = r.u8()?;
    let from = r.u64()?;
    let to = r.u64()?;
    let side = |r: &mut R| -> Result<u8, WireError> { Ok(r.u8()? % 4) };
    let msg = match kind {
        1 => Msg::Hello(Hello {
            place: r.str(MAX_PLACE)?,
            tz_min: r.u16()? as i16,
            sun: r.u8()?.min(100),
            asleep: r.u8()? != 0,
            free: r.u8()?.min(4),
            pop: r.u16()?,
        }),
        2 => Msg::LinkReq {
            side: side(&mut r)?,
        },
        3 => Msg::LinkAck {
            side: side(&mut r)?,
            yours: side(&mut r)?,
        },
        4 => Msg::LinkNo,
        5 => Msg::Unlink {
            side: side(&mut r)?,
        },
        6 => Msg::Mote {
            seq: r.u64()?,
            side: side(&mut r)?,
            traveler: get_traveler(&mut r)?,
        },
        7 => Msg::MoteAck { seq: r.u64()? },
        8 => {
            let n = r.u8()? as usize;
            if n > MAX_CENSUS {
                return Err(WireError::TooLong);
            }
            let mut entries = Vec::with_capacity(n);
            for _ in 0..n {
                entries.push(CensusEntry {
                    lineage: r.u64()?,
                    count: r.u32()?,
                    name: r.str(MAX_NAME)?,
                    author: r.str(MAX_NAME)?,
                });
            }
            Msg::Census(entries)
        }
        9 => Msg::Bye,
        10 => {
            let islands = r.u32()?;
            let n = r.u8()? as usize;
            if n > MAX_CENSUS {
                return Err(WireError::TooLong);
            }
            let mut entries = Vec::with_capacity(n);
            for _ in 0..n {
                entries.push(WorldEntry {
                    lineage: r.u64()?,
                    tabs: r.u32()?,
                    count: r.u32()?,
                    name: r.str(MAX_NAME)?,
                    author: r.str(MAX_NAME)?,
                });
            }
            Msg::World { islands, entries }
        }
        11 => {
            let epoch = r.u32()?;
            let world = sketch(&mut r)?;
            let world_exact = match r.u8()? {
                0xff => None,
                k if k as usize <= EXACT_WORLD => {
                    Some((0..k).map(|_| r.u64()).collect::<Result<Vec<u64>, _>>()?)
                }
                _ => return Err(WireError::TooLong),
            };
            let n = r.u8()? as usize;
            if n > GOSSIP_LINEAGES {
                return Err(WireError::TooLong);
            }
            let mut lineages = Vec::with_capacity(n);
            for _ in 0..n {
                let lineage = r.u64()?;
                let name = r.str(MAX_NAME)?;
                let author = r.str(MAX_NAME)?;
                let islands = sketch(&mut r)?;
                let motes = sketch(&mut r)?;
                let exact = exact(&mut r)?;
                lineages.push((
                    lineage,
                    Tally {
                        name,
                        author,
                        islands,
                        motes,
                        exact,
                    },
                ));
            }
            Msg::Gossip(Gossip {
                epoch,
                world,
                world_exact,
                lineages,
            })
        }
        _ => return Err(WireError::Kind),
    };
    Ok(Envelope { from, to, msg })
}
