//! Whether the page loads itself again for something newer than it: a
//! build `/version.txt` names, or a room on a newer protocol. CI puts the
//! server out first and the pages after it, so a page can hear of a newer
//! one before it is there to load. Each reload leaves a mark (this page,
//! what it was for, when), kept across it: the same page back for the
//! same thing soon after waits, since another try now would only loop.
//! Something else to load for, or a while later, is worth one more try.

/// How long a page that reloaded for something, and came back the same,
/// waits before it tries again for it (ms: the pages follow the server
/// out within a minute or two).
pub const RETRY: f64 = 60_000.0;

/// A reload for a newer build (`/version.txt` named one).
pub const BUILD: &str = "build";

/// A reload for a room on protocol `v`.
pub fn proto(v: u8) -> String {
    format!("proto{v}")
}

/// The mark a reload leaves: this page, what it was for, when (ms of the
/// wall clock, which goes on across a reload).
pub fn mark(page: &str, what: &str, now: f64) -> String {
    format!("{page} {what} {}", now.max(0.0) as u64)
}

/// Whether `page` may load again for `what` at `now`, after the mark the
/// last reload left: not if it was this page, for this, less than
/// `RETRY` ago. A mark it cannot read (none, an old one) holds nothing.
pub fn may(last: Option<&str>, page: &str, what: &str, now: f64) -> bool {
    let mut w = last.unwrap_or_default().split(' ');
    let (p, f) = (w.next(), w.next());
    let at = w.next().and_then(|t| t.parse::<f64>().ok());
    match (p, f, at, w.next()) {
        (Some(p), Some(f), Some(at), None) if p == page && f == what => {
            // The clock set back is no reason to wait.
            !(at..at + RETRY).contains(&now)
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deploy as CI does it: the server (protocol 18) out first, the
    /// pages (A to B) a while after.
    #[test]
    fn a_page_ahead_of_its_pages_waits_then_tries_again() {
        let (a, b) = ("1a2b3c4d", "5e6f7a8b");
        let t0 = 1.79e12;
        let room = proto(18);
        // A meets the new room: it reloads for it...
        assert!(may(None, a, &room, t0));
        let m = mark(a, &room, t0);
        // ...and comes back as A (B is not out yet): it waits, not loops.
        assert!(!may(Some(&m), a, &room, t0 + 2_000.0));
        // Play pressed again soon: still A, still waiting.
        assert!(!may(Some(&m), a, &room, t0 + 30_000.0));
        // B is out, `/version.txt` says so: one more try, for that.
        assert!(may(Some(&m), a, BUILD, t0 + 40_000.0));
        let m2 = mark(a, BUILD, t0 + 40_000.0);
        // If the same A came back again, that waits too...
        assert!(!may(Some(&m2), a, BUILD, t0 + 45_000.0));
        // ...but the room's own try is its own.
        assert!(may(Some(&m2), a, &room, t0 + 45_000.0));
        // The pages slower than that: a minute on, one more try anyway.
        assert!(may(Some(&m), a, &room, t0 + RETRY));
        // B, loaded, never waits on what A left.
        assert!(may(Some(&m), b, &room, t0 + 2_000.0));
        assert!(may(Some(&m), b, BUILD, t0 + 2_000.0));
    }

    #[test]
    fn a_mark_it_cannot_read_holds_nothing() {
        let a = "1a2b3c4d";
        // The old mark ("from <page>"), none, nonsense.
        for last in [
            Some("from 1a2b3c4d"),
            None,
            Some(""),
            Some("1a2b3c4d build x"),
        ] {
            assert!(may(last, a, BUILD, 1.0e12), "{last:?}");
        }
        assert!(may(Some("1a2b3c4d build 5 more"), a, BUILD, 6.0));
        // The clock set back past the mark.
        let m = mark(a, BUILD, 5.0e11);
        assert!(may(Some(&m), a, BUILD, 4.0e11));
        assert!(!may(Some(&m), a, BUILD, 5.0e11));
    }
}
