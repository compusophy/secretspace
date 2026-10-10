//! A socket to a room that stays up: it says the platform Hello first on
//! every open, and when it drops (or the server says `Still`, a deploy) it
//! tries again after 250 ms, then 1.6 times longer each time, jittered,
//! never more than 4 s apart. Only a socket that lasted starts the wait
//! over, so a server that takes the page and drops it at once is not
//! asked four times a second. A socket gone quiet (every room and the hub
//! say something every second or two) or one still connecting after a
//! while is given up on, as after a change of network; but only while the
//! page runs: a page that stalled itself does not blame the socket.
//!
//! The page polls it once a frame. A page that is not polled (a hidden
//! tab) is not sent a backlog to play through all at once when it comes
//! back: a watcher lets go after a few seconds, anyone after
//! `MOST_QUEUED` messages, and the next poll connects again, afresh.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use web_sys::{BinaryType, MessageEvent, WebSocket};

const FIRST: f64 = 250.0;
const GROW: f64 = 1.6;
const MOST: f64 = 4000.0;
/// A socket up this long worked: the wait after it starts from `FIRST`.
const LASTED: f64 = 5000.0;
/// Nothing heard for this long, an open socket is taken for dead.
const QUIET: f64 = 5000.0;
/// Still connecting after this long, a socket is given up on.
const CONNECTING: f64 = 6000.0;
/// Polls this far apart, the page itself stalled (a long build, a phone
/// compiling pipelines, a frozen tab woken): what the socket said
/// meanwhile may still be queued behind this very poll.
const STALLED: f64 = QUIET / 2.0;
/// Not polled for this long, a watcher lets go (nobody is looking).
const AWAY: f64 = 5000.0;
/// The most messages kept for the next poll; past them, the link lets go.
pub const MOST_QUEUED: usize = 2048;
/// Bytes waiting to go out past which a send is dropped (the socket is
/// stuck, and what it says would arrive late).
const BACKED_UP: u32 = 64 * 1024;

/// What happened since the last poll.
pub enum Net {
    /// Connected, and Hello said: say again whatever the room needs.
    Up,
    /// Lost, or told to hold still: keep the picture, it is coming back.
    Holding,
    /// A message: the platform's `Seen`, or the room's own.
    Message(Vec<u8>),
}

/// A link's clock, apart from its socket: when to try again, and when a
/// socket is given up on.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pace {
    /// The wait after the next loss (before jitter).
    delay: f64,
    /// When to connect again, while there is no socket.
    retry_at: f64,
    /// When the socket was made, when it opened (if it has), and when
    /// anything was last heard on it.
    made: f64,
    opened: Option<f64>,
    heard: f64,
}

impl Pace {
    fn new() -> Pace {
        Pace {
            delay: FIRST,
            retry_at: 0.0,
            made: 0.0,
            opened: None,
            heard: 0.0,
        }
    }

    fn made(&mut self, now: f64) {
        (self.made, self.opened, self.heard) = (now, None, now);
    }

    fn opened(&mut self, now: f64) {
        (self.opened, self.heard) = (Some(now), now);
    }

    /// Polled at `now`, `gap` after the last poll. After a stall the
    /// socket's silence is not held against it: the clock starts again
    /// now, so only a socket quiet (or connecting) while the page runs
    /// is given up on.
    fn polled(&mut self, gap: f64, now: f64) {
        if gap > STALLED {
            self.heard = self.heard.max(now);
            if self.opened.is_none() {
                self.made = self.made.max(now);
            }
        }
    }

    /// Whether the socket is to be given up on: open and quiet too long,
    /// or still connecting.
    fn dead(&self, now: f64) -> bool {
        match self.opened {
            Some(_) => now - self.heard > QUIET,
            None => now - self.made > CONNECTING,
        }
    }

    /// The socket is lost: try again after the wait (times `jitter`, 0.8
    /// to 1.2), and wait longer the next time unless this one lasted.
    fn lost(&mut self, now: f64, jitter: f64) {
        if self.opened.is_some_and(|t| now - t >= LASTED) {
            self.delay = FIRST;
        }
        self.retry_at = now + self.delay * jitter;
        self.delay = (self.delay * GROW).min(MOST);
        self.opened = None;
    }

