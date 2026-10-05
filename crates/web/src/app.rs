//! The tab's state: its island, its portals, and what the page shows about
//! them. Everything here is driven by `pump` (the clock) and `receive`
//! (envelopes from other tabs); the page only draws it.

use std::collections::{BTreeMap, HashMap, VecDeque};

use space::founders::genesis;
use space::island::{EventKind, Island};
use space::laws::{TICKS_PER_SEC, W};
use space::net::{Net, NetEvent, Portal, WorldLineage};
use space::rng::splitmix;
use space::wire::{decode, encode, CensusEntry, Hello, Msg, WorldEntry};

use crate::place::{clock, Place};
use crate::store;

pub const TICK_MS: f64 = 1000.0 / TICKS_PER_SEC as f64;
/// The most ticks one pump catches up (a minute), after a throttled timer.
const MAX_CATCH_UP: u32 = 600;
const FEED_LEN: usize = 7;
const FOSSIL_KEEP: usize = 48;

pub struct FeedItem {
    pub at: f64,
    pub text: String,
    pub hue: Option<f64>,
}

/// A mote leaving through an edge, drawn flying out.
pub struct Flyer {
    pub x: f64,
    pub y: f64,
    pub side: u8,
    pub hue: f64,
    pub born: f64,
}

/// A mote landing at an edge, drawn as a ripple.
pub struct Ripple {
    pub x: f64,
    pub y: f64,
    pub hue: f64,
    pub born: f64,
}

/// What the island looked like when no one was watching any more.
struct Away {
    tick: u64,
    deaths: u64,
    departures: u64,
    arrivals: u64,
}

pub struct Summary {
    pub at: f64,
    pub lines: Vec<String>,
}

pub struct App {
    pub island: Island,
    pub net: Net,
    pub place: Place,
    pub author: String,
    pub mine: Vec<u64>,
    pub feed: VecDeque<FeedItem>,
    pub flyers: Vec<Flyer>,
    pub ripples: Vec<Ripple>,
    pub prev: HashMap<u64, (u16, u16)>,
    pub last_tick: f64,
    next_tick: f64,
    pub selected: Option<u64>,
    pub world: Vec<WorldLineage>,
    pub summary: Option<Summary>,
    away: Option<Away>,
    /// Encoded envelopes waiting for the transports: (to, is a census, bytes).
    pub outbox: Vec<(u64, bool, Vec<u8>)>,
    /// The relay's count of the whole world, when it was heard.
    relay_world: Option<(f64, u32, Vec<WorldEntry>)>,
    /// Islands online, by the relay's count.
    pub islands_online: Option<u32>,
    /// Direct channels to other devices, and whether the relay is up.
    pub links_open: usize,
    pub relay_up: bool,
    /// The last place each side opened to, for the feed after it shuts.
    last_beyond: [Option<String>; 4],
}

/// The founders' colours, fixed so they read apart at a glance.
const FOUNDER_HUES: [(&str, f64); 5] = [
    ("sprout", 135.0),
    ("grazer", 188.0),
    ("nomad", 262.0),
    ("drifter", 318.0),
    ("wolf", 4.0),
];

/// A lineage's hue on the colour wheel.
pub fn hue(lineage: u64) -> f64 {
    static FOUNDERS: std::sync::OnceLock<Vec<(u64, f64)>> = std::sync::OnceLock::new();
    let founders = FOUNDERS.get_or_init(|| {
        FOUNDER_HUES
            .iter()
            .map(|&(name, h)| (space::island::founder_lineage(name), h))
            .collect()
    });
    match founders.iter().find(|f| f.0 == lineage) {
        Some(&(_, h)) => h,
        None => (splitmix(lineage) % 360) as f64,
    }
}

const SIDES: [&str; 4] = ["north", "east", "south", "west"];

impl App {
    pub fn new(id: u64, place: Place, author: String, now: f64) -> App {
        let mut island = Island::new(id);
        let fossils = store::take_fossils();
        let woke = fossils.len();
        island.revive(fossils);
        if woke < 12 {
            genesis(&mut island);
        }
        let mut app = App {
            island,
            net: Net::new(id),
            place,
            author,
            mine: store::mine(),
            feed: VecDeque::new(),
            flyers: Vec::new(),
            ripples: Vec::new(),
            prev: HashMap::new(),
            last_tick: now,
            next_tick: now,
            selected: None,
            world: Vec::new(),
            summary: None,
            away: None,
            outbox: Vec::new(),
            relay_world: None,
            islands_online: None,
            links_open: 0,
            relay_up: false,
            last_beyond: Default::default(),
        };
        if woke > 0 {
            app.say(now, format!("{woke} motes woke from amber"), None);
        }
        app
    }

    pub fn say(&mut self, now: f64, text: String, hue: Option<f64>) {
        if self.feed.len() == FEED_LEN {
            self.feed.pop_front();
        }
        self.feed.push_back(FeedItem { at: now, text, hue });
    }

