//! Portals between islands: who is out there, which of our four sides is
//! linked to whom, and motes in the wire until the far side acknowledges
//! them.
//!
//! Pure: driven by the clock it is given and the envelopes it receives, it
//! queues the envelopes to send. Any transport carries them: one browser's
//! BroadcastChannel, or WebRTC between devices.
//!
//! A mote is never duplicated. The sender keeps it in flight and resends
//! until it is acknowledged; the receiver acknowledges every copy but
//! admits a sequence number once. If the far island vanishes first, the
//! mote is lost between worlds.

use std::collections::{BTreeMap, VecDeque};

use crate::island::Traveler;
use crate::laws::opposite;
use crate::rng::splitmix;
use crate::wire::{CensusEntry, Envelope, Hello, Msg};

pub const HELLO_MS: u64 = 1000;
pub const PEER_TIMEOUT_MS: u64 = 6000;
pub const LINK_TIMEOUT_MS: u64 = 2500;
pub const RESEND_MS: u64 = 700;
pub const CENSUS_MS: u64 = 3000;
pub const CENSUS_TTL_MS: u64 = 12_000;
const MAX_PEERS: usize = 64;
const DEDUPE: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Portal {
    Shut,
    Asking { peer: u64, since: u64 },
    Open { peer: u64, theirs: u8 },
}

#[derive(Clone, Debug)]
pub struct Peer {
    pub hello: Hello,
    pub seen: u64,
    pub census: Vec<CensusEntry>,
    pub census_at: u64,
}

#[derive(Clone, Debug)]
struct Flight {
    peer: u64,
    side: u8,
    traveler: Traveler,
    sent: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetEvent {
    Arrive { side: u8, traveler: Traveler },
    Delivered { peer: u64 },
    Lost { traveler: Traveler },
    Opened { side: u8, peer: u64 },
    Closed { side: u8, peer: u64 },
}

/// One lineage across every island this one can hear.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldLineage {
    pub lineage: u64,
    pub name: String,
    pub author: String,
    /// Islands it is alive on.
    pub tabs: u32,
    pub count: u32,
}

pub struct Net {
    pub me: u64,
    pub portals: [Portal; 4],
    pub peers: BTreeMap<u64, Peer>,
    pub asleep: bool,
    flights: BTreeMap<u64, Flight>,
    next_seq: u64,
    seen: BTreeMap<u64, VecDeque<u64>>,
    last_hello: Option<u64>,
    last_census: Option<u64>,
    out: Vec<Envelope>,
    events: Vec<NetEvent>,
}

impl Net {
    pub fn new(me: u64) -> Net {
        Net {
            me,
            portals: [Portal::Shut; 4],
            peers: BTreeMap::new(),
            asleep: false,
            flights: BTreeMap::new(),
            next_seq: 1,
            seen: BTreeMap::new(),
            last_hello: None,
            last_census: None,
            out: Vec::new(),
            events: Vec::new(),
        }
    }

    fn send(&mut self, to: u64, msg: Msg) {
        self.out.push(Envelope {
            from: self.me,
            to,
            msg,
        });
    }

    /// Envelopes to send and events for the island, since the last call.
    pub fn drain(&mut self) -> (Vec<Envelope>, Vec<NetEvent>) {
        (
            std::mem::take(&mut self.out),
            std::mem::take(&mut self.events),
        )
    }

    pub fn free_sides(&self) -> u8 {
        self.portals.iter().filter(|p| **p == Portal::Shut).count() as u8
    }

    fn linked_to(&self, peer: u64) -> Option<usize> {
        self.portals
            .iter()
            .position(|p| matches!(p, Portal::Open { peer: q, .. } | Portal::Asking { peer: q, .. } if *q == peer))
    }

    fn close(&mut self, side: usize) {
        if let Portal::Open { peer, .. } = self.portals[side] {
            self.events.push(NetEvent::Closed {
                side: side as u8,
                peer,
            });
        }
        self.portals[side] = Portal::Shut;
    }

    fn drop_peer(&mut self, peer: u64) {
        self.peers.remove(&peer);
        self.seen.remove(&peer);
        for side in 0..4 {
            match self.portals[side] {
                Portal::Open { peer: q, .. } | Portal::Asking { peer: q, .. } if q == peer => {
                    self.close(side)
                }
                _ => {}
            }
        }
        let lost: Vec<u64> = self
            .flights
            .iter()
            .filter(|(_, f)| f.peer == peer)
            .map(|(&s, _)| s)
            .collect();
        for seq in lost {
            if let Some(f) = self.flights.remove(&seq) {
                self.events.push(NetEvent::Lost {
                    traveler: f.traveler,
                });
            }
        }
    }

    /// The island's view of its portals: 0 shut, else 1 + the sun beyond.
    pub fn portal_view(&self) -> [u32; 4] {
        let mut v = [0; 4];
        for (side, p) in self.portals.iter().enumerate() {
            if let Portal::Open { peer, .. } = p {
                if let Some(q) = self.peers.get(peer) {
                    v[side] = 1 + q.hello.sun as u32;
                }
            }
        }
        v
    }

