//! A browser's WebSocket once it is open: in a game (`play`) or on the
//! hub (`hub`). This thread reads what it says; a writer writes what it is
//! told. The writer pings it every `PING`, which a browser answers on its
//! own, so a page with nothing to say (the hub, a watcher, a wizard
//! knocked out and watching) is not taken for gone; a page that has
//! stopped reading is let go after `WRITE_WAIT`.

use std::io::{self, ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use engine::room::Who;
use engine::who::{Hello, PLATFORM};

use crate::host::{unix, Event, Game};
use crate::{ws, Shared};

/// Messages a browser may send a second.
const RATE: u32 = 90;
/// A browser that says nothing for this long is gone; it is pinged every
/// `PING`, and a browser answers a ping at once.
const IDLE: Duration = Duration::from_secs(45);
const PING: Duration = Duration::from_secs(15);
/// A browser that takes this long to take what it is sent is gone.
const WRITE_WAIT: Duration = Duration::from_secs(10);
/// A page that has not said Hello by now is a guest.
const HELLO_WAIT: Duration = Duration::from_secs(1);
/// Hellos after the first (a new name): this many at once, then one every
/// `RENAME_EVERY`.
const RENAMES: u32 = 3;
const RENAME_EVERY: Duration = Duration::from_secs(2);

/// Read a browser's frames until it goes quiet, closes, or floods; hand
/// each binary one to `said`.
fn listen(mut reader: impl Read, mut said: impl FnMut(Vec<u8>)) -> io::Result<()> {
    let (mut window, mut count) = (Instant::now(), 0u32);
    loop {
        let frame = ws::read(&mut reader)?;
        if window.elapsed() > Duration::from_secs(1) {
            window = Instant::now();
            count = 0;
        }
        count += 1;
        if count > RATE {
            return Ok(());
        }
        match frame {
            ws::Frame::Close => return Ok(()),
            // Browsers do not ping, and these protocols have no text.
            ws::Frame::Ping(p) => drop(p),
            ws::Frame::Text(t) => drop(t),
            ws::Frame::Pong => {}
            ws::Frame::Binary(b) => said(b),
        }
    }
}

/// Write what the room sends until it lets the page go (or the page stops
/// taking it): what waits at each wakeup in one write, and a Ping each
/// time `ping` has gone by.
fn pump(rx: &Receiver<Vec<u8>>, w: &mut impl Write, ping: Duration) -> io::Result<()> {
    let mut pinged = Instant::now();
    loop {
        let mut out = Vec::new();
        match rx.recv_timeout(ping.saturating_sub(pinged.elapsed())) {
            Ok(msg) => {
                ws::frame(&mut out, ws::BINARY, &msg);
                for msg in rx.try_iter() {
                    ws::frame(&mut out, ws::BINARY, &msg);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        }
        if pinged.elapsed() >= ping {
            ws::frame(&mut out, ws::PING, &[]);
            pinged = Instant::now();
        }
        w.write_all(&out)?;
        w.flush()?;
    }
}

/// Hellos after the first: the same one again is nothing new; a new one
/// is honoured `RENAMES` at once and then one every `RENAME_EVERY`, so a
/// page cannot churn its name (and every page's roster) every tick.
struct Renames {
    last: Vec<u8>,
    left: u32,
    since: Instant,
}

impl Renames {
    fn new(now: Instant) -> Renames {
        Renames {
            last: Vec::new(),
            left: RENAMES,
            since: now,
        }
    }

    /// Whether to honour this Hello now.
    fn allow(&mut self, hello: &[u8], now: Instant) -> bool {
        if hello == self.last {
            return false;
        }
        let back = (now.duration_since(self.since).as_millis() / RENAME_EVERY.as_millis()) as u32;
        if back > 0 {
            self.left = (self.left + back).min(RENAMES);
            self.since = now;
        }
        if self.left == 0 {
            return false;
        }
        if self.left == RENAMES {
            self.since = now;
        }
        self.left -= 1;
        self.last = hello.to_vec();
        true
    }
}

/// A browser in a game: a writer thread drains what its room sends it;
/// this thread reads what it says. Its first message is the platform Hello
/// (anything else, or a second of silence, and it is a guest); platform
/// messages go no further than here.
pub fn play(
    mut reader: impl Read,
    stream: Arc<TcpStream>,
    conn: u32,
    watch: bool,
    addr: &str,
    game: &Game,
    shared: &Shared,
) -> io::Result<()> {
    if game.state() != "running" {
        return Ok(());
    }
    stream.set_write_timeout(Some(WRITE_WAIT))?;
    let (tx, rx) = sync_channel::<Vec<u8>>(game.backlog);
    let w = stream.clone();
    let pump = thread::Builder::new().spawn(move || {
        let mut s: &TcpStream = &w;
        let _ = pump(&rx, &mut s, PING);
        let _ = ws::write(&mut s, ws::CLOSE, &[]);
        let _ = w.shutdown(Shutdown::Both);
    })?;
    let mut who = Who::guest(watch);
    let mut first = None;
    let mut renames = Renames::new(Instant::now());
    if !watch {
        stream.set_read_timeout(Some(HELLO_WAIT))?;
        match ws::read(&mut reader) {
            Ok(ws::Frame::Binary(b)) => match Hello::decode(&b) {
                Some(h) => {
                    let (w, seen) = shared.souls().hello(&h, addr, unix(), watch);
                    who = w;
                    let _ = tx.try_send(seen.encode());
                    renames.last = b;
                }
                None => first = Some(b),
            },
            Ok(ws::Frame::Close) => return Ok(()),
            Ok(_) => {}
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) => return Err(e),
        }
    }
    stream.set_read_timeout(Some(IDLE))?;
    let soul = |w: &Who| if w.watch { 0 } else { w.soul };
    let me = soul(&who);
    let started = game.events.send(Event::Open(conn, tx, who)).is_ok();
    if let (true, Some(b)) = (started, first) {
        let _ = game.events.send(Event::Say(conn, b));
    }
    let result = if started {
        shared.souls().enter(conn, me);
        listen(reader, |b| {
            if b.first() != Some(&PLATFORM) {
                let _ = game.events.send(Event::Say(conn, b));
            } else if let Some(h) = Hello::decode(&b).filter(|_| renames.allow(&b, Instant::now()))
            {
                // Hello again: a new name, most likely.
                let mut souls = shared.souls();
                let (w, seen) = souls.hello(&h, addr, unix(), watch);
                souls.enter(conn, soul(&w));
                drop(souls);
                let _ = game.events.send(Event::Who(conn, w, seen.encode()));
            }
        })
    } else {
        Ok(())
    };
    shared.souls().leave(conn);
    let _ = game.events.send(Event::Close(conn));
    let _ = stream.shutdown(Shutdown::Both);
    let _ = pump.join();
    result
}

/// A browser on the hub: told the numbers now and once a second after.
pub fn hub(reader: impl Read, stream: &TcpStream, shared: &Shared) -> io::Result<()> {
    stream.set_read_timeout(Some(IDLE))?;
    stream.set_write_timeout(Some(WRITE_WAIT))?;
    shared.hub.fetch_add(1, Ordering::Relaxed);
    let done = AtomicBool::new(false);
    let result = thread::scope(|scope| {
        let writer = thread::Builder::new().spawn_scoped(scope, || {
            let mut w = stream;
            let mut pinged = Instant::now();
            while !done.load(Ordering::Relaxed) {
                let mut out = Vec::new();
                ws::frame(&mut out, ws::BINARY, &shared.stats().encode());
                if pinged.elapsed() >= PING {
                    ws::frame(&mut out, ws::PING, &[]);
                    pinged = Instant::now();
                }
                if w.write_all(&out).is_err() {
                    break;
                }
                for _ in 0..10 {
                    if done.load(Ordering::Relaxed) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
            let _ = stream.shutdown(Shutdown::Both);
        });
        if writer.is_err() {
            return Ok(());
        }
        let r = listen(reader, drop);
        done.store(true, Ordering::Relaxed);
        r
    });
    let _ = stream.shutdown(Shutdown::Both);
    shared.hub.fetch_sub(1, Ordering::Relaxed);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quiet_page_is_pinged_and_a_busy_one_too() {
        // Nothing to say for a while: pings.
        let (tx, rx) = sync_channel::<Vec<u8>>(8);
        let t = thread::spawn(move || {
            let mut out = Vec::new();
            pump(&rx, &mut out, Duration::from_millis(20)).unwrap();
            out
        });
        thread::sleep(Duration::from_millis(110));
        drop(tx);
        let out = t.join().unwrap();
        assert!(out.len() >= 6, "pinged: {out:?}");
        assert!(out.chunks(2).all(|c| c == [0x80 | ws::PING, 0]), "{out:?}");

        // A message a tick: still pinged on the clock, and what waits
        // goes out together.
        let (tx, rx) = sync_channel::<Vec<u8>>(8);
        let t = thread::spawn(move || {
            let mut out = Vec::new();
            pump(&rx, &mut out, Duration::from_millis(20)).unwrap();
            out
        });
        for _ in 0..30 {
            tx.send(vec![1, 2, 3]).unwrap();
            thread::sleep(Duration::from_millis(4));
        }
        drop(tx);
        let out = t.join().unwrap();
        let (mut msgs, mut pings) = (0, 0);
        let mut r = out.as_slice();
        while !r.is_empty() {
            let (op, n) = (r[0] & 0x0f, r[1] as usize);
            match op {
                ws::BINARY => msgs += 1,
                ws::PING => pings += 1,
                _ => panic!("opcode {op}"),
            }
            r = &r[2 + n..];
        }
        assert_eq!(msgs, 30);
        assert!(pings >= 2, "{pings}");
    }

    #[test]
    fn a_page_renames_now_and_then_never_every_tick() {
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        let mut r = Renames::new(t0);
        r.last = b"first".to_vec();
        assert!(
            !r.allow(b"first", at(10)),
            "the same Hello again is nothing"
        );
        // A few at once (a name typed, then fixed)...
        assert!(r.allow(b"a", at(20)));
        assert!(r.allow(b"b", at(30)));
        assert!(r.allow(b"c", at(40)));
        // ...then no more than one every RENAME_EVERY.
        assert!(!r.allow(b"d", at(50)));
        assert!(!r.allow(b"d", at(1500)));
        assert!(r.allow(b"d", at(2100)));
        assert!(!r.allow(b"e", at(2200)));
        let flood = (0..1000)
            .filter(|&k| r.allow(&[k as u8, 1], at(2300 + k * 10)))
            .count();
        assert!(flood <= 6, "{flood} in ten seconds");
    }
}
