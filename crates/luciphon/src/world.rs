//! The Vale and everyone in it. `World::step` runs one tick in this order:
//! one Intent per Lumen (fitted to a thumb), their control and motion, the
//! actions they swing (settled in `hits`), hazards, deaths and returns,
//! motes and pickups, Flame, Flow, residents, bodies pushing apart, and the
//! history ring that lag compensation and residents' late eyes read.

use std::collections::{HashMap, VecDeque};

use engine::fixed::{len, Fx};
use engine::rng::Rng;

use crate::bots::Brain;
use crate::combat::{control, Me};
use crate::island::{self, ring, Ring, Sites};
use crate::laws::Laws;
use crate::motion::{Body, Intent, Move, Moved};
use crate::thumb::Thumb;
use crate::tiles::{obj, Tiles};

/// How a Lumen last fell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Cause {
    #[default]
    None,
    /// Into the void.
    Dark,
    Strike,
    Thorns,
    Slam,
    Mote,
}

#[derive(Clone, Debug, Default)]
pub struct Lumen {
    pub id: u16,
    /// 0 for a resident or a guest.
    pub soul: u64,
    pub name: String,
    pub hue: u8,
    /// A resident's mind; None for a person.
    pub bot: Option<Brain>,
    pub me: Me,
    /// Flame in thousandths, and ticks until it regenerates.
    pub flame: i32,
    pub flame_rest: u32,
    pub glim: u32,
    pub wood: u32,
    pub stone: u32,
    pub ghost: u32,
    pub flow: u8,
    pub flow_at: u32,
    /// Lumens it struck or was struck by, until a tick.
    pub engaged: Vec<(u16, u32)>,
    /// Attackers who may not damage it in the Glow, until a tick.
    pub spared: Vec<(u16, u32)>,
    /// The target of its strikes in a row: who, how many, when last.
    pub combo: (u16, u8, u32),
    /// Knocked down in the Glow: ticks left.
    pub down: u32,
    /// In the Underlight: ticks until it returns.
    pub descent: u32,
    pub cause: Cause,
    /// Who hit it last, and when; who it fell to.
    pub last_hit: (u16, u32),
    pub killer: u16,
    /// Ticks of its costly deaths (for Rekindled).
    pub deaths: Vec<u32>,
    pub rekindled_until: u32,
    pub thorn_rest: u32,
    /// Its page's round trip, ms.
    pub rtt: u32,
    /// Intents waiting, by seq; the last one used; how it is being fitted.
    pub queue: VecDeque<(u16, Intent)>,
    pub last: Intent,
    pub thumb: Thumb,
    pub ack: u16,
    pub repeats: u8,
    pub dropped: bool,
    /// v0.2. Sunwheat carried; the claim it owns (0: none yet).
    pub wheat: u32,
    pub claim: u16,
    /// Kindle mode, and the open wick: its tiles and since when.
    pub kindle: bool,
    pub wick: Vec<u16>,
    pub wick_since: u32,
    /// Build mode (a piece), and a placement or removal channelling:
    /// the tile, the piece, ticks left, and whether it removes.
    pub build: Option<u8>,
    pub build_idle: u32,
    pub channel: Option<(u16, u8, u32, bool)>,
    /// Dwelling on a node: which, and when it is struck next.
    pub dwell: Option<(u16, u32)>,
    /// Gathered fractions, thousandths: wood, stone, glim.
    pub frac: [i32; 3],
    /// Resonant strikes in a row on one node.
    pub rings: (u16, u8),
    /// Skill XP: hewing, delving, kindling, tending, valor, wayfaring, voice.
    pub xp: [u32; 7],
    /// Ticks played, all told (Sparks, the claim cap).
    pub played: u32,
    pub sparks_off: bool,
    /// Ticks with no input; ticks left Lingering after the page left.
    pub idle: u32,
    pub linger: u32,
    /// A Rekindle (ticks left), a Recall (ticks left).
    pub rekindle: u32,
    pub recall: u32,
    /// Return at the hearth (else at the Luciphon).
    pub home: bool,
}

impl Lumen {
    /// Wood, stone and Sunwheat together: what weighs.
    pub fn materials(&self) -> u32 {
        self.wood + self.stone + self.wheat
    }

    /// Whether Sparks still guard it (its first half hour).
    pub fn spark(&self, l: &Laws) -> bool {
        l.sparks && self.bot.is_none() && !self.sparks_off && self.played < l.sparks_ticks
    }
}