    fn beyond_name(&self, side: u8) -> String {
        self.net
            .beyond(side as usize)
            .map(|(_, p)| p.hello.place.clone())
            .or_else(|| self.last_beyond[side as usize].clone())
            .unwrap_or_else(|| format!("the {}", SIDES[side as usize % 4]))
    }

    fn census(&self) -> Vec<CensusEntry> {
        self.island
            .census()
            .iter()
            .take(24)
            .map(|l| CensusEntry {
                lineage: l.id,
                count: l.count,
                name: l.name.to_string(),
                author: l.author.to_string(),
            })
            .collect()
    }

    fn hello(&self) -> Hello {
        Hello {
            place: self.place.name.clone(),
            tz_min: self.place.tz_min,
            sun: self.island.sun as u8,
            asleep: false,
            free: 0,
            pop: self.island.motes().len().min(u16::MAX as usize) as u16,
        }
    }

    pub fn set_watched(&mut self, watched: bool, now: f64) {
        if watched == self.island.watched {
            return;
        }
        self.island.watched = watched;
        let c = self.island.counts;
        if !watched {
            self.away = Some(Away {
                tick: self.island.tick,
                deaths: c.deaths,
                departures: c.departures,
                arrivals: c.arrivals,
            });
            return;
        }
        if let Some(a) = self.away.take() {
            let ticks = self.island.tick.saturating_sub(a.tick);
            if ticks >= 50 {
                let secs = ticks / TICKS_PER_SEC;
                let mut lines = vec![format!(
                    "while no one watched: {}",
                    if secs >= 120 {
                        format!("{} minutes", secs / 60)
                    } else {
                        format!("{secs} seconds")
                    }
                )];
                lines.push(format!("{} motes died in the dark", c.deaths - a.deaths));
                if c.departures > a.departures {
                    lines.push(format!(
                        "{} fled through the portals",
                        c.departures - a.departures
                    ));
                }
                if c.arrivals > a.arrivals {
                    lines.push(format!("{} arrived anyway", c.arrivals - a.arrivals));
                }
                self.summary = Some(Summary { at: now, lines });
            }
        }
    }

    /// Run every tick that is due; a throttled timer catches up.
    pub fn pump(&mut self, now: f64) {
        let mut n = 0;
        while self.next_tick <= now && n < MAX_CATCH_UP {
            self.tick(self.next_tick);
            self.next_tick += TICK_MS;
            n += 1;
        }
        if self.next_tick < now - TICK_MS {
            self.next_tick = now + TICK_MS;
        }
    }

    fn tick(&mut self, now: f64) {
        let ms = now as u64;
        // Night: no light, portals shut. Dawn: open again.
        if !self.island.watched && self.island.sun == 0 {
            self.net.sleep();
        } else if self.island.watched {
            self.net.wake();
        }
        self.island.portals = self.net.portal_view();
        self.prev.clear();
        for m in self.island.motes() {
            self.prev.insert(m.id, (m.x, m.y));
        }
        self.island.step();
        self.last_tick = now;
        self.departures(now, ms);
        self.island_events(now);
        let census = self.census();
        self.net.tick(ms, self.hello(), &census);
        self.flush(now);
        self.world = self.net.world(ms, &census);
        self.merge_relay_world(now);
        for (side, p) in self.net.portals.iter().enumerate() {
            if let Portal::Open { peer, .. } = p {
                if let Some(q) = self.net.peers.get(peer) {
                    self.last_beyond[side] = Some(q.hello.place.clone());
                }
            }
        }
        self.flyers.retain(|f| now - f.born < 900.0);
        self.ripples.retain(|r| now - r.born < 1200.0);
    }

    fn departures(&mut self, now: f64, ms: u64) {
        let mut by: BTreeMap<(u8, String), (u32, f64)> = BTreeMap::new();
        for (side, t) in self.island.take_departures() {
            let (x, y) = edge_point(side, t.offset);
            let h = hue(t.lineage);
            self.flyers.push(Flyer {
                x,
                y,
                side,
                hue: h,
                born: now,
            });
            let e = by.entry((side, t.name.clone())).or_insert((0, h));
            e.0 += 1;
            if let Err(t) = self.net.send_mote(ms, side, t) {
                // The portal shut under it: it steps back in.
                self.island.arrive(side, t);
            }
        }
        for ((side, name), (n, h)) in by {
            let to = self.beyond_name(side);
            let who = if n == 1 {
                format!("a {name}")
            } else {
                format!("{n} {name}s")
            };
            self.say(now, format!("{who} left for {to}"), Some(h));
        }
    }

    fn island_events(&mut self, now: f64) {
        let tick = self.island.tick;
        let fresh: Vec<_> = self
            .island
            .events()
            .iter()
            .filter(|e| e.tick == tick)
            .map(|e| (e.kind, e.name.to_string(), e.lineage))
            .collect();
        for (kind, name, lineage) in fresh {
            let h = Some(hue(lineage));
            match kind {
                EventKind::Arrived(side) => {
                    let from = self.beyond_name(side);
                    self.say(now, format!("a {name} arrived from {from}"), h);
                }
                EventKind::Stillborn => {
                    self.say(now, format!("a {name} was stillborn at the border"), h)
                }
                EventKind::Crushed => {
                    self.say(now, format!("a {name} found no room at the border"), h)
                }
                _ => {}
            }
        }
    }

