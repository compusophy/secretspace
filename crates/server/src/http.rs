//! Just enough HTTP/1.1 for the server: a request's head, read within
//! bounds (so many bytes, all of it soon), answers, the pages from disk,
//! and who is really asking.

use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::net::{IpAddr, TcpStream};
use std::path::Path;
use std::time::Instant;

/// A request's head is at most this long (a browser's is well under a
/// kilobyte); a longer one is let go.
pub const HEAD_MOST: u64 = 16 * 1024;

/// A socket read under a deadline while one is set: each read waits no
/// longer than what is left of it, so a request sent a byte at a time is
/// let go when the deadline comes, not kept for ever.
pub struct Paced<'a> {
    pub stream: &'a TcpStream,
    pub until: Option<Instant>,
}

impl Read for Paced<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(until) = self.until {
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            self.stream.set_read_timeout(Some(left))?;
        }
        let mut s = self.stream;
        s.read(buf)
    }
}

/// A request's head: its first line and its headers.
pub struct Head {
    pub request: String,
    lines: Vec<String>,
}

impl Head {
    /// A header's value, if it was sent.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.lines.iter().find_map(|h| {
            let (k, v) = h.split_once(':')?;
            k.trim().eq_ignore_ascii_case(name).then(|| v.trim())
        })
    }

    /// Its path and query (`/ws/wandfall`, `v=1`).
    pub fn target(&self) -> (&str, &str) {
        let target = self.request.split_whitespace().nth(1).unwrap_or("/");
        target.split_once('?').unwrap_or((target, ""))
    }
}

/// Read a request's head; None when the connection ends first or the head
/// runs past `HEAD_MOST`.
pub fn read_head(r: &mut impl BufRead) -> io::Result<Option<Head>> {
    let mut lines = Vec::new();
    let mut left = HEAD_MOST;
    loop {
        let mut line = String::new();
        let n = r.by_ref().take(left).read_line(&mut line)?;
        // Ended, or cut off by the bound before its end.
        if n == 0 || !line.ends_with('\n') {
            return Ok(None);
        }
        left -= n as u64;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        lines.push(line.to_string());
    }
    let request = if lines.is_empty() {
        String::new()
    } else {
        lines.remove(0)
    };
    Ok(Some(Head { request, lines }))
}

/// Who is really asking: the address Railway's edge says it came from
/// (`X-Real-IP`, which the edge writes itself), else the first of
/// `X-Forwarded-For`, else the socket's own. An IPv6 address counts as its
/// /64 (one home's or one phone's network, which has addresses to spare).
pub fn client(head: &Head, peer: Option<IpAddr>) -> String {
    let said = head.header("X-Real-IP").or_else(|| {
        head.header("X-Forwarded-For")
            .and_then(|f| f.split(',').next())
    });
    let ip = match said.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => match s.parse::<IpAddr>() {
            Ok(ip) => ip,
            // Not an address: kept as said, but never long.
            Err(_) => return s.chars().take(64).collect(),
        },
        None => match peer {
            Some(ip) => ip,
            None => return String::new(),
        },
    };
    match ip {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => {
                let s = v6.segments();
                format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
            }
        },
        v4 => v4.to_string(),
    }
}

pub fn respond(out: &mut impl Write, status: &str, kind: &str, body: &[u8]) -> io::Result<()> {
    let mut msg = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    msg.extend_from_slice(body);
    out.write_all(&msg)
}

