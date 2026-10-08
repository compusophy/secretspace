//! The world: wizards, bolts in flight, and the match. A match goes
//! lobby (anyone here warms up, unhurt) → the fight (everyone drops from
//! the sky, bots fill the island to MATCH_SIZE, the storm closes) → over
//! (the last one standing is shown) → lobby again. Whoever comes during
//! a fight watches until the next. Spells are in `spells`, chests,
//! scrolls and levels in `loot`.

use std::collections::VecDeque;

use engine::rng::Rng;

use crate::bots::{self, Mind};
use crate::laws::*;
use crate::loot::{self, Chest, Scroll};
use crate::map::Map;
use crate::motion::{self, cast, keys, Body, Input};
use crate::practice::{self, Practice};
use crate::spells::{self, Zone};
use crate::storm::{self, Storm};
use crate::trig;

/// A spell in a slot, and its rank (1 to MAX_RANK).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub spell: u8,
    pub rank: u8,
}

/// What a bolt is: the wand's, or a spell's.
pub const WAND: u8 = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Waiting for people; the fight starts at `World::until`.
    Lobby,
    Fight,
    /// The winner is shown until `World::until`.
    Over,
}

pub struct Player {
    pub id: u16,
    pub name: String,
    pub soul: u64,
    pub bot: bool,
    pub body: Body,
    pub yaw: u16,
    pub pitch: i16,
    pub hp: i32,
    pub alive: bool,
    /// In this match (not watching it).
    pub entrant: bool,
    pub kills: u16,
    /// Where it finished (1 won), once out.
    pub place: u16,
    /// Ticks until the wand is ready.
    pub cool: u32,
    /// The last input applied (its seq is what the page is told).
    pub last: Input,
    pub queue: VecDeque<Input>,
    /// Ticks since an input came (a stalled page still falls).
    pub idle: u32,
    pub hurt_at: u32,
    pub mind: Mind,
    /// 1 to MAX_LEVEL, and XP toward the next.
    pub level: u8,
    pub xp: u32,
    /// Two offensive slots, then two utility; ticks until each is ready.
    pub slots: [Option<Slot>; 4],
    pub cds: [u32; 4],
    /// A ward's shield, until when; healing still to come, until when.
    pub shield: i32,
    pub shield_until: u32,
    pub mend: i32,
    pub mend_until: u32,
    /// Asked to take the scroll underfoot (over one already held).
    pub take: bool,
}

impl Player {
    pub fn eye(&self) -> [f32; 3] {
        [self.body.p[0], self.body.p[1] + EYE, self.body.p[2]]
    }

    pub fn max_hp(&self) -> i32 {
        loot::max_hp(self.level)
    }
}

pub struct Bolt {
    pub id: u16,
    pub by: u16,
    /// `WAND`, or the spell it carries.
    pub kind: u8,
    pub rank: u8,
    pub p: [f32; 3],
    pub v: [f32; 3],
    pub life: u32,
    /// Its damage, and ticks it holds its mark (Root).
    pub power: i32,
    pub hold: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Hit {
        by: u16,
        to: u16,
        amount: u16,
    },
    /// Someone is out: who, by whom (0: the storm), their place.
    Out {
        who: u16,
        by: u16,
        place: u16,
    },
    Win {
        who: u16,
    },
    /// The fight began; the lobby began.
    Begin,
    Lobby,
    /// A spell cast (stage 0) or landing (stage 1), at a point.
    Cast {
        by: u16,
        spell: u8,
        stage: u8,
        at: [f32; 3],
    },
    /// A chain spark leaping from one wizard to another.
    Link {
        from: u16,
        to: u16,
    },
    Level {
        who: u16,
        level: u8,
    },
}

pub struct World {
    pub tick: u32,
    pub map: Map,
    pub phase: Phase,
    /// When the lobby or the result ends (a tick).
    pub until: u32,
    /// When the fight began (a tick).
    pub began: u32,
    pub storm: Storm,
    pub players: Vec<Player>,
    pub bolts: Vec<Bolt>,
    pub chests: Vec<Chest>,
    pub scrolls: Vec<Scroll>,
    pub zones: Vec<Zone>,
    pub winner: u16,
    pub matches: u32,
    /// Names changed: the room sends the roster again; and the loot.
    pub roster_dirty: bool,
    pub loot_dirty: bool,
    /// The practice range (a page's own world): its tools and rules.
    pub practice: Option<Practice>,
    pub(crate) rng: Rng,
    next_id: u16,
    pub(crate) next_bolt: u16,
    pub(crate) next_loot: u16,
}