    /// The peer beyond a side, if that portal is open.
    pub fn beyond(&self, side: usize) -> Option<(u64, &Peer)> {
        match self.portals[side] {
            Portal::Open { peer, .. } => self.peers.get(&peer).map(|p| (peer, p)),
            _ => None,
        }
    }

    /// Advance the clock: greet, expire, link, resend.
    pub fn tick(&mut self, now: u64, hello: Hello, census: &[CensusEntry]) {
        let greet = self.last_hello.is_none_or(|t| now >= t + HELLO_MS);
        if greet {
            self.last_hello = Some(now);
            let mut hello = hello;
            hello.asleep = self.asleep;
            hello.free = self.free_sides();
            self.send(0, Msg::Hello(hello));
        }
        if self.last_census.is_none_or(|t| now >= t + CENSUS_MS) {
            self.last_census = Some(now);
            self.send(0, Msg::Census(census.to_vec()));
        }
        let gone: Vec<u64> = self
            .peers
            .iter()
            .filter(|(_, p)| now > p.seen + PEER_TIMEOUT_MS)
            .map(|(&id, _)| id)
            .collect();
        for id in gone {
            self.drop_peer(id);
        }
        for side in 0..4 {
            if let Portal::Asking { since, .. } = self.portals[side] {
                if now > since + LINK_TIMEOUT_MS {
                    self.portals[side] = Portal::Shut;
                }
            }
        }
        if greet && !self.asleep {
            self.seek(now);
        }
        let due: Vec<u64> = self
            .flights
            .iter()
            .filter(|(_, f)| now >= f.sent + RESEND_MS)
            .map(|(&s, _)| s)
            .collect();
        for seq in due {
            let f = self.flights.get_mut(&seq).expect("due flight");
            f.sent = now;
            let (peer, side, traveler) = (f.peer, f.side, f.traveler.clone());
            self.send(
                peer,
                Msg::Mote {
                    seq,
                    side,
                    traveler,
                },
            );
        }
    }

    /// Ask one awake stranger to link a shut side.
    fn seek(&mut self, now: u64) {
        let Some(side) = self.portals.iter().position(|p| *p == Portal::Shut) else {
            return;
        };
        let me = self.me;
        let pick = self
            .peers
            .iter()
            .filter(|(&id, p)| {
                !p.hello.asleep
                    && p.hello.free > 0
                    && now <= p.seen + PEER_TIMEOUT_MS
                    && self.linked_to(id).is_none()
            })
            .max_by_key(|(&id, p)| (p.hello.free, splitmix(me ^ id)))
            .map(|(&id, _)| id);
        if let Some(peer) = pick {
            self.portals[side] = Portal::Asking { peer, since: now };
            self.send(peer, Msg::LinkReq { side: side as u8 });
        }
    }

    /// Hand a departing mote to the wire. Err gives it back when the
    /// portal shut since the island last looked.
    // Handing the mote back whole is the point of the Err.
    #[allow(clippy::result_large_err)]
    pub fn send_mote(&mut self, now: u64, side: u8, traveler: Traveler) -> Result<(), Traveler> {
        let Portal::Open { peer, .. } = self.portals[side as usize % 4] else {
            return Err(traveler);
        };
        let seq = self.next_seq;
        self.next_seq += 1;
        self.flights.insert(
            seq,
            Flight {
                peer,
                side,
                traveler: traveler.clone(),
                sent: now,
            },
        );
        self.send(
            peer,
            Msg::Mote {
                seq,
                side,
                traveler,
            },
        );
        Ok(())
    }

    pub fn in_flight(&self) -> usize {
        self.flights.len()
    }

    /// Night: shut every portal. The island keeps greeting, as asleep.
    pub fn sleep(&mut self) {
        if self.asleep {
            return;
        }
        self.asleep = true;
        self.last_hello = None;
        for side in 0..4 {
            match self.portals[side] {
                Portal::Open { peer, .. } => {
                    self.send(peer, Msg::Unlink { side: side as u8 });
                    self.close(side);
                }
                Portal::Asking { .. } => self.portals[side] = Portal::Shut,
                Portal::Shut => {}
            }
        }
    }

    pub fn wake(&mut self) {
        if self.asleep {
            self.asleep = false;
            self.last_hello = None;
        }
    }

    /// The tab is closing.
    pub fn bye(&mut self) {
        self.send(0, Msg::Bye);
    }

