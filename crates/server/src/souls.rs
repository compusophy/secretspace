//! The platform's souls: who each key is, and the one name it goes by in
//! every game. A page's Hello carries its secret key; the soul is the first
//! 8 bytes of the key's SHA-1, and only the soul is kept.
//!
//! - Names are unique once folded (case, 0/o, 1/l/i, 5/s, spaces), and the
//!   names rooms give their bots are nobody's.
//! - A soul is written down once it has played two minutes; one that played
//!   under ten minutes and stays away fourteen days is forgotten, and its
//!   name is free again.
//! - An address makes at most ten new souls an hour; past that its pages
//!   play as guests.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use engine::snap::{self, Snap};
use engine::who::{clean_name, fold, soul_of, Hello, Seen, Status, Who};
use engine::wire::{Reader, Writer};

/// Play, in seconds, before a soul is written down.
const KEEP_AFTER: u32 = 120;
/// Play below which a soul is forgotten after `FORGET_AFTER` away.
const SETTLED: u32 = 600;
const FORGET_AFTER: u64 = 14 * 86_400;
/// New souls an address may make an hour.
const BORN_AN_HOUR: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Soul {
    pub name: String,
    pub created: u64,
    pub seen: u64,
    /// Seconds played, every game together.
    pub played: u32,
}

#[derive(Default)]
pub struct Souls {
    path: Option<PathBuf>,
    souls: HashMap<u64, Soul>,
    /// Folded name -> its soul.
    names: HashMap<String, u64>,
    reserved: HashSet<String>,
    /// When each address last made new souls.
    born: HashMap<String, VecDeque<u64>>,
    /// Connections playing now, and whose they are.
    live: HashMap<u32, u64>,
    dirty: bool,
}

impl Souls {
    /// The store kept at `path` (or only in memory, with none).
    pub fn open(path: Option<PathBuf>, now: u64) -> Souls {
        let mut s = Souls {
            path,
            ..Souls::default()
        };
        if let Some(bytes) = s.path.as_ref().and_then(|p| fs::read(p).ok()) {
            match decode(&bytes) {
                Some(souls) => s.souls = souls,
                None => eprintln!("souls: the file does not read; starting empty"),
            }
        }
        s.forget(now);
        s
    }

    /// Names nobody may take: the bots'.
    pub fn reserve(&mut self, names: &[&str]) {
        self.reserved.extend(names.iter().map(|n| fold(n)));
    }

    fn reindex(&mut self) {
        self.names = self
            .souls
            .iter()
            .filter(|(_, s)| !s.name.is_empty())
            .map(|(&id, s)| (fold(&s.name), id))
            .collect();
    }

    fn forget(&mut self, now: u64) {
        let live: HashSet<u64> = self.live.values().copied().collect();
        let before = self.souls.len();
        self.souls.retain(|id, s| {
            let away = now.saturating_sub(s.seen);
            live.contains(id)
                || (s.played >= SETTLED || away < FORGET_AFTER)
                    // Souls never written down are let go after a day.
                    && (s.played >= KEEP_AFTER || away < 86_400)
        });
        self.dirty |= self.souls.len() != before;
        self.born.retain(|_, q| {
            q.retain(|&t| now.saturating_sub(t) < 3600);
            !q.is_empty()
        });
        self.reindex();
    }

    fn free(&self, name: &str, soul: u64) -> bool {
        let f = fold(name);
        !f.is_empty()
            && !self.reserved.contains(&f)
            && self.names.get(&f).is_none_or(|&o| o == soul)
    }

