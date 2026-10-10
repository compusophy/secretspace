//! One room's own thread: events in, a tick, messages out, a snapshot every
//! ten seconds, forever.
//!
//! Every call into the room is behind `catch_unwind`. A room that panics is
//! rebuilt from its factory and its newest snapshot, and every page there
//! is told `Still` and let go, so it reconnects into the rebuilt world. A
//! third panic in ten minutes goes back one snapshot further; a fourth
//! stops the room for good (`/health` names it), and the other rooms carry
//! on.

use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use engine::room::{Outbox, Room, Who};
use engine::snap::Snap;
use engine::who;

use crate::store::{Factory, Store};

pub enum Event {
    /// A browser is here: its sender, and who it is.
    Open(u32, SyncSender<Vec<u8>>, Who),
    /// It said Hello again: who it is now, and the `Seen` to send it.
    Who(u32, Who, Vec<u8>),
    /// An answer for the page alone (the room is not told), sent in its
    /// turn with what the room sends. Only the room holds a page's
    /// sender, so letting go of it still lets the page go.
    Tell(u32, Vec<u8>),
    Say(u32, Vec<u8>),
    Close(u32),
}

pub const RUNNING: u8 = 0;
pub const QUARANTINED: u8 = 1;
pub const STOPPED: u8 = 2;

/// A room as every connection thread sees it.
pub struct Game {
    pub id: &'static str,
    /// People connected (watchers aside), and everyone in the game, bots too.
    pub people: AtomicUsize,
    pub playing: AtomicUsize,
    pub backlog: usize,
    pub reserved: &'static [&'static str],
    pub state: AtomicU8,
    pub panics: AtomicUsize,
    pub stats: Mutex<Vec<(&'static str, i64)>>,
    pub events: Sender<Event>,
    /// It has held still and saved, and the server may stop.
    pub done: AtomicBool,
}

impl Game {
    pub fn state(&self) -> &'static str {
        match self.state.load(Ordering::Relaxed) {
            RUNNING => "running",
            QUARANTINED => "quarantined",
            _ => "stopped",
        }
    }
}

pub fn unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Boot a room (from its newest snapshot that loads) and host it in a
/// thread of its own. `stopping` says when the server is going down.
pub fn start(factory: Factory, seed: u64, store: Store, stopping: fn() -> bool) -> Arc<Game> {
    let probe = factory(seed);
    let (events, inbox) = channel::<Event>();
    let game = Arc::new(Game {
        id: probe.id(),
        people: AtomicUsize::new(0),
        playing: AtomicUsize::new(0),
        backlog: probe.backlog().max(1),
        reserved: probe.reserved(),
        state: AtomicU8::new(RUNNING),
        panics: AtomicUsize::new(0),
        stats: Mutex::new(Vec::new()),
        events,
        done: AtomicBool::new(false),
    });
    drop(probe);
    let g = game.clone();
    match store.boot(factory, seed, 0) {
        Some((room, _)) => {
            let host = Host {
                factory,
                seed,
                store,
                game: g,
                stopping,
            };
            thread::spawn(move || host.run(room, inbox));
        }
        None => {
            let to = store.quarantine(game.id, unix());
            eprintln!(
                "{}: no snapshot loads; closed, its files kept in {}",
                game.id,
                to.map(|p| p.display().to_string()).unwrap_or_default()
            );
            game.state.store(QUARANTINED, Ordering::Relaxed);
            thread::spawn(move || closed(inbox, &g));
        }
    }
    game
}

/// A room that is not running: everyone who comes is let go at once.
fn closed(inbox: Receiver<Event>, game: &Game) {
    game.done.store(true, Ordering::Relaxed);
    for ev in inbox {
        drop(ev);
    }
}

struct Host {
    factory: Factory,
    seed: u64,
    store: Store,
    game: Arc<Game>,
    stopping: fn() -> bool,
}