impl World {
    pub fn new(seed: u64) -> World {
        World {
            tick: 0,
            map: Map::new(seed),
            phase: Phase::Lobby,
            until: 0,
            began: 0,
            storm: Storm::default(),
            players: Vec::new(),
            bolts: Vec::new(),
            chests: Vec::new(),
            scrolls: Vec::new(),
            zones: Vec::new(),
            winner: 0,
            matches: 0,
            roster_dirty: true,
            loot_dirty: true,
            practice: None,
            rng: Rng::new(seed ^ 0xbad5eed),
            next_id: 1,
            next_bolt: 1,
            next_loot: 1,
        }
    }

    pub fn seed(&self) -> u64 {
        self.map.seed
    }

    pub fn find(&self, id: u16) -> Option<&Player> {
        self.players.iter().find(|p| p.id == id)
    }

    fn fresh_id(&mut self) -> u16 {
        loop {
            let id = self.next_id;
            self.next_id = self.next_id.wrapping_add(1).max(1);
            if self.find(id).is_none() {
                return id;
            }
        }
    }

    fn standing(&mut self) -> Body {
        let [x, z] = self.map.spot(&mut self.rng);
        Body {
            p: [x, self.map.height(x, z), z],
            ground: true,
            ..Body::default()
        }
    }

    pub(crate) fn player(&mut self, name: &str, soul: u64, bot: bool) -> Player {
        let id = self.fresh_id();
        let body = self.standing();
        Player {
            id,
            name: name.to_string(),
            soul,
            bot,
            body,
            yaw: (self.rng.next_u64() >> 48) as u16,
            pitch: 0,
            hp: HEALTH,
            alive: false,
            entrant: false,
            kills: 0,
            place: 0,
            cool: 0,
            last: Input::default(),
            queue: VecDeque::new(),
            idle: 0,
            hurt_at: 0,
            mind: Mind::default(),
            level: 1,
            xp: 0,
            slots: [None; 4],
            cds: [0; 4],
            shield: 0,
            shield_until: 0,
            mend: 0,
            mend_until: 0,
            take: false,
        }
    }

    pub fn find_mut(&mut self, id: u16) -> Option<&mut Player> {
        self.players.iter_mut().find(|p| p.id == id)
    }

    /// A person arrives: warming up in the lobby, or watching a fight.
    pub fn join(&mut self, name: &str, soul: u64) -> u16 {
        if self.practice.is_some() {
            return practice::arrive(self, name);
        }
        let mut p = self.player(name, soul, false);
        p.alive = self.phase == Phase::Lobby;
        if p.alive {
            warmup(&mut p, &mut self.rng);
        }
        let id = p.id;
        self.players.push(p);
        self.roster_dirty = true;
        if self.phase == Phase::Lobby && self.until <= self.tick {
            self.until = self.tick + LOBBY_SECS * TICK_HZ;
        }
        id
    }

    /// A person leaves: out of the match if they were in it.
    pub fn leave(&mut self, id: u16, ev: &mut Vec<Event>) {
        if let Some(p) = self.players.iter().find(|p| p.id == id) {
            if self.phase == Phase::Fight && p.entrant && p.alive {
                self.out(id, 0, ev);
            }
        }
        self.players.retain(|p| p.id != id);
        self.roster_dirty = true;
    }

    /// A page's inputs, to apply one a tick. A flood is cut short.
    pub fn input(&mut self, id: u16, i: Input) {
        if let Some(p) = self.players.iter_mut().find(|p| p.id == id && !p.bot) {
            if p.queue.len() < 30 {
                p.queue.push_back(i);
            }
        }
    }

    pub fn humans(&self) -> usize {
        self.players.iter().filter(|p| !p.bot).count()
    }

    pub fn alive(&self) -> usize {
        self.players.iter().filter(|p| p.entrant && p.alive).count()
    }

    pub fn entrants(&self) -> usize {
        self.players.iter().filter(|p| p.entrant).count()
    }

    pub fn storm_now(&self) -> storm::Now {
        match self.phase {
            Phase::Fight if self.practice.is_none() => self.storm.at(self.tick - self.began),
            _ => storm::Now {
                r: STORM_START,
                next: ([0.0, 0.0], STORM_START),
                ..storm::Now::default()
            },
        }
    }