    /// A page said Hello from `addr`: who it is, and what to tell it.
    pub fn hello(&mut self, h: &Hello, addr: &str, now: u64, watch: bool) -> (Who, Seen) {
        let want = clean_name(&h.name);
        let soul = soul_of(&h.key);
        let mut status = Status::Ok;
        if !self.souls.contains_key(&soul) {
            let q = self.born.entry(addr.to_string()).or_default();
            q.retain(|&t| now.saturating_sub(t) < 3600);
            if q.len() >= BORN_AN_HOUR {
                // Too many new souls from here: a guest, named if the name
                // is nobody's.
                let name = if self.free(&want, 0) {
                    want
                } else {
                    String::new()
                };
                let who = Who {
                    soul: 0,
                    name: name.clone(),
                    watch,
                    build: h.build,
                };
                return (who, Seen { status, name });
            }
            q.push_back(now);
            self.souls.insert(
                soul,
                Soul {
                    name: String::new(),
                    created: now,
                    seen: now,
                    played: 0,
                },
            );
            status = Status::New;
        }
        let free = self.free(&want, soul);
        let s = self.souls.get_mut(&soul).expect("just made");
        s.seen = now;
        let asks = !want.is_empty() && want != s.name && (h.rename || s.name.is_empty());
        if asks {
            if free {
                self.names.remove(&fold(&s.name));
                s.name = want;
                self.names.insert(fold(&s.name), soul);
                self.dirty = true;
            } else {
                status = Status::Taken;
            }
        }
        let name = s.name.clone();
        let who = Who {
            soul,
            name: name.clone(),
            watch,
            build: h.build,
        };
        (who, Seen { status, name })
    }

    pub fn enter(&mut self, conn: u32, soul: u64) {
        if soul != 0 {
            self.live.insert(conn, soul);
        }
    }

    pub fn leave(&mut self, conn: u32) {
        self.live.remove(&conn);
    }

    /// `secs` more of play for every soul playing now.
    pub fn tick(&mut self, secs: u32, now: u64) {
        for soul in self.live.values() {
            if let Some(s) = self.souls.get_mut(soul) {
                let was = s.played;
                s.played = s.played.saturating_add(secs);
                s.seen = now;
                self.dirty |= s.played >= KEEP_AFTER && (was < KEEP_AFTER || secs > 0);
            }
        }
        if now % 3600 < secs as u64 {
            self.forget(now);
        }
    }

    #[cfg(test)]
    pub fn get(&self, soul: u64) -> Option<&Soul> {
        self.souls.get(&soul)
    }

    /// Souls known (written down or not).
    pub fn len(&self) -> usize {
        self.souls.len()
    }

    /// Write the store down if anything worth keeping changed: a temporary
    /// file, synced, then renamed over the old one.
    pub fn save(&mut self, now: u64) {
        let Some(path) = self.path.clone() else {
            return;
        };
        if !self.dirty {
            return;
        }
        let bytes = encode(&self.souls, now);
        let tmp = path.with_extension("tmp");
        let ok = fs::File::create(&tmp)
            .and_then(|mut f| f.write_all(&bytes).and_then(|_| f.sync_all()))
            .and_then(|_| fs::rename(&tmp, &path));
        match ok {
            Ok(()) => self.dirty = false,
            Err(e) => eprintln!("souls: not saved: {e}"),
        }
    }
}

fn encode(souls: &HashMap<u64, Soul>, now: u64) -> Vec<u8> {
    let kept: Vec<(&u64, &Soul)> = souls
        .iter()
        .filter(|(_, s)| s.played >= KEEP_AFTER)
        .collect();
    let mut w = Writer::default();
    w.u32(kept.len() as u32);
    for (&id, s) in kept {
        w.u64(id)
            .str(&s.name)
            .u64(s.created)
            .u64(s.seen)
            .u32(s.played);
    }
    Snap {
        room: "souls".into(),
        schema: 1,
        tick: 0,
        unix: now,
        payload: w.0,
    }
    .pack()
}

