//! secretspace server: hosts every game, and the hub's live numbers.
//!
//! Each game is a `Room` running in a thread of its own at its own tick
//! rate. A browser opens a WebSocket to `/ws/<game>` (plain `/ws` is the
//! arena, for pages from before there was a hub) and the room takes it
//! from there; each connection has a reader thread and a writer thread,
//! and a browser that cannot keep up is let go rather than allowed to hold
//! its room back. `/ws/hub` is told once a second who is online where and
//! how many visits there have been. A page counts as a visit when its first
//! connection asks with `?v=1`; visits are kept in `$DATA_DIR/visits` when
//! DATA_DIR is set, so they outlive a redeploy.
//!
//! Optionally serves the pages too (`--static dist`). std only.
//!
//! `server [--port 8787] [--static dist]`; PORT in the environment wins.

mod ws;

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use engine::hub::Stats;
use engine::room::{Outbox, Room};

/// Messages a browser may fall behind by before it is let go.
const BACKLOG: usize = 60;
/// Messages a browser may send a second.
const RATE: u32 = 90;
/// A browser that says nothing for this long is gone (pages send a
/// heartbeat every ten seconds).
const IDLE: Duration = Duration::from_secs(45);

enum Event {
    Open(u32, SyncSender<Vec<u8>>),
    Say(u32, Vec<u8>),
    Close(u32),
}

struct Game {
    id: &'static str,
    people: AtomicUsize,
    events: Sender<Event>,
}

/// What every connection thread can see.
struct Shared {
    games: Vec<Arc<Game>>,
    hub: AtomicUsize,
    visits: AtomicU64,
}

impl Shared {
    fn stats(&self) -> Stats {
        let games: Vec<(String, u32)> = self
            .games
            .iter()
            .map(|g| (g.id.to_string(), g.people.load(Ordering::Relaxed) as u32))
            .collect();
        Stats {
            online: self.hub.load(Ordering::Relaxed) as u32
                + games.iter().map(|g| g.1).sum::<u32>(),
            visits: self.visits.load(Ordering::Relaxed),
            games,
        }
    }
}

fn main() {
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
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);

    // Every game there is.
    let rooms: Vec<Box<dyn Room>> = vec![Box::new(arena::room::Arena::new(seed))];

    let visits = data
        .as_ref()
        .and_then(|d| fs::read_to_string(d.join("visits")).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    let mut games = Vec::new();
    for room in rooms {
        let (events, inbox) = channel::<Event>();
        let game = Arc::new(Game {
            id: room.id(),
            people: AtomicUsize::new(0),
            events,
        });
        let g = game.clone();
        thread::spawn(move || host(room, inbox, &g));
        games.push(game);
    }
    let shared = Arc::new(Shared {
        games,
        hub: AtomicUsize::new(0),
        visits: AtomicU64::new(visits),
    });
    if let Some(dir) = data.clone() {
        let s = shared.clone();
        thread::spawn(move || keep_visits(&dir, &s));
    }

    let listener = TcpListener::bind(("0.0.0.0", port)).expect("bind");
    eprintln!(
        "secretspace on :{port}{}{}",
        root.as_ref()
            .map(|r| format!(", serving {}", r.display()))
            .unwrap_or_default(),
        data.as_ref()
            .map(|d| format!(", visits kept in {}", d.display()))
            .unwrap_or_default()
    );
    let mut next_conn = 0u32;
    for stream in listener.incoming().flatten() {
        next_conn = next_conn.wrapping_add(1).max(1);
        let (conn, shared, root) = (next_conn, shared.clone(), root.clone());
        thread::spawn(move || {
            let _ = serve(stream, conn, &shared, root.as_deref());
        });
    }
}

/// Write the visit count down every few seconds when it changed.
fn keep_visits(dir: &Path, shared: &Shared) {
    let _ = fs::create_dir_all(dir);
    let mut saved = shared.visits.load(Ordering::Relaxed);
    loop {
        thread::sleep(Duration::from_secs(5));
        let now = shared.visits.load(Ordering::Relaxed);
        if now != saved {
            let tmp = dir.join("visits.tmp");
            if fs::write(&tmp, now.to_string()).is_ok()
                && fs::rename(&tmp, dir.join("visits")).is_ok()
            {
                saved = now;
            }
        }
    }
}

/// One room's own thread: events in, a tick, messages out, forever.
fn host(mut room: Box<dyn Room>, inbox: Receiver<Event>, game: &Game) {
    let mut clients: HashMap<u32, SyncSender<Vec<u8>>> = HashMap::new();
    let tick = Duration::from_micros(1_000_000 / room.hz().max(1) as u64);
    let mut next = Instant::now();
    loop {
        let mut out = Outbox::default();
        while let Ok(ev) = inbox.try_recv() {
            match ev {
                Event::Open(conn, tx) => {
                    clients.insert(conn, tx);
                    room.open(conn, &mut out);
                }
                Event::Say(conn, bytes) => room.message(conn, &bytes, &mut out),
                Event::Close(conn) => {
                    if clients.remove(&conn).is_some() {
                        room.close(conn);
                    }
                }
            }
        }
        room.tick(&mut out);
        let mut gone = Vec::new();
        for (conn, msg) in out.0 {
            if let Some(tx) = clients.get(&conn) {
                if let Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) = tx.try_send(msg)
                {
                    gone.push(conn);
                }
            }
        }
        for conn in gone {
            if clients.remove(&conn).is_some() {
                room.close(conn);
            }
        }
        game.people.store(room.people(), Ordering::Relaxed);

        next += tick;
        let now = Instant::now();
        if next > now {
            thread::sleep(next - now);
        } else if now - next > Duration::from_secs(1) {
            next = now;
        }
    }
}

