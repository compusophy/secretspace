//! Who is on the other end of a socket. A page opens every game socket with
//! the platform `Hello`: a secret 128-bit key it keeps in its storage, the
//! name it would like, and its build. The server turns the key into a soul
//! (the first 8 bytes of its SHA-1; the key itself is never stored), answers
//! with `Seen` (the name it will go by) and tells the room a `Who`. A page
//! that says anything else first is a guest, soul 0.

use crate::sha1::sha1;
use crate::wire::{Reader, Writer};

/// The first byte of every platform message, both ways. Rooms never use it.
/// The second byte is its kind.
pub const PLATFORM: u8 = 0xFE;
/// Up: `Hello`. Down: `Seen`.
pub const HELLO: u8 = 1;
pub const SEEN: u8 = 1;
/// Down: the world is holding still (a deploy, or a room rebuilt after a
/// fault); keep the picture, reconnect, and carry on where you were.
pub const STILL: u8 = 2;
/// The platform protocol this build speaks.
pub const PROTO: u8 = 1;
pub const MAX_NAME: usize = 16;

/// Who a connection is, as the server hands it to a room.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Who {
    /// 0 for a guest.
    pub soul: u64,
    /// The name it goes by everywhere ("" for a guest who gave none).
    pub name: String,
    /// It only watches: it cannot play and is never one of the people.
    pub watch: bool,
    /// The page's build, for telling an old page it is old.
    pub build: u32,
}

impl Who {
    pub fn guest(watch: bool) -> Who {
        Who {
            watch,
            ..Who::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hello {
    pub proto: u8,
    pub key: [u8; 16],
    pub name: String,
    /// Take this name even if the soul already has one.
    pub rename: bool,
    pub build: u32,
}

impl Hello {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.u8(PLATFORM).u8(HELLO).u8(self.proto);
        w.0.extend_from_slice(&self.key);
        w.str(&self.name).u8(self.rename as u8).u32(self.build);
        w.0
    }

    pub fn decode(b: &[u8]) -> Option<Hello> {
        let mut r = Reader::new(b);
        if r.u8()? != PLATFORM || r.u8()? != HELLO {
            return None;
        }
        let proto = r.u8()?;
        let mut key = [0u8; 16];
        for k in &mut key {
            *k = r.u8()?;
        }
        Some(Hello {
            proto,
            key,
            name: r.str()?,
            rename: r.u8()? != 0,
            build: r.u32()?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// A soul the server knew, under its name.
    Ok = 0,
    /// A soul the server had not seen; the name was free and is now its.
    New = 1,
    /// The name asked for belongs to someone else.
    Taken = 2,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seen {
    pub status: Status,
    pub name: String,
}

impl Seen {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.u8(PLATFORM)
            .u8(SEEN)
            .u8(self.status as u8)
            .str(&self.name);
        w.0
    }

    pub fn decode(b: &[u8]) -> Option<Seen> {
        let mut r = Reader::new(b);
        if r.u8()? != PLATFORM || r.u8()? != SEEN {
            return None;
        }
        let status = match r.u8()? {
            0 => Status::Ok,
            1 => Status::New,
            2 => Status::Taken,
            _ => return None,
        };
        Some(Seen {
            status,
            name: r.str()?,
        })
    }
}

/// The platform's "hold still" message.
pub fn still() -> Vec<u8> {
    vec![PLATFORM, STILL]
}

/// Whether a message from the server is the platform's Still.
pub fn is_still(b: &[u8]) -> bool {
    b == [PLATFORM, STILL]
}

/// A key's soul: the first 8 bytes of its SHA-1, never 0 (0 is a guest).
pub fn soul_of(key: &[u8; 16]) -> u64 {
    let h = sha1(key);
    let mut b = [0u8; 8];
    b.copy_from_slice(&h[..8]);
    u64::from_le_bytes(b).max(1)
}

/// A name as it may be shown: no control characters, no runs of spaces,
/// trimmed, at most `MAX_NAME` characters.
pub fn clean_name(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars().filter(|c| !c.is_control()) {
        if c.is_whitespace() {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
        if out.chars().count() >= MAX_NAME {
            break;
        }
    }
    out.trim_end().to_string()
}

/// A name folded so look-alikes collide: case, 0/o, 1/l/i, 5/s, spaces.
/// Two names that fold the same are the same name.
pub fn fold(name: &str) -> String {
    clean_name(name)
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            '0' => 'o',
            '1' | 'i' => 'l',
            '5' => 's',
            c => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_and_seen_round_trip() {
        let h = Hello {
            proto: PROTO,
            key: [7; 16],
            name: "zoë".into(),
            rename: true,
            build: 42,
        };
        assert_eq!(Hello::decode(&h.encode()), Some(h));
        let s = Seen {
            status: Status::Taken,
            name: "zoë".into(),
        };
        assert_eq!(Seen::decode(&s.encode()), Some(s));
        assert_eq!(Hello::decode(&[PLATFORM, HELLO, 1, 2]), None);
        assert!(is_still(&still()));
        assert_eq!(Seen::decode(&still()), None);
        assert_eq!(Hello::decode(&[2, 1, 0]), None);
    }

    #[test]
    fn a_key_makes_one_soul_and_never_a_guest() {
        let a = soul_of(&[1; 16]);
        assert_eq!(a, soul_of(&[1; 16]));
        assert_ne!(a, soul_of(&[2; 16]));
        assert_ne!(a, 0);
    }

    #[test]
    fn names_are_cleaned_and_look_alikes_fold_together() {
        assert_eq!(clean_name("  a\u{7}  b   c "), "a b c");
        assert_eq!(
            clean_name("abcdefghijklmnopqrstu").chars().count(),
            MAX_NAME
        );
        assert_eq!(fold("N00dle"), fold("noodle"));
        assert_eq!(fold("Il1"), fold("lll"));
        assert_eq!(fold("5am"), fold("Sam"));
        assert_eq!(fold("big boss"), fold("bigboss"));
        assert_ne!(fold("noodle"), fold("noodles"));
    }
}
