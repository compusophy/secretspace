//! The world: wizards, bolts in flight, and the match. A match goes
//! lobby (anyone here warms up, unhurt) → the fight (everyone drops from
//! the sky, bots fill the island to MATCH_SIZE, the storm closes) → over
//! (the last one standing is shown) → lobby again. Whoever comes during
//! a fight watches until the next. Moving (inputs, the wand, bolts in
//! flight) is in `moving`, spells in `spells`, spell cubes and levels in
//! `loot`.

use std::collections::VecDeque;

use engine::rng::Rng;

use crate::bots::Mind;
use crate::laws::*;
use crate::loot::{self, Scroll};
use crate::map::Map;
use crate::motion::{Body, Input};
use crate::practice::{self, Practice};
use crate::spells::{self, Zone};
use crate::storm::{self, Storm};

mod moving;
#[cfg(test)]
mod tests;

pub use moving::through;

/// A spell in a slot, and its rank (1 to MAX_RANK).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub spell: u8,
    pub rank: u8,
}

/// What a bolt is: the wand's, or a spell's.
pub const WAND: u8 = 200;
/// What hurt someone, when it was the storm.
pub const STORM: u8 = 255;

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
    /// How many ticks behind its page drew everyone else when it last
    /// cast (0: as they are now).
    pub behind: u32,
    /// Ticks since an input came (a stalled page still falls).
    pub idle: u32,
    /// Inputs it may apply now: one more each tick, banked up to
    /// `INPUT_BANK` (never more inputs than ticks, give or take the bank;
    /// a stalled page's stand-in steps are free).
    pub credit: u32,
    /// When it was last hurt (by anything); by whom it last was, and
    /// when (the storm or a quit then knocks it out to their credit).
    pub hurt_at: u32,
    pub hurt_by: u16,
    pub hurt_by_at: u32,
    /// Ticks spent outside the storm since it last burned.
    pub burning: u32,
    pub mind: Mind,
    /// 1 to MAX_LEVEL, and XP toward the next; damage dealt not yet worth
    /// an XP.
    pub level: u8,
    pub xp: u32,
    pub dealt: i32,
    /// The spellbook: each spell's rank, 0 for one not known.
    pub book: [u8; SPELLS.len()],
    /// Two offensive slots, then two utility; ticks until each is ready
    /// (its spell's cooldown, or the wait of one just put in).
    pub slots: [Option<Slot>; 4],
    pub cds: [u32; 4],
    /// Ticks until each spell is ready, slotted or not: a spell keeps its
    /// cooldown from slot to slot.
    pub spell_cds: [u32; SPELLS.len()],
    /// A ward's shield, until when; healing still to come, until when.
    pub shield: i32,
    pub shield_until: u32,
    pub mend: i32,
    pub mend_until: u32,
}

