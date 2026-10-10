//! Wandfall as a room the server hosts: one island, one match at a time,
//! a frame a tick for every page.
//!
//! A soul's wizard is its own: a page that comes back for it (a
//! reconnect, a reload, another tab) is given the wizard its soul has, and
//! one whose page went mid-fight stands where it was for
//! `RECONNECT_SECS`, waiting. Kept across deploys, in sections (schema 2):
//! the hall of wizards, and the island (its day, its matches, and its
//! seed). People held still on it by a deploy come back to the island they
//! were on; any other boot (nobody there, a crash, a room rebuilt after a
//! panic) makes a new one, so no island is for ever.

use std::collections::HashMap;

use engine::room::{Outbox, Room, Who};
use engine::snap::{self, Sections};
use engine::who::guest_name;
use engine::wire::{Reader, Writer};

use crate::hall::Hall;
use crate::laws::{BOT_NAMES, HOURS, MATCH_SIZE, MAX_PEOPLE, RECONNECT_SECS, TICK_HZ};
use crate::proto::{self, Up, PROTO};
use crate::view;
use crate::world::{Event, Phase, World};

/// The snapshot's sections: the hall, and the island.
const HALL: u16 = 1;
const ISLAND: u16 = 2;

// Every wizard goes on the wire, a byte counting them: the people, and
// the bots that fill a match.
const _: () = assert!(MAX_PEOPLE + MATCH_SIZE <= 255);

pub struct Wandfall {
    world: World,
    /// Each connection's wizard (0: not joined).
    you: HashMap<u32, u16>,
    whos: HashMap<u32, Who>,
    /// Wizards whose page went mid-fight: the tick each is let go at,
    /// unless its soul comes back for it first.
    away: HashMap<u16, u32>,
    /// Events from between ticks (a leaver's knockout), sent with the next.
    pending: Vec<Event>,
    /// The hall of wizards (kept in the room's snapshot).
    hall: Hall,
    /// The server is stopping with people here: the next boot puts them
    /// back on this island.
    resume: bool,
}

impl Wandfall {
    pub fn new(seed: u64) -> Wandfall {
        Wandfall {
            world: World::new(seed),
            you: HashMap::new(),
            whos: HashMap::new(),
            away: HashMap::new(),
            pending: Vec::new(),
            hall: Hall::default(),
            resume: false,
        }
    }

    /// The practice range, for a page to run itself.
    pub fn practice(seed: u64) -> Wandfall {
        let mut r = Wandfall::new(seed);
        crate::practice::setup(&mut r.world);
        r
    }

    /// The world, for the practice range's tools.
    pub fn world(&mut self) -> &mut World {
        &mut self.world
    }

    fn roster(&self) -> Vec<u8> {
        let list: Vec<(u16, bool, String)> = self
            .world
            .players
            .iter()
            .map(|p| (p.id, p.bot, p.name.clone()))
            .collect();
        proto::roster(&list)
    }

    fn welcome(&self, you: u16) -> Vec<u8> {
        proto::welcome(you, self.world.seed(), TICK_HZ as u8)
    }

    /// A page asks to play: its wizard (the one it has, the one its soul
    /// has, or a new one), or none if the island is full.
    fn join(&mut self, conn: u32, soul: u64, name: String, out: &mut Outbox) {
        let you = self.you.get(&conn).copied().unwrap_or(0);
        if you != 0 && self.world.find(you).is_some() {
            out.send(conn, self.welcome(you));
            return;
        }
        if let Some(id) = self.claim(conn, soul, out) {
            out.send(conn, self.welcome(id));
            return;
        }
        if self.world.practice.is_none() && self.world.humans() >= MAX_PEOPLE {
            // Full: it watches.
            out.send(conn, self.welcome(0));
            return;
        }
        let id = self.world.join(&name, soul);
        if name.is_empty() {
            if let Some(p) = self.world.find_mut(id) {
                p.name = guest_name(id);
            }
        }
        self.you.insert(conn, id);
        out.send(conn, self.welcome(id));
    }

