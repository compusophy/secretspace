//! Where rooms keep their worlds: `$DATA_DIR/rooms/<id>/snap-<unix>.bin`.
//!
//! - A writer thread takes each snapshot, writes it to a temporary file,
//!   syncs it, renames it into place and syncs the directory, so a crash
//!   leaves the old file or the new one, never half of either
//!   (`write_atomic`, which the souls and the visit count use too).
//! - It keeps the newest 6, one an hour for a day, one a day for 14 days.
//! - On boot each attempt is a fresh room from its factory, given the
//!   newest snapshot, then older ones. A fresh world is made only when
//!   there is no snapshot at all; when there are some and none loads, the
//!   room stays closed and its files are copied to `quarantine/<unix>/`.
//!   Each is told the schema it was written in (`Room::load_snap`).

use std::fs;
use std::io::Write;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::thread;

use engine::room::Room;
use engine::snap::{self, Snap};

pub type Factory = fn(u64) -> Box<dyn Room>;

const NEWEST: usize = 6;
const HOURLY: u64 = 24;
const DAILY: u64 = 14;

struct Job {
    snap: Snap,
    done: Option<Sender<()>>,
}

#[derive(Clone)]
pub struct Store {
    dir: Option<PathBuf>,
    jobs: Option<Sender<Job>>,
}

impl Store {
    /// Rooms kept under `data/rooms`, or nowhere with no data directory.
    pub fn new(data: Option<&Path>) -> Store {
        let Some(dir) = data.map(|d| d.join("rooms")) else {
            return Store {
                dir: None,
                jobs: None,
            };
        };
        let (tx, rx) = channel::<Job>();
        let d = dir.clone();
        thread::spawn(move || {
            for job in rx {
                if let Err(e) = write(&d, &job.snap) {
                    eprintln!("{}: snapshot not written: {e}", job.snap.room);
                }
                if let Some(done) = job.done {
                    let _ = done.send(());
                }
            }
        });
        Store {
            dir: Some(dir),
            jobs: Some(tx),
        }
    }

    /// Keep this snapshot, soon.
    pub fn put(&self, snap: Snap) {
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(Job { snap, done: None });
        }
    }

    /// Keep this snapshot, and return once it is on disk.
    pub fn put_now(&self, snap: Snap) {
        if let Some(jobs) = &self.jobs {
            let (tx, rx) = channel();
            if jobs
                .send(Job {
                    snap,
                    done: Some(tx),
                })
                .is_ok()
            {
                let _ = rx.recv();
            }
        }
    }

    /// A room's snapshots, newest first.
    pub fn snaps(&self, id: &str) -> Vec<(u64, PathBuf)> {
        let Some(dir) = &self.dir else {
            return Vec::new();
        };
        list(&dir.join(id))
    }

    /// A room to host: a fresh one given the newest snapshot that loads
    /// (after skipping `skip`), or a new world if there are none. None when
    /// snapshots exist and not one of them loads.
    pub fn boot(&self, factory: Factory, seed: u64, skip: usize) -> Option<(Box<dyn Room>, u64)> {
        let mut fresh = factory(seed);
        let id = fresh.id();
        let snaps = self.snaps(id);
        if snaps.is_empty() {
            return Some((fresh, 0));
        }
        for (unix, path) in snaps.iter().skip(skip) {
            let loaded = fs::read(path)
                .map_err(|_| "unreadable")
                .and_then(|b| snap::unpack(&b))
                .and_then(|s| match s.room == id {
                    true => Ok(s),
                    false => Err("another room's"),
                })
                .and_then(|s| {
                    let mut room = std::mem::replace(&mut fresh, factory(seed));
                    match catch_unwind(AssertUnwindSafe(|| room.load_snap(s.schema, &s.payload))) {
                        Ok(Ok(())) => Ok(room),
                        Ok(Err(e)) => Err(e),
                        Err(_) => Err("panicked loading"),
                    }
                });
            match loaded {
                Ok(room) => {
                    eprintln!("{id}: loaded {}", path.display());
                    return Some((room, *unix));
                }
                Err(e) => eprintln!("{id}: {} does not load: {e}", path.display()),
            }
        }
        None
    }

    /// Set a room's snapshots aside where nothing rotates them.
    pub fn quarantine(&self, id: &str, now: u64) -> Option<PathBuf> {
        let dir = self.dir.as_ref()?;
        let to = dir
            .parent()?
            .join("quarantine")
            .join(now.to_string())
            .join(id);
        fs::create_dir_all(&to).ok()?;
        for (_, p) in self.snaps(id) {
            if let Some(name) = p.file_name() {
                let _ = fs::copy(&p, to.join(name));
            }
        }
        Some(to)
    }
}

fn list(dir: &Path) -> Vec<(u64, PathBuf)> {
    let mut out: Vec<(u64, PathBuf)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let unix = name
                .strip_prefix("snap-")?
                .strip_suffix(".bin")?
                .parse()
                .ok()?;
            Some((unix, e.path()))
        })
        .collect();
    out.sort_by_key(|s| std::cmp::Reverse(s.0));
    out
}

fn write(root: &Path, snap: &Snap) -> std::io::Result<()> {
    let dir = root.join(&snap.room);
    fs::create_dir_all(&dir)?;
    write_atomic(&dir.join(format!("snap-{}.bin", snap.unix)), &snap.pack())?;
    for gone in rotate(&list(&dir), snap.unix) {
        let _ = fs::remove_file(gone);
    }
    Ok(())
}

