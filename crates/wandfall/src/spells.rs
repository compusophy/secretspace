//! Spells: what each does when cast, when its bolt lands, and over time.
//! Nine, one verb each; damage, shields and healing grow with the
//! caster's level, every effect with the spell's rank.
//!
//! - Fireball: a ball of fire bursting where it lands (the splash finds
//!   cover).
//! - Lance: an instant beam, far (the sniper's).
//! - Frost: a fan of ice shards, deadly close, that chills (slows).
//! - Lightning: strikes where you look, a breath later, from above.
//! - Blink: a step through the air that shakes off chill.
//! - Ward: a brief shield that eats the next big hit.
//! - Mend: heal, quickly.
//! - Gust: throws back everyone near you, and blows their bolts away.
//! - Tether: a rope of light that catches where you look and hauls you
//!   there (`tether`, in your body's motion, so the page predicts it).

use crate::laws::*;
use crate::loot::{cooldown, level_scale, power};
use crate::motion::keys;
use crate::trig;
use crate::world::{through, Bolt, Event, Phase, Player, World, WAND};

/// Lightning's mark: where it will strike, and when.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    pub by: u16,
    pub at: [f32; 3],
    pub power: i32,
    pub land: u32,
}

fn ahead(p: [f32; 3], d: [f32; 3], k: f32) -> [f32; 3] {
    [p[0] + d[0] * k, p[1] + d[1] * k, p[2] + d[2] * k]
}

/// What a look from `eye` along `d` meets within `range`: the point, and
/// the wizard there (0: the ground, something standing, or nothing).
pub fn sight(w: &World, by: u16, eye: [f32; 3], d: [f32; 3], range: f32) -> ([f32; 3], u16) {
    let end = ahead(eye, d, range);
    let mut first = (w.map.strikes(eye, end).unwrap_or(1.0), 0);
    let fight = w.phase == Phase::Fight;
    for p in &w.players {
        if p.id == by || !p.alive || p.entrant != fight {
            continue;
        }
        if let Some(t) = through(eye, end, p.body.p, p.body.tall()) {
            if t < first.0 {
                first = (t, p.id);
            }
        }
    }
    (ahead(eye, d, range * first.0), first.1)
}

/// As `sight`, but others where they stood at the end of tick `then` (as
/// a page saw them), if that is still known.
pub fn sight_then(
    w: &World,
    by: u16,
    (eye, d): ([f32; 3], [f32; 3]),
    range: f32,
    then: u32,
) -> ([f32; 3], u16) {
    let Some((_, then)) = w.past.iter().find(|p| p.0 == then && then < w.tick) else {
        return sight(w, by, eye, d, range);
    };
    let end = ahead(eye, d, range);
    let mut first = (w.map.strikes(eye, end).unwrap_or(1.0), 0);
    let fight = w.phase == Phase::Fight;
    for &(id, feet, tall) in then {
        // Still in it, now: a page cannot strike the knocked out.
        let in_it = w
            .players
            .iter()
            .any(|p| p.id == id && p.alive && p.entrant == fight);
        if id == by || !in_it {
            continue;
        }
        if let Some(t) = through(eye, end, feet, tall) {
            if t < first.0 {
                first = (t, id);
            }
        }
    }
    (ahead(eye, d, range * first.0), first.1)
}

/// Where a cast was aimed, as of the input that cast it: the eye, the
/// look, the keys held (Blink steps the way you steer), and how many
/// ticks behind its page drew everyone else.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Aim {
    pub eye: [f32; 3],
    pub yaw: u16,
    pub pitch: i16,
    pub keys: u16,
    pub behind: u32,
}

impl Aim {
    /// As `p` stands and looks now.
    pub fn of(p: &Player) -> Aim {
        Aim {
            eye: p.eye(),
            yaw: p.yaw,
            pitch: p.pitch,
            keys: p.last.keys,
            behind: p.behind,
        }
    }
}

