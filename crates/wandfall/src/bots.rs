//! Bots: wizards that fill each match. In the drop each picks a spot of
//! its own to land on; then they loot a while, keep inside the storm
//! (hopping, timed to each landing, when they have far to go; up a launch
//! rune and gliding when the storm comes and one is on the way), and
//! duel whoever they come across: strafing and hopping, keeping their
//! distance, out from under Lightning, leading each spell as it flies, a
//! little off (more against a wizard who moves). They see as people do
//! (ahead of them, not through hills, trees or pillars; anyone near is
//! heard), take a moment to notice, keep to the one they fight, and turn
//! on whoever hurts them. Hurt, they Ward, Mend, Blink or run off to heal.
//!
//! What they see is in `sense`, where they go in `walk`, how they fight
//! in `fight`.

use engine::rng::splitmix;

use crate::laws::*;
use crate::motion::{keys, Input};
use crate::storm::Now;
use crate::trig;
use crate::world::{Player, World};

mod fight;
mod sense;
#[cfg(test)]
mod tests;
mod walk;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mind {
    pub seed: u64,
    pub target: u16,
    /// When it may start shooting at its target; when it last saw it, and
    /// where (its chest).
    pub ready_at: u32,
    pub seen_at: u32,
    pub last: [f32; 3],
    /// Strafing: which way (−1, 1), until when.
    pub strafe: i8,
    pub strafe_until: u32,
    /// Where it walks when no one is about, until when, and for which of
    /// the storm's phases; whether it has picked where it lands.
    pub goal: [f32; 2],
    pub goal_at: u32,
    pub goal_phase: u8,
    pub landing: bool,
    /// Where it stood a second ago (to know when it is stuck), and when
    /// it last was (0: never); a cube it gave up on, until when.
    pub was: [f32; 3],
    pub stuck: u32,
    pub skip: u16,
    pub skip_until: u32,
    /// Running off to heal, until when.
    pub retreat_until: u32,
    /// A practice dummy: 0 not one, 1 stands, 2 strafes, 3 spars when
    /// sparring is on (and strafes when not); where it stands.
    pub dummy: u8,
    pub home: [f32; 2],
}

impl Mind {
    pub fn new(seed: u64) -> Mind {
        Mind {
            seed,
            strafe: 1,
            ..Mind::default()
        }
    }
}

/// A number in 0..1 from a bot's seed, the tick and what it is for.
fn unit(seed: u64, tick: u32, k: u64) -> f32 {
    (splitmix(seed ^ (tick as u64) << 8 ^ k) >> 40) as f32 / (1u64 << 24) as f32
}

/// How steady a bot's hand is: a share of the usual miss, its own.
fn shake(seed: u64) -> f32 {
    1.0 + BOT_SKILL * (unit(seed, 0, 40) * 2.0 - 1.0)
}

/// The keys that walk toward `heading` while facing `yaw`.
fn keys_toward(yaw: u16, heading: u16) -> u16 {
    let rel = heading.wrapping_sub(yaw);
    [
        keys::FWD,
        keys::FWD | keys::RIGHT,
        keys::RIGHT,
        keys::BACK | keys::RIGHT,
        keys::BACK,
        keys::BACK | keys::LEFT,
        keys::LEFT,
        keys::FWD | keys::LEFT,
    ][((rel as u32 + 4096) / 8192 % 8) as usize]
}

/// `from` turned toward `to`, at most `most` (of 65536 a turn); whether
/// it got there.
fn turn(from: u16, to: u16, most: i32) -> (u16, bool) {
    let diff = to.wrapping_sub(from) as i16 as i32;
    let step = diff.clamp(-most, most);
    (from.wrapping_add(step as i16 as u16), step == diff)
}

/// What a bot does this tick, and what it now has in mind.
pub fn think(w: &World, k: usize, storm: &Now, tick: u32) -> (Input, Mind) {
    let me = &w.players[k];
    let mut m = me.mind;
    if let Some(i) = dummy(w, me, &mut m, tick) {
        return (i, m);
    }
    let mark = sense::mark(w, me, &mut m, tick);
    let plan = walk::plan(w, me, &mut m, storm, mark.as_ref(), tick);
    let mut i = fight::act(w, k, &mut m, &plan, mark.as_ref(), tick);
    walk::unstick(me, &mut m, &plan, &mut i, tick);
    (i, m)
}

/// A practice dummy's tick (None: one that spars, thinking as a bot).
fn dummy(w: &World, me: &Player, m: &mut Mind, tick: u32) -> Option<Input> {
    let sparring = w.practice.as_ref().is_some_and(|p| p.sparring);
    match m.dummy {
        1 => Some(Input {
            yaw: me.yaw,
            ..Input::default()
        }),
        2 | 3 if !(m.dummy == 3 && sparring) => {
            // Side to side about its post: back the other way when a
            // leg is done or it strays too far, a hop now and then.
            let (s, c) = trig::sin_cos(me.yaw);
            let off = (me.body.p[0] - m.home[0]) * -s + (me.body.p[2] - m.home[1]) * c;
            let strayed = off * m.strafe as f32 > DUMMY_LEASH;
            if tick >= m.strafe_until || strayed {
                m.strafe = if m.strafe > 0 { -1 } else { 1 };
                m.strafe_until =
                    tick + DUMMY_STRAFE + (unit(m.seed, tick, 30) * DUMMY_STRAFE_MORE) as u32;
            }
            let side = if m.strafe > 0 {
                keys::RIGHT
            } else {
                keys::LEFT
            };
            let jump = if unit(m.seed, tick, 31) < DUMMY_JUMP_ODDS {
                keys::JUMP
            } else {
                0
            };
            Some(Input {
                yaw: me.yaw,
                keys: side | jump,
                ..Input::default()
            })
        }
        _ => None,
    }
}

/// Whether a cube is worth a bot's detour: a spell it can still learn
/// or rank up.
fn wants(me: &Player, spell: u8) -> bool {
    me.book[spell as usize % SPELLS.len()] < MAX_RANK
}

/// The distance a bot likes to fight at: close with Frost, far with the
/// Lance.
pub fn range(me: &Player) -> f32 {
    let has = |sp| me.slots.iter().flatten().any(|s| s.spell == sp);
    if has(spell::FROST) {
        BOT_RANGE * BOT_RANGE_CLOSE
    } else if has(spell::LANCE) {
        BOT_RANGE * BOT_RANGE_FAR
    } else {
        BOT_RANGE
    }
}