impl Lumen {
    pub fn alive(&self) -> bool {
        self.descent == 0
    }

    pub fn pos(&self) -> (Fx, Fx) {
        (self.me.body.x, self.me.body.y)
    }

    /// Whether others may touch it at all.
    pub fn solid(&self) -> bool {
        self.alive() && self.ghost == 0 && self.me.body.mv != Move::Fallen
    }

    pub fn engaged_with(&self, id: u16, tick: u32) -> bool {
        self.engaged
            .iter()
            .any(|&(o, until)| o == id && until > tick)
    }
}

/// A thrown mote in flight.
#[derive(Clone, Debug, PartialEq)]
pub struct Mote {
    pub id: u16,
    pub x: Fx,
    pub y: Fx,
    pub vx: Fx,
    pub vy: Fx,
    pub left: Fx,
    pub owner: u16,
    pub damage: i32,
    pub pierce: bool,
    pub hit: Vec<u16>,
}

/// Glim on the ground, anyone's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pickup {
    pub id: u16,
    pub x: Fx,
    pub y: Fx,
    pub glim: u32,
    /// Or materials: wood, stone, Sunwheat.
    pub wood: u32,
    pub stone: u32,
    pub wheat: u32,
}

/// Something worth showing that happened this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Gather {
        id: u16,
        idx: u16,
        resonant: bool,
        out: bool,
    },
    Banked {
        id: u16,
    },
    Loop {
        id: u16,
        n: u16,
    },
    Snuffed {
        id: u16,
    },
    Kindle {
        id: u16,
        on: bool,
    },
    Emote {
        id: u16,
        what: u8,
    },
    Level {
        id: u16,
        skill: u8,
        level: u8,
    },
    Placed {
        id: u16,
        idx: u16,
        piece: u8,
    },
    Removed {
        id: u16,
        idx: u16,
    },
    Refused {
        id: u16,
    },
    Dream {
        id: u16,
    },
    Strike {
        id: u16,
    },
    Hit {
        by: u16,
        on: u16,
        damage: u8,
        big: bool,
    },
    Whiff {
        id: u16,
    },
    Sling {
        id: u16,
        gold: bool,
    },
    Spin {
        id: u16,
    },
    Dash {
        id: u16,
    },
    Kick {
        id: u16,
    },
    Save {
        id: u16,
    },
    Dodge {
        id: u16,
    },
    Thorns {
        id: u16,
    },
    Down {
        id: u16,
    },
    Gutter {
        id: u16,
        by: u16,
    },
    Return {
        id: u16,
    },
}

pub struct World {
    pub laws: Laws,
    pub tick: u32,
    pub seed: u64,
    pub tiles: Tiles,
    pub sites: Sites,
    pub lumens: Vec<Lumen>,
    pub motes: Vec<Mote>,
    pub pickups: Vec<Pickup>,
    pub events: Vec<Event>,
    /// Where every Lumen was, newest first, for the last 12 ticks.
    pub history: VecDeque<Vec<(u16, Fx, Fx)>>,
    /// Techniques and falls seen, by name: the proxy check's count.
    pub tally: HashMap<&'static str, u64>,
    pub rng: Rng,
    next_id: u16,
    /// v0.2: nodes struck since they were whole, crops, claims (hearths,
    /// vaults and lodgings), faded land's outlines, sleepers.
    pub nodes: HashMap<u16, crate::gather::Node>,
    pub crops: HashMap<u16, crate::gather::Crop>,
    pub claims: Vec<crate::land::Claim>,
    pub outlines: HashMap<u16, (u16, u64)>,
    pub dreamers: HashMap<u64, crate::life::Dreamer>,
    /// Tiles changed this tick, for the views.
    pub dirty: Vec<u16>,
    /// Seconds since 1970, as the room last said (for days-long timers).
    pub unix: u64,
}

pub const HISTORY: usize = 12;

impl World {
    pub fn new(laws: Laws, seed: u64) -> World {
        let (tiles, sites) = island::generate(&laws, seed);
        World {
            laws,
            tick: 0,
            seed,
            tiles,
            sites,
            lumens: Vec::new(),
            motes: Vec::new(),
            pickups: Vec::new(),
            events: Vec::new(),
            history: VecDeque::new(),
            tally: HashMap::new(),
            rng: Rng::new(seed ^ 0xc0ffee),
            next_id: 1,
            nodes: HashMap::new(),
            crops: HashMap::new(),
            claims: Vec::new(),
            outlines: HashMap::new(),
            dreamers: HashMap::new(),
            dirty: Vec::new(),
            unix: 0,
        }
    }

