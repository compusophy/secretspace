//! The hall of wizards: who has won the most matches, and knocked out the
//! most wizards, over every match the island has seen. People only (no
//! bots, no guests), each by its soul; kept in the room's snapshot across
//! deploys, the best of them sent to every page.

use engine::wire::{Reader, Writer};

use crate::world::{Event, World};

/// How many are kept, and how many shown.
pub const KEEP: usize = 500;
pub const SHOWN: usize = 10;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    pub soul: u64,
    pub name: String,
    pub wins: u32,
    pub outs: u32,
    pub matches: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hall {
    pub rows: Vec<Row>,
}

impl Hall {
    fn row(&mut self, soul: u64, name: &str) -> &mut Row {
        let k = match self.rows.iter().position(|r| r.soul == soul) {
            Some(k) => k,
            None => {
                self.rows.push(Row {
                    soul,
                    ..Row::default()
                });
                self.rows.len() - 1
            }
        };
        let r = &mut self.rows[k];
        r.name = name.chars().take(24).collect();
        r
    }

    /// What this tick's events mean for the hall; whether it changed.
    pub fn heed(&mut self, w: &World, ev: &[Event]) -> bool {
        let person = |id: u16| {
            w.players
                .iter()
                .find(|p| p.id == id && !p.bot && p.soul != 0)
                .map(|p| (p.soul, p.name.clone()))
        };
        let mut changed = false;
        for e in ev {
            match *e {
                Event::Begin => {
                    for p in w
                        .players
                        .iter()
                        .filter(|p| p.entrant && !p.bot && p.soul != 0)
                    {
                        self.row(p.soul, &p.name).matches += 1;
                        changed = true;
                    }
                }
                Event::Win { who } => {
                    if let Some((soul, name)) = person(who) {
                        self.row(soul, &name).wins += 1;
                        changed = true;
                    }
                }
                Event::Out { who, by, .. } if by != 0 && by != who => {
                    if let Some((soul, name)) = person(by) {
                        self.row(soul, &name).outs += 1;
                        changed = true;
                    }
                }
                _ => {}
            }
        }
        if changed {
            self.rows
                .sort_by(|a, b| (b.wins, b.outs, a.matches).cmp(&(a.wins, a.outs, b.matches)));
            self.rows.truncate(KEEP);
        }
        changed
    }

    /// The best: (name, wins, knockouts), those with any.
    pub fn best(&self) -> Vec<(String, u32, u32)> {
        self.rows
            .iter()
            .filter(|r| r.wins + r.outs > 0)
            .take(SHOWN)
            .map(|r| (r.name.clone(), r.wins, r.outs))
            .collect()
    }

    pub fn save(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.u16(self.rows.len() as u16);
        for r in &self.rows {
            w.u64(r.soul)
                .str(&r.name)
                .u32(r.wins)
                .u32(r.outs)
                .u32(r.matches);
        }
        w.0
    }

    pub fn load(b: &[u8]) -> Option<Hall> {
        let mut r = Reader::new(b);
        let n = r.u16()? as usize;
        if n > KEEP {
            return None;
        }
        let mut rows = Vec::with_capacity(n);
        for _ in 0..n {
            rows.push(Row {
                soul: r.u64()?,
                name: r.str()?,
                wins: r.u32()?,
                outs: r.u32()?,
                matches: r.u32()?,
            });
        }
        Some(Hall { rows })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Phase;

    #[test]
    fn the_hall_counts_people_only_and_keeps() {
        let mut w = World::new(5);
        let a = w.join("ash", 11);
        let guest = w.join("guest", 0);
        while w.phase != Phase::Fight {
            w.step();
        }
        let bot = w.players.iter().find(|p| p.bot).unwrap().id;
        let mut h = Hall::default();
        assert!(h.heed(&w, &[Event::Begin]));
        h.heed(
            &w,
            &[
                Event::Out {
                    who: bot,
                    by: a,
                    place: 9,
                },
                Event::Out {
                    who: a,
                    by: guest,
                    place: 8,
                },
                Event::Out {
                    who: guest,
                    by: bot,
                    place: 7,
                },
                Event::Win { who: a },
            ],
        );
        assert_eq!(h.rows.len(), 1, "a guest and a bot are not kept");
        assert_eq!(h.best(), vec![("ash".to_string(), 1, 1)]);
        assert_eq!(h.rows[0].matches, 1);
        let back = Hall::load(&h.save()).unwrap();
        assert_eq!(back, h);
        assert!(Hall::load(&[0xff, 0xff]).is_none());
        let mut rng = engine::rng::Rng::new(3);
        for len in 0..120 {
            let b: Vec<u8> = (0..len).map(|_| rng.next_u64() as u8).collect();
            let _ = Hall::load(&b);
        }
    }
}
