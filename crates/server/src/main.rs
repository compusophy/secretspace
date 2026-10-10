//! secretspace server: hosts every game, and the hub's live numbers.
//!
//! Each game is a `Room` running in a thread of its own at its own tick
//! rate (`host`), kept on disk (`store`). A browser opens a WebSocket to
//! `/ws/<game>` (plain `/ws` and `/ws/arena` are wyrm, for pages from
//! before) and says the platform Hello: the server knows its soul and name
//! (`souls`) and the room takes it from there. Each connection has a reader
//! thread and a writer thread (`sockets`), and a browser that cannot keep
//! up is let go rather than allowed to hold its room back. `/ws/hub` is
//! told once a second who is online where and how many visits there have
//! been. A page counts as a visit when its first connection asks with
//! `?v=1`. Pages send what players tell us, and their crashes, to
//! `/feedback`.
//!
//! Nothing a browser sends is trusted: a request's head is bounded and
//! must come soon (`http`), and connections are counted, everyone's
//! together and each address's, so no one can take all there are.
//!
//! What is kept, under `$DATA_DIR`: `visits`, `souls`, `rooms/<id>/`,
//! `feedback`. A SIGTERM (a deploy) holds every room still and saves it
//! first (`signal`).
//!
//! Optionally serves the pages too (`--static dist`). std only.
//!
//! `server [--port 8787] [--static dist]`; PORT in the environment wins.

mod feedback;
mod host;
mod http;
mod signal;
mod sockets;
mod souls;
mod store;
mod ws;

use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use engine::hub::Stats;
use engine::room::Room;

use host::{unix, Game};
use http::{respond, Head, Paced};
use souls::Souls;
use store::{Factory, Store};

/// Every game there is.
const ROOMS: &[Factory] = &[wyrm_room, luciphon_room, wandfall_room];

fn wyrm_room(seed: u64) -> Box<dyn Room> {
    Box::new(wyrm::room::Wyrm::new(seed))
}

fn luciphon_room(seed: u64) -> Box<dyn Room> {
    Box::new(luciphon::room::Luciphon::new(seed))
}

fn wandfall_room(seed: u64) -> Box<dyn Room> {
    Box::new(wandfall::room::Wandfall::new(seed))
}

/// The server's build: a hash of everything it is made from (`ship.sh`).
const BUILD: &str = match option_env!("SECRETSPACE_BUILD") {
    Some(b) => b,
    None => "dev",
};
/// Connections open at once, everyone's together, and WebSockets from one
/// address (a classroom shares one, each page two or three); past these a
/// new one is closed at once rather than let in to starve the rest.
const MOST_OPEN: usize = 2000;
const MOST_FROM_ONE: usize = 128;
/// A request (its head, and a report's body) must all be here this soon
/// after its connection, and an answer must be taken as patiently: a
/// slow drip either way is let go.
const REQUEST_WAIT: Duration = Duration::from_secs(10);

/// What every connection thread can see.
struct Shared {
    games: Vec<Arc<Game>>,
    hub: AtomicUsize,
    visits: AtomicU64,
    souls: Mutex<Souls>,
    feedback: feedback::Feedback,
    /// Connections open, and WebSockets open from each address.
    open: AtomicUsize,
    from: Mutex<HashMap<String, usize>>,
}

impl Shared {
    fn stats(&self) -> Stats {
        let load = |a: &AtomicUsize| a.load(Ordering::Relaxed) as u32;
        // Online is people (bots are not online); a card shows everyone
        // playing, bots too, as the game itself does.
        let people: u32 = self.games.iter().map(|g| load(&g.people)).sum();
        Stats {
            online: load(&self.hub) + people,
            visits: self.visits.load(Ordering::Relaxed),
            games: self
                .games
                .iter()
                .map(|g| (g.id.to_string(), load(&g.playing)))
                .collect(),
        }
    }