    pub fn count(&mut self, what: &'static str) {
        *self.tally.entry(what).or_default() += 1;
    }

    /// A free entity id.
    pub fn new_id(&mut self) -> u16 {
        loop {
            let id = self.next_id;
            self.next_id = self.next_id.wrapping_add(1).max(1);
            let used = self.lumens.iter().any(|l| l.id == id)
                || self.motes.iter().any(|m| m.id == id)
                || self.pickups.iter().any(|p| p.id == id);
            if !used {
                return id;
            }
        }
    }

    pub fn find(&self, id: u16) -> Option<&Lumen> {
        self.lumens.iter().find(|l| l.id == id)
    }

    pub fn index(&self, id: u16) -> Option<usize> {
        self.lumens.iter().position(|l| l.id == id)
    }

    pub fn ring_at(&self, x: Fx, y: Fx) -> Ring {
        ring(&self.laws, x.floor(), y.floor())
    }

    /// A clear spot in the Sanctum, around the Luciphon.
    pub fn spawn_spot(&mut self) -> (Fx, Fx) {
        for _ in 0..32 {
            let h = self.rng.below(65536) as u16;
            let r = Fx::milli(3_000 + self.rng.below(2_500) as i32);
            let (ux, uy) = engine::fixed::unit(h);
            let (x, y) = (ux.mul(r), uy.mul(r));
            let t = self.tiles.under(x, y);
            if !t.void() && !t.solid() {
                return (x, y);
            }
        }
        (Fx::int(4), Fx::HALF)
    }

    /// A new Lumen in the Sanctum, a ghost for a moment. Its id.
    pub fn spawn(&mut self, name: &str, soul: u64, bot: Option<Brain>) -> u16 {
        let id = self.new_id();
        let (x, y) = self.spawn_spot();
        let l = &self.laws;
        let resident = bot.is_some();
        let lumen = Lumen {
            id,
            soul,
            name: engine::who::clean_name(name),
            hue: (soul as u8) ^ (self.rng.below(256) as u8),
            bot,
            me: Me {
                body: Body::at(x, y, l),
                ..Me::default()
            },
            flame: l.flame,
            // A resident keeps a stash; a newcomer starts with nothing.
            glim: if resident { 30 } else { l.join_glim },
            ghost: l.ghost,
            home: true,
            played: if resident { l.sparks_ticks } else { 0 },
            ..Lumen::default()
        };
        self.lumens.push(lumen);
        id
    }

    pub fn remove(&mut self, id: u16) {
        self.lumens.retain(|l| l.id != id);
    }

    /// Queue a Lumen's Intent (a person's, by its seq). At most four wait;
    /// past that the oldest goes.
    pub fn intend(&mut self, id: u16, seq: u16, it: Intent) {
        if let Some(l) = self.lumens.iter_mut().find(|l| l.id == id) {
            l.queue.push_back((seq, it));
            while l.queue.len() > 4 {
                l.queue.pop_front();
                l.dropped = true;
            }
        }
    }

