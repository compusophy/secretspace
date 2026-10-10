//! secretspace server: hosts every game, and the hub's live numbers.
//!
//! Each game is a `Room` running in a thread of its own at its own tick
//! rate (`host`), kept on disk (`store`). A browser opens a WebSocket to
//! `/ws/<game>` (plain `/ws` and `/ws/arena` are wyrm, for pages from
//! before) and says the platform Hello: the server knows its soul and name
//! (`souls`) and the room takes it from there. Each connection has a reader
//! thread and a writer thread, and a browser that cannot keep up is let go
//! rather than allowed to hold its room back. `/ws/hub` is told once a
//! second who is online where and how many visits there have been. A page
//! counts as a visit when its first connection asks with `?v=1`. Pages
//! send what players tell us, and their crashes, to `/feedback`.
//!
//! What is kept, under `$DATA_DIR`: `visits`, `souls`, `rooms/<id>/`,
//! `feedback`. A
//! SIGTERM (a deploy) holds every room still and saves it first (`signal`).
//!
//! Optionally serves the pages too (`--static dist`). std only.
//!
//! `server [--port 8787] [--static dist]`; PORT in the environment wins.

mod feedback;
mod host;
mod signal;
mod souls;
mod store;
mod ws;

use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::sync_channel;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use engine::hub::Stats;
use engine::room::{Room, Who};
use engine::who::{Hello, PLATFORM};

use host::{unix, Event, Game};
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
/// Messages a browser may send a second.
const RATE: u32 = 90;
/// A browser that says nothing for this long is gone (pages send a
/// heartbeat every ten seconds).
const IDLE: Duration = Duration::from_secs(45);
/// A page that has not said Hello by now is a guest.
const HELLO_WAIT: Duration = Duration::from_secs(1);

/// What every connection thread can see.
struct Shared {
    games: Vec<Arc<Game>>,
    hub: AtomicUsize,
    visits: AtomicU64,
    souls: Mutex<Souls>,
    feedback: feedback::Feedback,
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

