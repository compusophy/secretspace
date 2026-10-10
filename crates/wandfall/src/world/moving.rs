//! Moving, a tick at a time: bots think, everyone moves by their inputs
//! (as many as the clock allows), the wand fires, and bolts fly into
//! whatever is first in their way.

use crate::bots;
use crate::laws::*;
use crate::loot;
use crate::motion::{self, cast, keys, Input};
use crate::spells::{self, Aim};
use crate::trig;

use super::{Bolt, Event, Phase, World, WAND};

impl World {
    /// Each bot thinks what to do this tick.
    pub(super) fn minds(&mut self) {
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
    /// for: who, which slot, and aimed as the input that asked.
    pub(super) fn move_all(&mut self) -> Vec<(usize, usize, Aim)> {
        let mut shots = Vec::new();
        let mut casts = Vec::new();
        let tick = self.tick;
        for (k, p) in self.players.iter_mut().enumerate() {
            p.cool = p.cool.saturating_sub(1);
            if !p.alive {
                p.queue.clear();
                continue;
            }
            // One input a tick, and those held back past the jitter's
            // allowance caught up on, as far as the bank goes.
            p.credit = (p.credit + 1).min(INPUT_BANK);
            let steps = (1 + p.queue.len().saturating_sub(INPUT_JITTER)).min(p.credit as usize);
            for _ in 0..steps {
                let i = match p.queue.pop_front() {
                    Some(i) => {
                        p.idle = 0;
                        p.credit -= 1;
                        i
                    }
                    // A stalled page: its wizard stands in, at no cost
                    // (the inputs it is late with are its record of these
                    // ticks, and are caught up on when they come).
                    None => {
                        p.idle += 1;
                        if p.idle < INPUT_IDLE {
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
                if i.cast != 0 {
                    // What its page saw as it cast (a bot sees now).
                    let back = (tick as u16).wrapping_sub(i.view) as u32;
                    p.behind = if p.bot { 0 } else { back.min(REWIND) };
                }
                for (slot, bit) in cast::SLOT.iter().enumerate() {
                    if i.cast & bit != 0
                        && !casts
                            .iter()
                            .any(|c: &(usize, usize, Aim)| c.0 == k && c.1 == slot)
                    {
                        casts.push((k, slot, Aim::of(p)));
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
            // A page running ahead of the clock: what it sent beyond the
            // jitter's allowance is dropped (its prediction corrects), its
            // casts kept for the next.
            while p.queue.len() > INPUT_JITTER {
                let cast = p.queue.pop_front().map_or(0, |i| i.cast);
                if let Some(next) = p.queue.front_mut() {
                    next.cast |= cast;
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

    /// Every bolt on a tick: on, or into the first wizard or thing in
    /// its way.
    pub(super) fn fly(&mut self, ev: &mut Vec<Event>) {
        let mut impacts = Vec::new();
        let mut keep = Vec::with_capacity(self.bolts.len());
        let fight = self.phase == Phase::Fight;
        for mut b in std::mem::take(&mut self.bolts) {
            let a = b.p;
            let e = [a[0] + b.v[0] * DT, a[1] + b.v[1] * DT, a[2] + b.v[2] * DT];
            let mut first = self.map.strikes(a, e).map(|t| (t, 0u16));
            for p in &self.players {
                if p.id == b.by || !p.alive || p.entrant != fight {
                    continue;
                }
                if let Some(t) = through(a, e, p.body.p, p.body.tall()) {
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
                // A fireball bursts at the end of its flight too.
                None if b.kind == spell::FIREBALL => impacts.push((b, e, 0)),
                None => {}
            }
        }
        self.bolts = keep;
        for (b, at, who) in impacts {
            spells::impact(self, &b, at, who, ev);
        }
    }
}

/// Where along a bolt's step from `a` to `e` (0..1) it passes through a
/// wizard standing at `feet`, `tall` metres tall, if it does.
pub fn through(a: [f32; 3], e: [f32; 3], feet: [f32; 3], tall: f32) -> Option<f32> {
    // The wizard as a segment from shin to crown, `RADIUS` thick.
    let (lo, hi) = (feet[1] + RADIUS * 0.5, feet[1] + tall - RADIUS * 0.5);
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