    /// One tick of the world.
    pub fn step(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        self.events.clear();
        self.dirty.clear();
        crate::bots::think(self);

        // Every Lumen: one Intent, fitted to a thumb, then its control.
        let mut swings = Vec::new();
        let mut moves: Vec<(usize, Moved, bool)> = Vec::new();
        let mut acts = Vec::new();
        for i in 0..self.lumens.len() {
            let l = &mut self.lumens[i];
            if !l.alive() {
                continue;
            }
            let raw = match l.queue.pop_front() {
                Some((seq, it)) => {
                    l.ack = seq;
                    l.repeats = 0;
                    it
                }
                None => {
                    l.repeats = l.repeats.saturating_add(1);
                    Intent {
                        verb: crate::motion::Verb::None,
                        ..l.last
                    }
                }
            };
            l.last = raw;
            let (mut it, _) = l.thumb.fit(raw);
            if l.down > 0 || l.linger > 0 {
                it = Intent::default();
            }
            let active = it.throttle > 0 || it.verb != crate::motion::Verb::None;
            l.idle = if active { 0 } else { l.idle.saturating_add(1) };
            if active {
                l.dwell = None;
            }
            if l.bot.is_none() {
                l.played = l.played.saturating_add(1);
            }
            l.me.act.build = l.build.is_some();
            let flying = l.me.body.mv == Move::Stun;
            let recovering = l.me.act.act == crate::combat::Act::Recover;
            let (moved, swing) = control(&mut l.me, &it, l.glim, l.flow, &self.tiles, &self.laws);
            if moved.dashed && recovering {
                *self.tally.entry("dash cancel").or_default() += 1;
            }
            if l.me.body.mv == Move::Teeter && !moved.saved && l.me.body.t == self.laws.teeter_ticks
            {
                *self.tally.entry("teeter").or_default() += 1;
            }
            if let Some(s) = swing {
                swings.push((i, s));
            }
            acts.push((i, it.verb, it.throttle > 0));
            moves.push((i, moved, flying));
        }
        for &(i, m, _) in &moves {
            self.techniques(i, m);
        }
        for (i, s) in swings {
            self.swing(i, s);
        }
        for (i, m, flying) in moves {
            self.hazards(i, m, flying);
        }
        for (i, verb, stick) in acts {
            if i < self.lumens.len() && self.lumens[i].alive() {
                self.acts(i, verb, stick);
                self.kindle(i);
                self.bank(i);
            }
        }
        self.snuff();
        self.dwell();
        if self.tick.is_multiple_of(crate::laws::HZ) {
            self.grow();
            self.tend_land();
        }
        self.motes_and_pickups();
        self.vitals();
        self.sleepers();
        self.push_apart();
        let now: Vec<(u16, Fx, Fx)> = self
            .lumens
            .iter()
            .filter(|l| l.alive())
            .map(|l| (l.id, l.me.body.x, l.me.body.y))
            .collect();
        self.history.push_front(now);
        self.history.truncate(HISTORY);
    }

    /// Where a Lumen was `back` ticks ago (its newest place if not known).
    pub fn was(&self, id: u16, back: usize) -> Option<(Fx, Fx)> {
        let at = |h: &Vec<(u16, Fx, Fx)>| h.iter().find(|e| e.0 == id).map(|e| (e.1, e.2));
        self.history
            .get(back.min(self.history.len().saturating_sub(1)))
            .and_then(at)
            .or_else(|| self.find(id).map(|l| l.pos()))
    }

    /// A technique: Flow rises if it follows another within the window.
    pub fn technique(&mut self, i: usize, what: &'static str) {
        self.count(what);
        let (tick, w) = (self.tick, self.laws.flow_window);
        let l = &mut self.lumens[i];
        if tick.wrapping_sub(l.flow_at) <= w {
            l.flow = (l.flow + 1).min(self.laws.flow_max as u8);
            if l.flow == 3 {
                *self.tally.entry("flow 3").or_default() += 1;
            }
        }
        l.flow_at = tick;
    }

    fn techniques(&mut self, i: usize, m: Moved) {
        let id = self.lumens[i].id;
        if m.sling > 0 {
            self.events.push(Event::Sling {
                id,
                gold: m.sling == 2,
            });
            self.technique(
                i,
                if m.sling == 2 {
                    "gold slingshot"
                } else {
                    "blue slingshot"
                },
            );
        }
        if m.spun {
            self.events.push(Event::Spin { id });
            self.count("spin-out");
        }
        if m.kicked {
            self.events.push(Event::Kick { id });
            self.technique(i, "wall-kick");
        } else if m.dashed {
            self.events.push(Event::Dash { id });
            self.count("dash");
        }
        if m.saved {
            self.events.push(Event::Save { id });
            self.technique(i, "ledge save");
        }
        if m.sling > 0 || m.kicked || m.saved {
            let xp = self.laws.xp_way;
            self.gain(i, 5, xp);
        }
    }

