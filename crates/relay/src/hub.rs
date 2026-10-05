//! Who is connected, introductions between them, and the world's census.
//!
//! The relay never sees a mote or an island: it forwards the few messages
//! two browsers need to open a direct channel, and sums the lineage counts
//! each island volunteers.

use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use space::wire::{encode, CensusEntry, Envelope, Msg, WorldEntry, MAX_CENSUS};

/// How many strangers a newcomer is introduced to.
pub const INTRODUCE: usize = 6;
pub const MAX_CLIENTS: usize = 20_000;
const CENSUS_TTL: Duration = Duration::from_secs(12);

pub enum Out {
    Text(String),
    Binary(Vec<u8>),
    Pong(Vec<u8>),
    Close,
}

struct Client {
    tx: Sender<Out>,
    census: Vec<CensusEntry>,
    census_at: Option<Instant>,
    joined: Instant,
}

pub struct Hub {
    inner: Mutex<Inner>,
}

struct Inner {
    clients: HashMap<u64, Client>,
    rng: u64,
}

impl Inner {
    fn roll(&mut self) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng
    }
}

impl Hub {
    pub fn new(seed: u64) -> Hub {
        Hub {
            inner: Mutex::new(Inner {
                clients: HashMap::new(),
                rng: seed | 1,
            }),
        }
    }

    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().clients.len()
    }

    /// Register an island. False if the id is taken or the hub is full.
    pub fn join(&self, id: u64, tx: Sender<Out>) -> bool {
        let mut g = self.inner.lock().unwrap();
        if id == 0 || g.clients.contains_key(&id) || g.clients.len() >= MAX_CLIENTS {
            return false;
        }
        g.clients.insert(
            id,
            Client {
                tx,
                census: Vec::new(),
                census_at: None,
                joined: Instant::now(),
            },
        );
        true
    }

    pub fn leave(&self, id: u64) {
        self.inner.lock().unwrap().clients.remove(&id);
    }

    /// Up to INTRODUCE other islands, at random, the newest favoured a
    /// little so fresh islands find each other.
    pub fn introduce(&self, id: u64) -> Vec<u64> {
        let mut g = self.inner.lock().unwrap();
        let mut others: Vec<(u64, u64)> = Vec::new();
        let ids: Vec<(u64, Instant)> = g
            .clients
            .iter()
            .filter(|(&k, _)| k != id)
            .map(|(&k, c)| (k, c.joined))
            .collect();
        for (k, joined) in ids {
            let age = joined.elapsed().as_secs().min(600);
            others.push((g.roll() % 1000 + age, k));
        }
        others.sort();
        others.into_iter().take(INTRODUCE).map(|(_, k)| k).collect()
    }

    /// Forward a signaling payload. False if the target is gone.
    pub fn forward(&self, from: u64, to: u64, payload: &str) -> bool {
        let g = self.inner.lock().unwrap();
        match g.clients.get(&to) {
            Some(c) => {
                c.tx.send(Out::Text(format!("from {from:x} {payload}")))
                    .is_ok()
            }
            None => false,
        }
    }

    pub fn census(&self, id: u64, entries: Vec<CensusEntry>) {
        let mut g = self.inner.lock().unwrap();
        if let Some(c) = g.clients.get_mut(&id) {
            c.census = entries;
            c.census_at = Some(Instant::now());
        }
    }

    /// Sum every fresh census and send the world to everyone.
    pub fn broadcast_world(&self) {
        let g = self.inner.lock().unwrap();
        let mut by: BTreeMap<u64, WorldEntry> = BTreeMap::new();
        for c in g.clients.values() {
            if !c.census_at.is_some_and(|t| t.elapsed() < CENSUS_TTL) {
                continue;
            }
            for e in c.census.iter().filter(|e| e.count > 0) {
                let w = by.entry(e.lineage).or_insert_with(|| WorldEntry {
                    lineage: e.lineage,
                    tabs: 0,
                    count: 0,
                    name: e.name.clone(),
                    author: e.author.clone(),
                });
                w.tabs += 1;
                w.count = w.count.saturating_add(e.count);
            }
        }
        let mut entries: Vec<WorldEntry> = by.into_values().collect();
        entries.sort_by(|a, b| {
            b.tabs
                .cmp(&a.tabs)
                .then(b.count.cmp(&a.count))
                .then(a.lineage.cmp(&b.lineage))
        });
        entries.truncate(MAX_CENSUS);
        let bytes = encode(&Envelope {
            from: 0,
            to: 0,
            msg: Msg::World {
                islands: g.clients.len() as u32,
                entries,
            },
        });
        for c in g.clients.values() {
            let _ = c.tx.send(Out::Binary(bytes.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn introductions_and_forwarding() {
        let hub = Hub::new(7);
        let (ta, ra) = channel();
        let (tb, rb) = channel();
        assert!(hub.join(1, ta));
        assert!(hub.join(2, tb.clone()));
        assert!(!hub.join(2, tb), "ids are unique");
        assert_eq!(hub.introduce(1), vec![2]);
        assert!(hub.forward(2, 1, "offer abc"));
        match ra.try_recv().unwrap() {
            Out::Text(s) => assert_eq!(s, "from 2 offer abc"),
            _ => panic!(),
        }
        hub.leave(1);
        assert!(!hub.forward(2, 1, "x"));
        drop(rb);
    }

    #[test]
    fn the_world_is_the_sum_of_its_islands() {
        let hub = Hub::new(3);
        let (ta, ra) = channel();
        let (tb, _rb) = channel();
        hub.join(1, ta);
        hub.join(2, tb);
        let e = |count| CensusEntry {
            lineage: 42,
            count,
            name: "nomad".into(),
            author: "x".into(),
        };
        hub.census(1, vec![e(10)]);
        hub.census(2, vec![e(5)]);
        hub.broadcast_world();
        let Out::Binary(b) = ra.try_recv().unwrap() else {
            panic!()
        };
        let Msg::World { islands, entries } = space::wire::decode(&b).unwrap().msg else {
            panic!()
        };
        assert_eq!(islands, 2);
        assert_eq!((entries[0].tabs, entries[0].count), (2, 15));
    }
}
