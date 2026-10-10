//! What a bot sees: the wizard it fights, if any. It sees ahead of it
//! (and hears anyone near), never through hills, trees or pillars, and
//! always knows who just hurt it; it keeps to the one it fights, and for
//! a moment remembers where one went from sight.

use super::{unit, Mind};
use crate::laws::*;
use crate::trig;
use crate::world::{Player, World};

/// The wizard a bot fights: where its chest is (or was, when last seen),
/// its feet, how it moves, how far it is, and whether it is in sight now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    pub id: u16,
    pub chest: [f32; 3],
    pub feet: [f32; 3],
    pub v: [f32; 3],
    pub d: f32,
    pub seen: bool,
    pub ground: bool,
    /// Easy prey for Lightning: still, warded or mending.
    pub still: bool,
}

fn chest(p: &Player) -> [f32; 3] {
    [p.body.p[0], p.body.p[1] + 1.1, p.body.p[2]]
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn mark_of(p: &Player, eye: [f32; 3], seen: bool) -> Mark {
    let v = p.body.v;
    let still = v[0].hypot(v[2]) < BOT_STILL || p.shield > 0 || p.mend > 0;
    Mark {
        id: p.id,
        chest: chest(p),
        feet: p.body.p,
        v,
        d: dist(eye, chest(p)),
        seen,
        ground: p.body.ground,
        still,
    }
}

/// Who `me` fights this tick (it notices someone new, and keeps to or
/// forgets the one it had, in `m`).
pub fn mark(w: &World, me: &Player, m: &mut Mind, tick: u32) -> Option<Mark> {
    if me.body.glide {
        m.target = 0;
        return None;
    }
    let eye = me.eye();
    let (s, c) = trig::sin_cos(me.yaw);
    let hurt_by = (tick.saturating_sub(me.hurt_by_at) < BOT_MEMORY).then_some(me.hurt_by);
    // The first of a fight they loot, minding only the near.
    let calm = w.practice.is_none() && tick < w.began + BOT_CALM_SECS * TICK_HZ;
    let seen = w
        .players
        .iter()
        .filter(|p| p.id != me.id && p.alive && p.entrant)
        .filter_map(|p| {
            let at = chest(p);
            let d = dist(eye, at);
            let known = Some(p.id) == hurt_by || p.id == m.target;
            let reach = if Some(p.id) == hurt_by {
                LANCE_RANGE
            } else if calm {
                BOT_CALM_RANGE
            } else {
                BOT_SIGHT
            };
            let flat = (at[0] - eye[0]).hypot(at[2] - eye[2]).max(1e-3);
            let ahead = ((at[0] - eye[0]) * c + (at[2] - eye[2]) * s) / flat;
            let noticed = known || d < BOT_HEAR || ahead > BOT_FOV_COS;
            (d < reach && noticed && w.map.strikes(eye, at).is_none()).then(|| {
                // The one it fights, or who hurt it, seems nearer.
                let k = if known { BOT_STICKY } else { 1.0 };
                (p, d * k)
            })
        })
        .min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((p, _)) = seen {
        if p.id != m.target {
            m.target = p.id;
            m.ready_at = tick + BOT_NOTICE + (unit(m.seed, tick, 1) * BOT_NOTICE_MORE) as u32;
        }
        m.seen_at = tick;
        m.last = chest(p);
        return Some(mark_of(p, eye, true));
    }
    // Gone from sight a moment: where it was (it keeps its aim there).
    let gone = w.find(m.target).filter(|p| p.alive && p.entrant);
    if let Some(p) = gone.filter(|_| tick.saturating_sub(m.seen_at) < BOT_MEMORY) {
        return Some(Mark {
            chest: m.last,
            d: dist(eye, m.last),
            seen: false,
            ..mark_of(p, eye, false)
        });
    }
    m.target = 0;
    // Hurt by someone it cannot see: it turns to look.
    let by = hurt_by.and_then(|id| w.find(id)).filter(|p| p.alive);
    by.map(|p| mark_of(p, eye, false))
        .filter(|mk| mk.d < LANCE_RANGE)
}
