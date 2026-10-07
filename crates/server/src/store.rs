//! Where rooms keep their worlds: `$DATA_DIR/rooms/<id>/snap-<unix>.bin`.
//!
//! - A writer thread takes each snapshot, writes it to a temporary file,
//!   syncs it, renames it into place and syncs the directory, so a crash
//!   leaves the old file or the new one, never half of either.
//! - It keeps the newest 6, one an hour for a day, one a day for 14 days.
//! - On boot each attempt is a fresh room from its factory, given the
//!   newest snapshot, then older ones. A fresh world is made only when
//!   there is no snapshot at all; when there are some and none loads, the
//!   room stays closed and its files are copied to `quarantine/<unix>/`.

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
                    match catch_unwind(AssertUnwindSafe(|| room.load(&s.payload))) {
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
    let path = dir.join(format!("snap-{}.bin", snap.unix));
    let tmp = dir.join(format!("snap-{}.tmp", snap.unix));
    let mut f = fs::File::create(&tmp)?;
    f.write_all(&snap.pack())?;
    f.sync_all()?;
    fs::rename(&tmp, &path)?;
    fs::File::open(&dir)?.sync_all()?;
    for gone in rotate(&list(&dir), snap.unix) {
        let _ = fs::remove_file(gone);
    }
    Ok(())
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
}