    /// The wizard `soul` has here already, if it has one (its page came
    /// back before the old connection was let go, or after, to one
    /// waiting for it; or it opened in another tab): this connection's
    /// now, and no longer any other's, which is told it only watches.
    fn claim(&mut self, conn: u32, soul: u64, out: &mut Outbox) -> Option<u16> {
        if soul == 0 || self.world.practice.is_some() {
            return None;
        }
        let id = self
            .world
            .players
            .iter()
            .find(|p| !p.bot && p.soul == soul)?
            .id;
        let watch = self.welcome(0);
        for (&c, other) in self.you.iter_mut() {
            if c != conn && *other == id {
                *other = 0;
                out.send(c, watch.clone());
            }
        }
        self.you.insert(conn, id);
        self.away.remove(&id);
        Some(id)
    }

    /// A wizard whose page is gone for good: out of the match (to the
    /// credit of whoever brought it low, if anyone), and off the island.
    fn leave(&mut self, id: u16) {
        let mut ev = Vec::new();
        self.world.leave(id, &mut ev);
        // One that left, felled by no one, is not news (the roster says it
        // is gone, and the storm did not take it).
        ev.retain(|e| !matches!(e, Event::Out { by: 0, .. }));
        self.pending.extend(ev);
    }

    /// Wizards whose page did not come back in time, or whose match is
    /// over (a new lobby has begun): gone.
    fn let_go(&mut self) {
        let (tick, lobby) = (self.world.tick, self.world.phase == Phase::Lobby);
        let mut gone: Vec<u16> = self
            .away
            .iter()
            .filter(|&(_, &until)| lobby || tick >= until)
            .map(|(&id, _)| id)
            .collect();
        gone.sort_unstable();
        for id in gone {
            self.away.remove(&id);
            self.leave(id);
        }
    }
}