    fn souls(&self) -> MutexGuard<'_, Souls> {
        self.souls.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A WebSocket from `addr`, counted while it is open; None when that
    /// address has `MOST_FROM_ONE` open already.
    fn count_from(&self, addr: &str) -> Option<FromOne<'_>> {
        let mut from = self.from.lock().unwrap_or_else(|e| e.into_inner());
        let n = from.entry(addr.to_string()).or_default();
        if *n >= MOST_FROM_ONE {
            return None;
        }
        *n += 1;
        Some(FromOne {
            shared: self,
            addr: addr.to_string(),
        })
    }
}

/// One connection counted open, until it is dropped.
struct Open(Arc<Shared>);

impl Drop for Open {
    fn drop(&mut self) {
        self.0.open.fetch_sub(1, Ordering::Relaxed);
    }
}

/// One WebSocket counted against its address, until it is dropped.
struct FromOne<'a> {
    shared: &'a Shared,
    addr: String,
}

impl Drop for FromOne<'_> {
    fn drop(&mut self) {
        let mut from = self.shared.from.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(n) = from.get_mut(&self.addr) {
            *n -= 1;
            if *n == 0 {
                from.remove(&self.addr);
            }
        }
    }
}

fn main() {
    signal::install();
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let port: u16 = std::env::var("PORT")
        .ok()
        .or_else(|| arg("--port"))
        .and_then(|p| p.parse().ok())
        .unwrap_or(8787);
    let root = arg("--static").map(PathBuf::from);
    let data = std::env::var("DATA_DIR").ok().map(PathBuf::from);
    if let Some(d) = &data {
        let _ = fs::create_dir_all(d);
    }
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);

    let visits = data.as_deref().map_or(0, |d| read_visits(d, unix()));
    let store = Store::new(data.as_deref());
    let games: Vec<Arc<Game>> = ROOMS
        .iter()
        .map(|&f| host::start(f, seed, store.clone(), signal::stopping))
        .collect();
    let mut souls = Souls::open(data.as_ref().map(|d| d.join("souls")), unix());
    for g in &games {
        souls.reserve(g.reserved);
    }
    let shared = Arc::new(Shared {
        games,
        hub: AtomicUsize::new(0),
        visits: AtomicU64::new(visits),
        souls: Mutex::new(souls),
        feedback: feedback::Feedback::new(data.as_deref()),
        open: AtomicUsize::new(0),
        from: Mutex::new(HashMap::new()),
    });
    {
        let (s, d) = (shared.clone(), data.clone());
        thread::spawn(move || keep(d.as_deref(), &s));
    }

    let listener = TcpListener::bind(("0.0.0.0", port)).expect("bind");
    eprintln!(
        "secretspace {BUILD} on :{port}{}{}",
        root.as_ref()
            .map(|r| format!(", serving {}", r.display()))
            .unwrap_or_default(),
        data.as_ref()
            .map(|d| format!(", keeping {}", d.display()))
            .unwrap_or_default()
    );
    let mut next_conn = 0u32;
    loop {
        let stream = match listener.accept() {
            Ok((stream, _)) => stream,
            // Out of file handles, most likely: a breath, not a spin.
            Err(_) => {
                thread::sleep(Duration::from_millis(20));
                continue;
            }
        };
        if shared.open.fetch_add(1, Ordering::Relaxed) >= MOST_OPEN {
            shared.open.fetch_sub(1, Ordering::Relaxed);
            continue;
        }
        let open = Open(shared.clone());
        next_conn = next_conn.wrapping_add(1).max(1);
        let (conn, root) = (next_conn, root.clone());
        // A thread the system will not give closes the connection (the
        // closure, and so the stream, is dropped), never the server.
        let _ = thread::Builder::new().spawn(move || {
            let _ = serve(stream, conn, &open.0, root.as_deref());
        });
    }
}

/// The visit count kept in `dir`. One that does not read is set aside, not
/// written over, and counting starts again from nothing.
fn read_visits(dir: &Path, now: u64) -> u64 {
    let path = dir.join("visits");
    let Ok(text) = fs::read_to_string(&path) else {
        return 0;
    };
    match text.trim().parse() {
        Ok(n) => n,
        Err(_) => {
            let to = store::set_aside(&path, now);
            eprintln!("visits: {text:?} does not read; set aside as {to:?}");
            0
        }
    }
}

