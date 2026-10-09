//! What players send us (`POST /feedback`): their own words, or what went
//! wrong on their page (a crash, a browser that cannot run a game), each
//! with what the page knew of itself. Cleaned and kept, newest last, in
//! `$DATA_DIR/feedback` (the one before in `feedback.old` once it grows
//! past `MOST_KEPT`), and told to the log so a deploy's logs show it.
//! Read back with `GET /feedback?key=...` when `$FEEDBACK_KEY` is set.
//!
//! Never trusting a browser: a report is at most `MOST` bytes, a sender
//! at most `EACH` reports in `WINDOW`, everyone together at most `ALL`;
//! control characters are dropped and senders are kept only as a hash.

use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The most a report may be (bytes; pages send at most 4000).
pub const MOST: usize = 4096;
/// Reports a sender may send in a window, and everyone together.
const EACH: usize = 6;
const ALL: usize = 240;
const WINDOW: Duration = Duration::from_secs(600);
/// The file is started afresh past this many bytes.
const MOST_KEPT: u64 = 16 << 20;
/// How much a read gives back (the newest bytes).
const READ: u64 = 512 << 10;

pub struct Feedback {
    path: Option<PathBuf>,
    /// Who sent what lately: (when, the sender's hash).
    recent: Mutex<VecDeque<(Instant, u64)>>,
    count: AtomicU64,
}

/// Why a report was turned away.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    Empty,
    TooBig,
    TooMany,
}

impl Feedback {
    pub fn new(dir: Option<&std::path::Path>) -> Feedback {
        Feedback {
            path: dir.map(|d| d.join("feedback")),
            recent: Mutex::new(VecDeque::new()),
            count: AtomicU64::new(0),
        }
    }

    /// Reports taken since the server started.
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Take a report from `addr`, sent at `unix` seconds.
    pub fn take(&self, addr: &str, body: &[u8], unix: u64) -> Result<(), Refused> {
        if body.len() > MOST {
            return Err(Refused::TooBig);
        }
        let text = clean(body);
        if text.trim().is_empty() {
            return Err(Refused::Empty);
        }
        let who = hash(addr);
        {
            let mut recent = self.recent.lock().unwrap_or_else(|e| e.into_inner());
            let now = Instant::now();
            while recent
                .front()
                .is_some_and(|r| now.duration_since(r.0) > WINDOW)
            {
                recent.pop_front();
            }
            if recent.len() >= ALL || recent.iter().filter(|r| r.1 == who).count() >= EACH {
                return Err(Refused::TooMany);
            }
            recent.push_back((now, who));
        }
        self.count.fetch_add(1, Ordering::Relaxed);
        let kind = text
            .lines()
            .find_map(|l| l.strip_prefix("kind: "))
            .unwrap_or("feedback");
        let said: String = text
            .split("\n\n")
            .nth(1)
            .unwrap_or(&text)
            .chars()
            .map(|c| if c == '\n' { ' ' } else { c })
            .take(300)
            .collect();
        eprintln!("feedback ({kind}, {who:08x}): {said}");
        if let Some(path) = &self.path {
            if fs::metadata(path).is_ok_and(|m| m.len() > MOST_KEPT) {
                let _ = fs::rename(path, path.with_extension("old"));
            }
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = write!(f, "--- {unix} {who:08x}\n{}\n", text.trim_end());
            }
        }
        Ok(())
    }

    /// The newest reports kept, if `key` is the one set (`$FEEDBACK_KEY`).
    pub fn read(&self, key: &str) -> Option<Vec<u8>> {
        let want = std::env::var("FEEDBACK_KEY")
            .ok()
            .filter(|k| k.len() >= 8)?;
        if !same(want.as_bytes(), key.as_bytes()) {
            return None;
        }
        let bytes = self
            .path
            .as_ref()
            .and_then(|p| fs::read(p).ok())
            .unwrap_or_default();
        let from = bytes.len().saturating_sub(READ as usize);
        Some(bytes[from..].to_vec())
    }
}

/// The report as text: valid UTF-8, no control characters but line
/// breaks and tabs.
fn clean(body: &[u8]) -> String {
    String::from_utf8_lossy(body)
        .chars()
        .filter(|&c| c == '\n' || c == '\t' || !c.is_control())
        .collect()
}

/// A sender kept only as a hash of its address.
fn hash(addr: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in addr.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h & 0xffff_ffff
}

/// Whether two keys are the same, taking as long either way.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_are_cleaned_bounded_and_kept() {
        let dir = std::env::temp_dir().join(format!("feedback-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let f = Feedback::new(Some(&dir));
        assert_eq!(f.take("a", b"   ", 1), Err(Refused::Empty));
        assert_eq!(f.take("a", &[b'x'; MOST + 1], 1), Err(Refused::TooBig));
        let body = b"kind: feedback\npage: /wandfall/\n\nthe storm\x07 is \xff great\x1b[2J";
        assert_eq!(f.take("a", body, 7), Ok(()));
        let kept = fs::read_to_string(dir.join("feedback")).unwrap();
        assert!(kept.starts_with("--- 7 "));
        assert!(kept.contains("the storm is \u{fffd} great[2J"), "{kept}");
        assert!(!kept.contains('\x07') && !kept.contains('\x1b'));
        for _ in 1..EACH {
            assert_eq!(f.take("a", b"again", 8), Ok(()));
        }
        assert_eq!(f.take("a", b"one more", 9), Err(Refused::TooMany));
        assert_eq!(f.take("b", b"someone else", 9), Ok(()));
        assert_eq!(f.count(), EACH as u64 + 1);
        assert!(same(b"abcdefgh", b"abcdefgh") && !same(b"abcdefgh", b"abcdefgx"));
        let _ = fs::remove_dir_all(&dir);
    }
}
