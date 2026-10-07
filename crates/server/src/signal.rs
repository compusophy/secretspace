//! SIGTERM (a deploy) and SIGINT (ctrl-c) set a flag and nothing else; the
//! rooms see it, hold still, save, and the process leaves cleanly. Having a
//! handler at all matters in a container: the kernel ignores SIGTERM sent
//! to a PID 1 that has none.

use std::sync::atomic::{AtomicBool, Ordering};

static STOP: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
extern "C" {
    fn signal(sig: i32, handler: extern "C" fn(i32)) -> usize;
}

#[cfg(unix)]
extern "C" fn on_signal(_: i32) {
    STOP.store(true, Ordering::SeqCst);
}

pub fn install() {
    #[cfg(unix)]
    // SAFETY: the handler only stores to an atomic, which is
    // async-signal-safe.
    unsafe {
        signal(15, on_signal);
        signal(2, on_signal);
    }
}

/// Whether the server has been asked to stop.
pub fn stopping() -> bool {
    STOP.load(Ordering::Relaxed)
}