/// `k` casts the spell in `slot`, if it is ready, aimed as `aim`.
pub fn cast(w: &mut World, k: usize, slot: usize, aim: Aim, ev: &mut Vec<Event>) {
    let tick = w.tick;
    let p = &w.players[k];
    let Some(s) = p.slots[slot] else {
        return;
    };
    if !p.alive || p.body.glide || p.cds[slot] > 0 {
        return;
    }
    let pw = power(s.spell, s.rank);
    // A Blink with nowhere to go is not spent.
    let blink_at = match s.spell {
        spell::BLINK => match blink_to(w, k, aim, pw as f32) {
            Some(at) => Some(at),
            None => return,
        },
        _ => None,
    };
    let p = &mut w.players[k];
    p.cds[slot] = cooldown(s.spell, s.rank);
    p.spell_cds[s.spell as usize % SPELLS.len()] = p.cds[slot];
    let (by, level, eye) = (p.id, p.level, aim.eye);
    let d = trig::look(aim.yaw, aim.pitch);
    let hit = level_scale(level, pw);
    let from = ahead(eye, d, 0.6);
    // Others where its caster's page saw them (not too far back).
    let then = tick.saturating_sub(aim.behind.min(REWIND));
    let bolt = |d: [f32; 3], speed: f32, life| Bolt {
        id: 0,
        by,
        kind: s.spell,
        rank: s.rank,
        p: from,
        v: [d[0] * speed, d[1] * speed, d[2] * speed],
        life,
        power: hit,
    };
    ev.push(Event::Cast {
        by,
        spell: s.spell,
        stage: 0,
        at: from,
    });
    match s.spell {
        spell::FIREBALL => w.bolt(bolt(d, FIREBALL_SPEED, FIREBALL_LIFE)),
        spell::LANCE => {
            let (to, who) = sight_then(w, by, (eye, d), LANCE_RANGE, then);
            ev.push(Event::Beam {
                by,
                spell: s.spell,
                from,
                to,
            });
            if who != 0 {
                w.hurt(by, who, hit, s.spell, ev);
            }
        }
        spell::FROST => {
            let half = FROST_SHARDS as i32 / 2;
            for n in 0..FROST_SHARDS as i32 {
                let turn = ((n - half) * FROST_SPREAD) as i16 as u16;
                let d = trig::look(aim.yaw.wrapping_add(turn), aim.pitch);
                w.bolt(bolt(d, FROST_SPEED, FROST_LIFE));
            }
        }
        spell::LIGHTNING => {
            let (to, _) = sight_then(w, by, (eye, d), LIGHTNING_RANGE, then);
            let at = [
                to[0],
                w.map.floor(to[0], to[2], to[1] + 0.3).max(SEA),
                to[2],
            ];
            w.zones.push(Zone {
                by,
                at,
                power: hit,
                land: tick + LIGHTNING_DELAY,
            });
            ev.push(Event::Cast {
                by,
                spell: s.spell,
                stage: 2,
                at,
            });
        }
        spell::BLINK => {
            if let Some(at) = blink_at {
                blink(w, k, at, ev);
            }
        }
        spell::WARD => {
            let p = &mut w.players[k];
            p.shield = p.shield.max(level_scale(level, pw));
            p.shield_until = tick + WARD_TICKS;
        }
        spell::MEND => {
            let p = &mut w.players[k];
            p.mend = level_scale(level, pw);
            p.mend_until = tick + MEND_TICKS;
        }
        spell::GUST => gust(w, k, pw as f32, ev),
        spell::TETHER => {
            // It catches the first thing on the line, or the air itself
            // short of its reach, and pulls as long as its reach needs.
            let reach = pw as f32;
            let (mut to, _) = sight_then(w, by, (eye, d), reach, then);
            let far =
                ((to[0] - eye[0]).powi(2) + (to[1] - eye[1]).powi(2) + (to[2] - eye[2]).powi(2))
                    .sqrt();
            if far > reach - 0.01 {
                to = ahead(eye, d, reach * TETHER_AIR);
            }
            let b = &mut w.players[k].body;
            b.anchor = to;
            b.tether = tether_ticks(pw);
            b.mantle = 0;
            ev.push(Event::Cast {
                by,
                spell: spell::TETHER,
                stage: 1,
                at: to,
            });
        }
        _ => {}
    }
}

/// How long a Tether of `reach` metres pulls (ticks): `TETHER_TICKS` at
/// the first rank's reach, longer as it reaches further.
pub fn tether_ticks(reach: i32) -> u8 {
    let first = SPELLS[spell::TETHER as usize].power.max(1);
    (TETHER_TICKS as i32 * reach / first).clamp(1, u8::MAX as i32) as u8
}