/// Every five seconds: play time for souls, and the souls and the visit
/// count written down when they changed (the writing done outside the
/// souls' lock, so no Hello waits on a disk). On a stop: wait for every
/// room to hold still and save, write everything down, and leave.
fn keep(dir: Option<&Path>, shared: &Shared) {
    let mut saved = shared.visits.load(Ordering::Relaxed);
    let write_visits = |saved: &mut u64| {
        let now = shared.visits.load(Ordering::Relaxed);
        let Some(dir) = dir.filter(|_| now != *saved) else {
            return;
        };
        match store::write_atomic(&dir.join("visits"), now.to_string().as_bytes()) {
            Ok(()) => *saved = now,
            Err(e) => eprintln!("visits: not saved: {e}"),
        }
    };
    let save_souls = |all: bool| {
        let due = shared.souls().due(unix(), all);
        if let Some((path, bytes)) = due {
            if let Err(e) = store::write_atomic(&path, &bytes) {
                eprintln!("souls: not saved: {e}");
                shared.souls().unsaved();
            }
        }
    };
    let mut last = Instant::now();
    loop {
        thread::sleep(Duration::from_millis(50));
        if signal::stopping() {
            eprintln!("stopping: every room holds still");
            let t = Instant::now();
            while shared.games.iter().any(|g| !g.done.load(Ordering::Relaxed))
                && t.elapsed() < Duration::from_secs(5)
            {
                thread::sleep(Duration::from_millis(20));
            }
            // Let the Stills reach the pages.
            thread::sleep(Duration::from_millis(300));
            save_souls(true);
            write_visits(&mut saved);
            eprintln!("stopped after {} ms", t.elapsed().as_millis());
            std::process::exit(0);
        }
        if last.elapsed() >= Duration::from_secs(5) {
            last = Instant::now();
            shared.souls().tick(5, unix());
            save_souls(false);
            write_visits(&mut saved);
        }
    }
}

fn serve(
    stream: TcpStream,
    conn: u32,
    shared: &Shared,
    root: Option<&Path>,
) -> std::io::Result<()> {
    stream.set_nodelay(true)?;
    stream.set_write_timeout(Some(REQUEST_WAIT))?;
    let peer = stream.peer_addr().ok().map(|a| a.ip());
    // One socket for reading and writing (its threads share it, so a
    // connection is one file handle).
    let stream = Arc::new(stream);
    let mut reader = BufReader::new(Paced {
        stream: &stream,
        until: Some(Instant::now() + REQUEST_WAIT),
    });
    let Some(head) = http::read_head(&mut reader)? else {
        return Ok(());
    };
    let (path, query) = head.target();
    let addr = http::client(&head, peer);
    let mut out: &TcpStream = &stream;
    if path == "/feedback" {
        return feedback(&mut out, &mut reader, &head, query, &addr, shared);
    }
    if let Some(room) = path.strip_prefix("/ws") {
        let room = room.trim_start_matches('/');
        // Pages from before the hub (`/ws`) and before the rename (`arena`)
        // still reach wyrm.
        let room = match room {
            "" | "arena" => "wyrm",
            r => r,
        };
        let game = shared.games.iter().find(|g| g.id == room).cloned();
        if room != "hub" && game.is_none() {
            return respond(&mut out, "404 Not Found", "text/plain", b"no such game");
        }
        let Some(key) = head.header("Sec-WebSocket-Key") else {
            return respond(&mut out, "400 Bad Request", "text/plain", b"websocket only");
        };
        let Some(_counted) = shared.count_from(&addr) else {
            return respond(&mut out, "429 Too Many Requests", "text/plain", b"later");
        };
        write!(
            out,
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
            ws::accept_key(key)
        )?;
        if query.split('&').any(|kv| kv == "v=1") {
            shared.visits.fetch_add(1, Ordering::Relaxed);
        }
        let watch = query.split('&').any(|kv| kv == "watch=1");
        // Open now: the socket's own timeouts, no deadline.
        reader.get_mut().until = None;
        return match game {
            Some(game) => {
                let s = stream.clone();
                sockets::play(reader, s, conn, watch, &addr, &game, shared)
            }
            None => sockets::hub(reader, &stream, shared),
        };
    }
    if path == "/health" {
        let s = shared.stats();
        let mut body = format!("ok {BUILD} · {} online · {} visits", s.online, s.visits);
        for g in shared.games.iter().filter(|g| g.state() != "running") {
            body += &format!(" · {} {}", g.id, g.state());
        }
        body.push('\n');
        return respond(&mut out, "200 OK", "text/plain", body.as_bytes());
    }
    if path == "/stats" {
        let body = stats_json(shared);
        return respond(&mut out, "200 OK", "application/json", body.as_bytes());
    }
    match root {
        Some(root) => http::file(&mut out, root, path),
        None => respond(&mut out, "200 OK", "text/plain", b"secretspace\n"),
    }
}

