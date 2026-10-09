//! Bots: wizards that fill each match. They land where they aimed in the
//! drop, keep inside the storm's next circle (hopping, timed to each
//! landing, when they have far to go; up a launch rune and gliding when
//! the storm comes and one is on the way), pick the nearest wizard they
//! can see, and duel: strafing, keeping their distance, leading their
//! shots, a little off. They see as people do (not through hills, trees
//! or pillars) and take a moment to notice.

use engine::rng::splitmix;

use crate::laws::*;

use crate::motion::{cast, keys, Input};
use crate::storm::Now;
use crate::trig;
use crate::world::{Player, World};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mind {
    pub seed: u64,
    pub target: u16,
    /// When it may start shooting at its target.
    pub ready_at: u32,
    /// Strafing: which way (−1, 1), until when.
    pub strafe: i8,
    pub strafe_until: u32,
    /// Where it walks when no one is about, and since when.
    pub goal: [f32; 2],
    pub goal_at: u32,
    /// Where it stood a second ago (to know when it is stuck).
    pub was: [f32; 3],
    /// A practice dummy: 0 not one, 1 stands, 2 strafes, 3 spars when
    /// sparring is on (and strafes when not).
    pub dummy: u8,
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

/// A heading `a` from `at` that steps around whatever stands just ahead.
fn around(w: &World, at: [f32; 3], a: f32) -> f32 {
    let (dx, dz) = (a.cos(), a.sin());
    for q in w.map.near(at[0], at[2], 6.0) {
        if at[1] > q.y + q.h {
            continue;
        }
        let (ox, oz) = (q.x - at[0], q.z - at[2]);
        let ahead = ox * dx + oz * dz;
        // Which side of the path its centre is (left positive), and how near.
        let side = oz * dx - ox * dz;
        if ahead > 0.0 && ahead < q.r + 4.0 && side.abs() < q.r + RADIUS + 0.4 {
            return a + if side > 0.0 { -1.1 } else { 1.1 };
        }
    }
    a
}

fn unit(seed: u64, tick: u32, k: u64) -> f32 {
    (splitmix(seed ^ (tick as u64) << 8 ^ k) >> 40) as f32 / (1u64 << 24) as f32
}

/// What a bot does this tick, and what it now has in mind.
pub fn think(w: &World, k: usize, storm: &Now, tick: u32) -> (Input, Mind) {
    let me = &w.players[k];
    let mut m = me.mind;
    let sparring = w.practice.as_ref().is_some_and(|p| p.sparring);
    match m.dummy {
        1 => {
            return (
                Input {
                    yaw: me.yaw,
                    ..Input::default()
                },
                m,
            )
        }
        2 | 3 if !(m.dummy == 3 && sparring) => {
            // Side to side, turning now and then.
            if tick >= m.strafe_until {
                m.strafe = if m.strafe > 0 { -1 } else { 1 };
                m.strafe_until = tick + 35 + (unit(m.seed, tick, 30) * 40.0) as u32;
            }
            let keys = if m.strafe > 0 {
                keys::RIGHT
            } else {
                keys::LEFT
            };
            let jump = if unit(m.seed, tick, 31) < 0.01 {
                keys::JUMP
            } else {
                0
            };
            return (
                Input {
                    yaw: me.yaw,
                    keys: keys | jump,
                    ..Input::default()
                },
                m,
            );
        }
        _ => {}
    }
    let eye = me.eye();
    let chest = |p: &crate::world::Player| [p.body.p[0], p.body.p[1] + 1.1, p.body.p[2]];
    // Who it can see.
    let seen = w
        .players
        .iter()
        .filter(|p| p.id != me.id && p.alive && p.entrant)
        .map(|p| {
            let c = chest(p);
            let d = ((c[0] - eye[0]).powi(2) + (c[1] - eye[1]).powi(2) + (c[2] - eye[2]).powi(2))
                .sqrt();
            (p, d)
        })
        .filter(|&(p, d)| d < BOT_SIGHT && !me.body.glide && w.map.strikes(eye, chest(p)).is_none())
        .min_by(|a, b| a.1.total_cmp(&b.1));
    match seen {
        Some((p, _)) if p.id != m.target => {
            m.target = p.id;
            m.ready_at = tick + BOT_NOTICE + (unit(m.seed, tick, 1) * 10.0) as u32;
        }
        None => m.target = 0,
        _ => {}
    }
    // Where it wants to be.
    let next = storm.next;
    let from_next =
        ((me.body.p[0] - next.0[0]).powi(2) + (me.body.p[2] - next.0[1]).powi(2)).sqrt();
    let flee = from_next > next.1 * 0.85
        && (storm.shrinking || storm.secs < 15 || storm.outside(me.body.p[0], me.body.p[2]));
    if tick >= m.goal_at
        || ((me.body.p[0] - m.goal[0]).powi(2) + (me.body.p[2] - m.goal[1]).powi(2)) < 9.0
    {
        let a = unit(m.seed, tick, 2) * std::f32::consts::TAU;
        let r = next.1 * 0.7 * unit(m.seed, tick, 3).sqrt();
        m.goal = [next.0[0] + a.cos() * r, next.0[1] + a.sin() * r];
        m.goal_at = tick + 20 * TICK_HZ;
    }
    let mut keys = 0;
    let mut yaw = me.yaw;
    let mut pitch: i16 = 0;
    // Walking: toward a point, as keys relative to where it faces.
    let walk_to = |to: [f32; 2]| (to[1] - me.body.p[2]).atan2(to[0] - me.body.p[0]);
    let mut walk: Option<f32> = None;
    let mut far = false;
    if let Some((p, d)) = seen.filter(|_| !flee) {
        let c = chest(p);
        // Lead the shot, and miss a little.
        let lead = d / BOLT_SPEED;
        let aim = [
            c[0] + p.body.v[0] * lead,
            c[1] + p.body.v[1] * lead * 0.5,
            c[2] + p.body.v[2] * lead,
        ];
        let (dx, dy, dz) = (aim[0] - eye[0], aim[1] - eye[1], aim[2] - eye[2]);
        let off = BOT_AIM_ERROR * (1.0 + d / 30.0);
        let jy = (unit(m.seed, tick / 4, 4) - 0.5) * 2.0 * off;
        let jp = (unit(m.seed, tick / 4, 5) - 0.5) * 2.0 * off;
        yaw = trig::heading(dz.atan2(dx) + jy);
        pitch = trig::pitch(dy.atan2((dx * dx + dz * dz).sqrt()) + jp);
        if tick >= m.ready_at {
            keys |= keys::FIRE;
        }
        if tick >= m.strafe_until {
            m.strafe = if unit(m.seed, tick, 6) < 0.5 { -1 } else { 1 };
            m.strafe_until = tick + 15 + (unit(m.seed, tick, 7) * 40.0) as u32;
        }
        keys |= if m.strafe > 0 {
            keys::RIGHT
        } else {
            keys::LEFT
        };
        let want = range(me);
        if d > want * 1.4 {
            keys |= keys::FWD;
        } else if d < want * 0.6 {
            keys |= keys::BACK;
        }
        if unit(m.seed, tick, 8) < 0.02 {
            keys |= keys::JUMP;
        }
    } else {
        // A cube worth having nearby (inside the storm), else its goal.
        let near = |q: [f32; 3]| (q[0] - me.body.p[0]).powi(2) + (q[2] - me.body.p[2]).powi(2);
        let cube = w
            .scrolls
            .iter()
            .filter(|s| {
                near(s.p) < LOOT_SIGHT * LOOT_SIGHT
                    && !storm.outside(s.p[0], s.p[2])
                    && wants(me, s.spell, s.rank)
            })
            .map(|s| (s.p, near(s.p)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        // Running from the storm: a launch rune near and on the way
        // throws it up to glide there.
        let pad = w
            .map
            .pads
            .iter()
            .filter(|q| {
                let (dx, dz) = (q[0] - me.body.p[0], q[2] - me.body.p[2]);
                let (tx, tz) = (next.0[0] - me.body.p[0], next.0[1] - me.body.p[2]);
                flee && me.body.ground
                    && dx * dx + dz * dz < BOT_PAD * BOT_PAD
                    && dx * tx + dz * tz > 0.0
            })
            .map(|q| [q[0], q[2]])
            .next();
        let to = match (flee, pad, cube) {
            (true, Some(q), _) => q,
            (true, None, _) => next.0,
            (false, _, Some((p, _))) => [p[0], p[2]],
            (false, _, None) => m.goal,
        };
        far = flee || (to[0] - me.body.p[0]).hypot(to[1] - me.body.p[2]) > BOT_HOP_FAR;
        walk = Some(around(w, me.body.p, walk_to(to)));
    }
    if let Some(a) = walk {
        // Turn toward where it walks, a little at a time.
        let want = trig::heading(a);
        let diff = want.wrapping_sub(yaw) as i16;
        yaw = yaw.wrapping_add((diff as i32).clamp(-1800, 1800) as i16 as u16);
        // Going somewhere (not fighting): at a sprint; far, hopping, each
        // hop timed to the landing (a press, so let go between).
        keys |= keys::FWD | keys::SPRINT;
        let b = &me.body;
        if far && b.ground && !b.held && (b.landed <= 1 || unit(m.seed, tick, 9) < 0.05) {
            keys |= keys::JUMP;
        }
    }
    // Stuck against something: jump, and walk elsewhere.
    if tick.is_multiple_of(TICK_HZ) {
        let moved = (me.body.p[0] - m.was[0]).powi(2) + (me.body.p[2] - m.was[2]).powi(2);
        if moved < 0.5 && keys & (keys::FWD | keys::BACK) != 0 && me.body.ground {
            keys |= keys::JUMP;
            m.goal_at = tick;
        }
        m.was = me.body.p;
    }
    let cast = spells(me, seen.map(|s| s.1), flee, tick, m.seed);
    let input = Input {
        seq: 0,
        yaw,
        pitch,
        keys,
        cast,
    };
    (input, m)
}

/// Whether a cube is worth a bot's detour: a spell it can still learn
/// or rank up.
fn wants(me: &Player, spell: u8, _rank: u8) -> bool {
    me.book[spell as usize % SPELLS.len()] < MAX_RANK
}

/// The distance a bot likes to fight at: close with Frost, far with the
/// Lance.
pub fn range(me: &Player) -> f32 {
    let has = |sp| me.slots.iter().flatten().any(|s| s.spell == sp);
    if has(spell::FROST) {
        BOT_RANGE * 0.45
    } else if has(spell::LANCE) {
        BOT_RANGE * 1.5
    } else {
        BOT_RANGE
    }
}

/// Which spells a bot casts now: at its mark when it has one (each from
/// the distance it is good at), to save itself when hurt, to run when the
/// storm comes or chill holds it.
fn spells(me: &Player, mark: Option<f32>, flee: bool, tick: u32, seed: u64) -> u8 {
    let mut bits = 0;
    let hurt = me.hp * 100 / me.max_hp().max(1);
    let struck = tick.saturating_sub(me.hurt_at) < 20;
    for (k, slot) in me.slots.iter().enumerate() {
        let Some(s) = slot else {
            continue;
        };
        if me.cds[k] > 0 || unit(seed, tick, 20 + k as u64) > 0.08 {
            continue;
        }
        let go = match (s.spell, mark) {
            (spell::LANCE, Some(d)) => d < 50.0,
            (spell::FIREBALL, Some(d)) => d < 32.0,
            (spell::FROST, Some(d)) => d < 9.0,
            (spell::LIGHTNING, Some(d)) => d > 6.0 && d < 40.0,
            (spell::GUST, Some(d)) => d < GUST_RADIUS * 0.7,
            (spell::WARD, _) => struck && hurt < 85,
            (spell::MEND, _) => hurt < 55,
            (spell::BLINK, _) => {
                flee || (mark.is_some() && (me.body.chill > 0 || (struck && hurt < 30)))
            }
            // Away from the storm, or after a mark that keeps its
            // distance.
            (spell::TETHER, Some(d)) => d > range(me) * 1.6,
            (spell::TETHER, None) => flee,
            _ => false,
        };
        if go {
            bits |= cast::SLOT[k];
        }
    }
    bits
}