/// Where a Blink from `k` would land: up to `reach` metres the way it
/// steers (or faces), stopping short of anything (or anyone) in the way;
/// nowhere (None) if that is not even `BLINK_MIN` away.
pub fn blink_to(w: &World, k: usize, aim: Aim, reach: f32) -> Option<[f32; 3]> {
    let p = &w.players[k];
    let others: Vec<[f32; 3]> = w
        .players
        .iter()
        .filter(|q| q.id != p.id && q.alive)
        .map(|q| q.body.p)
        .collect();
    let (s, c) = trig::sin_cos(aim.yaw);
    let has = |key| (aim.keys & key != 0) as i32 as f32;
    let (f, r) = (
        has(keys::FWD) - has(keys::BACK),
        has(keys::RIGHT) - has(keys::LEFT),
    );
    // Forward is (c, s) on the ground; right is (-s, c).
    let (mut dx, mut dz) = (c * f - s * r, s * f + c * r);
    let len = (dx * dx + dz * dz).sqrt();
    (dx, dz) = if len > 1e-6 {
        (dx / len, dz / len)
    } else {
        (c, s)
    };
    let start = p.body.p;
    let mut at = start;
    let lim = MAP_HALF - 2.0;
    let mut d = BLINK_STEP;
    while d <= reach {
        let x = (start[0] + dx * d).clamp(-lim, lim);
        let z = (start[2] + dz * d).clamp(-lim, lim);
        let ground = w.map.height(x, z).max(SEA - 0.9);
        let y = start[1].max(ground);
        let crowded = others.iter().any(|q| {
            (q[0] - x).powi(2) + (q[2] - z).powi(2) < BLINK_CROWD * BLINK_CROWD
                && (q[1] - y).abs() < HEIGHT
        });
        if y - start[1] > BLINK_CLIMB
            || crowded
            || w.map
                .near(x, z, RADIUS)
                .any(|q| y < q.y + q.h && y + HEIGHT > q.y)
        {
            break;
        }
        at = [x, y, z];
        d += BLINK_STEP;
    }
    let moved = (at[0] - start[0]).hypot(at[2] - start[2]);
    (moved >= BLINK_MIN).then_some(at)
}

/// `k` steps through the air to `at`; chill falls away.
fn blink(w: &mut World, k: usize, at: [f32; 3], ev: &mut Vec<Event>) {
    let p = &mut w.players[k];
    p.body.p = at;
    p.body.ground = false;
    p.body.v[1] = 0.0;
    p.body.chill = 0;
    ev.push(Event::Cast {
        by: p.id,
        spell: spell::BLINK,
        stage: 1,
        at: [at[0], at[1] + 1.0, at[2]],
    });
}

/// Everyone near `k` thrown back at `push` m/s (and up), a little hurt,
/// and let go of whatever held them (a Tether, a climb); everyone else's
/// bolts near it blown away.
fn gust(w: &mut World, k: usize, push: f32, ev: &mut Vec<Event>) {
    let (by, at, level) = (w.players[k].id, w.players[k].body.p, w.players[k].level);
    let near = |p: [f32; 3], up: f32| {
        let (dx, dz) = (p[0] - at[0], p[2] - at[2]);
        (dx * dx + dz * dz).sqrt() <= GUST_RADIUS && (p[1] - at[1] - up).abs() < GUST_BAND
    };
    w.bolts.retain(|b| b.by == by || !near(b.p, 1.0));
    let fight = w.phase == Phase::Fight;
    let mut hit = Vec::new();
    for q in w
        .players
        .iter_mut()
        .filter(|q| q.id != by && q.alive && q.entrant == fight)
    {
        if !near(q.body.p, 0.0) {
            continue;
        }
        let (dx, dz) = (q.body.p[0] - at[0], q.body.p[2] - at[2]);
        let dist = (dx * dx + dz * dz).sqrt();
        let (nx, nz) = if dist > 0.01 {
            (dx / dist, dz / dist)
        } else {
            (1.0, 0.0)
        };
        q.body.v = [nx * push, GUST_LIFT, nz * push];
        q.body.ground = false;
        q.body.tether = 0;
        q.body.mantle = 0;
        hit.push(q.id);
    }
    for id in hit {
        w.hurt(by, id, level_scale(level, GUST_DAMAGE), spell::GUST, ev);
    }
}