fn decode(bytes: &[u8]) -> Option<HashMap<u64, Soul>> {
    let snap = snap::unpack(bytes).ok()?;
    let mut r = Reader::new(&snap.payload);
    let n = r.u32()? as usize;
    r.room(n, 29)?;
    let mut out = HashMap::with_capacity(n);
    for _ in 0..n {
        let id = r.u64()?;
        let soul = Soul {
            name: clean_name(&r.str()?),
            created: r.u64()?,
            seen: r.u64()?,
            played: r.u32()?,
        };
        out.insert(id, soul);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello(key: u8, name: &str, rename: bool) -> Hello {
        Hello {
            proto: engine::who::PROTO,
            key: [key; 16],
            name: name.into(),
            rename,
            build: 0,
        }
    }

    #[test]
    fn a_soul_keeps_one_unique_name() {
        let mut s = Souls::open(None, 1000);
        s.reserve(&["noodle"]);
        let (a, seen) = s.hello(&hello(1, "Zoë", false), "x", 1000, false);
        assert_eq!((seen.status, seen.name.as_str()), (Status::New, "Zoë"));
        assert_ne!(a.soul, 0);
        // Someone else cannot take it, or anything that looks like it.
        let (b, seen) = s.hello(&hello(2, "zoë", true), "y", 1000, false);
        assert_eq!(seen.status, Status::Taken);
        assert_eq!(b.name, "");
        let (_, seen) = s.hello(&hello(2, "N00DLE", true), "y", 1000, false);
        assert_eq!(seen.status, Status::Taken);
        // The stored name wins unless the page renames.
        let (a2, seen) = s.hello(&hello(1, "other", false), "x", 1001, false);
        assert_eq!((a2.soul, seen.name.as_str()), (a.soul, "Zoë"));
        let (_, seen) = s.hello(&hello(1, "other", true), "x", 1002, false);
        assert_eq!((seen.status, seen.name.as_str()), (Status::Ok, "other"));
        // And the old name is free.
        let (_, seen) = s.hello(&hello(2, "zoe", true), "y", 1003, false);
        assert_eq!((seen.status, seen.name.as_str()), (Status::Ok, "zoe"));
    }

    #[test]
    fn an_address_makes_ten_souls_an_hour() {
        let mut s = Souls::open(None, 0);
        for k in 0..10 {
            let (who, _) = s.hello(&hello(k, "", false), "flood", 10, false);
            assert_ne!(who.soul, 0);
        }
        let (who, _) = s.hello(&hello(99, "", false), "flood", 10, false);
        assert_eq!(who.soul, 0, "the eleventh is a guest");
        let (who, _) = s.hello(&hello(99, "", false), "flood", 3700, false);
        assert_ne!(who.soul, 0, "an hour later it may");
        let (who, _) = s.hello(&hello(1, "", false), "flood", 3700, false);
        assert_ne!(who.soul, 0, "known souls are always welcome");
    }

    #[test]
    fn souls_that_played_are_kept_and_the_rest_forgotten() {
        let dir = std::env::temp_dir().join(format!("souls-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("souls");
        let mut s = Souls::open(Some(path.clone()), 0);
        let (a, _) = s.hello(&hello(1, "stayer", false), "x", 0, false);
        let (b, _) = s.hello(&hello(2, "passer", false), "x", 0, false);
        s.enter(1, a.soul);
        s.enter(2, b.soul);
        s.tick(60, 60);
        s.leave(2);
        s.tick(60, 120);
        s.save(120);
        let back = Souls::open(Some(path.clone()), 200);
        assert_eq!(back.get(a.soul).map(|s| s.name.as_str()), Some("stayer"));
        assert!(back.get(b.soul).is_none(), "under two minutes is not kept");
        // Two weeks away with little play: forgotten, and the name freed.
        let mut later = Souls::open(Some(path), 120 + FORGET_AFTER + 1);
        assert!(later.get(a.soul).is_none());
        let (_, seen) = later.hello(&hello(3, "stayer", false), "z", 0, false);
        assert_eq!(seen.name, "stayer");
        let _ = fs::remove_dir_all(&dir);
        assert!(decode(b"junk").is_none());
    }
}
