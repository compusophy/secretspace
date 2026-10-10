//! Just enough WebSocket (RFC 6455) for the server: the handshake, and
//! frames in both directions. std only; SHA-1 comes from the engine.

use std::io::{self, Read, Write};

use engine::sha1::sha1;

const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
/// The largest frame the server accepts; a browser's messages are tiny.
pub const MAX_FRAME: u64 = 64 * 1024;

pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        s.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    s
}

/// The Sec-WebSocket-Accept for a client's key.
pub fn accept_key(key: &str) -> String {
    base64(&sha1(format!("{}{GUID}", key.trim()).as_bytes()))
}

pub enum Frame {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    /// The page answering our Ping: it is there.
    Pong,
    Close,
}

/// Read one whole message (continuations folded in).
pub fn read(r: &mut impl Read) -> io::Result<Frame> {
    let mut whole: Vec<u8> = Vec::new();
    let mut kind = 0u8;
    loop {
        let mut head = [0u8; 2];
        r.read_exact(&mut head)?;
        let fin = head[0] & 0x80 != 0;
        let opcode = head[0] & 0x0f;
        let masked = head[1] & 0x80 != 0;
        let mut len = (head[1] & 0x7f) as u64;
        if len == 126 {
            let mut b = [0u8; 2];
            r.read_exact(&mut b)?;
            len = u16::from_be_bytes(b) as u64;
        } else if len == 127 {
            let mut b = [0u8; 8];
            r.read_exact(&mut b)?;
            len = u64::from_be_bytes(b);
        }
        if len > MAX_FRAME || whole.len() as u64 + len > MAX_FRAME {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "frame too large",
            ));
        }
        let mut mask = [0u8; 4];
        if masked {
            r.read_exact(&mut mask)?;
        }
        let mut payload = vec![0u8; len as usize];
        r.read_exact(&mut payload)?;
        if masked {
            for (i, b) in payload.iter_mut().enumerate() {
                *b ^= mask[i % 4];
            }
        }
        match opcode {
            0x8 => return Ok(Frame::Close),
            0x9 => return Ok(Frame::Ping(payload)),
            0xA => return Ok(Frame::Pong),
            0x0 => whole.extend_from_slice(&payload),
            1 | 2 => {
                kind = opcode;
                whole = payload;
            }
            _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "unknown opcode")),
        }
        if fin {
            return Ok(if kind == 1 {
                Frame::Text(
                    String::from_utf8(whole)
                        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "utf-8"))?,
                )
            } else {
                Frame::Binary(whole)
            });
        }
    }
}

/// Opcodes the server sends.
pub const BINARY: u8 = 2;
pub const CLOSE: u8 = 8;
pub const PING: u8 = 9;

/// One frame, put on the end of `out`: several go out in one write (each
/// write is a segment on the wire, the socket sending at once).
pub fn frame(out: &mut Vec<u8>, opcode: u8, payload: &[u8]) {
    out.push(0x80 | opcode);
    let n = payload.len();
    if n < 126 {
        out.push(n as u8);
    } else if n <= u16::MAX as usize {
        out.push(126);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else {
        out.push(127);
        out.extend_from_slice(&(n as u64).to_be_bytes());
    }
    out.extend_from_slice(payload);
}

pub fn write(w: &mut impl Write, opcode: u8, payload: &[u8]) -> io::Result<()> {
    let mut out = Vec::with_capacity(payload.len() + 10);
    frame(&mut out, opcode, payload);
    w.write_all(&out)?;
    w.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc_6455_example_key() {
        assert_eq!(
            accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    #[test]
    fn frames_round_trip_through_a_masking_client() {
        let mut buf = Vec::new();
        write(&mut buf, 1, "hello".as_bytes()).unwrap();
        // Mask it the way a browser would.
        let mut masked = vec![buf[0], buf[1] | 0x80, 1, 2, 3, 4];
        masked.extend(
            buf[2..]
                .iter()
                .enumerate()
                .map(|(i, b)| b ^ [1, 2, 3, 4][i % 4]),
        );
        match read(&mut masked.as_slice()).unwrap() {
            Frame::Text(s) => assert_eq!(s, "hello"),
            _ => panic!("expected text"),
        }
    }

    #[test]
    fn a_frame_is_one_write_whatever_its_length() {
        for n in [0, 125, 126, 65_535, 65_536] {
            let payload = vec![7u8; n];
            let mut buf = Vec::new();
            write(&mut buf, BINARY, &payload).unwrap();
            let head = match n {
                0..=125 => 2,
                126..=65_535 => 4,
                _ => 10,
            };
            assert_eq!(buf.len(), head + n);
            assert_eq!(buf[0], 0x80 | BINARY);
            if n <= MAX_FRAME as usize {
                match read(&mut buf.as_slice()).unwrap() {
                    Frame::Binary(b) => assert_eq!(b, payload),
                    _ => panic!("expected binary"),
                }
            }
        }
    }
}