    fn begin(&mut self, ev: &mut Vec<Event>) {
        self.players.retain(|p| !p.bot);
        let humans = self.players.len();
        for k in 0..MATCH_SIZE.saturating_sub(humans) {
            let name = BOT_NAMES[(k + self.matches as usize * 5) % BOT_NAMES.len()];
            let mut b = self.player(name, 0, true);
            b.mind = Mind::new(self.rng.next_u64());
            self.players.push(b);
        }
        let mut drops = Vec::new();
        for _ in 0..self.players.len() {
            drops.push(self.standing());
        }
        for (p, mut body) in self.players.iter_mut().zip(drops) {
            body.p[1] += DROP_HEIGHT;
            body.ground = false;
            body.glide = true;
            p.body = body;
            p.alive = true;
            p.entrant = true;
            p.kills = 0;
            p.place = 0;
            p.cool = 0;
            p.hurt_at = 0;
            fresh(p);
        }
        self.storm = Storm::plan(&self.map, &mut self.rng);
        self.bolts.clear();
        self.zones.clear();
        loot::scatter(self);
        self.phase = Phase::Fight;
        self.began = self.tick;
        self.winner = 0;
        self.matches += 1;
        self.roster_dirty = true;
        ev.push(Event::Begin);
    }

    fn lobby(&mut self, ev: &mut Vec<Event>) {
        self.players.retain(|p| !p.bot);
        let bodies: Vec<Body> = (0..self.players.len()).map(|_| self.standing()).collect();
        for (p, b) in self.players.iter_mut().zip(bodies) {
            p.body = b;
            p.alive = true;
            p.entrant = false;
            fresh(p);
            warmup(p, &mut self.rng);
        }
        self.bolts.clear();
        self.zones.clear();
        self.chests.clear();
        self.scrolls.clear();
        self.loot_dirty = true;
        self.phase = Phase::Lobby;
        self.until = if self.players.is_empty() {
            0
        } else {
            self.tick + LOBBY_SECS * TICK_HZ
        };
        self.roster_dirty = true;
        ev.push(Event::Lobby);
    }

    fn out(&mut self, who: u16, by: u16, ev: &mut Vec<Event>) {
        if self.practice.is_some() {
            return practice::fallen(self, who, by, ev);
        }
        let place = self.alive() as u16;
        let mut level = 1;
        if let Some(k) = self.players.iter().position(|p| p.id == who) {
            level = self.players[k].level;
            loot::drop_spells(self, k);
            let p = &mut self.players[k];
            p.alive = false;
            p.place = place;
            p.hp = 0;
        }
        let killer = self.find_mut(by).map(|k| {
            k.kills += 1;
            k.level
        });
        if let Some(mine) = killer {
            let more = level.saturating_sub(mine) as u32;
            loot::gain(self, by, XP_KNOCKOUT + more * XP_KNOCKOUT_LEVEL, ev);
        }
        ev.push(Event::Out { who, by, place });
    }

    /// `by` (0: the storm) hurts `to`: a ward takes it first.
    pub(crate) fn hurt(&mut self, by: u16, to: u16, amount: i32, ev: &mut Vec<Event>) {
        if self.phase != Phase::Fight || amount <= 0 {
            return;
        }
        // On the practice range you are hurt only when sparring.
        if self.practice.as_ref().is_some_and(|r| !r.sparring)
            && self.find(to).is_some_and(|p| !p.bot)
        {
            return;
        }
        let tick = self.tick;
        let Some(p) = self
            .players
            .iter_mut()
            .find(|p| p.id == to && p.alive && p.entrant)
        else {
            return;
        };
        // What was really taken (not past the last of it) earns XP.
        let dealt = amount.min(p.hp.max(0) + p.shield);
        let soaked = amount.min(p.shield);
        p.shield -= soaked;
        p.hp -= amount - soaked;
        p.hurt_at = tick;
        let dead = p.hp <= 0;
        if by != 0 {
            ev.push(Event::Hit {
                by,
                to,
                amount: amount as u16,
            });
            loot::gain(self, by, (dealt / XP_DAMAGE) as u32, ev);
        }
        if dead {
            self.out(to, by, ev);
        }
    }

