//! Who is connected, introductions between them, and the world's census.
//!
//! Introductions speak the WebTorrent tracker protocol, so a page can use
//! this relay and public trackers interchangeably. An island announces
//! itself in a swarm (the info hash) with a few WebRTC offers; each offer is
//! handed to a random other island in the swarm, whose answer comes back by
//! peer id. The relay never sees a mote or an island: it forwards those
//! introductions, and sums the lineage counts each island volunteers.

use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use space::wire::{encode, CensusEntry, Envelope, Msg, WorldEntry, MAX_CENSUS};

use crate::json::{self, obj, s, Json};

pub const MAX_CLIENTS: usize = 20_000;
/// Offers one announce may carry.
const MAX_OFFERS: usize = 10;
/// Seconds between announces the relay asks for.
const INTERVAL: f64 = 60.0;
const CENSUS_TTL: Duration = Duration::from_secs(12);

pub enum Out {
    Text(String),
    Binary(Vec<u8>),
    Pong(Vec<u8>),
    Close,
}

struct Client {
    tx: Sender<Out>,
    /// (info hash, peer id) this connection announced as.
    swarms: Vec<(String, String)>,
    census: Vec<CensusEntry>,
    census_at: Option<Instant>,
}

pub struct Hub {
    inner: Mutex<Inner>,
}

struct Inner {
    clients: HashMap<u64, Client>,
    /// info hash -> peer id -> connection.
    swarms: HashMap<String, BTreeMap<String, u64>>,
    next: u64,
    rng: u64,
}

impl Inner {
    fn roll(&mut self) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng
    }

    fn send(&self, conn: u64, msg: &Json) {
        if let Some(c) = self.clients.get(&conn) {
            let _ = c.tx.send(Out::Text(json::write(msg)));
        }
    }
}

/// A tracker id: exactly 20 characters.
fn id20(v: Option<&str>) -> Option<String> {
    v.filter(|s| s.chars().count() == 20).map(str::to_string)
}

impl Hub {
    pub fn new(seed: u64) -> Hub {
        Hub {
            inner: Mutex::new(Inner {
                clients: HashMap::new(),
                swarms: HashMap::new(),
                next: 1,
                rng: seed | 1,
            }),
        }
    }

