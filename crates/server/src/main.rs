//! secretspace server: runs the one arena everybody plays in.
//!
//! A browser opens a WebSocket to `/ws`. It is shown the arena at once
//! (watching the biggest snake), joins with a name, then steers; twenty
//! times a second it is sent what changed in its view. One thread runs the
//! world; each connection has a reader thread and a writer thread, and a
//! browser that cannot keep up is let go rather than allowed to hold the
//! world back.
//!
//! Optionally serves the page too (`--static dist`), so one process can be
//! the whole game. std only.
//!
//! `server [--port 8787] [--static dist]`; PORT in the environment wins.

mod ws;

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use game::laws::{ARENA, TICK_HZ};
use game::proto::{self, angle_from_u16, Up};
use game::view::{self, Viewer};
use game::world::World;

/// Frames a browser may fall behind by before it is let go (three seconds).
const BACKLOG: usize = 60;
/// Messages a browser may send a second.
const RATE: u32 = 90;
/// A browser that says nothing for this long is gone (pages send a
/// heartbeat every ten seconds).
const IDLE: Duration = Duration::from_secs(45);
/// Boards (leaderboard, minimap) go out this often, in ticks.
const BOARD_EVERY: u32 = TICK_HZ / 2;

enum Event {
    Open(u32, SyncSender<Vec<u8>>),
    Say(u32, Up),
    Close(u32),
}

struct Client {
    tx: SyncSender<Vec<u8>>,
    view: Viewer,
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
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);

    let (events, inbox) = channel::<Event>();
    let people = Arc::new(AtomicUsize::new(0));
    let counter = people.clone();
    thread::spawn(move || run(seed, inbox, counter));

    let listener = TcpListener::bind(("0.0.0.0", port)).expect("bind");
    eprintln!(
        "secretspace arena on :{port}{}",
        root.as_ref()
            .map(|r| format!(", serving {}", r.display()))
            .unwrap_or_default()
    );
    let mut next_conn = 0u32;
    for stream in listener.incoming().flatten() {
        next_conn = next_conn.wrapping_add(1);
        let (conn, events, root, people) =
            (next_conn, events.clone(), root.clone(), people.clone());
        thread::spawn(move || {
            let _ = serve(stream, conn, events, root.as_deref(), &people);
        });
    }
}

/// The world's own thread: events in, a tick, frames out, forever.
fn run(seed: u64, inbox: Receiver<Event>, people: Arc<AtomicUsize>) {
    let mut world = World::new(seed);
    let mut clients: HashMap<u32, Client> = HashMap::new();
    let tick = Duration::from_micros(1_000_000 / TICK_HZ as u64);
    let mut next = Instant::now();
    loop {
        while let Ok(ev) = inbox.try_recv() {
            match ev {
                Event::Open(conn, tx) => {
                    let _ = tx.try_send(proto::hello(ARENA as u16, TICK_HZ as u8));
                    clients.insert(
                        conn,
                        Client {
                            tx,
                            view: Viewer::default(),
                        },
                    );
                }
                Event::Say(conn, up) => {
                    let Some(c) = clients.get_mut(&conn) else {
                        continue;
                    };
                    match up {
                        Up::Join { name } => {
                            if world.find(c.view.you).is_none() {
                                c.view.you = world.spawn(&name, None);
                            }
                        }
                        Up::Steer { angle, boost } => {
                            world.steer(c.view.you, angle_from_u16(angle), boost);
                        }
                        Up::Screen { w, h } => {
                            c.view.screen = (w as f32, h as f32);
                        }
                    }
                }
                Event::Close(conn) => {
                    if let Some(c) = clients.remove(&conn) {
                        world.remove(c.view.you);
                    }
                }
            }
        }

        for d in world.step() {
            if d.human {
                if let Some(c) = clients.values_mut().find(|c| c.view.you == d.id) {
                    let by = d.killer.as_ref().map_or("", |k| k.1.as_str());
                    let _ = c.tx.try_send(proto::died(by, d.score));
                    c.view.you = 0;
                    c.view.watching = d.killer.as_ref().map_or(0, |k| k.0);
                }
            }
            if let Some((_, killer)) = &d.killer {
                let line = proto::feed(killer, &d.name, d.score);
                for c in clients.values() {
                    let _ = c.tx.try_send(line.clone());
                }
            }
        }

        let board = world.tick.is_multiple_of(BOARD_EVERY);
        let people_here = clients.len().min(u16::MAX as usize) as u16;
        let mut gone = Vec::new();
        for (&conn, c) in clients.iter_mut() {
            let mut out = vec![c.view.frame(&world)];
            if board {
                out.push(view::board(&world, c.view.you, people_here));
            }
            for msg in out {
                if let Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) =
                    c.tx.try_send(msg)
                {
                    gone.push(conn);
                    break;
                }
            }
        }
        for conn in gone {
            if let Some(c) = clients.remove(&conn) {
                world.remove(c.view.you);
            }
        }
        people.store(clients.len(), Ordering::Relaxed);

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
    events: Sender<Event>,
    root: Option<&Path>,
    people: &AtomicUsize,
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
    let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
    let header = |name: &str| {
        head.iter().find_map(|h| {
            let (k, v) = h.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case(name)
                .then(|| v.trim().to_string())
        })
    };
    let mut out = stream;
    if path.starts_with("/ws") {
        let Some(key) = header("Sec-WebSocket-Key") else {
            return respond(&mut out, "400 Bad Request", "text/plain", b"websocket only");
        };
        write!(
            out,
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
            ws::accept_key(&key)
        )?;
        return session(reader, out, conn, events);
    }
    if path == "/health" {
        let body = format!(
            "secretspace arena · {} here\n",
            people.load(Ordering::Relaxed)
        );
        return respond(&mut out, "200 OK", "text/plain", body.as_bytes());
    }
    match root {
        Some(root) => file(&mut out, root, &path),
        None => respond(&mut out, "200 OK", "text/plain", b"secretspace arena\n"),
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
    let path = path.split(['?', '#']).next().unwrap_or("/");
    if path.split('/').any(|seg| seg == "..") {
        return respond(out, "400 Bad Request", "text/plain", b"no");
    }
    let rel = path.trim_start_matches('/');
    let p = if rel.is_empty() {
        root.join("index.html")
    } else {
        root.join(rel)
    };
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

/// One browser's WebSocket: a writer thread drains what the world sends it;
/// this thread reads what it says.
fn session(
    mut reader: impl Read,
    writer: TcpStream,
    conn: u32,
    events: Sender<Event>,
) -> std::io::Result<()> {
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
    if events.send(Event::Open(conn, tx)).is_err() {
        return Ok(());
    }
    let (mut window, mut count) = (Instant::now(), 0u32);
    let result = loop {
        let frame = match ws::read(&mut reader) {
            Ok(f) => f,
            Err(e) => break Err(e),
        };
        if window.elapsed() > Duration::from_secs(1) {
            window = Instant::now();
            count = 0;
        }
        count += 1;
        if count > RATE {
            break Ok(());
        }
        match frame {
            ws::Frame::Close => break Ok(()),
            // Browsers do not ping, and this protocol has no text.
            ws::Frame::Ping(p) => drop(p),
            ws::Frame::Text(t) => drop(t),
            ws::Frame::Binary(b) => {
                if let Some(up) = Up::decode(&b) {
                    let _ = events.send(Event::Say(conn, up));
                }
            }
        }
    };
    let _ = events.send(Event::Close(conn));
    let _ = writer.shutdown(std::net::Shutdown::Both);
    let _ = pump.join();
    result
}