/// Put `bytes` at `path` so a crash leaves the old file or the new one,
/// never half of either: a temporary file beside it, synced, renamed into
/// place, and the directory synced (so the rename itself is kept).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let mut f = fs::File::create(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    fs::rename(&tmp, path)?;
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::File::open(dir)?.sync_all()?;
    }
    Ok(())
}

/// What a kept file (the souls, the visit count) holds, on boot.
#[derive(Debug, PartialEq, Eq)]
pub enum Kept<T> {
    /// It read.
    Read(T),
    /// There is none yet, or there was one that does not read and it is
    /// set aside now: start from nothing.
    Fresh,
    /// There is one that does not read, and it could not be moved: start
    /// from nothing, and write nothing there.
    Stuck,
}

/// The file kept at `path`, made sense of by `parse`. One that does not
/// read (the disk will not give it, or `parse` refuses it) is set aside,
/// so that starting again from nothing never writes over it.
pub fn read_kept<T>(
    path: &Path,
    now: u64,
    parse: impl FnOnce(&[u8]) -> Result<T, &'static str>,
) -> Kept<T> {
    let why = match fs::read(path) {
        Ok(bytes) => match parse(&bytes) {
            Ok(t) => return Kept::Read(t),
            Err(e) => e.to_string(),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Kept::Fresh,
        Err(e) => e.to_string(),
    };
    match set_aside(path, now) {
        Some(to) => {
            eprintln!(
                "{}: does not read ({why}); set aside as {}",
                path.display(),
                to.display()
            );
            Kept::Fresh
        }
        None => {
            eprintln!(
                "{}: does not read ({why}), and cannot be moved; not written",
                path.display()
            );
            Kept::Stuck
        }
    }
}

/// A file that does not read, renamed to `<name>.bad-<now>` so nothing
/// writes over it (someone may yet read it); where it went, if it could.
pub fn set_aside(path: &Path, now: u64) -> Option<PathBuf> {
    let mut to = path.as_os_str().to_owned();
    to.push(format!(".bad-{now}"));
    let to = PathBuf::from(to);
    fs::rename(path, &to).ok().map(|_| to)
}

/// The snapshots to delete, of these (newest first) at time `now`.
fn rotate(snaps: &[(u64, PathBuf)], now: u64) -> Vec<PathBuf> {
    let mut hours = std::collections::HashSet::new();
    let mut days = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (i, (unix, path)) in snaps.iter().enumerate() {
        let age = now.saturating_sub(*unix);
        // The newest of each hour and each day: the first one seen.
        let hour = hours.insert(unix / 3600) && age < HOURLY * 3600;
        let day = days.insert(unix / 86_400) && age < DAILY * 86_400;
        if i >= NEWEST && !hour && !day {
            out.push(path.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_keeps_the_newest_then_hours_then_days() {
        let now = 100 * 86_400;
        // A snapshot every ten seconds for twenty days.
        let snaps: Vec<(u64, PathBuf)> = (0..20 * 8640)
            .map(|i| now - i * 10)
            .map(|t| (t, PathBuf::from(t.to_string())))
            .collect();
        let gone = rotate(&snaps, now).len();
        let kept = snaps.len() - gone;
        // 6 newest, about 24 hours, about 14 days (they overlap a little).
        assert!((40..=46).contains(&kept), "{kept}");
        let kept_now = rotate(&snaps[..6], now);
        assert!(kept_now.is_empty());
    }

    #[test]
    fn a_file_is_written_whole_and_one_that_does_not_read_is_kept_aside() {
        let dir = std::env::temp_dir().join(format!("store-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("visits");
        write_atomic(&path, b"12").unwrap();
        write_atomic(&path, b"345").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"345");
        assert!(!dir.join("visits.tmp").exists());
        let to = set_aside(&path, 99).unwrap();
        assert_eq!(to, dir.join("visits.bad-99"));
        assert!(!path.exists() && fs::read(&to).unwrap() == b"345");
        assert_eq!(set_aside(&path, 100), None, "nothing there to set aside");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_kept_file_that_does_not_read_is_never_written_over() {
        let dir = std::env::temp_dir().join(format!("store-kept-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("count");
        let number = |b: &[u8]| -> Result<u64, &'static str> {
            let text = std::str::from_utf8(b).map_err(|_| "not text")?;
            text.trim().parse().map_err(|_| "not a number")
        };
        assert_eq!(read_kept(&path, 1, number), Kept::Fresh, "none yet");
        fs::write(&path, b"41\n").unwrap();
        assert_eq!(read_kept(&path, 2, number), Kept::Read(41));
        // Refused by its reader: set aside.
        fs::write(&path, b"4\xff1").unwrap();
        assert_eq!(read_kept(&path, 3, number), Kept::Fresh);
        assert_eq!(fs::read(dir.join("count.bad-3")).unwrap(), b"4\xff1");
        // The disk will not give it (here, a directory where the file
        // was): set aside too, not taken for none.
        fs::create_dir(&path).unwrap();
        assert_eq!(read_kept(&path, 4, number), Kept::Fresh);
        assert!(dir.join("count.bad-4").is_dir() && !path.exists());
        // And where it cannot be moved (a directory where it would go),
        // nothing is to be written.
        fs::write(&path, b"junk").unwrap();
        fs::create_dir(dir.join("count.bad-5")).unwrap();
        assert_eq!(read_kept(&path, 5, number), Kept::Stuck);
        assert_eq!(fs::read(&path).unwrap(), b"junk");
        let _ = fs::remove_dir_all(&dir);
    }
}