fn serve(
    stream: TcpStream,
    conn: u32,
    shared: &Shared,
    root: Option<&Path>,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_nodelay(true)?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut head = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || head.len() > 64 {
            return Ok(());
        }
        let line = line.trim_end().to_string();
        if line.is_empty() {
            break;
        }
        head.push(line);
    }
    let request = head.first().cloned().unwrap_or_default();
    let target = request.split_whitespace().nth(1).unwrap_or("/").to_string();
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    let header = |name: &str| {
        head.iter().find_map(|h| {
            let (k, v) = h.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case(name)
                .then(|| v.trim().to_string())
        })
    };
    let mut out = stream;
    if let Some(room) = path.strip_prefix("/ws") {
        let room = room.trim_start_matches('/');
        let room = if room.is_empty() { "arena" } else { room };
        let game = shared.games.iter().find(|g| g.id == room).cloned();
        if room != "hub" && game.is_none() {
            return respond(&mut out, "404 Not Found", "text/plain", b"no such game");
        }
        let Some(key) = header("Sec-WebSocket-Key") else {
            return respond(&mut out, "400 Bad Request", "text/plain", b"websocket only");
        };
        write!(
            out,
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
            ws::accept_key(&key)
        )?;
        if query.split('&').any(|kv| kv == "v=1") {
            shared.visits.fetch_add(1, Ordering::Relaxed);
        }
        return match game {
            Some(game) => play(reader, out, conn, &game),
            None => hub(reader, out, shared),
        };
    }
    if path == "/health" || path == "/stats" {
        let s = shared.stats();
        if path == "/health" {
            let body = format!("secretspace · {} online · {} visits\n", s.online, s.visits);
            return respond(&mut out, "200 OK", "text/plain", body.as_bytes());
        }
        let games: Vec<String> = s
            .games
            .iter()
            .map(|(id, n)| format!("\"{id}\":{n}"))
            .collect();
        let body = format!(
            "{{\"online\":{},\"visits\":{},\"games\":{{{}}}}}\n",
            s.online,
            s.visits,
            games.join(",")
        );
        return respond(&mut out, "200 OK", "application/json", body.as_bytes());
    }
    match root {
        Some(root) => file(&mut out, root, path),
        None => respond(&mut out, "200 OK", "text/plain", b"secretspace\n"),
    }
}

fn respond(out: &mut TcpStream, status: &str, kind: &str, body: &[u8]) -> std::io::Result<()> {
    write!(
        out,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    out.write_all(body)
}

fn file(out: &mut TcpStream, root: &Path, path: &str) -> std::io::Result<()> {
    if path.split('/').any(|seg| seg == "..") {
        return respond(out, "400 Bad Request", "text/plain", b"no");
    }
    let rel = path.trim_start_matches('/');
    let mut p = root.join(rel);
    if rel.is_empty() || rel.ends_with('/') || p.is_dir() {
        p = p.join("index.html");
    }
    let kind = match p.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript",
        Some("wasm") => "application/wasm",
        Some("css") => "text/css",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    };
    match fs::read(&p) {
        Ok(body) => respond(out, "200 OK", kind, &body),
        Err(_) => respond(out, "404 Not Found", "text/plain", b"not here"),
    }
}

/// Read a browser's frames until it goes quiet, closes, or floods; hand
/// each binary one to `said`.
fn listen(mut reader: impl Read, mut said: impl FnMut(Vec<u8>)) -> std::io::Result<()> {
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
            ws::Frame::Binary(b) => said(b),
        }
    }
}

/// A browser in a game: a writer thread drains what its room sends it;
/// this thread reads what it says.
fn play(reader: impl Read, writer: TcpStream, conn: u32, game: &Game) -> std::io::Result<()> {
    writer.set_read_timeout(Some(IDLE))?;
    let (tx, rx) = sync_channel::<Vec<u8>>(BACKLOG);
    let mut w = writer.try_clone()?;
    let pump = thread::spawn(move || {
        for msg in rx {
            if ws::write(&mut w, 2, &msg).is_err() {
                break;
            }
        }
        let _ = ws::write(&mut w, 8, &[]);
        let _ = w.shutdown(std::net::Shutdown::Both);
    });
    if game.events.send(Event::Open(conn, tx)).is_err() {
        return Ok(());
    }
    let result = listen(reader, |b| {
        let _ = game.events.send(Event::Say(conn, b));
    });
    let _ = game.events.send(Event::Close(conn));
    let _ = writer.shutdown(std::net::Shutdown::Both);
    let _ = pump.join();
    result
}

/// A browser on the hub: told the numbers now and once a second after.
fn hub(reader: impl Read, writer: TcpStream, shared: &Shared) -> std::io::Result<()> {
    writer.set_read_timeout(Some(IDLE))?;
    shared.hub.fetch_add(1, Ordering::Relaxed);
    let done = Arc::new(AtomicBool::new(false));
    let mut w = writer.try_clone()?;
    let result = thread::scope(|scope| {
        let stop = done.clone();
        scope.spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                if ws::write(&mut w, 2, &shared.stats().encode()).is_err() {
                    break;
                }
                for _ in 0..10 {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
            let _ = w.shutdown(std::net::Shutdown::Both);
        });
        let r = listen(reader, drop);
        done.store(true, Ordering::Relaxed);
        r
    });
    let _ = writer.shutdown(std::net::Shutdown::Both);
    shared.hub.fetch_sub(1, Ordering::Relaxed);
    result
}
