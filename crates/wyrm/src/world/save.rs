//! The arena kept across a restart: every snake, people's by soul and the
//! bots, but not the food, which grows back. A person's snake comes back
//! frozen and harmless, waiting for them (`HOLD_TICKS`).

use std::collections::VecDeque;

use engine::snap::{sections, Sections};
use engine::wire::{Reader, Writer};

use super::{Snake, World};
use crate::bots::Bot;
use crate::laws::*;

const SNAKES: u16 = 1;
/// More than an arena holds; a file that asks for more is not ours.
const MAX_SNAKES: usize = 1024;
const MAX_POINTS: usize = 4096;

impl World {
    pub fn save(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.u16(self.snakes.len() as u16);
        for s in &self.snakes {
            w.u64(s.soul).str(&s.name).u8(s.hue);
            w.u32(s.mass.to_bits()).u32(s.angle.to_bits()).u32(s.kills);
            match &s.bot {
                Some(b) => w.u8(1).u8(b.nerve),
                None => w.u8(0).u8(0),
            };
            w.u16(s.body.len() as u16);
            for &(x, y) in &s.body {
                w.u32(x.to_bits()).u32(y.to_bits());
            }
        }
        let mut out = Sections::default();
        out.add(SNAKES, &w.0);
        out.finish()
    }

    /// Put a save's snakes in place of this world's own. False, and the
    /// world as it was, if the bytes are not a save this build reads.
    pub fn load(&mut self, bytes: &[u8]) -> bool {
        let Ok(secs) = sections(bytes) else {
            return false;
        };
        let Some(snakes) = secs
            .iter()
            .find(|s| s.0 == SNAKES)
            .and_then(|s| self.read_snakes(s.1))
        else {
            return false;
        };
        self.next_snake = snakes.len() as u16 + 1;
        self.snakes = snakes;
        self.rebuild_bodies();
        self.fit_boxes();
        true
    }

    fn read_snakes(&self, b: &[u8]) -> Option<Vec<Snake>> {
        let mut r = Reader::new(b);
        let n = r.u16()? as usize;
        if n > MAX_SNAKES {
            return None;
        }
        let place = |v: f32| v.is_finite() && v.abs() <= ARENA * 2.0;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let soul = r.u64()?;
            let name = engine::who::clean_name(&r.str()?);
            let hue = r.u8()?;
            let mass = f32::from_bits(r.u32()?);
            let angle = f32::from_bits(r.u32()?);
            let kills = r.u32()?;
            let (is_bot, nerve) = (r.u8()?, r.u8()?);
            let len = r.u16()? as usize;
            if len > MAX_POINTS || r.room(len, 8).is_none() {
                return None;
            }
            let mut body = VecDeque::with_capacity(len);
            for _ in 0..len {
                let (x, y) = (f32::from_bits(r.u32()?), f32::from_bits(r.u32()?));
                if !place(x) || !place(y) {
                    return None;
                }
                body.push_back((x, y));
            }
            if !(mass.is_finite() && mass > 0.0 && angle.is_finite()) || body.len() < 2 {
                return None;
            }
            let bot = (is_bot != 0).then_some(Bot { nerve });
            let held_until = if bot.is_none() {
                self.tick + HOLD_TICKS
            } else {
                0
            };
            let (x, y) = body[0];
            out.push(Snake {
                id: i as u16 + 1,
                soul: if bot.is_none() { soul } else { 0 },
                name,
                hue,
                body,
                angle,
                want: angle,
                boost: false,
                boosting: false,
                mass,
                bot,
                moved: 0,
                kills,
                born: self.tick,
                ghost_until: 0,
                held_until,
                owed: 0.0,
                bbox: (x, y, x, y),
            });
        }
        r.done().then_some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_arena_comes_back_with_people_waiting() {
        let mut w = World::new(5);
        let me = w.spawn("me", None);
        w.snakes.iter_mut().find(|s| s.id == me).unwrap().soul = 77;
        for _ in 0..40 {
            w.step();
        }
        let before: Vec<(u64, u32, usize)> = w
            .snakes
            .iter()
            .map(|s| (s.soul, s.mass.to_bits(), s.body.len()))
            .collect();
        let bytes = w.save();

        let mut back = World::new(9);
        assert!(back.load(&bytes));
        let after: Vec<(u64, u32, usize)> = back
            .snakes
            .iter()
            .map(|s| (s.soul, s.mass.to_bits(), s.body.len()))
            .collect();
        assert_eq!(before, after);

        // Mine waits, frozen and harmless, then is mine again.
        let id = back.snakes.iter().find(|s| s.soul == 77).unwrap().id;
        let head = back.find(id).unwrap().head();
        for _ in 0..20 {
            back.step();
        }
        let s = back.find(id).unwrap();
        assert_eq!(s.head(), head);
        assert!(s.ghost(back.tick));
        assert_eq!(back.claim(77), Some(id));
        back.step();
        assert_ne!(back.find(id).unwrap().head(), head);
        assert_eq!(back.claim(5), None);
    }

    #[test]
    fn a_snake_nobody_comes_back_for_bursts() {
        let mut w = World::new(5);
        let me = w.spawn("me", None);
        w.snakes.iter_mut().find(|s| s.id == me).unwrap().soul = 3;
        let mut back = World::new(6);
        assert!(back.load(&w.save()));
        let gone = (0..HOLD_TICKS + 2).any(|_| back.step().iter().any(|d| d.human));
        assert!(gone);
        assert!(back.snakes.iter().all(|s| s.soul != 3));
    }

    #[test]
    fn hostile_saves_never_panic_or_load() {
        let mut w = World::new(1);
        let good = w.save();
        let mut rng = engine::rng::Rng::new(4);
        for i in 0..2000 {
            let mut bad = good.clone();
            let at = rng.below(bad.len() as u64) as usize;
            bad[at] ^= 1 << (i % 8);
            let _ = w.load(&bad);
            let _ = w.load(&bad[..at]);
        }
        let mut junk = Sections::default();
        junk.add(SNAKES, &[255, 255, 1, 2, 3]);
        assert!(!w.load(&junk.finish()));
    }
}