/// A page's file from `root` (`--static dist`); a page's folder asked for
/// without its slash is sent to it (`query` kept).
pub fn file(out: &mut impl Write, root: &Path, path: &str, query: &str) -> io::Result<()> {
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
    if let Some(to) = slash_redirect(rel, query, p.is_dir()) {
        return write!(
            out,
            "HTTP/1.1 301 Moved Permanently\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
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

/// Where a page's folder asked for without its slash (`/wandfall`) is
/// sent: to the slash, or its own paths (`./pkg/`) would miss. `rel` is
/// the path with its leading slashes gone, so the answer starts with
/// exactly one: `//wandfall` stays on this site rather than naming a host
/// called wandfall. No folder here has a backslash in its name, and a
/// browser reads `/\` as `//`, so one is never sent anywhere.
fn slash_redirect(rel: &str, query: &str, is_dir: bool) -> Option<String> {
    if rel.is_empty() || rel.ends_with('/') || rel.contains('\\') || !is_dir {
        return None;
    }
    let q = if query.is_empty() { "" } else { "?" };
    Some(format!("/{rel}/{q}{query}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_without_its_slash_is_sent_to_it_on_this_site() {
        let to = |path: &str, query: &str, dir: bool| {
            slash_redirect(path.trim_start_matches('/'), query, dir)
        };
        assert_eq!(to("/wandfall", "", true).as_deref(), Some("/wandfall/"));
        assert_eq!(
            to("/wandfall", "x=1", true).as_deref(),
            Some("/wandfall/?x=1")
        );
        assert_eq!(to("/wyrm/pkg", "", true).as_deref(), Some("/wyrm/pkg/"));
        // A protocol-relative path never leaves the site.
        assert_eq!(to("//wandfall", "", true).as_deref(), Some("/wandfall/"));
        assert_eq!(
            to("///wandfall", "a", true).as_deref(),
            Some("/wandfall/?a")
        );
        assert_eq!(to("/\\wandfall", "", true), None);
        // The page itself, a folder with its slash, and a file: served.
        assert_eq!(to("/", "", true), None);
        assert_eq!(to("/wandfall/", "", true), None);
        assert_eq!(to("/wandfall/index.html", "", false), None);
    }

    fn head(text: &str) -> Option<Head> {
        read_head(&mut text.as_bytes()).unwrap()
    }

    #[test]
    fn a_head_is_read_and_bounded() {
        let h = head(
            "GET /ws/wandfall?v=1&watch=1 HTTP/1.1\r\nHost: x\r\nUpgrade:  websocket \r\n\r\nrest",
        )
        .unwrap();
        assert_eq!(h.target(), ("/ws/wandfall", "v=1&watch=1"));
        assert_eq!(h.header("upgrade"), Some("websocket"));
        assert_eq!(h.header("Origin"), None);
        // A head that never ends, or runs on past the bound: let go.
        assert!(head("GET / HTTP/1.1\r\nHost: x\r\n").is_none());
        let long = format!(
            "GET / HTTP/1.1\r\nX: {}\r\n\r\n",
            "a".repeat(HEAD_MOST as usize)
        );
        assert!(head(&long).is_none());
        let many = format!("GET / HTTP/1.1\r\n{}\r\n", "X: y\r\n".repeat(4000));
        assert!(head(&many).is_none(), "many short lines count too");
    }

    #[test]
    fn a_request_sent_a_byte_at_a_time_is_let_go_at_its_deadline() {
        use std::io::BufReader;
        use std::net::TcpListener;
        use std::time::Duration;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        // A slow drip: a byte every 5 ms, never stopping by itself.
        let drip = |text: &'static [u8]| {
            let mut s = TcpStream::connect(addr).unwrap();
            std::thread::spawn(move || {
                for b in text.iter().cycle() {
                    if s.write_all(&[*b]).is_err() {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            });
            listener.accept().unwrap().0
        };
        let wait = Duration::from_millis(300);
        // A head with no end...
        let stream = drip(b"GET / HTTP/1.1\r\nX: ");
        let t = Instant::now();
        let mut r = BufReader::new(Paced {
            stream: &stream,
            until: Some(t + wait),
        });
        assert!(read_head(&mut r).is_err());
        assert!(t.elapsed() < wait * 3, "{:?}", t.elapsed());
        drop(stream);
        // ...and a report's body, after a head that came in time.
        let stream = drip(b"POST /feedback HTTP/1.1\r\nContent-Length: 4000\r\n\r\nslow");
        let (t, wait) = (Instant::now(), Duration::from_secs(1));
        let mut r = BufReader::new(Paced {
            stream: &stream,
            until: Some(t + wait),
        });
        let head = read_head(&mut r).unwrap().unwrap();
        assert_eq!(head.header("Content-Length"), Some("4000"));
        let mut body = vec![0u8; 4000];
        assert!(r.read_exact(&mut body).is_err());
        let took = t.elapsed();
        assert!(took >= wait * 9 / 10 && took < wait * 3, "{took:?}");
    }

    #[test]
    fn the_client_is_the_edge_s_word_and_ipv6_counts_by_network() {
        let peer = Some("10.0.0.9".parse().unwrap());
        let ask = |headers: &str| {
            let h = head(&format!("GET / HTTP/1.1\r\n{headers}\r\n")).unwrap();
            client(&h, peer)
        };
        assert_eq!(ask(""), "10.0.0.9");
        assert_eq!(ask("X-Forwarded-For: 1.2.3.4, 10.0.0.1\r\n"), "1.2.3.4");
        // The edge's own X-Real-IP wins over a forwarded list a page wrote.
        assert_eq!(
            ask("X-Forwarded-For: 6.6.6.6\r\nX-Real-IP: 5.6.7.8\r\n"),
            "5.6.7.8"
        );
        let a = ask("X-Real-IP: 2001:db8:1:2:aaaa::1\r\n");
        let b = ask("X-Real-IP: 2001:db8:1:2:bbbb:cccc:dddd:eeee\r\n");
        assert_eq!(a, "2001:db8:1:2::/64");
        assert_eq!(a, b, "one network, one address");
        assert_eq!(ask("X-Real-IP: ::ffff:9.8.7.6\r\n"), "9.8.7.6");
        assert_eq!(
            ask(&format!("X-Real-IP: {}\r\n", "z".repeat(500))).len(),
            64
        );
    }
}