    let visits = data
        .as_ref()
        .and_then(|d| fs::read_to_string(d.join("visits")).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
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
    for stream in listener.incoming().flatten() {
        next_conn = next_conn.wrapping_add(1).max(1);
        let (conn, shared, root) = (next_conn, shared.clone(), root.clone());
        thread::spawn(move || {
            let _ = serve(stream, conn, &shared, root.as_deref());
        });
    }
}

/// Every five seconds: play time for souls, and the souls and the visit
/// count written down when they changed. On a stop: wait for every room to
/// hold still and save, write everything down, and leave.
fn keep(dir: Option<&Path>, shared: &Shared) {
    let mut saved = shared.visits.load(Ordering::Relaxed);
    let write_visits = |saved: &mut u64| {
        let now = shared.visits.load(Ordering::Relaxed);
        let Some(dir) = dir.filter(|_| now != *saved) else {
            return;
        };
        let tmp = dir.join("visits.tmp");
        if fs::write(&tmp, now.to_string()).is_ok() && fs::rename(&tmp, dir.join("visits")).is_ok()
        {
            *saved = now;
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
            shared.souls().save(unix());
            write_visits(&mut saved);
            eprintln!("stopped after {} ms", t.elapsed().as_millis());
            std::process::exit(0);
        }
        if last.elapsed() >= Duration::from_secs(5) {
            last = Instant::now();
            let mut souls = shared.souls();
            souls.tick(5, unix());
            souls.save(unix());
            drop(souls);
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
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_nodelay(true)?;
    let peer = stream
        .peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_default();
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
    // Railway's edge says who is really asking.
    let addr = header("X-Forwarded-For")
        .and_then(|f| f.split(',').next().map(|a| a.trim().to_string()))
        .unwrap_or(peer);
    if path == "/feedback" {
        return feedback(
            &mut out,
            &mut reader,
            &request,
            query,
            &addr,
            shared,
            header("Content-Length"),
        );
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
        let watch = query.split('&').any(|kv| kv == "watch=1");
        return match game {
            Some(game) => play(reader, out, conn, watch, &addr, &game, shared),
            None => hub(reader, out, shared),
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
        Some(root) => file(&mut out, root, path, query),
        None => respond(&mut out, "200 OK", "text/plain", b"secretspace\n"),
    }
}

/// `POST /feedback`: a report from a page, kept (`feedback`). `GET
/// /feedback?key=`: the newest kept, for whoever holds `$FEEDBACK_KEY`.
fn feedback(
    out: &mut TcpStream,
    reader: &mut impl Read,
    request: &str,
    query: &str,
    addr: &str,
    shared: &Shared,
    length: Option<String>,
) -> std::io::Result<()> {
    if request.starts_with("GET ") {
        let key = query
            .split('&')
            .find_map(|kv| kv.strip_prefix("key="))
            .unwrap_or("");
        return match shared.feedback.read(key) {
            Some(body) => respond(out, "200 OK", "text/plain; charset=utf-8", &body),
            None => respond(out, "404 Not Found", "text/plain", b"no"),
        };
    }
    if !request.starts_with("POST ") {
        return respond(
            out,
            "405 Method Not Allowed",
            "text/plain",
            b"POST a report",
        );
    }
    let Some(n) = length.and_then(|l| l.parse::<usize>().ok()) else {
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

fn respond(out: &mut TcpStream, status: &str, kind: &str, body: &[u8]) -> std::io::Result<()> {
    write!(
        out,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    out.write_all(body)
}

fn file(out: &mut TcpStream, root: &Path, path: &str, query: &str) -> std::io::Result<()> {
    if path.split('/').any(|seg| seg == "..") {
        return respond(out, "400 Bad Request", "text/plain", b"no");
    }
    // The game was called arena once; old links still land on it.
    if path == "/arena" || path.starts_with("/arena/") {
        return write!(
            out,
            "HTTP/1.1 302 Found\r\nLocation: /wyrm/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
    }
    let rel = path.trim_start_matches('/');
    let mut p = root.join(rel);
    // A page's folder without its slash (`/wandfall`): its own paths
    // (`./pkg/`) would miss, so it is sent to the slash.
    if !rel.is_empty() && !rel.ends_with('/') && p.is_dir() {
        let q = if query.is_empty() { "" } else { "?" };
        return write!(
            out,
            "HTTP/1.1 301 Moved Permanently\r\nLocation: {path}/{q}{query}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
    }
    if rel.is_empty() || rel.ends_with('/') {
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
/// this thread reads what it says. Its first message is the platform Hello
/// (anything else, or a second of silence, and it is a guest); platform
/// messages go no further than here.
fn play(
    mut reader: impl Read,
    writer: TcpStream,
    conn: u32,
    watch: bool,
    addr: &str,
    game: &Game,
    shared: &Shared,
) -> std::io::Result<()> {
    if game.state() != "running" {
        return Ok(());
    }
    let (tx, rx) = sync_channel::<Vec<u8>>(game.backlog);
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
    let mut who = Who::guest(watch);
    let mut first = None;
    if !watch {
        writer.set_read_timeout(Some(HELLO_WAIT))?;
        match ws::read(&mut reader) {
            Ok(ws::Frame::Binary(b)) => match Hello::decode(&b) {
                Some(h) => {
                    let (w, seen) = shared.souls().hello(&h, addr, unix(), watch);
                    who = w;
                    let _ = tx.try_send(seen.encode());
                }
                None => first = Some(b),
            },
            Ok(ws::Frame::Close) => return Ok(()),
            Ok(_) => {}
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) => return Err(e),
        }
    }
    writer.set_read_timeout(Some(IDLE))?;
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
            } else if let Some(h) = Hello::decode(&b) {
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
