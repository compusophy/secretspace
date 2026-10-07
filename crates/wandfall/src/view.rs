//! What a page is told each tick: the match, the storm, its own wizard
//! exactly, every other wizard, and the bolts in flight. The island is
//! small enough to tell everyone everything.

use crate::proto::{flag, BoltSeen, Frame, Own, Seen};
use crate::world::{Phase, World};

pub fn frame(w: &World, you: u16) -> Frame {
    let storm = w.storm_now();
    let secs = match w.phase {
        Phase::Lobby | Phase::Over => {
            if w.until > w.tick {
                (w.until - w.tick).div_ceil(crate::laws::TICK_HZ)
            } else {
                0
            }
        }
        Phase::Fight => storm.secs,
    };
    let own = w.find(you).filter(|p| p.alive).map(|p| Own {
        body: p.body,
        seq: p.last.seq,
        hp: p.hp.clamp(0, 255) as u8,
        kills: p.kills.min(255) as u8,
        cool: p.cool.min(255) as u8,
    });
    Frame {
        tick: w.tick,
        phase: match w.phase {
            Phase::Lobby => 0,
            Phase::Fight => 1,
            Phase::Over => 2,
        },
        secs: secs.min(u16::MAX as u32) as u16,
        alive: w.alive().min(255) as u8,
        entrants: w.entrants().min(255) as u8,
        storm: (storm.centre, storm.r),
        next: storm.next,
        shrinking: storm.shrinking,
        storm_phase: storm.phase as u8,
        winner: w.winner,
        you: own,
        players: w
            .players
            .iter()
            .filter(|p| p.alive)
            .map(|p| Seen {
                id: p.id,
                p: p.body.p,
                yaw: p.yaw,
                pitch: p.pitch,
                hp: p.hp.clamp(0, 255) as u8,
                flags: flag::ALIVE
                    | if p.body.glide { flag::GLIDE } else { 0 }
                    | if p.body.ground { flag::GROUND } else { 0 }
                    | if p.bot { flag::BOT } else { 0 }
                    | if p.entrant { flag::ENTRANT } else { 0 },
            })
            .collect(),
        bolts: w
            .bolts
            .iter()
            .map(|b| BoltSeen {
                id: b.id,
                by: b.by,
                p: b.p,
                v: b.v,
            })
            .collect(),
    }
}