    /// Let go on purpose (nobody polls): connect again at the next poll.
    fn let_go(&mut self, now: f64) {
        (self.delay, self.retry_at, self.opened) = (FIRST, now, None);
    }
}

struct Inner {
    room: String,
    counted: bool,
    watch: bool,
    hello: Vec<u8>,
    ws: Option<WebSocket>,
    /// Which socket is current; events from older ones are ignored.
    gen: u32,
    events: VecDeque<Net>,
    pace: Pace,
    /// When the page last polled.
    polled: f64,
    up: bool,
}

pub struct Link(Rc<RefCell<Inner>>);

impl Link {
    /// A link to `room`; its first connection to open counts a visit
    /// unless it only watches (`watch`, which never says Hello). An empty
    /// `hello` is not said (the hub's numbers need none).
    pub fn open(room: &str, hello: Vec<u8>, watch: bool) -> Link {
        Link(Rc::new(RefCell::new(Inner {
            room: room.to_string(),
            counted: watch,
            watch,
            hello,
            ws: None,
            gen: 0,
            events: VecDeque::new(),
            pace: Pace::new(),
            polled: 0.0,
            up: false,
        })))
    }

    /// The Hello said on every open from now on.
    pub fn set_hello(&self, hello: Vec<u8>) {
        self.0.borrow_mut().hello = hello;
    }

    pub fn up(&self) -> bool {
        self.0.borrow().up
    }

    /// Hang up: events from the socket are ignored from now on. Nothing
    /// reconnects while the page does not poll; polled again, it does.
    pub fn close(&self) {
        let mut i = self.0.borrow_mut();
        i.gen += 1;
        i.up = false;
        if let Some(ws) = i.ws.take() {
            let _ = ws.close();
        }
    }

    pub fn send(&self, bytes: &[u8]) {
        let inner = self.0.borrow();
        if let Some(ws) = inner.ws.as_ref().filter(|_| inner.up) {
            if ws.buffered_amount() <= BACKED_UP {
                let _ = ws.send_with_u8_array(bytes);
            }
        }
    }

    /// Connect when it is time (or give up on a socket gone quiet); what
    /// happened since the last poll.
    pub fn poll(&self, now: f64) -> Vec<Net> {
        let (due, dead, gen) = {
            let mut i = self.0.borrow_mut();
            let gap = now - i.polled;
            i.polled = now;
            i.pace.polled(gap, now);
            let dead = i.ws.is_some() && i.pace.dead(now);
            (i.ws.is_none() && now >= i.pace.retry_at, dead, i.gen)
        };
        if dead {
            self.lost(gen);
        } else if due {
            self.connect(now);
        }
        self.0.borrow_mut().events.drain(..).collect()
    }

    fn connect(&self, now: f64) {
        let (url, gen) = {
            let mut i = self.0.borrow_mut();
            let q = match (i.watch, i.counted) {
                (true, _) => "watch=1",
                (false, false) => "v=1",
                _ => "",
            };
            i.gen += 1;
            i.pace.made(now);
            (crate::room_url(&i.room, q), i.gen)
        };
        let Ok(ws) = WebSocket::new(&url) else {
            self.lost(gen);
            return;
        };
        ws.set_binary_type(BinaryType::Arraybuffer);
        let me = self.0.clone();
        crate::on(&ws, "open", move |_| {
            let mut i = me.borrow_mut();
            if i.gen != gen {
                return;
            }
            i.up = true;
            // The visit is counted once a connection has really been made.
            i.counted = true;
            i.pace.opened(crate::now());
            if !i.watch && !i.hello.is_empty() {
                if let Some(ws) = &i.ws {
                    let _ = ws.send_with_u8_array(&i.hello);
                }
            }
            i.events.push_back(Net::Up);
        });
        let me = self.0.clone();
        crate::on(&ws, "message", move |e| {
            let Ok(e) = e.dyn_into::<MessageEvent>() else {
                return;
            };
            let bytes = js_sys::Uint8Array::new(&e.data()).to_vec();
            let mut i = me.borrow_mut();
            if i.gen != gen {
                return;
            }
            let now = crate::now();
            i.pace.heard = now;
            let away = i.watch && now - i.polled > AWAY;
            if away || i.events.len() >= MOST_QUEUED {
                // Nobody is reading: let go, and start afresh when they do.
                if let Some(ws) = i.ws.take() {
                    let _ = ws.close();
                }
                i.events.clear();
                i.events.push_back(Net::Holding);
                i.up = false;
                i.gen += 1;
                i.pace.let_go(now);
                return;
            }
            if engine::who::is_still(&bytes) {
                if let Some(ws) = i.ws.take() {
                    let _ = ws.close();
                }
                drop(i);
                Link(me.clone()).lost(gen);
                return;
            }
            i.events.push_back(Net::Message(bytes));
        });
        for ev in ["close", "error"] {
            let me = self.0.clone();
            crate::on(&ws, ev, move |_| Link(me.clone()).lost(gen));
        }
        self.0.borrow_mut().ws = Some(ws);
    }