/// What a tick reports, out of the room.
struct Ticked {
    people: usize,
    playing: usize,
    save: Option<Vec<u8>>,
    stats: Option<Vec<(&'static str, i64)>>,
}

type Clients = HashMap<u32, SyncSender<Vec<u8>>>;

fn step(
    room: &mut dyn Room,
    clients: &mut Clients,
    events: Vec<Event>,
    out: &mut Outbox,
    n: u64,
) -> Ticked {
    for ev in events {
        match ev {
            Event::Open(conn, tx, who) => {
                clients.insert(conn, tx);
                room.open(conn, &who, out);
            }
            Event::Who(conn, who, seen) => {
                if clients.contains_key(&conn) {
                    out.send(conn, seen);
                    room.who(conn, &who);
                }
            }
            Event::Tell(conn, msg) => {
                if clients.contains_key(&conn) {
                    out.send(conn, msg);
                }
            }
            Event::Say(conn, bytes) => room.message(conn, &bytes, out),
            Event::Close(conn) => {
                if clients.remove(&conn).is_some() {
                    room.close(conn);
                }
            }
        }
    }
    room.tick(out);
    let hz = room.hz().max(1) as u64;
    Ticked {
        people: room.people(),
        playing: room.playing(),
        save: n.is_multiple_of(10 * hz).then(|| room.save()).flatten(),
        stats: n.is_multiple_of(hz).then(|| room.stats()),
    }
}

/// Send each message to its browser; the ones that cannot keep up. Once
/// one cannot take a message it is sent nothing more (it is let go next
/// tick): a stream with a hole in it is worse than none.
fn deliver(out: Outbox, clients: &Clients) -> Vec<u32> {
    let mut gone = Vec::new();
    for (conn, msg) in out.0 {
        if gone.contains(&conn) {
            continue;
        }
        if let Some(tx) = clients.get(&conn) {
            if let Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) = tx.try_send(msg) {
                gone.push(conn);
            }
        }
    }
    gone
}

impl Host {
    fn snap(&self, room: &dyn Room, n: u64, payload: Vec<u8>) -> Snap {
        Snap {
            room: self.game.id.into(),
            schema: room.schema(),
            tick: n,
            unix: unix(),
            payload,
        }
    }

    fn run(self, mut room: Box<dyn Room>, inbox: Receiver<Event>) {
        let id = self.game.id;
        let mut clients = Clients::new();
        let tick = Duration::from_micros(1_000_000 / room.hz().max(1) as u64);
        let mut panics: Vec<Instant> = Vec::new();
        let mut gone: Vec<u32> = Vec::new();
        let mut n = 0u64;
        let mut next = Instant::now();
        loop {
            if (self.stopping)() {
                return self.hold(room, clients, inbox, n);
            }
            let events: Vec<Event> = gone
                .drain(..)
                .map(Event::Close)
                .chain(inbox.try_iter())
                .collect();
            let mut out = Outbox::default();
            n += 1;
            let ticked = catch_unwind(AssertUnwindSafe(|| {
                step(&mut *room, &mut clients, events, &mut out, n)
            }));
            match ticked {
                Ok(t) => {
                    gone = deliver(out, &clients);
                    self.game.people.store(t.people, Ordering::Relaxed);
                    self.game.playing.store(t.playing, Ordering::Relaxed);
                    if let Some(payload) = t.save {
                        self.store.put(self.snap(&*room, n, payload));
                    }
                    if let Some(stats) = t.stats {
                        *self.game.stats.lock().unwrap_or_else(|e| e.into_inner()) = stats;
                    }
                }
                Err(_) => {
                    let now = Instant::now();
                    panics.retain(|t| now - *t < Duration::from_secs(600));
                    panics.push(now);
                    self.game.panics.fetch_add(1, Ordering::Relaxed);
                    // Everyone holds still, and comes back to the rebuilt
                    // room; dropping a sender lets its connection go.
                    for (_, tx) in clients.drain() {
                        let _ = tx.try_send(who::still());
                    }
                    gone.clear();
                    let rebuilt = match panics.len() {
                        1 | 2 => self.store.boot(self.factory, self.seed, 0),
                        3 => self.store.boot(self.factory, self.seed, 1),
                        _ => None,
                    };
                    let Some((r, from)) = rebuilt else {
                        eprintln!("{id}: panicked {} times; stopped", panics.len());
                        self.game.state.store(STOPPED, Ordering::Relaxed);
                        self.game.people.store(0, Ordering::Relaxed);
                        self.game.playing.store(0, Ordering::Relaxed);
                        return closed(inbox, &self.game);
                    };
                    eprintln!("{id}: panicked; rebuilt from snapshot {from}");
                    room = r;
                }
            }
            next += tick;
            let now = Instant::now();
            if next > now {
                thread::sleep(next - now);
            } else if now - next > Duration::from_secs(1) {
                next = now;
            }
        }
    }

