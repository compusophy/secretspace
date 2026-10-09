//! Spells: what each does when cast, when its bolt lands, and over time.
//! Eight, one verb each; damage grows with the caster's level, every
//! effect with the spell's rank.
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

use crate::laws::*;
use crate::loot::{cooldown, level_scale, power};
use crate::trig;
use crate::world::{through, Bolt, Event, Phase, World, WAND};

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

/// `k` casts the spell in `slot`, if it is ready.
pub fn cast(w: &mut World, k: usize, slot: usize, ev: &mut Vec<Event>) {
    let tick = w.tick;
    let p = &mut w.players[k];
    let Some(s) = p.slots[slot] else {
        return;
    };
    if !p.alive || p.body.glide || p.cds[slot] > 0 {
        return;
    }
    p.cds[slot] = cooldown(s.spell, s.rank);
    let (by, level, eye, yaw, pitch) = (p.id, p.level, p.eye(), p.yaw, p.pitch);
    let d = trig::look(yaw, pitch);
    let pw = power(s.spell, s.rank);
    let hit = level_scale(level, pw);
    let from = ahead(eye, d, 0.6);
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
            let (to, who) = sight(w, by, eye, d, LANCE_RANGE);
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
                let d = trig::look(yaw.wrapping_add(turn), pitch);
                w.bolt(bolt(d, FROST_SPEED, FROST_LIFE));
            }
        }
        spell::LIGHTNING => {
            let (to, _) = sight(w, by, eye, d, LIGHTNING_RANGE);
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
        spell::BLINK => blink(w, k, pw as f32, ev),
        spell::WARD => {
            let p = &mut w.players[k];
            p.shield = p.shield.max(pw);
            p.shield_until = tick + WARD_TICKS;
        }
        spell::MEND => {
            let p = &mut w.players[k];
            p.mend = pw;
            p.mend_until = tick + MEND_TICKS;
        }
        spell::GUST => gust(w, k, pw as f32, ev),
        _ => {}
    }
}

/// Step through the air toward where `k` looks, up to `reach` metres,
/// stopping short of anything (or anyone) in the way; chill falls away.
fn blink(w: &mut World, k: usize, reach: f32, ev: &mut Vec<Event>) {
    let p = &w.players[k];
    let others: Vec<[f32; 3]> = w
        .players
        .iter()
        .filter(|q| q.id != p.id && q.alive)
        .map(|q| q.body.p)
        .collect();
    let (s, c) = trig::sin_cos(p.yaw);
    let start = p.body.p;
    let mut at = start;
    let lim = MAP_HALF - 2.0;
    let mut d = 0.5;
    while d <= reach {
        let x = (start[0] + c * d).clamp(-lim, lim);
        let z = (start[2] + s * d).clamp(-lim, lim);
        let ground = w.map.height(x, z).max(SEA - 0.9);
        let y = start[1].max(ground);
        let crowded = others.iter().any(|q| {
            (q[0] - x).powi(2) + (q[2] - z).powi(2) < (RADIUS * 2.2).powi(2)
                && (q[1] - y).abs() < HEIGHT
        });
        if y - start[1] > 3.0
            || crowded
            || w.map
                .near(x, z, RADIUS)
                .any(|q| y < q.y + q.h && y + HEIGHT > q.y)
        {
            break;
        }
        at = [x, y, z];
        d += 0.5;
    }
    let by = p.id;
    let p = &mut w.players[k];
    p.body.p = at;
    p.body.ground = false;
    p.body.v[1] = 0.0;
    p.body.chill = 0;
    ev.push(Event::Cast {
        by,
        spell: spell::BLINK,
        stage: 1,
        at: [at[0], at[1] + 1.0, at[2]],
    });
}