    /// Hazards after the move: walls hit in flight, thorns, the void.
    fn hazards(&mut self, i: usize, m: Moved, flying: bool) {
        let l = &self.laws;
        let (x, y) = self.lumens[i].pos();
        let sanctum = self.ring_at(x, y) == Ring::Sanctum;
        if flying && m.slam > l.slam_speed && !sanctum {
            let over = m.slam.sub(l.slam_speed);
            let dmg = (over.0 as i64 * crate::laws::HZ as i64 * l.slam as i64 / 65536) as i32;
            let by = self.recent_hitter(i);
            self.hurt(i, dmg, by, Cause::Slam);
            self.count("wall slam");
        }
        // Thorns: brambles under or beside the body.
        let lum = &self.lumens[i];
        if lum.thorn_rest == 0 && lum.ghost == 0 && lum.alive() && lum.me.body.mv != Move::Fallen {
            let r = self.laws.body;
            let near = [
                (r, Fx::ZERO),
                (r.neg(), Fx::ZERO),
                (Fx::ZERO, r),
                (Fx::ZERO, r.neg()),
                (Fx::ZERO, Fx::ZERO),
            ]
            .iter()
            .map(|&(dx, dy)| (x.add(dx).floor(), y.add(dy).floor()))
            .find(|&(tx, ty)| {
                let t = self.tiles.get(tx, ty);
                t.obj == obj::BRAMBLE || (t.obj == obj::THORNS && t.owner() != lum.claim)
            });
            if let Some((tx, ty)) = near {
                let (cx, cy) = (Fx::int(tx).add(Fx::HALF), Fx::int(ty).add(Fx::HALF));
                let mut h = engine::fixed::atan2(y.sub(cy), x.sub(cx));
                if x == cx && y == cy {
                    h = self.lumens[i].me.body.facing.wrapping_add(32768);
                }
                let (ux, uy) = engine::fixed::unit(h);
                let bounce = self.laws.thorn_bounce;
                let id = self.lumens[i].id;
                self.lumens[i].thorn_rest = self.laws.thorn_rest;
                self.lumens[i].me.body.launch(
                    ux.mul(bounce),
                    uy.mul(bounce),
                    crate::combat::stun(bounce),
                );
                let by = self.recent_hitter(i);
                self.events.push(Event::Thorns { id });
                self.hurt(i, self.laws.thorns, by, Cause::Thorns);
                self.count("thorns");
            }
        }
        if m.fell {
            let by = self.recent_hitter(i);
            self.gutter(i, by, Cause::Dark);
        }
    }

    /// Whoever hit this Lumen in the last 10 s, for credit.
    fn recent_hitter(&self, i: usize) -> u16 {
        let (by, at) = self.lumens[i].last_hit;
        if self.tick.wrapping_sub(at) <= self.laws.engaged {
            by
        } else {
            0
        }
    }

    /// Damage, by ring: the Sanctum takes none, a Lumen down takes none,
    /// the Glow knocks down at 0 Flame, the Dim and the Rim gutter.
    pub fn hurt(&mut self, i: usize, dmg: i32, by: u16, cause: Cause) {
        let tick = self.tick;
        let (x, y) = self.lumens[i].pos();
        let ring = self.ring_at(x, y);
        let l = &mut self.lumens[i];
        if dmg <= 0 || !l.alive() || l.ghost > 0 || l.down > 0 || ring == Ring::Sanctum {
            return;
        }
        if ring == Ring::Glow && l.spared.iter().any(|&(o, until)| o == by && until > tick) {
            return;
        }
        l.flame -= dmg;
        l.flame_rest = self.laws.flame_rest;
        if dmg >= self.laws.interrupt && l.me.act.act == crate::combat::Act::Charge {
            l.me.act = Default::default();
        }
        if l.flame <= 0 {
            match ring {
                Ring::Glow | Ring::Sanctum => {
                    l.flame = 0;
                    l.down = self.laws.down;
                    if by != 0 {
                        l.spared.push((by, tick + self.laws.spared));
                    }
                    let id = l.id;
                    self.events.push(Event::Down { id });
                    self.count("knocked down");
                }
                Ring::Dim | Ring::Rim => self.gutter(i, by, cause),
            }
        }
    }