    /// The server is stopping: hold still, tell every page, save, and wait
    /// for the end with the connections open.
    fn hold(self, mut room: Box<dyn Room>, clients: Clients, inbox: Receiver<Event>, n: u64) {
        let mut out = Outbox::default();
        let saved = catch_unwind(AssertUnwindSafe(|| {
            room.still(&mut out);
            room.save()
        }));
        deliver(out, &clients);
        for tx in clients.values() {
            let _ = tx.try_send(who::still());
        }
        match saved {
            Ok(Some(payload)) => {
                self.store.put_now(self.snap(&*room, n, payload));
                eprintln!("{}: held still and saved", self.game.id);
            }
            Ok(None) => {}
            Err(_) => eprintln!("{}: panicked holding still; not saved", self.game.id),
        }
        self.game.done.store(true, Ordering::Relaxed);
        // Pages that arrive now are let go; the ones here keep their
        // picture until the process ends.
        for ev in inbox {
            drop(ev);
        }
        drop(clients);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Counter {
        id: &'static str,
        ticks: u64,
        boom: u64,
    }

    impl Room for Counter {
        fn id(&self) -> &'static str {
            self.id
        }
        fn hz(&self) -> u32 {
            200
        }
        fn open(&mut self, _: u32, _: &Who, _: &mut Outbox) {}
        fn message(&mut self, _: u32, _: &[u8], _: &mut Outbox) {}
        fn close(&mut self, _: u32) {}
        fn tick(&mut self, _: &mut Outbox) {
            self.ticks += 1;
            if self.ticks == self.boom {
                panic!("boom (a test room, on purpose)");
            }
        }
        fn people(&self) -> usize {
            0
        }
        fn playing(&self) -> usize {
            self.ticks as usize
        }
    }

    fn calm(_: u64) -> Box<dyn Room> {
        Box::new(Counter {
            id: "calm",
            ticks: 0,
            boom: 0,
        })
    }

    fn boom(_: u64) -> Box<dyn Room> {
        Box::new(Counter {
            id: "boom",
            ticks: 0,
            boom: 5,
        })
    }

    fn never() -> bool {
        false
    }

    /// Wait until `ok` holds (at most five seconds, so a busy machine
    /// does not fail the test); whether it did.
    fn soon(ok: impl Fn() -> bool) -> bool {
        let end = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < end {
            if ok() {
                return true;
            }
            thread::sleep(Duration::from_millis(10));
        }
        ok()
    }

    #[test]
    fn host_survives_a_panicking_room() {
        let a = start(boom, 1, Store::new(None), never);
        let b = start(calm, 1, Store::new(None), never);
        assert!(soon(|| a.panics.load(Ordering::Relaxed) >= 1));
        assert!(
            soon(|| b.playing.load(Ordering::Relaxed) > 20),
            "the calm room ticks"
        );
        let before = b.playing.load(Ordering::Relaxed);
        assert!(
            soon(|| b.playing.load(Ordering::Relaxed) > before),
            "and keeps ticking"
        );
        assert_eq!(b.state(), "running");
    }

    #[test]
    fn a_page_that_falls_behind_is_sent_nothing_more() {
        let (tx, rx) = std::sync::mpsc::sync_channel(2);
        let (tx2, rx2) = std::sync::mpsc::sync_channel(8);
        let clients: Clients = [(1, tx), (2, tx2)].into_iter().collect();
        let mut out = Outbox::default();
        for k in 0..4u8 {
            out.send(1, vec![k]);
            out.send(2, vec![k]);
        }
        // The first room for two: the third is turned away, and so is
        // the fourth, though by then there may be room again.
        assert_eq!(deliver(out, &clients), vec![1]);
        assert_eq!(rx.try_iter().collect::<Vec<_>>(), vec![vec![0], vec![1]]);
        assert_eq!(rx2.try_iter().count(), 4);
    }

    #[test]
    fn a_room_that_keeps_panicking_is_stopped() {
        let a = start(boom, 1, Store::new(None), never);
        assert!(soon(|| a.state() == "stopped"));
        assert_eq!(a.panics.load(Ordering::Relaxed), 4);
        // Anyone who comes now is let go.
        let (tx, rx) = std::sync::mpsc::sync_channel(4);
        let _ = a.events.send(Event::Open(1, tx, Who::guest(false)));
        thread::sleep(Duration::from_millis(50));
        assert!(rx.recv().is_err());
    }
}