    /// One tick of the world.
    pub fn step(&mut self) -> Vec<Event> {
        let mut ev = Vec::new();
        self.tick += 1;
        if self.practice.is_some() {
            practice::step(self, &mut ev);
        }
        match self.phase {
            _ if self.practice.is_some() => {}
            Phase::Lobby if self.until != 0 && self.tick >= self.until => {
                if self.humans() > 0 {
                    self.begin(&mut ev);
                } else {
                    self.until = 0;
                }
            }
            Phase::Fight => {
                if self.players.iter().all(|p| p.bot) {
                    self.lobby(&mut ev);
                } else if self.alive() <= 1 {
                    self.winner = self
                        .players
                        .iter()
                        .find(|p| p.entrant && p.alive)
                        .map_or(0, |p| p.id);
                    if let Some(w) = self.players.iter_mut().find(|p| p.id == self.winner) {
                        w.place = 1;
                    }
                    ev.push(Event::Win { who: self.winner });
                    self.phase = Phase::Over;
                    self.until = self.tick + OVER_SECS * TICK_HZ;
                }
            }
            Phase::Over if self.tick >= self.until => self.lobby(&mut ev),
            _ => {}
        }
        self.minds();
        let casts = self.move_all();
        for (k, slot) in casts {
            spells::cast(self, k, slot, &mut ev);
        }
        self.fly(&mut ev);
        spells::tick(self, &mut ev);
        loot::touch(self, &mut ev);
        self.weather(&mut ev);
        ev
    }

    fn minds(&mut self) {
        let storm = self.storm_now();
        let tick = self.tick;
        for k in 0..self.players.len() {
            if !self.players[k].bot || !self.players[k].alive {
                continue;
            }
            let (i, mind) = bots::think(self, k, &storm, tick);
            let p = &mut self.players[k];
            p.mind = mind;
            p.queue.clear();
            p.queue.push_back(i);
        }
    }

    /// Everyone moves by their inputs; the wand fires. The spells asked
    /// for: (who, slot).
    fn move_all(&mut self) -> Vec<(usize, usize)> {
        let mut shots = Vec::new();
        let mut casts = Vec::new();
        for (k, p) in self.players.iter_mut().enumerate() {
            p.cool = p.cool.saturating_sub(1);
            p.take = false;
            if !p.alive {
                p.queue.clear();
                continue;
            }
            let mut steps = 1;
            if p.queue.len() > 3 {
                steps += p.queue.len() - 3;
            }
            for _ in 0..steps {
                let i = match p.queue.pop_front() {
                    Some(i) => {
                        p.idle = 0;
                        i
                    }
                    None => {
                        p.idle += 1;
                        if p.idle < TICK_HZ / 3 {
                            break;
                        }
                        Input {
                            keys: 0,
                            cast: 0,
                            ..p.last
                        }
                    }
                };
                p.yaw = i.yaw;
                p.pitch = i.pitch.clamp(-16000, 16000);
                motion::step(&mut p.body, &i, &self.map);
                p.last = i;
                p.take |= i.cast & cast::TAKE != 0;
                for (slot, bit) in cast::SLOT.iter().enumerate() {
                    if i.cast & bit != 0 && !casts.contains(&(k, slot)) {
                        casts.push((k, slot));
                    }
                }
                if i.keys & keys::FIRE != 0 && p.cool == 0 && !p.body.glide {
                    p.cool = BOLT_COOLDOWN;
                    let d = trig::look(p.yaw, p.pitch);
                    let e = p.eye();
                    shots.push((
                        p.id,
                        loot::level_scale(p.level, BOLT_DAMAGE),
                        [e[0] + d[0] * 0.5, e[1] + d[1] * 0.5, e[2] + d[2] * 0.5],
                        [d[0] * BOLT_SPEED, d[1] * BOLT_SPEED, d[2] * BOLT_SPEED],
                    ));
                }
            }
        }
        for (by, power, p, v) in shots {
            self.bolt(Bolt {
                id: 0,
                by,
                kind: WAND,
                rank: 1,
                p,
                v,
                life: BOLT_LIFE,
                power,
                hold: 0,
            });
        }
        // Fallen off into the deep (should not happen): back on land.
        for k in 0..self.players.len() {
            if self.players[k].body.p[1] < SEA - 20.0 {
                let b = self.standing();
                self.players[k].body = b;
            }
        }
        casts
    }

    /// Set a bolt flying.
    pub(crate) fn bolt(&mut self, mut b: Bolt) {
        b.id = self.next_bolt;
        self.next_bolt = self.next_bolt.wrapping_add(1).max(1);
        self.bolts.push(b);
    }

