//! Who this browser is, for every game here: a secret 128-bit key kept in
//! `secretspace/key` (shared by every page on the origin) and sent only in
//! the platform Hello, never in a URL; the server knows the soul it makes
//! and the name it goes by. The key can be written down as words, and
//! given back on another device to be the same soul there.

use engine::who::{Hello, PROTO};
use engine::words;

pub const KEY: &str = "secretspace/key";
pub const NAME: &str = "secretspace/name";

pub struct Session {
    key: [u8; 16],
}

fn hex(key: &[u8; 16]) -> String {
    key.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<[u8; 16]> {
    let s = s.trim();
    if s.len() != 32 {
        return None;
    }
    let mut key = [0u8; 16];
    for (i, k) in key.iter_mut().enumerate() {
        *k = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(key)
}

fn random_key() -> [u8; 16] {
    let mut key = [0u8; 16];
    let crypto = crate::window().crypto().ok();
    if crypto.is_none_or(|c| c.get_random_values_with_u8_array(&mut key).is_err()) {
        // No crypto: still a key, if a weaker one.
        for (i, k) in key.iter_mut().enumerate() {
            *k = (js_sys::Math::random() * 256.0) as u8 ^ (crate::now() as u64 >> i) as u8;
        }
    }
    key
}

impl Session {
    /// This browser's session, made the first time any page asks.
    pub fn load() -> Session {
        let key = crate::load(KEY).and_then(|k| unhex(&k)).unwrap_or_else(|| {
            let key = random_key();
            crate::save(KEY, &hex(&key));
            key
        });
        Session { key }
    }

    pub fn key(&self) -> [u8; 16] {
        self.key
    }

    /// The name last used here.
    pub fn name(&self) -> String {
        crate::load(NAME).unwrap_or_default()
    }

    pub fn set_name(&self, name: &str) {
        crate::save(NAME, name);
    }

    /// The key as words to write down.
    pub fn words(&self) -> String {
        words::to_words(&self.key).join(" ")
    }

    /// Become the soul these words are; false if they are not words of ours.
    pub fn restore(&mut self, text: &str) -> bool {
        match words::from_words(text) {
            Some(key) => {
                self.key = key;
                crate::save(KEY, &hex(&key));
                true
            }
            None => false,
        }
    }

    /// The platform Hello: who this is, the name it would like, whether to
    /// take that name even if the soul already has one.
    pub fn hello(&self, name: &str, rename: bool) -> Vec<u8> {
        Hello {
            proto: PROTO,
            key: self.key,
            name: name.to_string(),
            rename,
            build: crate::version::build(),
        }
        .encode()
    }
}
