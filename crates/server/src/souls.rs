//! The platform's souls: who each key is, and the one name it goes by in
//! every game. A page's Hello carries its secret key; the soul is the first
//! 8 bytes of the key's SHA-1, and only the soul is kept.
//!
//! - Names are unique once folded (case, 0/o, 1/l/i, 5/s, spaces), and the
//!   names rooms give their bots, and their guests ("wizard 7"), are
//!   nobody's.
//! - A soul is written down once it has played two minutes; one that played
//!   under ten minutes and stays away fourteen days is forgotten, and its
//!   name is free again.
//! - An address makes at most ten new souls an hour, and everyone together
//!   at most `BORN_ALL_AN_HOUR`; past that pages play as guests.
//! - What changes is written down soon: a name at once, play time at most
//!   once a minute. A file that does not read is set aside, never written
//!   over.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use engine::snap::{self, Snap};
use engine::who::{clean_name, fold, is_guest_name, soul_of, Hello, Seen, Status, Who};
use engine::wire::{Reader, Writer};

use crate::store::{read_kept, Kept};

/// Play, in seconds, before a soul is written down.
const KEEP_AFTER: u32 = 120;
/// Play below which a soul is forgotten after `FORGET_AFTER` away.
const SETTLED: u32 = 600;
const FORGET_AFTER: u64 = 14 * 86_400;
/// New souls an address may make an hour, and everyone together (a bound
/// on what a crowd of addresses can fill memory and names with).
const BORN_AN_HOUR: usize = 10;
const BORN_ALL_AN_HOUR: usize = 5000;
/// Seconds between writing down play time alone.
const PLAYED_EVERY: u64 = 60;
/// The file's format.
const SCHEMA: u16 = 1;

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
    /// When each address last made new souls, and everyone did.
    born: HashMap<String, VecDeque<u64>>,
    born_all: VecDeque<u64>,
    /// Connections playing now, and whose they are.
    live: HashMap<u32, u64>,
    /// Names or souls kept have changed (written down at once); play time
    /// has (written down now and then); when it last was.
    names_dirty: bool,
    played_dirty: bool,
    saved: u64,
}

impl Souls {
    /// The store kept at `path` (or only in memory, with none). A file
    /// that does not read (damaged, a newer build's, or a disk that will
    /// not give it) is set aside as `souls.bad-<now>` first, so starting
    /// empty never writes over it.
    pub fn open(path: Option<PathBuf>, now: u64) -> Souls {
        let mut s = Souls {
            path,
            saved: now,
            ..Souls::default()
        };
        if let Some(p) = s.path.clone() {
            match read_kept(&p, now, decode) {
                Kept::Read(souls) => s.souls = souls,
                Kept::Fresh => {}
                // Not where a save would write over it: keep nothing
                // rather than lose it.
                Kept::Stuck => s.path = None,
            }
        }
        s.reindex();
        s.forget(now);
        s
    }

    /// Names nobody may take: the bots'.
    pub fn reserve(&mut self, names: &[&str]) {
        self.reserved.extend(names.iter().map(|n| fold(n)));
    }

    /// Each name to its soul. Two souls whose names now fold the same (the
    /// rules have changed under them) cannot both keep it: the one that
    /// has played longest does, and the others are asked for a new one.
    fn reindex(&mut self) {
        let mut ids: Vec<u64> = self.souls.keys().copied().collect();
        ids.sort_by_key(|id| {
            let s = &self.souls[id];
            (std::cmp::Reverse(s.played), s.created, *id)
        });
        self.names.clear();
        for id in ids {
            let s = self.souls.get_mut(&id).expect("listed");
            let f = fold(&s.name);
            if f.is_empty() {
                continue;
            }
            match self.names.entry(f) {
                Entry::Occupied(_) => {
                    s.name.clear();
                    self.names_dirty = true;
                }
                Entry::Vacant(e) => {
                    e.insert(id);
                }
            }
        }
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
        self.names_dirty |= self.souls.len() != before;
        let recent = |q: &mut VecDeque<u64>| q.retain(|&t| now.saturating_sub(t) < 3600);
        self.born.retain(|_, q| {
            recent(q);
            !q.is_empty()
        });
        recent(&mut self.born_all);
        self.reindex();
    }