impl Player {
    pub fn eye(&self) -> [f32; 3] {
        let b = &self.body;
        [b.p[0], b.p[1] + b.eye(), b.p[2]]
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
    /// Its damage.
    pub power: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// `what` hurt: `WAND`, a spell, or `STORM`.
    Hit {
        by: u16,
        to: u16,
        amount: u16,
        what: u8,
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
    /// A spell: cast (stage 0, at the wand), landing (1), or marking
    /// (2: Lightning's warning; a Ward breaking).
    Cast {
        by: u16,
        spell: u8,
        stage: u8,
        at: [f32; 3],
    },
    /// A beam of light from one point to another (the Lance).
    Beam {
        by: u16,
        spell: u8,
        from: [f32; 3],
        to: [f32; 3],
    },
    Level {
        who: u16,
        level: u8,
    },
}

/// Where a wizard stood at the end of a tick: who, its feet, how tall.
pub type Stood = (u16, [f32; 3], f32);

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
    pub scrolls: Vec<Scroll>,
    pub zones: Vec<Zone>,
    /// Where everyone stood at the end of each of the last few ticks:
    /// (tick, [(who, feet, how tall)]), for what a page saw (`REWIND`).
    pub(crate) past: VecDeque<(u32, Vec<Stood>)>,
    pub winner: u16,
    pub matches: u32,
    /// The hour of the island's day (`HOURS`) and its weather (0 clear,
    /// 1 mist, 2 rain): its look, nothing else.
    pub hour: u8,
    pub weather: u8,
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
            until: LOBBY_SECS * TICK_HZ,
            began: 0,
            storm: Storm::default(),
            players: Vec::new(),
            bolts: Vec::new(),
            scrolls: Vec::new(),
            zones: Vec::new(),
            past: VecDeque::new(),
            winner: 0,
            matches: 0,
            hour: (seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 56) as u8 % HOURS,
            weather: 0,
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
            behind: 0,
            idle: 0,
            credit: 0,
            hurt_at: 0,
            hurt_by: 0,
            hurt_by_at: 0,
            burning: 0,
            mind: Mind::default(),
            level: 1,
            xp: 0,
            dealt: 0,
            slots: [None; 4],
            cds: [0; 4],
            spell_cds: [0; SPELLS.len()],
            shield: 0,
            shield_until: 0,
            mend: 0,
            mend_until: 0,
            book: [0; SPELLS.len()],
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

    /// A person leaves: out of the match if they were in it, to the
    /// credit of whoever hurt them lately (no one: gone, said nothing of).
    pub fn leave(&mut self, id: u16, ev: &mut Vec<Event>) {
        if let Some(p) = self.players.iter().find(|p| p.id == id) {
            if self.phase == Phase::Fight && p.entrant && p.alive {
                let by = self.credit(id);
                let mut said = Vec::new();
                self.out(id, by, &mut said);
                if by != 0 {
                    ev.append(&mut said);
                }
            }
        }
        self.players.retain(|p| p.id != id);
        self.roster_dirty = true;
    }

    /// Whoever hurt `id` last, if lately (0: no one): the knockout is
    /// theirs when the storm finishes it, or it leaves.
    fn credit(&self, id: u16) -> u16 {
        let Some(p) = self.find(id) else {
            return 0;
        };
        let lately = self.tick.saturating_sub(p.hurt_by_at) < KILL_CREDIT_SECS * TICK_HZ;
        if lately && p.hurt_by != id && self.find(p.hurt_by).is_some() {
            p.hurt_by
        } else {
            0
        }
    }

    /// The spellbook: put a spell `id` knows in one of its slots.
    pub fn equip(&mut self, id: u16, slot: usize, spell: u8) -> bool {
        let lobby = self.phase == Phase::Lobby;
        self.find_mut(id)
            .filter(|p| p.alive || lobby)
            .is_some_and(|p| loot::equip(p, slot, spell))
    }

    /// A page's inputs, to apply one a tick. A flood is cut short.
    pub fn input(&mut self, id: u16, i: Input) {
        if let Some(p) = self.players.iter_mut().find(|p| p.id == id && !p.bot) {
            if p.queue.len() < INPUT_QUEUE {
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
        // The island holds MATCH_SIZE: the first here go; the rest watch
        // this one, and go first in the next.
        let humans = self.players.len().min(MATCH_SIZE);
        let mut watching: Vec<Player> = self.players.drain(humans..).collect();
        for k in 0..MATCH_SIZE - humans {
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
            // Everyone drops with a spell to hurt with; the rest is loot.
            let first = spell::OFFENSE[(self.rng.next_u64() % 4) as usize];
            loot::learn(p, first, 1);
        }
        for p in &mut watching {
            p.alive = false;
            p.entrant = false;
        }
        self.players.splice(0..0, watching);
        self.storm = Storm::plan(&self.map, &mut self.rng);
        self.bolts.clear();
        self.zones.clear();
        loot::scatter(self);
        self.phase = Phase::Fight;
        self.began = self.tick;
        self.until = 0;
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
        self.scrolls.clear();
        self.loot_dirty = true;
        self.phase = Phase::Lobby;
        self.until = self.tick + LOBBY_SECS * TICK_HZ;
        if self.practice.is_none() {
            self.hour = (self.hour + 1) % HOURS;
            // Its own dice (the world's are left as they were).
            let h = (self.map.seed ^ (self.matches as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15))
                .wrapping_mul(0xbf58_476d_1ce4_e5b9);
            self.weather = match (h >> 33) % 100 {
                r if r < RAIN_ODDS => 2,
                r if r < RAIN_ODDS + MIST_ODDS => 1,
                _ => 0,
            };
        }
        self.roster_dirty = true;
        ev.push(Event::Lobby);
    }

    fn out(&mut self, who: u16, by: u16, ev: &mut Vec<Event>) {
        if self.practice.is_some() {
            return practice::fallen(self, who, by, ev);
        }
        let place = self.alive() as u16;
        let (mut level, mut xp) = (1, 0);
        if let Some(k) = self.players.iter().position(|p| p.id == who) {
            level = self.players[k].level;
            // All it learned, its levels' too, go to the one who felled it.
            xp = (level as u32 - 1) * XP_PER_LEVEL + self.players[k].xp;
            loot::drop_spells(self, k);
            let p = &mut self.players[k];
            p.alive = false;
            p.place = place;
            p.hp = 0;
            // The last two fell together: the last to fall won.
            if place == 1 {
                self.winner = who;
            }
        }
        let killer = self.find_mut(by).map(|k| {
            k.kills += 1;
            k.level
        });
        if let Some(mine) = killer {
            let more = level.saturating_sub(mine) as u32;
            let share = xp * XP_SHARE / 100;
            loot::gain(self, by, XP_KNOCKOUT + more * XP_KNOCKOUT_LEVEL + share, ev);
        }
        ev.push(Event::Out { who, by, place });
    }

    /// `by` (0: the storm) hurts `to` with `what`: a ward takes it first.
    pub(crate) fn hurt(&mut self, by: u16, to: u16, amount: i32, what: u8, ev: &mut Vec<Event>) {
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
        let taken = amount.min(p.hp.max(0) + p.shield);
        let soaked = amount.min(p.shield);
        let broke = soaked > 0 && soaked == p.shield;
        p.shield -= soaked;
        p.hp -= amount - soaked;
        p.hurt_at = tick;
        if by != 0 && by != to {
            (p.hurt_by, p.hurt_by_at) = (by, tick);
        }
        let dead = p.hp <= 0;
        if broke {
            ev.push(Event::Cast {
                by: to,
                spell: spell::WARD,
                stage: 2,
                at: [p.body.p[0], p.body.p[1] + 1.0, p.body.p[2]],
            });
        }
        if by != 0 {
            ev.push(Event::Hit {
                by,
                to,
                amount: amount as u16,
                what,
            });
            // An XP for every XP_DAMAGE dealt, the rest kept for the next.
            let xp = self.find_mut(by).map_or(0, |k| {
                k.dealt += taken;
                let xp = k.dealt / XP_DAMAGE;
                k.dealt %= XP_DAMAGE;
                xp
            });
            loot::gain(self, by, xp as u32, ev);
        }
        if dead {
            let by = if by == 0 { self.credit(to) } else { by };
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
            // Matches always run: bots fight while no one is here, so there
            // is always one to watch (the hub's card shows it).
            Phase::Lobby if self.until != 0 && self.tick >= self.until => self.begin(&mut ev),
            Phase::Fight => {
                let people = || self.players.iter().filter(|p| !p.bot);
                let fighting = people().any(|p| p.entrant && p.alive);
                let fallen = people().any(|p| p.entrant && !p.alive);
                let late = people().any(|p| !p.entrant);
                if self.alive() <= 1 {
                    // The last one standing; or, the last two fallen
                    // together, the last to fall (if still here).
                    let standing = self.players.iter().find(|p| p.entrant && p.alive);
                    self.winner = match standing {
                        Some(p) => p.id,
                        None if self.find(self.winner).is_some() => self.winner,
                        None => 0,
                    };
                    if let Some(w) = self.players.iter_mut().find(|p| p.id == self.winner) {
                        w.place = 1;
                    }
                    ev.push(Event::Win { who: self.winner });
                    self.phase = Phase::Over;
                    self.until = self.tick + OVER_SECS * TICK_HZ;
                } else if !fighting && late && !fallen {
                    // Someone arrived to a match of bots alone: not made
                    // to wait it out, a new one gathers for them.
                    self.lobby(&mut ev);
                } else if !fighting && fallen {
                    // Everyone here is out: a moment to see how they
                    // fell, then a new match gathers for them (the bots'
                    // does not go on without them).
                    if self.until == 0 {
                        self.until = self.tick + OUT_LINGER_SECS * TICK_HZ;
                    } else if self.tick >= self.until {
                        self.lobby(&mut ev);
                    }
                }
            }
            Phase::Over if self.tick >= self.until => self.lobby(&mut ev),
            _ => {}
        }
        self.minds();
        let casts = self.move_all();
        for (k, slot, aim) in casts {
            spells::cast(self, k, slot, aim, &mut ev);
        }
        self.fly(&mut ev);
        spells::tick(self, &mut ev);
        loot::touch(self, &mut ev);
        self.storm_and_regen(&mut ev);
        // Where everyone stands now, for the pages that will see it.
        let now: Vec<_> = self
            .players
            .iter()
            .filter(|p| p.alive)
            .map(|p| (p.id, p.body.p, p.body.tall()))
            .collect();
        self.past.push_back((self.tick, now));
        while self.past.len() > REWIND as usize + 2 {
            self.past.pop_front();
        }
        ev
    }

    /// The storm burns whoever is outside it, for each second spent
    /// there (counted a tick at a time, so a step back in on the beat
    /// does not dodge it); the long unhurt heal, a second at a time.
    fn storm_and_regen(&mut self, ev: &mut Vec<Event>) {
        let storm = self.storm_now();
        let fight = self.phase == Phase::Fight;
        let (tick, second) = (self.tick, self.tick.is_multiple_of(TICK_HZ));
        let mut burned = Vec::new();
        for p in self.players.iter_mut().filter(|p| p.alive) {
            if fight && p.entrant && storm.outside(p.body.p[0], p.body.p[2]) {
                p.burning += 1;
                if p.burning >= TICK_HZ {
                    p.burning = 0;
                    burned.push(p.id);
                }
            } else if second && tick - p.hurt_at >= REGEN_AFTER * TICK_HZ {
                let max = p.max_hp();
                p.hp = (p.hp + REGEN * max / HEALTH).min(max);
            }
        }
        // Its damage is a share of whole health (as is the regen): the
        // last circles burn a wizard of any level.
        for id in burned {
            let max = self.find(id).map_or(HEALTH, |p| p.max_hp());
            self.hurt(0, id, storm.dps * max / HEALTH, STORM, ev);
        }
    }
}

/// A wizard as a match begins (or the lobby does): level 1, no spells.
fn fresh(p: &mut Player) {
    p.level = 1;
    p.xp = 0;
    p.dealt = 0;
    p.book = [0; SPELLS.len()];
    p.slots = [None; 4];
    p.cds = [0; 4];
    p.spell_cds = [0; SPELLS.len()];
    p.hurt_by = 0;
    p.burning = 0;
    p.shield = 0;
    p.mend = 0;
    p.hp = p.max_hp();
    p.body.chill = 0;
}

/// In the lobby, a spell of every slot to practise with (unhurt).
pub(crate) fn warmup(p: &mut Player, rng: &mut Rng) {
    let mut two = |from: &[u8]| {
        let n = from.len();
        let a = (rng.next_u64() % n as u64) as usize;
        let b = (a + 1 + (rng.next_u64() % (n as u64 - 1)) as usize) % n;
        [from[a], from[b]]
    };
    let [a, b] = two(&spell::OFFENSE);
    let [c, d] = two(&spell::UTILITY);
    p.slots = [a, b, c, d].map(|spell| Some(Slot { spell, rank: 1 }));
    p.book = [0; SPELLS.len()];
    for s in [a, b, c, d] {
        p.book[s as usize] = 1;
    }
}