    fn fly(&mut self, ev: &mut Vec<Event>) {
        let mut impacts = Vec::new();
        let mut keep = Vec::with_capacity(self.bolts.len());
        let fight = self.phase_fight();
        for mut b in std::mem::take(&mut self.bolts) {
            let a = b.p;
            let e = [a[0] + b.v[0] * DT, a[1] + b.v[1] * DT, a[2] + b.v[2] * DT];
            let mut first = self.map.strikes(a, e).map(|t| (t, 0u16));
            for p in &self.players {
                if p.id == b.by || !p.alive || p.entrant != fight {
                    continue;
                }
                if let Some(t) = through(a, e, p.body.p) {
                    if first.is_none_or(|f| t < f.0) {
                        first = Some((t, p.id));
                    }
                }
            }
            b.life = b.life.saturating_sub(1);
            match first {
                Some((t, who)) => {
                    let at = [
                        a[0] + (e[0] - a[0]) * t,
                        a[1] + (e[1] - a[1]) * t,
                        a[2] + (e[2] - a[2]) * t,
                    ];
                    impacts.push((b, at, who));
                }
                None if b.life > 0 => {
                    b.p = e;
                    keep.push(b);
                }
                // A comet bursts at the end of its flight too.
                None if b.kind == spell::COMET => impacts.push((b, e, 0)),
                None => {}
            }
        }
        self.bolts = keep;
        for (b, at, who) in impacts {
            spells::impact(self, &b, at, who, ev);
        }
    }

    fn phase_fight(&self) -> bool {
        self.phase == Phase::Fight
    }

    fn weather(&mut self, ev: &mut Vec<Event>) {
        if !self.tick.is_multiple_of(TICK_HZ) {
            return;
        }
        let storm = self.storm_now();
        let fight = self.phase == Phase::Fight;
        let mut burned = Vec::new();
        for p in self.players.iter_mut().filter(|p| p.alive) {
            if fight && p.entrant && storm.outside(p.body.p[0], p.body.p[2]) {
                burned.push(p.id);
            } else if self.tick - p.hurt_at >= REGEN_AFTER * TICK_HZ {
                p.hp = (p.hp + REGEN).min(p.max_hp());
            }
        }
        for id in burned {
            self.hurt(0, id, storm.dps, ev);
        }
    }
}

/// A wizard as a match begins (or the lobby does): level 1, no spells.
fn fresh(p: &mut Player) {
    p.level = 1;
    p.xp = 0;
    p.slots = [None; 4];
    p.cds = [0; 4];
    p.shield = 0;
    p.mend = 0;
    p.hp = p.max_hp();
    p.body.haste = 0;
    p.body.root = 0;
}

/// In the lobby, a spell of every slot to practise with (unhurt).
pub(crate) fn warmup(p: &mut Player, rng: &mut Rng) {
    let mut pick = |from: &[u8]| from[(rng.next_u64() % from.len() as u64) as usize];
    let off = [spell::LANCE, spell::COMET, spell::CHAIN, spell::STARFALL];
    let util = [
        spell::ROOT,
        spell::BLINK,
        spell::WARD,
        spell::MEND,
        spell::GUST,
        spell::HASTE,
    ];
    let a = pick(&off);
    let b = loop {
        let b = pick(&off);
        if b != a {
            break b;
        }
    };
    let c = pick(&util);
    let d = loop {
        let d = pick(&util);
        if d != c {
            break d;
        }
    };
    p.slots = [a, b, c, d].map(|spell| Some(Slot { spell, rank: 1 }));
}

/// Where along a bolt's step from `a` to `e` (0..1) it passes through a
/// wizard standing at `feet`, if it does.
pub fn through(a: [f32; 3], e: [f32; 3], feet: [f32; 3]) -> Option<f32> {
    // The wizard as a segment from shin to crown, `RADIUS` thick.
    let (lo, hi) = (feet[1] + RADIUS * 0.5, feet[1] + HEIGHT - RADIUS * 0.5);
    let d = [e[0] - a[0], e[1] - a[1], e[2] - a[2]];
    let reach = RADIUS + BOLT_RADIUS;
    // Closest approach on the ground's plane, then the height there.
    let (fx, fz) = (a[0] - feet[0], a[2] - feet[2]);
    let aa = d[0] * d[0] + d[2] * d[2];
    let t = if aa < 1e-9 {
        0.0
    } else {
        (-(fx * d[0] + fz * d[2]) / aa).clamp(0.0, 1.0)
    };
    let (px, pz) = (fx + d[0] * t, fz + d[2] * t);
    if px * px + pz * pz > reach * reach {
        return None;
    }
    let y = a[1] + d[1] * t;
    if y < lo - reach || y > hi + reach {
        return None;
    }
    Some(t)
}