    fn free(&self, name: &str, soul: u64) -> bool {
        let f = fold(name);
        !f.is_empty()
            && !self.reserved.contains(&f)
            && !is_guest_name(name)
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
            self.born_all.retain(|&t| now.saturating_sub(t) < 3600);
            if q.len() >= BORN_AN_HOUR || self.born_all.len() >= BORN_ALL_AN_HOUR {
                // Too many new souls: a guest, named if the name is
                // nobody's.
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
            self.born_all.push_back(now);
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
                self.names_dirty |= s.played >= KEEP_AFTER;
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
                // Written down for the first time: at once.
                self.names_dirty |= was < KEEP_AFTER && s.played >= KEEP_AFTER;
                self.played_dirty |= s.played >= KEEP_AFTER;
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

    /// The store to write down now, if it should be (`all`: everything not
    /// yet written, the server stopping): where, and the bytes. Taken as
    /// written; the caller writes it outside the lock and says if that
    /// failed (`unsaved`).
    pub fn due(&mut self, now: u64, all: bool) -> Option<(PathBuf, Vec<u8>)> {
        let path = self.path.clone()?;
        let played = self.played_dirty && (all || now.saturating_sub(self.saved) >= PLAYED_EVERY);
        if !self.names_dirty && !played {
            return None;
        }
        self.names_dirty = false;
        self.played_dirty = false;
        self.saved = now;
        Some((path, encode(&self.souls, now)))
    }

    /// What `due` gave was not written: write it down again soon.
    pub fn unsaved(&mut self) {
        self.names_dirty = true;
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
        schema: SCHEMA,
        tick: 0,
        unix: now,
        payload: w.0,
    }
    .pack()
}

fn decode(bytes: &[u8]) -> Result<HashMap<u64, Soul>, &'static str> {
    let snap = snap::unpack(bytes)?;
    if snap.room != "souls" {
        return Err("not the souls");
    }
    if snap.schema > SCHEMA {
        return Err("a newer build's");
    }
    let mut r = Reader::new(&snap.payload);
    let short = "short";
    let n = r.u32().ok_or(short)? as usize;
    r.room(n, 29).ok_or(short)?;
    let mut out = HashMap::with_capacity(n);
    for _ in 0..n {
        let id = r.u64().ok_or(short)?;
        let soul = Soul {
            name: clean_name(&r.str().ok_or(short)?),
            created: r.u64().ok_or(short)?,
            seen: r.u64().ok_or(short)?,
            played: r.u32().ok_or(short)?,
        };
        out.insert(id, soul);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn hello(key: u8, name: &str, rename: bool) -> Hello {
        Hello {
            proto: engine::who::PROTO,
            key: [key; 16],
            name: name.into(),
            rename,
            build: 0,
        }
    }

    /// A scratch directory of the test's own.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("souls-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Write the store down if it is due, as the server does.
    fn save(s: &mut Souls, now: u64, all: bool) -> bool {
        match s.due(now, all) {
            Some((path, bytes)) => {
                crate::store::write_atomic(&path, &bytes).unwrap();
                true
            }
            None => false,
        }
    }

    #[test]
    fn a_soul_keeps_one_unique_name() {
        let mut s = Souls::open(None, 1000);
        s.reserve(&["noodle"]);
        let (a, seen) = s.hello(&hello(1, "Zoë", false), "x", 1000, false);
        assert_eq!((seen.status, seen.name.as_str()), (Status::New, "Zoe"));
        assert_ne!(a.soul, 0);
        // Someone else cannot take it, or anything that looks like it.
        let (b, seen) = s.hello(&hello(2, "zoë", true), "y", 1000, false);
        assert_eq!(seen.status, Status::Taken);
        assert_eq!(b.name, "");
        let (_, seen) = s.hello(&hello(2, "N00DLE", true), "y", 1000, false);
        assert_eq!(seen.status, Status::Taken);
        // Nor pass for a guest.
        let (_, seen) = s.hello(&hello(2, "Wizard 12", true), "y", 1000, false);
        assert_eq!(seen.status, Status::Taken);
        // The stored name wins unless the page renames.
        let (a2, seen) = s.hello(&hello(1, "other", false), "x", 1001, false);
        assert_eq!((a2.soul, seen.name.as_str()), (a.soul, "Zoe"));
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
        // A crowd of addresses together: bounded too.
        s.born_all = (0..BORN_ALL_AN_HOUR).map(|_| 3700).collect();
        let (who, _) = s.hello(&hello(100, "", false), "elsewhere", 3700, false);
        assert_eq!(who.soul, 0);
    }

    #[test]
    fn souls_that_played_are_kept_and_the_rest_forgotten() {
        let dir = scratch("kept");
        let path = dir.join("souls");
        let mut s = Souls::open(Some(path.clone()), 0);
        let (a, _) = s.hello(&hello(1, "stayer", false), "x", 0, false);
        let (b, _) = s.hello(&hello(2, "passer", false), "x", 0, false);
        s.enter(1, a.soul);
        s.enter(2, b.soul);
        s.tick(60, 60);
        s.leave(2);
        s.tick(60, 120);
        assert!(save(&mut s, 120, false));
        let back = Souls::open(Some(path.clone()), 200);
        assert_eq!(back.get(a.soul).map(|s| s.name.as_str()), Some("stayer"));
        assert!(back.get(b.soul).is_none(), "under two minutes is not kept");
        // Two weeks away with little play: forgotten, and the name freed.
        let mut later = Souls::open(Some(path), 120 + FORGET_AFTER + 1);
        assert!(later.get(a.soul).is_none());
        let (_, seen) = later.hello(&hello(3, "stayer", false), "z", 0, false);
        assert_eq!(seen.name, "stayer");
        let _ = fs::remove_dir_all(&dir);
        assert!(decode(b"junk").is_err());
    }

    #[test]
    fn names_are_written_down_at_once_and_play_time_now_and_then() {
        let dir = scratch("due");
        let mut s = Souls::open(Some(dir.join("souls")), 0);
        let (a, _) = s.hello(&hello(1, "ash", false), "x", 0, false);
        s.enter(1, a.soul);
        assert!(!save(&mut s, 0, false), "nothing kept yet");
        s.tick(KEEP_AFTER, 120);
        assert!(save(&mut s, 120, false), "kept for the first time: at once");
        s.tick(5, 125);
        assert!(!save(&mut s, 125, false), "play time alone waits");
        s.tick(5, 130);
        assert!(save(&mut s, 120 + PLAYED_EVERY, false), "a minute on");
        let (_, seen) = s.hello(&hello(1, "oak", true), "x", 190, false);
        assert_eq!(seen.name, "oak");
        assert!(save(&mut s, 190, false), "a new name: at once");
        s.tick(5, 195);
        assert!(
            save(&mut s, 196, true),
            "and everything when the server stops"
        );
        let back = Souls::open(Some(dir.join("souls")), 200);
        assert_eq!(
            back.get(a.soul).map(|s| (s.name.as_str(), s.played)),
            Some(("oak", 135))
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_that_does_not_read_is_set_aside_never_written_over() {
        let dir = scratch("bad");
        let path = dir.join("souls");
        fs::write(&path, b"SSNP and then rubbish").unwrap();
        let mut s = Souls::open(Some(path.clone()), 77);
        assert_eq!(s.len(), 0);
        assert_eq!(
            fs::read(dir.join("souls.bad-77")).unwrap(),
            b"SSNP and then rubbish"
        );
        // Starting empty, and saving: the bad one is still there.
        let (a, _) = s.hello(&hello(1, "ash", false), "x", 80, false);
        s.enter(1, a.soul);
        s.tick(KEEP_AFTER, 200);
        assert!(save(&mut s, 200, false));
        assert!(dir.join("souls.bad-77").exists());
        // Another room's file, or a newer build's, is not read as ours.
        let other = Snap {
            room: "wyrm".into(),
            schema: SCHEMA,
            tick: 0,
            unix: 0,
            payload: Vec::new(),
        };
        assert!(decode(&other.pack()).is_err());
        let newer = Snap {
            room: "souls".into(),
            schema: SCHEMA + 1,
            ..other
        };
        assert!(decode(&newer.pack()).is_err());
        // One the disk will not give (here, a directory where the file is):
        // set aside too, never taken for no file at all.
        let _ = fs::remove_file(&path);
        fs::create_dir(&path).unwrap();
        let mut s = Souls::open(Some(path.clone()), 300);
        assert!(dir.join("souls.bad-300").is_dir());
        let (a, _) = s.hello(&hello(1, "ash", false), "x", 300, false);
        s.enter(1, a.soul);
        s.tick(KEEP_AFTER, 420);
        assert!(save(&mut s, 420, false));
        assert!(dir.join("souls.bad-300").is_dir() && path.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn names_that_now_fold_together_go_to_the_longest_played() {
        let mut s = Souls::open(None, 0);
        // Kept under the old rules: "Zoë" and "Zoe" were two names.
        let soul = |name: &str, played| Soul {
            name: name.into(),
            created: 0,
            seen: 0,
            played,
        };
        s.souls.insert(1, soul("Zoe", 900));
        s.souls.insert(2, soul("Zoe", 5000));
        s.reindex();
        assert_eq!(s.get(2).unwrap().name, "Zoe");
        assert_eq!(s.get(1).unwrap().name, "", "asked for a new name");
        assert!(s.names_dirty);
    }
}