    /// Islands connected.
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().clients.len()
    }

    /// A new connection, or None when the hub is full.
    pub fn connect(&self, tx: Sender<Out>) -> Option<u64> {
        let mut g = self.inner.lock().unwrap();
        if g.clients.len() >= MAX_CLIENTS {
            return None;
        }
        let conn = g.next;
        g.next += 1;
        g.clients.insert(
            conn,
            Client {
                tx,
                swarms: Vec::new(),
                census: Vec::new(),
                census_at: None,
            },
        );
        Some(conn)
    }

    pub fn disconnect(&self, conn: u64) {
        let mut g = self.inner.lock().unwrap();
        if let Some(c) = g.clients.remove(&conn) {
            for (hash, peer) in c.swarms {
                if let Some(swarm) = g.swarms.get_mut(&hash) {
                    if swarm.get(&peer) == Some(&conn) {
                        swarm.remove(&peer);
                    }
                    if swarm.is_empty() {
                        g.swarms.remove(&hash);
                    }
                }
            }
        }
    }

    /// One tracker message from a connection.
    pub fn message(&self, conn: u64, msg: &Json) {
        if msg.str("action") != Some("announce") {
            return;
        }
        let (Some(hash), Some(peer)) = (id20(msg.str("info_hash")), id20(msg.str("peer_id")))
        else {
            return;
        };
        let mut g = self.inner.lock().unwrap();
        // Join the swarm; a peer id belongs to the connection that took it.
        let owner = g
            .swarms
            .entry(hash.clone())
            .or_default()
            .get(&peer)
            .copied();
        match owner {
            Some(c) if c != conn => return,
            Some(_) => {}
            None => {
                g.swarms
                    .get_mut(&hash)
                    .expect("just made")
                    .insert(peer.clone(), conn);
                if let Some(c) = g.clients.get_mut(&conn) {
                    c.swarms.push((hash.clone(), peer.clone()));
                }
            }
        }

        // An answer goes to the island whose offer it answers.
        if let (Some(answer), Some(to), Some(offer_id)) = (
            msg.get("answer"),
            id20(msg.str("to_peer_id")),
            msg.str("offer_id"),
        ) {
            if let Some(&target) = g.swarms.get(&hash).and_then(|sw| sw.get(&to)) {
                let fwd = obj(vec![
                    ("action", s("announce")),
                    ("answer", answer.clone()),
                    ("offer_id", s(offer_id)),
                    ("peer_id", s(&peer)),
                    ("info_hash", s(&hash)),
                ]);
                g.send(target, &fwd);
            }
            return;
        }

        let size = g.swarms.get(&hash).map_or(0, |sw| sw.len());
        let stats = obj(vec![
            ("action", s("announce")),
            ("interval", Json::Num(INTERVAL)),
            ("info_hash", s(&hash)),
            ("complete", Json::Num(size as f64)),
            ("incomplete", Json::Num(0.0)),
        ]);
        g.send(conn, &stats);

        // Each offer goes to a different random island in the swarm.
        let Some(offers) = msg.arr("offers") else {
            return;
        };
        let mut others: Vec<(u64, u64)> = Vec::new();
        let members: Vec<u64> = g.swarms[&hash]
            .iter()
            .filter(|(p, _)| **p != peer)
            .map(|(_, &c)| c)
            .collect();
        for c in members {
            let r = g.roll();
            others.push((r, c));
        }
        others.sort();
        for (offer, (_, target)) in offers.iter().take(MAX_OFFERS).zip(others) {
            let (Some(o), Some(id)) = (offer.get("offer"), offer.str("offer_id")) else {
                continue;
            };
            if id.len() > 64 {
                continue;
            }
            let fwd = obj(vec![
                ("action", s("announce")),
                ("offer", o.clone()),
                ("offer_id", s(id)),
                ("peer_id", s(&peer)),
                ("info_hash", s(&hash)),
            ]);
            g.send(target, &fwd);
        }
    }

    pub fn census(&self, conn: u64, entries: Vec<CensusEntry>) {
        let mut g = self.inner.lock().unwrap();
        if let Some(c) = g.clients.get_mut(&conn) {
            c.census = entries;
            c.census_at = Some(Instant::now());
        }
    }

    /// Sum every fresh census and send the world to everyone who sent one.
    pub fn broadcast_world(&self) {
        let g = self.inner.lock().unwrap();
        let mut by: BTreeMap<u64, WorldEntry> = BTreeMap::new();
        let mut islands = 0u32;
        for c in g.clients.values() {
            if !c.census_at.is_some_and(|t| t.elapsed() < CENSUS_TTL) {
                continue;
            }
            islands += 1;
            for e in c.census.iter().filter(|e| e.count > 0) {
                let w = by.entry(e.lineage).or_insert_with(|| WorldEntry {
                    lineage: e.lineage,
                    tabs: 0,
                    count: 0,
                    name: e.name.clone(),
                    author: e.author.clone(),
                });
                w.tabs += 1;
                w.count = w
                    .count
                    .saturating_add(e.count.min(space::laws::CELLS as u32));
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
            msg: Msg::World { islands, entries },
        });
        for c in g.clients.values().filter(|c| c.census_at.is_some()) {
            let _ = c.tx.send(Out::Binary(bytes.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{channel, Receiver};

    const HASH: &str = "secretspace-world-v1";

    fn text(r: &Receiver<Out>) -> Vec<Json> {
        r.try_iter()
            .filter_map(|o| match o {
                Out::Text(t) => json::parse(&t),
                _ => None,
            })
            .collect()
    }

    fn announce(peer: &str, offers: usize) -> Json {
        let offers = (0..offers)
            .map(|i| {
                obj(vec![
                    (
                        "offer_id",
                        s(&format!(
                            "{i:03}{}",
                            peer.chars().take(17).collect::<String>()
                        )),
                    ),
                    (
                        "offer",
                        obj(vec![("type", s("offer")), ("sdp", s("v=0\r\n"))]),
                    ),
                ])
            })
            .collect();
        obj(vec![
            ("action", s("announce")),
            ("info_hash", s(HASH)),
            ("peer_id", s(peer)),
            ("numwant", Json::Num(2.0)),
            ("offers", Json::Arr(offers)),
        ])
    }

    #[test]
    fn offers_reach_others_and_answers_come_back() {
        let hub = Hub::new(7);
        let (ta, ra) = channel();
        let (tb, rb) = channel();
        let a = hub.connect(ta).unwrap();
        let b = hub.connect(tb).unwrap();
        let (pa, pb) = ("aaaaaaaaaaaaaaaaaaaa", "bbbbbbbbbbbbbbbbbbbb");
        hub.message(b, &announce(pb, 0));
        hub.message(a, &announce(pa, 2));
        // A hears the swarm's size; B gets one of A's offers (only one other island).
        assert!(text(&ra).iter().any(|m| m.num("complete") == Some(2.0)));
        let got = text(&rb);
        let offer = got
            .iter()
            .find(|m| m.get("offer").is_some())
            .expect("B got an offer");
        assert_eq!(offer.str("peer_id"), Some(pa));
        let offer_id = offer.str("offer_id").unwrap().to_string();
        hub.message(
            b,
            &obj(vec![
                ("action", s("announce")),
                ("info_hash", s(HASH)),
                ("peer_id", s(pb)),
                ("to_peer_id", s(pa)),
                ("offer_id", s(&offer_id)),
                (
                    "answer",
                    obj(vec![("type", s("answer")), ("sdp", s("v=0\r\n"))]),
                ),
            ]),
        );
        let got = text(&ra);
        let answer = got
            .iter()
            .find(|m| m.get("answer").is_some())
            .expect("A got the answer");
        assert_eq!(
            (answer.str("peer_id"), answer.str("offer_id")),
            (Some(pb), Some(offer_id.as_str()))
        );
        hub.disconnect(b);
        hub.message(a, &announce(pa, 1));
        assert!(
            text(&ra).iter().any(|m| m.num("complete") == Some(1.0)),
            "B left the swarm"
        );
    }

    #[test]
    fn a_peer_id_cannot_be_taken_and_bad_ids_are_ignored() {
        let hub = Hub::new(3);
        let (ta, _ra) = channel();
        let (tb, rb) = channel();
        let a = hub.connect(ta).unwrap();
        let b = hub.connect(tb).unwrap();
        hub.message(a, &announce("aaaaaaaaaaaaaaaaaaaa", 0));
        hub.message(b, &announce("aaaaaaaaaaaaaaaaaaaa", 1));
        assert!(text(&rb).is_empty(), "B may not announce as A");
        hub.message(b, &announce("short", 1));
        assert!(text(&rb).is_empty());
    }

    #[test]
    fn the_world_is_the_sum_of_its_islands() {
        let hub = Hub::new(3);
        let (ta, ra) = channel();
        let (tb, _rb) = channel();
        let a = hub.connect(ta).unwrap();
        let b = hub.connect(tb).unwrap();
        let e = |count| CensusEntry {
            lineage: 42,
            count,
            name: "nomad".into(),
            author: "x".into(),
        };
        hub.census(a, vec![e(10)]);
        hub.census(b, vec![e(5)]);
        hub.broadcast_world();
        let Out::Binary(bytes) = ra.try_recv().unwrap() else {
            panic!()
        };
        let Msg::World { islands, entries } = space::wire::decode(&bytes).unwrap().msg else {
            panic!()
        };
        assert_eq!(islands, 2);
        assert_eq!((entries[0].tabs, entries[0].count), (2, 15));
    }
}
