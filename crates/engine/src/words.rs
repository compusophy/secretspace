//! A key as words a person can write down: each word is four letters,
//! consonant-vowel-consonant-vowel ("bako", "rimu"), 12 bits apiece; eleven
//! of them carry the 128-bit key and a 4-bit check, so a slip in copying
//! is caught rather than restoring someone else's soul.

use crate::sha1::sha1;

const C: &[u8; 16] = b"bdfghjklmnprstvz";
const V: &[u8; 4] = b"aeio";
pub const COUNT: usize = 11;

fn word(n: u16) -> String {
    let n = n as usize;
    [C[n >> 8 & 15], V[n >> 6 & 3], C[n >> 2 & 15], V[n & 3]]
        .iter()
        .map(|&b| b as char)
        .collect()
}

fn unword(w: &str) -> Option<u16> {
    let b = w.as_bytes();
    if b.len() != 4 {
        return None;
    }
    let c = |x: u8| C.iter().position(|&k| k == x).map(|p| p as u16);
    let v = |x: u8| V.iter().position(|&k| k == x).map(|p| p as u16);
    Some(c(b[0])? << 8 | v(b[1])? << 6 | c(b[2])? << 2 | v(b[3])?)
}

fn check(key: &[u8; 16]) -> u16 {
    (sha1(key)[0] >> 4) as u16
}

/// The key as eleven words.
pub fn to_words(key: &[u8; 16]) -> Vec<String> {
    // 128 bits of key then 4 of check, 12 to a word.
    let mut bits = u128::from_be_bytes(*key);
    // From the end: the last word holds the low 8 key bits and the check.
    let mut words = [0u16; COUNT];
    words[COUNT - 1] = ((bits & 0xff) as u16) << 4 | check(key);
    bits >>= 8;
    for w in words[..COUNT - 1].iter_mut().rev() {
        *w = (bits & 0xfff) as u16;
        bits >>= 12;
    }
    words.into_iter().map(word).collect()
}

/// The key back from its words (any case, any spacing), or None if a word
/// is not one of ours or the check does not match.
pub fn from_words(text: &str) -> Option<[u8; 16]> {
    let ws: Vec<String> = text
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    if ws.len() != COUNT {
        return None;
    }
    let mut bits: u128 = 0;
    for w in &ws[..COUNT - 1] {
        bits = bits << 12 | unword(w)? as u128;
    }
    let last = unword(&ws[COUNT - 1])?;
    bits = bits << 8 | (last >> 4) as u128;
    let key = bits.to_be_bytes();
    (check(&key) == last & 15).then_some(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    #[test]
    fn keys_round_trip_through_words_and_slips_are_caught() {
        let mut rng = Rng::new(3);
        let mut missed = 0;
        for _ in 0..500 {
            let mut key = [0u8; 16];
            for k in &mut key {
                *k = rng.below(256) as u8;
            }
            let words = to_words(&key);
            assert_eq!(words.len(), COUNT);
            assert!(words.iter().all(|w| w.len() == 4));
            let text = words.join(" ").to_uppercase();
            assert_eq!(from_words(&text), Some(key));
            // Change one letter of one word to another valid one.
            let mut bad = words.clone();
            let w = &mut bad[rng.below(COUNT as u64) as usize];
            let first = w.as_bytes()[0];
            let swap = if first == b'b' { 'd' } else { 'b' };
            w.replace_range(0..1, &swap.to_string());
            assert_ne!(from_words(&bad.join(" ")), Some(key));
            missed += from_words(&bad.join(" ")).is_some() as u32;
        }
        // A 4-bit check misses one slip in sixteen.
        assert!(missed < 60, "{missed}");
        assert_eq!(from_words("bako"), None);
        assert_eq!(from_words(""), None);
    }
}
