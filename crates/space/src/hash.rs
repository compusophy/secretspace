//! FNV-1a, 64 bits. The island hash is the receipt: two islands fed the
//! same inputs hash the same, and anyone can recompute it.

const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Debug)]
pub struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Fnv(OFFSET)
    }
}

impl Fnv {
    pub fn bytes(&mut self, b: &[u8]) -> &mut Self {
        for &x in b {
            self.0 ^= x as u64;
            self.0 = self.0.wrapping_mul(PRIME);
        }
        self
    }

    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.bytes(&v.to_le_bytes())
    }

    pub fn i64(&mut self, v: i64) -> &mut Self {
        self.bytes(&v.to_le_bytes())
    }

    pub fn finish(&self) -> u64 {
        self.0
    }
}

pub fn hash_bytes(b: &[u8]) -> u64 {
    Fnv::default().bytes(b).finish()
}

pub fn hash_str(s: &str) -> u64 {
    hash_bytes(s.as_bytes())
}