    /// The relay's world, folded into what this tab hears itself: a
    /// lineage is on at least as many islands as either count says.
    fn merge_relay_world(&mut self, now: f64) {
        self.islands_online = None;
        let Some((at, islands, entries)) = &self.relay_world else {
            return;
        };
        if now - at > 12_000.0 {
            return;
        }
        self.islands_online = Some(*islands);
        for e in entries {
            match self.world.iter_mut().find(|w| w.lineage == e.lineage) {
                Some(w) => {
                    w.tabs = w.tabs.max(e.tabs);
                    w.count = w.count.max(e.count);
                }
                None => self.world.push(WorldLineage {
                    lineage: e.lineage,
                    name: e.name.clone(),
                    author: e.author.clone(),
                    tabs: e.tabs,
                    count: e.count,
                }),
            }
        }
        self.world.sort_by(|a, b| {
            b.tabs
                .cmp(&a.tabs)
                .then(b.count.cmp(&a.count))
                .then(a.lineage.cmp(&b.lineage))
        });
    }

    /// A binary message from the relay itself.
    pub fn relay(&mut self, now: f64, bytes: &[u8]) {
        if let Ok(env) = decode(bytes) {
            if let Msg::World { islands, entries } = env.msg {
                self.relay_world = Some((now, islands, entries));
            }
        }
    }

    /// Envelopes from another tab.
    pub fn receive(&mut self, now: f64, bytes: &[u8]) {
        let Ok(env) = decode(bytes) else { return };
        self.net.receive(now as u64, env);
        self.flush(now);
    }

    fn flush(&mut self, now: f64) {
        let (out, events) = self.net.drain();
        self.outbox.extend(
            out.iter()
                .map(|e| (e.to, matches!(e.msg, Msg::Census(_)), encode(e))),
        );
        for e in events {
            match e {
                NetEvent::Arrive { side, traveler } => {
                    let (x, y) = edge_point(side, traveler.offset);
                    self.ripples.push(Ripple {
                        x,
                        y,
                        hue: hue(traveler.lineage),
                        born: now,
                    });
                    self.island.arrive(side, traveler);
                }
                NetEvent::Lost { traveler } => {
                    let h = Some(hue(traveler.lineage));
                    self.say(
                        now,
                        format!("a {} was lost between worlds", traveler.name),
                        h,
                    );
                }
                NetEvent::Opened { side, peer } => {
                    if let Some(p) = self.net.peers.get(&peer) {
                        let text = format!(
                            "a portal opened {} to {} ({})",
                            SIDES[side as usize % 4],
                            p.hello.place,
                            clock(p.hello.tz_min)
                        );
                        self.last_beyond[side as usize] = Some(p.hello.place.clone());
                        self.say(now, text, None);
                    }
                }
                NetEvent::Closed { side, .. } => {
                    let to = self.beyond_name(side);
                    self.say(now, format!("the portal to {to} closed"), None);
                }
                NetEvent::Delivered { .. } => {}
            }
        }
    }

    /// A person's mind, released into the warm spot.
    pub fn release(&mut self, src: &str, name: &str, now: f64) -> Result<(), String> {
        let (x, y) = space::island::spot(self.island.tick);
        let lineage = self.island.release(src, name, &self.author, x, y)?;
        store::add_mine(lineage);
        self.mine.push(lineage);
        self.say(now, format!("you released {name}"), Some(hue(lineage)));
        Ok(())
    }

    /// The tab is closing: keep the richest as fossils, say goodbye.
    pub fn close(&mut self) {
        store::save_fossils(&self.island.fossils(FOSSIL_KEEP));
        self.net.bye();
        let (out, _) = self.net.drain();
        self.outbox
            .extend(out.iter().map(|e| (e.to, false, encode(e))));
    }

    /// Islands this one can hear directly, itself included.
    pub fn heard(&self) -> usize {
        1 + self.net.peers.len()
    }

    /// Islands in the world as far as this tab knows.
    pub fn islands(&self) -> usize {
        self.islands_online
            .map_or(0, |n| n as usize)
            .max(self.heard())
    }
}

/// Where along an edge, in cell coordinates, a crossing happens.
pub fn edge_point(side: u8, offset: u16) -> (f64, f64) {
    let o = offset as f64 + 0.5;
    let (w, h) = (W as f64, space::laws::H as f64);
    match side % 4 {
        0 => (o.min(w - 0.5), 0.0),
        1 => (w, o.min(h - 0.5)),
        2 => (o.min(w - 0.5), h),
        _ => (0.0, o.min(h - 0.5)),
    }
}