/// Everyone within `r` of `at` but its caster and `skip` (one already
/// struck), hurt by `what`: less toward the edge. Lightning shatters a
/// Ward first.
fn burst(
    w: &mut World,
    (by, skip): (u16, u16),
    (at, r): ([f32; 3], f32),
    dmg: i32,
    what: u8,
    ev: &mut Vec<Event>,
) {
    let fight = w.phase == Phase::Fight;
    let hit: Vec<(u16, f32)> = w
        .players
        .iter()
        .filter(|q| q.id != by && q.id != skip && q.alive && q.entrant == fight)
        .map(|q| {
            let c = [q.body.p[0], q.body.p[1] + 1.0, q.body.p[2]];
            let d =
                ((c[0] - at[0]).powi(2) + (c[1] - at[1]).powi(2) + (c[2] - at[2]).powi(2)).sqrt();
            (q.id, d)
        })
        .filter(|&(_, d)| d < r)
        .collect();
    for (id, d) in hit {
        if what == spell::LIGHTNING {
            shatter(w, id, ev);
        }
        let k = 1.0 - (1.0 - BURST_EDGE) * d / r;
        w.hurt(by, id, (dmg as f32 * k).round() as i32, what, ev);
    }
}

/// `id`'s Ward broken outright.
fn shatter(w: &mut World, id: u16, ev: &mut Vec<Event>) {
    if let Some(q) = w.find_mut(id).filter(|q| q.shield > 0) {
        q.shield = 0;
        let at = [q.body.p[0], q.body.p[1] + 1.0, q.body.p[2]];
        ev.push(Event::Cast {
            by: id,
            spell: spell::WARD,
            stage: 2,
            at,
        });
    }
}

/// A bolt lands at `at`, on `who` (0: the ground or something standing).
pub fn impact(w: &mut World, b: &Bolt, at: [f32; 3], who: u16, ev: &mut Vec<Event>) {
    match b.kind {
        WAND => {
            if who != 0 {
                w.hurt(b.by, who, b.power, WAND, ev);
            }
        }
        spell::FIREBALL => {
            ev.push(Event::Cast {
                by: b.by,
                spell: b.kind,
                stage: 1,
                at,
            });
            // Whoever it struck takes all of it; the splash, the rest.
            if who != 0 {
                w.hurt(b.by, who, b.power, b.kind, ev);
            }
            let r = FIREBALL_RADIUS * (1.0 + FIREBALL_RANK_RADIUS * (b.rank as f32 - 1.0));
            burst(w, (b.by, who), (at, r), b.power, b.kind, ev);
        }
        spell::FROST => {
            ev.push(Event::Cast {
                by: b.by,
                spell: b.kind,
                stage: 1,
                at,
            });
            if who != 0 {
                if let Some(q) = w.find_mut(who) {
                    q.body.chill = q.body.chill.max(CHILL_TICKS);
                }
                w.hurt(b.by, who, b.power, b.kind, ev);
            }
        }
        _ => {}
    }
}

/// What spells do over time: cooldowns run, wards fade, mending heals,
/// and lightning strikes.
pub fn tick(w: &mut World, ev: &mut Vec<Event>) {
    let tick = w.tick;
    for p in w.players.iter_mut() {
        for cd in p.cds.iter_mut().chain(p.spell_cds.iter_mut()) {
            *cd = cd.saturating_sub(1);
        }
        if tick >= p.shield_until {
            p.shield = 0;
        }
        if p.mend > 0 && p.alive {
            let left = p.mend_until.saturating_sub(tick).max(1) as i32;
            let now = (p.mend + left - 1) / left;
            p.mend -= now;
            p.hp = (p.hp + now).min(p.max_hp());
        }
    }
    let (due, rest): (Vec<Zone>, Vec<Zone>) = w.zones.iter().partition(|z| tick >= z.land);
    w.zones = rest;
    for z in due {
        ev.push(Event::Cast {
            by: z.by,
            spell: spell::LIGHTNING,
            stage: 1,
            at: z.at,
        });
        let at = [z.at[0], z.at[1] + 0.5, z.at[2]];
        burst(
            w,
            (z.by, 0),
            (at, LIGHTNING_RADIUS),
            z.power,
            spell::LIGHTNING,
            ev,
        );
    }
}

#[cfg(test)]
mod tests;