/// Everyone near `k` thrown back at `push` m/s (and up), a little hurt;
/// everyone else's bolts near it blown away.
fn gust(w: &mut World, k: usize, push: f32, ev: &mut Vec<Event>) {
    let (by, at, level) = (w.players[k].id, w.players[k].body.p, w.players[k].level);
    let near = |p: [f32; 3], up: f32| {
        let (dx, dz) = (p[0] - at[0], p[2] - at[2]);
        (dx * dx + dz * dz).sqrt() <= GUST_RADIUS && (p[1] - at[1] - up).abs() < 3.0
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
        hit.push(q.id);
    }
    for id in hit {
        w.hurt(by, id, level_scale(level, GUST_DAMAGE), spell::GUST, ev);
    }
}

/// Everyone within `r` of `at` but `by`, hurt by `what` (less toward the
/// edge).
fn burst(w: &mut World, by: u16, at: [f32; 3], r: f32, dmg: i32, what: u8, ev: &mut Vec<Event>) {
    let fight = w.phase == Phase::Fight;
    let hit: Vec<(u16, f32)> = w
        .players
        .iter()
        .filter(|q| q.id != by && q.alive && q.entrant == fight)
        .map(|q| {
            let c = [q.body.p[0], q.body.p[1] + 1.0, q.body.p[2]];
            let d =
                ((c[0] - at[0]).powi(2) + (c[1] - at[1]).powi(2) + (c[2] - at[2]).powi(2)).sqrt();
            (q.id, d)
        })
        .filter(|&(_, d)| d < r)
        .collect();
    for (id, d) in hit {
        let k = 1.0 - 0.5 * d / r;
        w.hurt(by, id, (dmg as f32 * k) as i32, what, ev);
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
            let r = FIREBALL_RADIUS * (1.0 + 0.1 * (b.rank as f32 - 1.0));
            burst(w, b.by, at, r, b.power, b.kind, ev);
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
        for cd in p.cds.iter_mut() {
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
        burst(w, z.by, at, LIGHTNING_RADIUS, z.power, spell::LIGHTNING, ev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{Phase, Slot};

    fn duel(gap: f32) -> (World, u16, u16) {
        let mut w = World::new(21);
        let a = w.join("a", 0);
        let b = w.join("b", 0);
        // Into the fight, then side by side on the Spire's open plaza.
        while w.phase != Phase::Fight {
            w.step();
        }
        w.players.retain(|p| !p.bot);
        let [x, z] = [-gap / 2.0, -12.0];
        for (k, dx) in [(0usize, 0.0f32), (1, gap)] {
            let p = &mut w.players[k];
            p.body.glide = false;
            p.body.ground = true;
            p.body.p = [x + dx, w.map.height(x + dx, z), z];
            p.yaw = 0;
            p.pitch = 0;
        }
        // Looking at the other's chest.
        let (e, c) = (w.players[0].eye(), w.players[1].body.p);
        let dy = c[1] + 1.0 - e[1];
        w.players[0].pitch = trig::pitch(dy.atan2(gap));
        (w, a, b)
    }

    fn give(w: &mut World, k: usize, slot: usize, spell: u8) {
        w.players[k].slots[slot] = Some(Slot { spell, rank: 1 });
        w.players[k].cds[slot] = 0;
    }

    fn fly(w: &mut World, ticks: u32, ev: &mut Vec<Event>) {
        for _ in 0..ticks {
            w.tick += 1;
            let mut more = Vec::new();
            for b in std::mem::take(&mut w.bolts) {
                let e = ahead(b.p, b.v, DT);
                match through(b.p, e, w.players[1].body.p, HEIGHT) {
                    Some(_) => impact(w, &b, e, w.players[1].id, &mut more),
                    None if b.life > 1 => w.bolts.push(Bolt {
                        p: e,
                        life: b.life - 1,
                        ..b
                    }),
                    None => impact(w, &b, e, 0, &mut more),
                }
            }
            tick(w, &mut more);
            ev.append(&mut more);
        }
    }

    #[test]
    fn a_ward_takes_the_hurt_first_breaks_and_mend_heals() {
        let (mut w, a, b) = duel(4.0);
        let mut ev = Vec::new();
        give(&mut w, 1, 2, spell::WARD);
        cast(&mut w, 1, 2, &mut ev);
        assert!(w.find(b).unwrap().shield >= 40);
        w.hurt(a, b, 20, WAND, &mut ev);
        assert_eq!(w.find(b).unwrap().hp, HEALTH, "the ward took it");
        ev.clear();
        w.hurt(a, b, 40, WAND, &mut ev);
        let hp = w.find(b).unwrap().hp;
        assert!(hp < HEALTH);
        assert!(
            ev.iter().any(|e| matches!(
                e,
                Event::Cast {
                    spell: spell::WARD,
                    stage: 2,
                    ..
                }
            )),
            "and it broke"
        );
        give(&mut w, 1, 3, spell::MEND);
        cast(&mut w, 1, 3, &mut ev);
        fly(&mut w, MEND_TICKS, &mut ev);
        assert!(w.find(b).unwrap().hp > hp, "mended");
    }

    #[test]
    fn the_lance_strikes_at_once_and_far() {
        let (mut w, _, b) = duel(30.0);
        let mut ev = Vec::new();
        give(&mut w, 0, 0, spell::LANCE);
        cast(&mut w, 0, 0, &mut ev);
        assert!(w.find(b).unwrap().hp < HEALTH, "struck, no flight");
        assert!(ev.iter().any(|e| matches!(e, Event::Beam { .. })));
    }

    #[test]
    fn frost_is_deadly_close_and_chills() {
        let mut dealt = Vec::new();
        for gap in [3.0, 18.0] {
            let (mut w, _, b) = duel(gap);
            let mut ev = Vec::new();
            give(&mut w, 0, 0, spell::FROST);
            cast(&mut w, 0, 0, &mut ev);
            assert_eq!(w.bolts.len(), FROST_SHARDS);
            fly(&mut w, FROST_LIFE + 1, &mut ev);
            let q = w.find(b).unwrap();
            dealt.push(HEALTH - q.hp);
            if gap < 5.0 {
                assert!(q.body.chill > 0, "chilled");
            }
        }
        assert!(dealt[0] >= 30, "close, most shards: {dealt:?}");
        assert!(dealt[0] > dealt[1] * 2, "far, few: {dealt:?}");
    }

    #[test]
    fn fireball_bursts_and_lightning_strikes_late() {
        let (mut w, _, b) = duel(12.0);
        let mut ev = Vec::new();
        give(&mut w, 0, 0, spell::FIREBALL);
        cast(&mut w, 0, 0, &mut ev);
        fly(&mut w, 20, &mut ev);
        let hp = w.find(b).unwrap().hp;
        assert!(hp < HEALTH - 20, "burst: {hp}");
        give(&mut w, 0, 1, spell::LIGHTNING);
        cast(&mut w, 0, 1, &mut ev);
        assert_eq!(w.find(b).unwrap().hp, hp, "not yet");
        fly(&mut w, LIGHTNING_DELAY + 1, &mut ev);
        assert!(w.find(b).unwrap().hp < hp - 20, "struck");
    }

    #[test]
    fn gust_throws_back_and_blows_bolts_away_and_blink_moves_you() {
        let (mut w, _, b) = duel(4.0);
        let mut ev = Vec::new();
        give(&mut w, 1, 0, spell::FIREBALL);
        w.players[1].yaw = 32768;
        cast(&mut w, 1, 0, &mut ev);
        assert_eq!(w.bolts.len(), 1);
        give(&mut w, 0, 2, spell::GUST);
        cast(&mut w, 0, 2, &mut ev);
        assert!(w.bolts.is_empty(), "blown away");
        let v = w.find(b).unwrap().body.v;
        assert!(v[0] > 5.0 && v[1] > 0.0, "thrown away and up: {v:?}");
        let before = w.players[0].body.p;
        w.players[0].body.chill = 30;
        give(&mut w, 0, 3, spell::BLINK);
        cast(&mut w, 0, 3, &mut ev);
        let after = w.players[0].body.p;
        assert!(after != before, "blinked");
        assert_eq!(w.players[0].body.chill, 0, "and shook off the chill");
        assert!(w.players[0].cds[3] > 0, "and now it cools down");
        cast(&mut w, 0, 3, &mut ev);
        assert_eq!(w.players[0].body.p, after, "not twice");
    }
}
