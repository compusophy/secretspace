//! secretspace relay: introduces islands to each other, then gets out of
//! the way.
//!
//! A browser opens a WebSocket to `/ws`, says `hi <id>`, and is told about a
//! few other islands (`peers <id> ...`). It offers each a WebRTC channel
//! through `to <id> <payload>` / `from <id> <payload>`; once a channel opens,
//! the two talk directly and nothing they say passes through here. Each
//! island also sends its census (a binary wire envelope) every few seconds;
//! the relay sums them and tells everyone the size of the world.
//!
//! Optionally serves the page itself (`--static dist`), so one process is
//! the whole deployment. std only, plus the world crate's wire format.
//!
//! `relay [--port 8787] [--static dist]`; PORT in the environment wins.

mod hub;
mod ws;

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use hub::{Hub, Out};
use space::wire::{decode, Msg};

/// Messages one island may send in a window before it is cut off.
const RATE: u32 = 400;
const RATE_WINDOW: Duration = Duration::from_secs(10);
const IDLE: Duration = Duration::from_secs(90);

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
    let hub = Arc::new(Hub::new(seed));

    let census = hub.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(3));
        census.broadcast_world();
    });

    let listener = TcpListener::bind(("0.0.0.0", port)).expect("bind");
    eprintln!(
        "secretspace relay on :{port}{}",
        root.as_ref()
            .map(|r| format!(", serving {}", r.display()))
            .unwrap_or_default()
    );
    for stream in listener.incoming().flatten() {
        let (hub, root) = (hub.clone(), root.clone());
        thread::spawn(move || {
            let _ = serve(stream, &hub, root.as_deref());
        });
    }
}

fn serve(stream: TcpStream, hub: &Hub, root: Option<&Path>) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
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
        return session(reader, out, hub);
    }
    if path == "/health" {
        let body = format!("secretspace relay · {} islands\n", hub.len());
        return respond(&mut out, "200 OK", "text/plain", body.as_bytes());
    }
    match root {
        Some(root) => file(&mut out, root, &path),
        None => respond(&mut out, "200 OK", "text/plain", b"secretspace relay\n"),
    }
}

fn respond(out: &mut TcpStream, status: &str, kind: &str, body: &[u8]) -> std::io::Result<()> {
    write!(
        out,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
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

fn session(mut reader: impl Read, writer: TcpStream, hub: &Hub) -> std::io::Result<()> {
    writer.set_read_timeout(Some(IDLE))?;
    let (tx, rx) = channel::<Out>();
    let mut w = writer.try_clone()?;
    let pump = thread::spawn(move || {
        for out in rx {
            let r = match out {
                Out::Text(s) => ws::write(&mut w, 1, s.as_bytes()),
                Out::Binary(b) => ws::write(&mut w, 2, &b),
                Out::Pong(b) => ws::write(&mut w, 0xA, &b),
                Out::Close => {
                    let _ = ws::write(&mut w, 8, &[]);
                    break;
                }
            };
            if r.is_err() {
                break;
            }
        }
        let _ = w.shutdown(std::net::Shutdown::Both);
    });

    let mut me: Option<u64> = None;
    let (mut window, mut count) = (Instant::now(), 0u32);
    let result = loop {
        let frame = match ws::read(&mut reader) {
            Ok(f) => f,
            Err(e) => break Err(e),
        };
        if window.elapsed() > RATE_WINDOW {
            window = Instant::now();
            count = 0;
        }
        count += 1;
        if count > RATE {
            break Ok(());
        }
        match frame {
            ws::Frame::Close => break Ok(()),
            ws::Frame::Ping(p) => {
                let _ = tx.send(Out::Pong(p));
            }
            ws::Frame::Text(line) => {
                let mut parts = line.splitn(3, ' ');
                match (parts.next(), parts.next(), parts.next()) {
                    (Some("hi"), Some(id), _) if me.is_none() => {
                        let Ok(id) = u64::from_str_radix(id, 16) else {
                            break Ok(());
                        };
                        if !hub.join(id, tx.clone()) {
                            let _ = tx.send(Out::Text("taken".into()));
                            break Ok(());
                        }
                        me = Some(id);
                        let peers: Vec<String> =
                            hub.introduce(id).iter().map(|p| format!("{p:x}")).collect();
                        let _ = tx.send(Out::Text(format!("peers {}", peers.join(" "))));
                    }
                    (Some("more"), _, _) => {
                        if let Some(id) = me {
                            let peers: Vec<String> =
                                hub.introduce(id).iter().map(|p| format!("{p:x}")).collect();
                            let _ = tx.send(Out::Text(format!("peers {}", peers.join(" "))));
                        }
                    }
                    (Some("to"), Some(to), Some(payload)) => {
                        if let (Some(from), Ok(to)) = (me, u64::from_str_radix(to, 16)) {
                            if !hub.forward(from, to, payload) {
                                let _ = tx.send(Out::Text(format!("gone {to:x}")));
                            }
                        }
                    }
                    _ => {}
                }
            }
            ws::Frame::Binary(b) => {
                if let (Some(id), Ok(env)) = (me, decode(&b)) {
                    if let Msg::Census(entries) = env.msg {
                        hub.census(id, entries);
                    }
                }
            }
        }
    };
    if let Some(id) = me {
        hub.leave(id);
    }
    let _ = tx.send(Out::Close);
    drop(tx);
    let _ = pump.join();
    result
}