    pub fn receive(&mut self, now: u64, env: Envelope) {
        let from = env.from;
        if from == self.me || (env.to != 0 && env.to != self.me) {
            return;
        }
        if let Msg::Hello(h) = env.msg {
            if !self.peers.contains_key(&from) && self.peers.len() >= MAX_PEERS {
                return;
            }
            let asleep = h.asleep;
            let p = self.peers.entry(from).or_insert_with(|| Peer {
                hello: h.clone(),
                seen: now,
                census: Vec::new(),
                census_at: 0,
            });
            p.hello = h;
            p.seen = now;
            if asleep {
                for side in 0..4 {
                    if matches!(self.portals[side], Portal::Open { peer, .. } if peer == from) {
                        self.close(side);
                    }
                }
            }
            return;
        }
        // Everything else only from islands we have heard greet.
        match self.peers.get_mut(&from) {
            Some(peer) => peer.seen = now,
            None => return,
        }
        match env.msg {
            Msg::Hello(_) => {}
            Msg::LinkReq { side: theirs } => self.on_link_req(from, theirs),
            Msg::LinkAck {
                side: theirs,
                yours,
            } => {
                let mine = yours as usize;
                match self.portals[mine] {
                    Portal::Asking { peer, .. } if peer == from => {
                        self.portals[mine] = Portal::Open { peer: from, theirs };
                        self.events.push(NetEvent::Opened {
                            side: yours,
                            peer: from,
                        });
                    }
                    Portal::Open { peer, theirs: t } if peer == from && t == theirs => {}
                    _ => self.send(from, Msg::Unlink { side: yours }),
                }
            }
            Msg::LinkNo => {
                for side in 0..4 {
                    if matches!(self.portals[side], Portal::Asking { peer, .. } if peer == from) {
                        self.portals[side] = Portal::Shut;
                    }
                }
            }
            Msg::Unlink { side: theirs } => {
                for side in 0..4 {
                    if self.portals[side] == (Portal::Open { peer: from, theirs }) {
                        self.close(side);
                    }
                }
            }
            Msg::Mote {
                seq,
                side: theirs,
                traveler,
            } => {
                self.send(from, Msg::MoteAck { seq });
                let seen = self.seen.entry(from).or_default();
                if seen.contains(&seq) {
                    return;
                }
                seen.push_back(seq);
                if seen.len() > DEDUPE {
                    seen.pop_front();
                }
                let side = self
                    .portals
                    .iter()
                    .position(|p| *p == Portal::Open { peer: from, theirs })
                    .map_or(opposite(theirs), |s| s as u8);
                self.events.push(NetEvent::Arrive { side, traveler });
            }
            Msg::MoteAck { seq } => {
                if self.flights.get(&seq).is_some_and(|f| f.peer == from) {
                    self.flights.remove(&seq);
                    self.events.push(NetEvent::Delivered { peer: from });
                }
            }
            Msg::Census(entries) => {
                if let Some(p) = self.peers.get_mut(&from) {
                    p.census = entries;
                    p.census_at = now;
                }
            }
            Msg::Bye => self.drop_peer(from),
            // The relay's to read, not a neighbor's.
            Msg::World { .. } => {}
        }
    }

    fn on_link_req(&mut self, from: u64, theirs: u8) {
        if self.asleep {
            self.send(from, Msg::LinkNo);
            return;
        }
        // Already linked with them: answer the same way again, or refuse a
        // second link between the same two islands.
        if let Some(side) = self.linked_to(from) {
            match self.portals[side] {
                Portal::Open { theirs: t, .. } if t == theirs => {
                    self.send(
                        from,
                        Msg::LinkAck {
                            side: side as u8,
                            yours: theirs,
                        },
                    );
                    return;
                }
                // Both asked at once: take theirs on the side we were asking with.
                Portal::Asking { .. } => {
                    self.open(side, from, theirs);
                    return;
                }
                _ => {
                    self.send(from, Msg::LinkNo);
                    return;
                }
            }
        }
        let want = opposite(theirs) as usize;
        let side = if self.portals[want] == Portal::Shut {
            Some(want)
        } else {
            (0..4).find(|&s| self.portals[s] == Portal::Shut)
        };
        match side {
            Some(side) => self.open(side, from, theirs),
            None => self.send(from, Msg::LinkNo),
        }
    }

    fn open(&mut self, side: usize, peer: u64, theirs: u8) {
        self.portals[side] = Portal::Open { peer, theirs };
        self.events.push(NetEvent::Opened {
            side: side as u8,
            peer,
        });
        self.send(
            peer,
            Msg::LinkAck {
                side: side as u8,
                yours: theirs,
            },
        );
    }

    /// Every lineage this island can hear of, with how many islands it is
    /// alive on. `mine` is this island's own census.
    pub fn world(&self, now: u64, mine: &[CensusEntry]) -> Vec<WorldLineage> {
        let mut by: BTreeMap<u64, WorldLineage> = BTreeMap::new();
        let fresh = self
            .peers
            .values()
            .filter(|p| p.census_at > 0 && now <= p.census_at + CENSUS_TTL_MS)
            .map(|p| p.census.as_slice());
        for census in std::iter::once(mine).chain(fresh) {
            for e in census.iter().filter(|e| e.count > 0) {
                let w = by.entry(e.lineage).or_insert_with(|| WorldLineage {
                    lineage: e.lineage,
                    name: e.name.clone(),
                    author: e.author.clone(),
                    tabs: 0,
                    count: 0,
                });
                w.tabs += 1;
                w.count += e.count;
            }
        }
        let mut v: Vec<WorldLineage> = by.into_values().collect();
        v.sort_by(|a, b| {
            b.tabs
                .cmp(&a.tabs)
                .then(b.count.cmp(&a.count))
                .then(a.lineage.cmp(&b.lineage))
        });
        v
    }
}
