//! Save files. A snapshot is a container (magic, version, the room it is
//! of, the room's schema, the tick and time it was taken, a payload and the
//! payload's CRC) holding a room's
//! payload, which is itself a list of sections (a tag, the bytes, their
//! CRC). A room reads the sections it knows, skips the ones it does not,
//! and refuses the whole file if any section it knows is damaged.

use crate::crc32::crc32;
use crate::wire::{Reader, Writer};

pub const MAGIC: &[u8; 4] = b"SSNP";
pub const VERSION: u16 = 1;
/// The largest snapshot anyone will read.
pub const MAX: usize = 64 << 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snap {
    pub room: String,
    /// The room's own format version, for its migrations.
    pub schema: u16,
    pub tick: u64,
    /// Seconds since 1970 when it was taken.
    pub unix: u64,
    pub payload: Vec<u8>,
}

impl Snap {
    pub fn pack(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.0.extend_from_slice(MAGIC);
        w.u16(VERSION).str(&self.room).u16(self.schema);
        w.u64(self.tick)
            .u64(self.unix)
            .u32(self.payload.len() as u32);
        w.0.extend_from_slice(&self.payload);
        w.u32(crc32(&self.payload));
        w.0
    }
}

pub fn unpack(b: &[u8]) -> Result<Snap, &'static str> {
    if b.len() > MAX {
        return Err("too big");
    }
    if b.get(..4) != Some(&MAGIC[..]) {
        return Err("not a snapshot");
    }
    let mut r = Reader::new(&b[4..]);
    let version = r.u16().ok_or("short")?;
    if version != VERSION {
        return Err("unknown container version");
    }
    let room = r.str().ok_or("short")?;
    let schema = r.u16().ok_or("short")?;
    let tick = r.u64().ok_or("short")?;
    let unix = r.u64().ok_or("short")?;
    let n = r.u32().ok_or("short")? as usize;
    let payload = r.bytes(n).ok_or("short")?.to_vec();
    let crc = r.u32().ok_or("short")?;
    if crc != crc32(&payload) {
        return Err("damaged");
    }
    Ok(Snap {
        room,
        schema,
        tick,
        unix,
        payload,
    })
}

/// A payload being written, a section at a time.
#[derive(Default)]
pub struct Sections(Writer);

impl Sections {
    pub fn add(&mut self, tag: u16, bytes: &[u8]) -> &mut Self {
        self.0.u16(tag).u32(bytes.len() as u32);
        self.0 .0.extend_from_slice(bytes);
        self.0.u32(crc32(bytes));
        self
    }

    pub fn finish(self) -> Vec<u8> {
        self.0 .0
    }
}

/// A payload's sections, in order. Any damaged section fails the lot: a
/// room cannot know whether it would have needed it.
pub fn sections(payload: &[u8]) -> Result<Vec<(u16, &[u8])>, &'static str> {
    let mut r = Reader::new(payload);
    let mut out = Vec::new();
    while !r.done() {
        let tag = r.u16().ok_or("short section")?;
        let n = r.u32().ok_or("short section")? as usize;
        let bytes = r.bytes(n).ok_or("short section")?;
        let crc = r.u32().ok_or("short section")?;
        if crc != crc32(bytes) {
            return Err("damaged section");
        }
        out.push((tag, bytes));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snapshot_round_trips() {
        let mut s = Sections::default();
        s.add(1, b"hello").add(7, &[0, 1, 2]);
        let payload = s.finish();
        let snap = Snap {
            room: "wyrm".into(),
            schema: 3,
            tick: 99,
            unix: 1_700_000_000,
            payload,
        };
        let back = unpack(&snap.pack()).unwrap();
        assert_eq!(back, snap);
        let secs = sections(&back.payload).unwrap();
        assert_eq!(secs, vec![(1, &b"hello"[..]), (7, &[0u8, 1, 2][..])]);
    }

    #[test]
    fn damage_is_noticed_never_a_panic() {
        let mut s = Sections::default();
        s.add(1, b"some world");
        let file = Snap {
            room: "wyrm".into(),
            schema: 0,
            tick: 1,
            unix: 1,
            payload: s.finish(),
        }
        .pack();
        for i in 0..file.len() {
            let mut bad = file.clone();
            bad[i] ^= 0x40;
            if let Ok(snap) = unpack(&bad) {
                // Only the header can change unnoticed.
                assert!(sections(&snap.payload).is_ok());
            }
            for cut in [0, i] {
                let _ = unpack(&file[..cut]);
            }
        }
        assert!(unpack(b"SSNP").is_err());
        assert!(sections(&[1, 0, 255, 255, 255, 255]).is_err());
    }
}
