//! Where a bot goes: in the drop, a spot of its own; then a cube worth
//! having, or a wander inside the storm's next circle; in when the storm
//! would otherwise catch it (up a launch rune on the way); away to heal;
//! and back off what it cannot walk up.

use super::sense::Mark;
use super::{keys_toward, unit, wants, Mind};
use crate::laws::*;
use crate::motion::{keys, Input};
use crate::storm::Now;
use crate::trig;
use crate::world::{Player, World};

/// Where a bot means to go this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    /// The way it walks (around whatever stands ahead).
    pub heading: u16,
    /// Running from the storm; far to go (hopping); running off to heal.
    pub flee: bool,
    pub far: bool,
    pub retreat: bool,
    /// Nowhere to go this tick: gliding down near where it means to
    /// land, or a sparring dummy at its post.
    pub stay: bool,
    /// The cube it goes for: which, and where.
    pub cube: Option<(u16, [f32; 3])>,
}

/// A heading `a` (radians) from `at` that steps around whatever stands
/// just ahead.
fn around(w: &World, at: [f32; 3], a: f32) -> f32 {
    let (dx, dz) = (a.cos(), a.sin());
    for q in w.map.near(at[0], at[2], BOT_AVOID + 2.0) {
        if at[1] > q.y + q.h {
            continue;
        }
        let (ox, oz) = (q.x - at[0], q.z - at[2]);
        let ahead = ox * dx + oz * dz;
        // Which side of the path its centre is (right positive), and how
        // near.
        let side = oz * dx - ox * dz;
        if ahead > 0.0 && ahead < q.r + BOT_AVOID && side.abs() < q.r + RADIUS + 0.4 {
            return a + if side > 0.0 {
                -BOT_AVOID_TURN
            } else {
                BOT_AVOID_TURN
            };
        }
    }
    a
}

/// A spot on open land within `r` of `c`, drawn from `n` (or `c` itself).
fn somewhere(w: &World, m: &Mind, tick: u32, (c, r): ([f32; 2], f32), n: u64) -> [f32; 2] {
    for k in 0..8 {
        let a = unit(m.seed, tick, n + 2 * k) * std::f32::consts::TAU;
        let d = r * unit(m.seed, tick, n + 2 * k + 1).sqrt();
        let g = [c[0] + a.cos() * d, c[1] + a.sin() * d];
        if w.map.land(g[0], g[1]) && w.map.near(g[0], g[1], 1.0).next().is_none() {
            return g;
        }
    }
    c
}

/// Where to land: of a few spots on the island, the one furthest from
/// where everyone else is or means to land.
fn drop_spot(w: &World, me: &Player, m: &Mind, tick: u32) -> [f32; 2] {
    let others: Vec<[f32; 2]> = w
        .players
        .iter()
        .filter(|p| p.id != me.id && p.alive && p.entrant)
        .map(|p| {
            if p.bot && p.mind.landing {
                p.mind.goal
            } else {
                [p.body.p[0], p.body.p[2]]
            }
        })
        .collect();
    // As far as the broom carries it on the way down.
    let reach = GLIDE_SPEED * me.body.p[1].max(0.0) / GLIDE_FALL * 0.8;
    let mut best = ([me.body.p[0], me.body.p[2]], -1.0);
    for n in 0..12 {
        let g = somewhere(w, m, tick, ([0.0, 0.0], BOT_DROP_SPREAD), 100 + n * 16);
        if (g[0] - me.body.p[0]).hypot(g[1] - me.body.p[2]) > reach {
            continue;
        }
        let room = others
            .iter()
            .map(|o| (o[0] - g[0]).hypot(o[1] - g[1]))
            .fold(f32::MAX, f32::min);
        if room > best.1 {
            best = (g, room);
        }
    }
    best.0
}

