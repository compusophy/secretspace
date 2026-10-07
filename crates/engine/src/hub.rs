//! The hub's live numbers: who is online, in which game, and how many
//! visits there have ever been. The server sends them to `/ws/hub` once a
//! second.

use crate::wire::{Reader, Writer};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Stats {
    /// Browsers connected anywhere: the hub and every game.
    pub online: u32,
    /// Pages ever opened.
    pub visits: u64,
    /// People in each game, by its id.
    pub games: Vec<(String, u32)>,
}

const STATS: u8 = 1;

impl Stats {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.u8(STATS).u32(self.online).u64(self.visits);
        w.u8(self.games.len().min(255) as u8);
        for (id, n) in self.games.iter().take(255) {
            w.str(id).u32(*n);
        }
        w.0
    }

    pub fn decode(b: &[u8]) -> Option<Stats> {
        let mut r = Reader::new(b);
        if r.u8()? != STATS {
            return None;
        }
        let (online, visits) = (r.u32()?, r.u64()?);
        let n = r.u8()? as usize;
        let mut games = Vec::with_capacity(r.room(n, 5)?);
        for _ in 0..n {
            games.push((r.str()?, r.u32()?));
        }
        Some(Stats {
            online,
            visits,
            games,
        })
    }

    pub fn players(&self, game: &str) -> u32 {
        self.games.iter().find(|g| g.0 == game).map_or(0, |g| g.1)
    }
}