/// `POST /feedback`: a report from a page, kept (`feedback`). `GET
/// /feedback?key=`: the newest kept, for whoever holds `$FEEDBACK_KEY`.
fn feedback(
    out: &mut impl Write,
    reader: &mut impl Read,
    head: &Head,
    query: &str,
    addr: &str,
    shared: &Shared,
) -> std::io::Result<()> {
    if head.request.starts_with("GET ") {
        let key = query
            .split('&')
            .find_map(|kv| kv.strip_prefix("key="))
            .unwrap_or("");
        return match shared.feedback.read(key) {
            Some(body) => respond(out, "200 OK", "text/plain; charset=utf-8", &body),
            None => respond(out, "404 Not Found", "text/plain", b"no"),
        };
    }
    if !head.request.starts_with("POST ") {
        return respond(
            out,
            "405 Method Not Allowed",
            "text/plain",
            b"POST a report",
        );
    }
    let Some(n) = head
        .header("Content-Length")
        .and_then(|l| l.parse::<usize>().ok())
    else {
        return respond(out, "411 Length Required", "text/plain", b"how long?");
    };
    if n > feedback::MOST {
        return respond(out, "413 Payload Too Large", "text/plain", b"too long");
    }
    let mut body = vec![0u8; n];
    reader.read_exact(&mut body)?;
    match shared.feedback.take(addr, &body, unix()) {
        Ok(()) => respond(out, "200 OK", "text/plain", b"thank you"),
        Err(feedback::Refused::TooMany) => {
            respond(out, "429 Too Many Requests", "text/plain", b"later")
        }
        Err(_) => respond(out, "400 Bad Request", "text/plain", b"no"),
    }
}

fn stats_json(shared: &Shared) -> String {
    let s = shared.stats();
    let games: Vec<String> = s
        .games
        .iter()
        .map(|(id, n)| format!("\"{id}\":{n}"))
        .collect();
    let rooms: Vec<String> = shared
        .games
        .iter()
        .map(|g| {
            let mut fields = vec![
                format!("\"state\":\"{}\"", g.state()),
                format!("\"playing\":{}", g.playing.load(Ordering::Relaxed)),
                format!("\"panics\":{}", g.panics.load(Ordering::Relaxed)),
            ];
            let stats = g.stats.lock().unwrap_or_else(|e| e.into_inner());
            fields.extend(stats.iter().map(|(k, v)| format!("\"{k}\":{v}")));
            format!("\"{}\":{{{}}}", g.id, fields.join(","))
        })
        .collect();
    format!(
        "{{\"build\":\"{BUILD}\",\"online\":{},\"visits\":{},\"souls\":{},\"feedback\":{},\"games\":{{{}}},\"rooms\":{{{}}}}}\n",
        s.online,
        s.visits,
        shared.souls().len(),
        shared.feedback.count(),
        games.join(","),
        rooms.join(",")
    )
}