/// Where `me` goes this tick (its goals renewed in `m`).
pub fn plan(
    w: &World,
    me: &Player,
    m: &mut Mind,
    storm: &Now,
    mark: Option<&Mark>,
    tick: u32,
) -> Plan {
    let p = me.body.p;
    let next = storm.next;
    let from_next = (p[0] - next.0[0]).hypot(p[2] - next.0[1]);
    // In, when it would not otherwise make it in time (nowhere is safer
    // than the last circle's middle).
    let beyond = from_next - next.1.max(BOT_SAFE_R);
    let shrink = STORM.get(storm.phase).map_or(0, |s| s.1);
    let left = storm.secs + if storm.shrinking { 0 } else { shrink };
    let flee = beyond > 0.0
        && (storm.outside(p[0], p[2]) || beyond / RUN + BOT_FLEE_SPARE > left as f32)
        && m.dummy == 0;
    // Its own spot to land on, in the drop.
    if me.body.glide && !m.landing && w.practice.is_none() {
        m.goal = drop_spot(w, me, m, tick);
        m.goal_at = tick + BOT_GOAL_SECS * TICK_HZ;
        m.landing = true;
    }
    let goal_far = (p[0] - m.goal[0]).hypot(p[2] - m.goal[1]);
    let first = storm.phase == 0 && !storm.shrinking;
    let stale = storm.phase as u8 != m.goal_phase
        || (!first && (m.goal[0] - next.0[0]).hypot(m.goal[1] - next.0[1]) > next.1);
    let backing = m.stuck > 0 && tick < m.stuck + BOT_BACK_SECS * TICK_HZ;
    let due = tick >= m.goal_at || goal_far < 3.0 || (stale && !backing);
    if !me.body.glide && due {
        // A wander: before the storm first closes, anywhere on the
        // island; then inside its next circle.
        let room = if first { (storm.centre, storm.r) } else { next };
        m.goal = somewhere(w, m, tick, (room.0, room.1 * BOT_GOAL_SHARE), 2);
        m.goal_at = tick + BOT_GOAL_SECS * TICK_HZ;
        m.goal_phase = storm.phase as u8;
    }
    if m.dummy == 3 {
        m.goal = m.home;
    }
    let goal_far = (p[0] - m.goal[0]).hypot(p[2] - m.goal[1]);
    // Hurt, with nothing ready to heal or shield: off, out of sight.
    let low = me.hp * 100 < BOT_RETREAT_HP * me.max_hp();
    let cover = me.slots.iter().zip(me.cds).any(|(s, cd)| {
        cd == 0 && s.is_some_and(|s| s.spell == spell::MEND || s.spell == spell::WARD)
    });
    if low && !cover && !flee && m.dummy == 0 && mark.is_some_and(|mk| mk.d > BOT_RANGE * 0.5) {
        m.retreat_until = m.retreat_until.max(tick + BOT_RETREAT_SECS * TICK_HZ);
    }
    let retreat = tick < m.retreat_until && !flee;
    // A cube worth having nearby, inside the storm and within reach.
    let near = |q: [f32; 3]| (q[0] - p[0]).powi(2) + (q[2] - p[2]).powi(2);
    let cube = w
        .scrolls
        .iter()
        .filter(|s| {
            near(s.p) < LOOT_SIGHT * LOOT_SIGHT
                && !storm.outside(s.p[0], s.p[2])
                && s.p[1] - w.map.height(s.p[0], s.p[2]) < BOT_REACH_UP
                && !(s.id == m.skip && tick < m.skip_until)
                && wants(me, s.spell)
        })
        .min_by(|a, b| near(a.p).total_cmp(&near(b.p)))
        .filter(|_| !me.body.glide && m.dummy == 0)
        .map(|s| (s.id, s.p));
    // Running from the storm: a launch rune near and on the way throws
    // it up to glide there.
    let pad = w
        .map
        .pads
        .iter()
        .filter(|q| {
            let (dx, dz) = (q[0] - p[0], q[2] - p[2]);
            let (tx, tz) = (next.0[0] - p[0], next.0[1] - p[2]);
            flee && me.body.ground
                && dx * dx + dz * dz < BOT_PAD * BOT_PAD
                && dx * tx + dz * tz > 0.0
        })
        .map(|q| [q[0], q[2]])
        .next();
    let to = if backing {
        m.goal
    } else if flee {
        pad.unwrap_or(next.0)
    } else if retreat {
        // Straight away from the one it ran from.
        let from = mark.map_or(m.last, |mk| mk.feet);
        [p[0] * 2.0 - from[0], p[2] * 2.0 - from[2]]
    } else if let Some((_, c)) = cube {
        [c[0], c[2]]
    } else {
        m.goal
    };
    let far = flee || retreat || (to[0] - p[0]).hypot(to[1] - p[2]) > BOT_HOP_FAR;
    let a = (to[1] - p[2]).atan2(to[0] - p[0]);
    Plan {
        heading: trig::heading(around(w, p, a)),
        flee,
        far,
        retreat,
        stay: (me.body.glide && goal_far < BOT_DROP_NEAR) || (m.dummy == 3 && goal_far < 2.0),
        cube,
    }
}

/// Stuck against something: jump, and back away a while (a cliff will
/// not be climbed by walking at it) before going elsewhere, giving up on
/// a cube it could not reach; just stuck and falling down a wall (out of
/// a pit), kick off it.
pub fn unstick(me: &Player, m: &mut Mind, plan: &Plan, i: &mut Input, tick: u32) {
    let b = &me.body;
    if m.stuck > 0 && tick < m.stuck + 2 * TICK_HZ && crate::wall::can(b) && b.v[1] < 0.0 && !b.held
    {
        i.keys |= keys::JUMP;
    }
    if !tick.is_multiple_of(TICK_HZ) {
        return;
    }
    let moved = (b.p[0] - m.was[0]).hypot(b.p[2] - m.was[2]);
    if moved < BOT_STUCK_MOVE && i.keys & (keys::FWD | keys::BACK) != 0 && b.ground {
        i.keys |= keys::JUMP;
        // Back the way it came.
        let (s, c) = trig::sin_cos(plan.heading.wrapping_add(32768));
        let p = b.p;
        m.goal = [p[0] + c * BOT_BACK_OFF, p[2] + s * BOT_BACK_OFF];
        m.goal_at = tick + BOT_BACK_SECS * TICK_HZ;
        m.stuck = tick;
        // Stuck by a cube: one out of reach.
        if let Some((id, c)) = plan.cube {
            if (c[0] - b.p[0]).hypot(c[2] - b.p[2]) < BOT_SKIP_NEAR {
                (m.skip, m.skip_until) = (id, tick + BOT_SKIP_SECS * TICK_HZ);
            }
        }
    }
    m.was = b.p;
}

/// The keys that walk `plan`'s way, facing `yaw`.
pub fn keys(plan: &Plan, yaw: u16) -> u16 {
    keys_toward(yaw, plan.heading)
}