    /// Gone into the Underlight: what it carried drops by ring (half the
    /// glim taken by the Dark), and it returns in three seconds.
    pub fn gutter(&mut self, i: usize, by: u16, cause: Cause) {
        let tick = self.tick;
        if !self.lumens[i].alive() {
            return;
        }
        self.drop_wick(i);
        let dropped = self.drop_bag(i);
        let id = self.lumens[i].id;
        let lum = &mut self.lumens[i];
        lum.descent = self.laws.descent;
        lum.cause = cause;
        lum.killer = by;
        lum.down = 0;
        lum.rekindle = 0;
        lum.recall = 0;
        lum.channel = None;
        lum.dwell = None;
        lum.me.body.vx = Fx::ZERO;
        lum.me.body.vy = Fx::ZERO;
        if dropped > 0 || cause != Cause::Dark {
            lum.deaths.push(tick);
        }
        self.events.push(Event::Gutter { id, by });
        self.count(match cause {
            Cause::Dark => "fell to the Dark",
            Cause::Thorns => "fell to thorns",
            Cause::Slam => "fell to a wall",
            Cause::Mote => "fell to a mote",
            _ => "fell to a strike",
        });
    }

    /// Flame, ghosts, knockdowns, the Underlight and the return.
    fn vitals(&mut self) {
        let tick = self.tick;
        let l = self.laws.clone();
        for i in 0..self.lumens.len() {
            if self.lumens[i].descent > 0 {
                self.lumens[i].descent -= 1;
                if self.lumens[i].descent == 0 {
                    let (x, y) = self.return_spot(i);
                    let lum = &mut self.lumens[i];
                    lum.me = Me {
                        body: Body::at(x, y, &l),
                        ..Me::default()
                    };
                    lum.me.body.claim = lum.claim;
                    lum.me.body.load = lum.materials();
                    lum.flame = l.flame;
                    lum.ghost = l.ghost;
                    lum.flow = 0;
                    lum.thumb = Thumb::default();
                    if let Some(b) = lum.bot.as_mut() {
                        (b.stick, b.holding, b.next) = (false, false, None);
                    }
                    lum.deaths.retain(|&t| tick.wrapping_sub(t) < 18_000);
                    let stacks = lum.deaths.len().min(3) as i32;
                    if stacks > 0 {
                        lum.rekindled_until = tick + l.rekindled;
                        lum.me.body.regen = 1000 + l.rekindled_breath * stacks;
                    }
                    let id = lum.id;
                    self.events.push(Event::Return { id });
                }
                continue;
            }
            let lum = &mut self.lumens[i];
            lum.ghost = lum.ghost.saturating_sub(1);
            lum.thorn_rest = lum.thorn_rest.saturating_sub(1);
            if lum.rekindled_until != 0 && tick >= lum.rekindled_until {
                lum.rekindled_until = 0;
                lum.me.body.regen = 1000;
            }
            if lum.down > 0 {
                lum.down -= 1;
                if lum.down == 0 {
                    lum.flame = l.stand_flame;
                    lum.ghost = l.stand_ghost;
                }
            } else if lum.flame_rest > 0 {
                lum.flame_rest -= 1;
            } else {
                lum.flame = (lum.flame + l.flame_regen).min(l.flame);
            }
            if lum.flow > 0 && tick.wrapping_sub(lum.flow_at) > l.flow_window {
                lum.flow -= 1;
                lum.flow_at = tick;
            }
            lum.engaged.retain(|e| e.1 > tick);
            lum.spared.retain(|e| e.1 > tick);
        }
    }

    /// Bodies push each other apart softly (not predicted).
    fn push_apart(&mut self) {
        let d = self.laws.body.mul_int(2);
        let n = self.lumens.len();
        for i in 0..n {
            for j in i + 1..n {
                let (a, b) = (&self.lumens[i], &self.lumens[j]);
                if !a.solid() || !b.solid() {
                    continue;
                }
                let (dx, dy) = (b.me.body.x.sub(a.me.body.x), b.me.body.y.sub(a.me.body.y));
                if dx.abs() >= d || dy.abs() >= d {
                    continue;
                }
                let dist = len(dx, dy);
                if dist >= d {
                    continue;
                }
                let push = d.sub(dist).div_int(4);
                let (ux, uy) = if dist.0 == 0 {
                    (Fx::ONE, Fx::ZERO)
                } else {
                    (dx.div(dist), dy.div(dist))
                };
                let (px, py) = (ux.mul(push), uy.mul(push));
                for (k, s) in [(i, -1), (j, 1)] {
                    let body = &mut self.lumens[k].me.body;
                    let (nx, ny) = (body.x.add(px.mul_int(s)), body.y.add(py.mul_int(s)));
                    let t = self.tiles.under(nx, ny);
                    if !t.solid() && !t.void() {
                        body.x = nx;
                        body.y = ny;
                    }
                }
            }
        }
    }
}
