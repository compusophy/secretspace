//! What crosses between tabs. A small binary format, versioned, bounded
//! everywhere: every byte comes from a stranger, so decoding never panics
//! and never allocates past the limits below.

use crate::island::Traveler;
use crate::laws::{MAX_SOURCE, MEM_SLOTS};

pub const MAGIC: [u8; 2] = *b"SS";
pub const VERSION: u8 = 1;
const MAX_PLACE: usize = 48;
const MAX_NAME: usize = 32;
const MAX_CENSUS: usize = 24;

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
    }
    w.0
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
        _ => return Err(WireError::Kind),
    };
    Ok(Envelope { from, to, msg })
}