impl Room for Wandfall {
    fn id(&self) -> &'static str {
        "wandfall"
    }

    fn hz(&self) -> u32 {
        TICK_HZ
    }

    fn open(&mut self, conn: u32, who: &Who, out: &mut Outbox) {
        self.whos.insert(conn, who.clone());
        self.you.insert(conn, 0);
        // Watchers see the island and the match, never a wizard of their own.
        out.send(conn, self.welcome(0));
        out.send(conn, self.roster());
        out.send(conn, view::loot(&self.world).encode());
        out.send(conn, proto::hall(&self.hall.best()));
    }

    fn who(&mut self, conn: u32, who: &Who) {
        if let Some(w) = self.whos.get_mut(&conn) {
            *w = who.clone();
        }
        let id = self.you.get(&conn).copied().unwrap_or(0);
        if let Some(p) = self.world.find_mut(id) {
            if !who.name.is_empty() && p.name != who.name {
                p.name = who.name.clone();
                self.world.roster_dirty = true;
            }
        }
    }

    fn message(&mut self, conn: u32, bytes: &[u8], out: &mut Outbox) {
        let (Some(who), Some(up)) = (self.whos.get(&conn), Up::decode(bytes)) else {
            return;
        };
        if who.watch {
            return;
        }
        let you = self.you.get(&conn).copied().unwrap_or(0);
        match up {
            Up::Join { proto } if proto != PROTO => {
                // An older page: it reloads on a Welcome without a wizard.
                out.send(conn, self.welcome(0));
            }
            Up::Join { .. } => {
                let (soul, name) = (who.soul, who.name.clone());
                self.join(conn, soul, name, out);
            }
            Up::Inputs(list) => {
                for i in list {
                    self.world.input(you, i);
                }
            }
            Up::Equip { slot, spell } => {
                self.world.equip(you, slot as usize, spell);
            }
        }
    }

    fn close(&mut self, conn: u32) {
        self.whos.remove(&conn);
        let Some(id) = self.you.remove(&conn).filter(|&id| id != 0) else {
            return;
        };
        // Mid-fight, a soul's page that drops (a phone between networks, a
        // reload) has a while to come back to its wizard. A guest's
        // cannot, so it does not wait.
        let waits = self.world.phase == Phase::Fight
            && self.world.practice.is_none()
            && self
                .world
                .find(id)
                .is_some_and(|p| p.soul != 0 && p.entrant);
        if waits {
            self.away
                .insert(id, self.world.tick + RECONNECT_SECS * TICK_HZ);
        } else {
            self.leave(id);
        }
    }

    fn tick(&mut self, out: &mut Outbox) {
        self.let_go();
        let mut ev = std::mem::take(&mut self.pending);
        ev.extend(self.world.step());
        // What every page is told: the events first (a leaver's name is
        // still in the roster its page has), then whatever changed.
        let mut all = Vec::new();
        if !ev.is_empty() {
            all.push(proto::events(&ev));
        }
        if self.world.practice.is_none() && self.hall.heed(&self.world, &ev) {
            all.push(proto::hall(&self.hall.best()));
        }
        if std::mem::take(&mut self.world.roster_dirty) {
            all.push(self.roster());
        }
        if std::mem::take(&mut self.world.loot_dirty) {
            all.push(view::loot(&self.world).encode());
        }
        for &conn in self.you.keys() {
            for m in &all {
                out.send(conn, m.clone());
            }
        }
        // A frame for each page with a wizard alive; every other (watching,
        // knocked out, past the island's people, or never joined) is sent
        // the same one, made once, so pages that never play cost a copy.
        let mut watching = None;
        for (&conn, &id) in &self.you {
            let frame = match view::yours(&self.world, id) {
                Some(_) => view::frame(&self.world, id).encode(),
                None => watching
                    .get_or_insert_with(|| view::frame(&self.world, 0).encode())
                    .clone(),
            };
            out.send(conn, frame);
        }
    }

    fn still(&mut self, _out: &mut Outbox) {
        self.resume = self.people() > 0;
    }

    fn save(&self) -> Option<Vec<u8>> {
        let w = &self.world;
        let mut island = Writer::default();
        island
            .u64(w.seed())
            .u32(w.matches)
            .u8(w.hour)
            .u8(self.resume as u8);
        let mut s = Sections::default();
        s.add(HALL, &self.hall.save()).add(ISLAND, &island.0);
        Some(s.finish())
    }

    fn load(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        self.load_snap(self.schema(), bytes)
    }

    fn load_snap(&mut self, schema: u16, bytes: &[u8]) -> Result<(), &'static str> {
        // Schema 1 was the hall alone.
        if schema < 2 {
            self.hall = Hall::load(bytes).ok_or("not a hall of wizards")?;
            return Ok(());
        }
        // A later build's sections are passed over: not ours to read.
        for (tag, b) in snap::sections(bytes)? {
            match tag {
                HALL => self.hall = Hall::load(b).ok_or("not a hall of wizards")?,
                ISLAND => {
                    let mut r = Reader::new(b);
                    let short = "a short island";
                    let seed = r.u64().ok_or(short)?;
                    let (matches, hour) = (r.u32().ok_or(short)?, r.u8().ok_or(short)?);
                    let resume = r.u8().ok_or(short)? != 0;
                    // The island people were held still on (nobody is on
                    // it yet), its dice this boot's; else this boot's
                    // own. Its day and its count go on either way.
                    if resume {
                        let fresh = std::mem::replace(&mut self.world, World::new(seed));
                        self.world.rng = fresh.rng;
                    }
                    self.world.matches = matches;
                    self.world.hour = hour % HOURS;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn schema(&self) -> u16 {
        2
    }

    fn people(&self) -> usize {
        self.whos.values().filter(|w| !w.watch).count()
    }

    fn playing(&self) -> usize {
        self.world.players.len()
    }

    fn reserved(&self) -> &'static [&'static str] {
        BOT_NAMES
    }

    fn stats(&self) -> Vec<(&'static str, i64)> {
        let w = &self.world;
        vec![
            ("people", self.people() as i64),
            ("bots", w.players.iter().filter(|p| p.bot).count() as i64),
            ("alive", w.alive() as i64),
            ("away", self.away.len() as i64),
            ("fighting", (w.phase == Phase::Fight) as i64),
            ("matches", w.matches as i64),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::Input;

    fn person(soul: u64, name: &str) -> Who {
        Who {
            soul,
            name: name.into(),
            watch: false,
            build: 0,
        }
    }

    /// A room, and a page `conn` in it as `who`, joined.
    fn joined(r: &mut Wandfall, conn: u32, who: &Who) -> u16 {
        let mut out = Outbox::default();
        r.open(conn, who, &mut out);
        r.message(conn, &Up::Join { proto: PROTO }.encode(), &mut out);
        welcomed(&out, conn).expect("welcomed")
    }

    /// The wizard the last Welcome to `conn` gave it.
    fn welcomed(out: &Outbox, conn: u32) -> Option<u16> {
        out.0
            .iter()
            .rev()
            .filter(|(c, _)| *c == conn)
            .find_map(|(_, b)| proto::read_welcome(b).map(|w| w.1))
    }

    fn ticks(r: &mut Wandfall, n: u32) -> Outbox {
        let mut out = Outbox::default();
        for _ in 0..n {
            r.tick(&mut out);
        }
        out
    }

    fn to_the_fight(r: &mut Wandfall) {
        while r.world.phase != Phase::Fight {
            ticks(r, 1);
        }
    }

    #[test]
    fn a_page_gone_mid_fight_comes_back_to_its_wizard() {
        let mut r = Wandfall::new(4);
        let ash = person(42, "ash");
        let id = joined(&mut r, 1, &ash);
        to_the_fight(&mut r);
        ticks(&mut r, 30);
        // Its connection closes (a phone between networks): the wizard
        // stays, and so does the match it is in.
        r.close(1);
        ticks(&mut r, 5 * TICK_HZ);
        assert!(r.world.find(id).is_some_and(|p| p.alive && p.entrant));
        assert_eq!(r.world.phase, Phase::Fight);
        // Its page comes back as the same soul: the same wizard.
        assert_eq!(joined(&mut r, 2, &ash), id);
        assert!(r.away.is_empty());
        ticks(&mut r, RECONNECT_SECS * TICK_HZ);
        assert!(r.world.find(id).is_some(), "and it is not let go");
        // A guest's page cannot come back for its wizard: it does not wait.
        let guest = joined(&mut r, 3, &person(0, ""));
        r.close(3);
        assert!(r.world.find(guest).is_none());
    }

    #[test]
    fn a_wizard_whose_page_does_not_come_back_is_let_go() {
        let mut r = Wandfall::new(4);
        let id = joined(&mut r, 1, &person(42, "ash"));
        to_the_fight(&mut r);
        r.close(1);
        ticks(&mut r, RECONNECT_SECS * TICK_HZ - 2);
        assert!(r.world.find(id).is_some());
        ticks(&mut r, 3);
        assert!(r.world.find(id).is_none());
        assert!(r.away.is_empty());
    }

    #[test]
    fn a_new_connection_takes_over_before_the_old_one_is_let_go() {
        let mut r = Wandfall::new(4);
        let ash = person(42, "ash");
        let id = joined(&mut r, 1, &ash);
        to_the_fight(&mut r);
        // The old connection has not closed yet (a silent drop takes the
        // server a while to see): the new one has the wizard all the same.
        let mut out = Outbox::default();
        r.open(2, &ash, &mut out);
        r.message(2, &Up::Join { proto: PROTO }.encode(), &mut out);
        assert_eq!(welcomed(&out, 2), Some(id));
        assert_eq!(welcomed(&out, 1), Some(0), "the old one only watches");
        // When it does close, nothing happens to the wizard.
        r.close(1);
        assert!(r.away.is_empty());
        ticks(&mut r, (RECONNECT_SECS + 1) * TICK_HZ);
        assert!(r.world.find(id).is_some());
        // And in the lobby, the same: one soul, one wizard.
        let mut r = Wandfall::new(4);
        let a = joined(&mut r, 1, &ash);
        assert_eq!(joined(&mut r, 2, &ash), a);
        assert_eq!(r.world.humans(), 1);
    }

    #[test]
    fn leaving_in_the_lobby_is_at_once_and_guests_are_told_apart() {
        let mut r = Wandfall::new(4);
        let a = joined(&mut r, 1, &person(42, "ash"));
        r.close(1);
        assert!(r.world.find(a).is_none());
        let (g, h) = (
            joined(&mut r, 2, &Who::default()),
            joined(&mut r, 3, &Who::default()),
        );
        let name = |id| r.world.find(id).map(|p| p.name.clone()).unwrap();
        assert_ne!(name(g), name(h));
        assert!(engine::who::is_guest_name(&name(g)), "{}", name(g));
    }

    #[test]
    fn an_island_that_is_full_lets_more_watch() {
        let mut r = Wandfall::new(4);
        for k in 0..MAX_PEOPLE as u32 {
            assert_ne!(joined(&mut r, k + 1, &person(k as u64 + 1, "")), 0);
        }
        let last = MAX_PEOPLE as u32 + 1;
        assert_eq!(joined(&mut r, last, &person(999, "")), 0);
        assert_eq!(r.world.humans(), MAX_PEOPLE);
        // Room again once someone goes.
        r.close(1);
        r.message(
            last,
            &Up::Join { proto: PROTO }.encode(),
            &mut Outbox::default(),
        );
        assert_eq!(r.world.humans(), MAX_PEOPLE);
    }

    #[test]
    fn events_come_before_the_roster_and_a_name_unchanged_is_not_news() {
        let mut r = Wandfall::new(4);
        let ash = person(42, "ash");
        joined(&mut r, 1, &ash);
        // The fight begins: the Begin, then the roster with its bots.
        let mut out = Outbox::default();
        while r.world.phase != Phase::Fight {
            out = ticks(&mut r, 1);
        }
        let kinds: Vec<u8> = out
            .0
            .iter()
            .filter(|(c, _)| *c == 1)
            .map(|(_, b)| b[0])
            .collect();
        let at = |tag| kinds.iter().position(|&k| k == tag).expect("sent");
        assert!(at(proto::tag::EVENTS) < at(proto::tag::ROSTER), "{kinds:?}");
        // Hello again under the same name: no roster for anyone.
        r.who(1, &ash);
        let out = ticks(&mut r, 1);
        assert!(!out.0.iter().any(|(_, b)| b[0] == proto::tag::ROSTER));
        r.who(1, &person(42, "oak"));
        let out = ticks(&mut r, 1);
        assert!(out.0.iter().any(|(_, b)| b[0] == proto::tag::ROSTER));
    }

    #[test]
    fn the_hall_and_the_island_are_kept_and_old_snapshots_still_read() {
        let mut r = Wandfall::new(77);
        r.hall.rows.push(crate::hall::Row {
            soul: 5,
            name: "ash".into(),
            wins: 2,
            ..Default::default()
        });
        r.world.matches = 9;
        r.world.hour = 3;
        let boot = |saved: &[u8]| {
            let mut back = Wandfall::new(1);
            back.load_snap(2, saved).unwrap();
            back
        };
        // Saved now and then: the hall and the day go on, on a new island.
        let back = boot(&r.save().unwrap());
        assert_eq!(back.hall, r.hall);
        assert_eq!(back.world.seed(), 1, "this boot's own island");
        assert_eq!((back.world.matches, back.world.hour), (9, 3));
        // Held still by a deploy with nobody on it (the hub's card only
        // watches): a new island too.
        let mut out = Outbox::default();
        r.open(1, &Who::guest(true), &mut out);
        r.still(&mut out);
        assert_eq!(boot(&r.save().unwrap()).world.seed(), 1);
        // With people on it: the island they were on.
        r.open(2, &person(42, "ash"), &mut out);
        r.still(&mut out);
        let saved = r.save().unwrap();
        let back = boot(&saved);
        assert_eq!(back.world.seed(), 77, "the island the pages were on");
        assert_eq!((back.world.matches, back.world.hour), (9, 3));
        // Schema 1: the hall alone.
        let mut old = Wandfall::new(1);
        old.load_snap(1, &r.hall.save()).unwrap();
        assert_eq!((old.hall.clone(), old.world.seed()), (r.hall.clone(), 1));
        // A later build's sections are passed over; damage is not.
        let mut s = Sections::default();
        s.add(HALL, &r.hall.save()).add(99, b"what is this");
        let mut later = Wandfall::new(1);
        later.load_snap(3, &s.finish()).unwrap();
        assert_eq!(later.hall, r.hall);
        let mut bad = saved.clone();
        bad[8] ^= 1;
        assert!(Wandfall::new(1).load_snap(2, &bad).is_err());
    }

    #[test]
    fn pages_without_a_wizard_share_one_frame_and_players_have_their_own() {
        let mut r = Wandfall::new(4);
        let mut out = Outbox::default();
        // Two watchers, a page that never joins, and two players.
        r.open(1, &Who::guest(true), &mut out);
        r.open(2, &Who::guest(true), &mut out);
        r.open(3, &person(7, "idle"), &mut out);
        let a = joined(&mut r, 4, &person(42, "ash"));
        joined(&mut r, 5, &person(43, "oak"));
        let frames = |r: &mut Wandfall| {
            let out = ticks(r, 1);
            let mut f: Vec<(u32, Vec<u8>)> = out
                .0
                .into_iter()
                .filter(|(_, b)| proto::Frame::decode(b).is_some())
                .collect();
            f.sort();
            f
        };
        let f = frames(&mut r);
        assert_eq!(f.len(), 5, "a frame for every page");
        assert!(f[0].1 == f[1].1 && f[1].1 == f[2].1);
        assert_eq!(f[0].1, view::frame(&r.world, 0).encode());
        assert!(f[3].1 != f[0].1 && f[4].1 != f[0].1 && f[3].1 != f[4].1);
        assert!(proto::Frame::decode(&f[3].1).unwrap().you.is_some());
        // A wizard knocked out watches: the same frame as the watchers.
        r.world.find_mut(a).unwrap().alive = false;
        let f = frames(&mut r);
        assert_eq!(f[3].1, f[0].1);
        assert!(proto::Frame::decode(&f[3].1).unwrap().you.is_none());
    }

    #[test]
    fn pages_that_send_anything_well_formed_break_nothing() {
        let mut r = Wandfall::new(6);
        let mut rng = engine::rng::Rng::new(11);
        let mut roll = |n: u64| rng.next_u64() % n;
        let mut out = Outbox::default();
        let pages = 6u32;
        for c in 1..=pages {
            r.open(c, &person(c as u64, "p"), &mut out);
        }
        for t in 0..4000u32 {
            for c in 1..=pages {
                let up = match roll(10) {
                    0 => Up::Join {
                        proto: if roll(8) == 0 { 3 } else { PROTO },
                    },
                    1 => Up::Equip {
                        slot: roll(6) as u8,
                        spell: roll(12) as u8,
                    },
                    _ => Up::Inputs(
                        (0..1 + roll(8))
                            .map(|_| Input {
                                seq: roll(65536) as u16,
                                yaw: roll(65536) as u16,
                                pitch: roll(65536) as u16 as i16,
                                keys: roll(65536) as u16,
                                cast: roll(256) as u8,
                                view: roll(65536) as u16,
                            })
                            .collect(),
                    ),
                };
                r.message(c, &up.encode(), &mut out);
            }
            // Now and then a page goes, and comes back.
            if t % 97 == 0 {
                let c = 1 + roll(pages as u64) as u32;
                r.close(c);
                r.open(c, &person(c as u64, "p"), &mut out);
            }
            out.0.clear();
            r.tick(&mut out);
            for p in &r.world.players {
                let ok = p.body.p.iter().all(|x| x.is_finite() && x.abs() < 1000.0);
                assert!(ok, "tick {t}: {} at {:?}", p.id, p.body.p);
            }
        }
        assert!(r.world.matches >= 1, "matches went on");
    }
}