    /// The socket of this generation is gone: hold, and try again later.
    fn lost(&self, gen: u32) {
        let mut i = self.0.borrow_mut();
        if i.gen != gen {
            return;
        }
        if let Some(ws) = i.ws.take() {
            let _ = ws.close();
        }
        i.up = false;
        let jitter = 0.8 + js_sys::Math::random() * 0.4;
        i.pace.lost(crate::now(), jitter);
        i.events.push_back(Net::Holding);
        // Events from this socket no longer count.
        i.gen += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_that_drops_the_page_at_once_is_asked_less_and_less() {
        let mut p = Pace::new();
        let mut now = 0.0;
        let mut waits = Vec::new();
        for _ in 0..8 {
            p.made(now);
            p.opened(now + 20.0);
            // A message (a Seen, a Welcome), then gone.
            p.heard = now + 30.0;
            p.lost(now + 40.0, 1.0);
            waits.push(p.retry_at - (now + 40.0));
            now = p.retry_at;
        }
        assert_eq!(waits[0], FIRST);
        assert!(waits.windows(2).all(|w| w[1] >= w[0]), "{waits:?}");
        assert_eq!(*waits.last().unwrap(), MOST);
    }

    #[test]
    fn a_socket_that_lasted_comes_back_fast() {
        let mut p = Pace::new();
        // A few failures first: the wait has grown.
        for k in 0..5 {
            p.made(k as f64 * 5000.0);
            p.lost(k as f64 * 5000.0 + 10.0, 1.0);
        }
        assert!(p.delay > FIRST);
        p.made(30_000.0);
        p.opened(30_050.0);
        // A deploy, a minute on: back after the first wait.
        p.lost(90_000.0, 1.0);
        assert_eq!(p.retry_at, 90_000.0 + FIRST);
    }

    #[test]
    fn a_quiet_socket_or_a_stuck_connect_is_given_up_on() {
        let mut p = Pace::new();
        p.made(1000.0);
        assert!(!p.dead(1000.0 + CONNECTING - 1.0));
        assert!(p.dead(1000.0 + CONNECTING + 1.0), "still connecting");
        p.opened(2000.0);
        p.heard = 9000.0;
        assert!(!p.dead(9000.0 + QUIET - 1.0));
        assert!(p.dead(9000.0 + QUIET + 1.0), "quiet");
        p.let_go(20_000.0);
        assert_eq!((p.retry_at, p.delay), (20_000.0, FIRST));
    }

    #[test]
    fn a_page_that_stalled_does_not_drop_a_healthy_socket() {
        let mut p = Pace::new();
        p.made(1000.0);
        p.opened(1100.0);
        p.heard = 2000.0;
        // The page hangs for 7 s (a build); the messages that came
        // meanwhile wait behind the first poll after it.
        p.polled(7000.0, 9000.0);
        assert!(!p.dead(9000.0), "the stall is not the socket's");
        // Polled every frame from then on and still quiet: gone.
        let mut now = 9000.0;
        while now < 9000.0 + QUIET + 100.0 {
            now += 16.0;
            p.polled(16.0, now);
        }
        assert!(p.dead(now), "quiet while the page ran");

        // Connecting when the page hung: its open may be queued too.
        let mut p = Pace::new();
        p.made(1000.0);
        p.polled(CONNECTING + 1000.0, CONNECTING + 2000.0);
        assert!(!p.dead(CONNECTING + 2000.0));
        p.polled(16.0, 2.0 * CONNECTING + 2100.0);
        assert!(p.dead(2.0 * CONNECTING + 2100.0), "stuck connecting");
    }
}
