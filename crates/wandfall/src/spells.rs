//! Spells: what each does when cast, when its bolt lands, and over time
//! (wards fading, mending, starfall landing). Damage grows with the
//! caster's level, every effect with the spell's rank.

use crate::laws::*;
use crate::loot::{cooldown, level_scale, power};
use crate::trig;
use crate::world::{Bolt, Event, Phase, World, WAND};

/// Starfall's mark: where light will land, and when.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    pub by: u16,
    pub at: [f32; 3],
    pub power: i32,
    pub land: u32,
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
    let (by, level, eye) = (p.id, p.level, p.eye());
    let d = trig::look(p.yaw, p.pitch);
    let pw = power(s.spell, s.rank);
    let from = [
        eye[0] + d[0] * 0.6,
        eye[1] + d[1] * 0.6,
        eye[2] + d[2] * 0.6,
    ];
    let bolt = |kind, speed: f32, life, power, hold| Bolt {
        id: 0,
        by,
        kind,
        rank: s.rank,
        p: from,
        v: [d[0] * speed, d[1] * speed, d[2] * speed],
        life,
        power,
        hold,
    };
    ev.push(Event::Cast {
        by,
        spell: s.spell,
        stage: 0,
        at: eye,
    });
    match s.spell {
        spell::LANCE => w.bolt(bolt(
            s.spell,
            LANCE_SPEED,
            LANCE_LIFE,
            level_scale(level, pw),
            0,
        )),
        spell::COMET => w.bolt(bolt(
            s.spell,
            COMET_SPEED,
            COMET_LIFE,
            level_scale(level, pw),
            0,
        )),
        spell::CHAIN => w.bolt(bolt(
            s.spell,
            CHAIN_SPEED,
            CHAIN_LIFE,
            level_scale(level, pw),
            0,
        )),
        spell::ROOT => {
            let dmg = level_scale(level, ROOT_DAMAGE);
            w.bolt(bolt(s.spell, ROOT_SPEED, ROOT_LIFE, dmg, pw as u16));
        }
        spell::STARFALL => {
            let end = [
                eye[0] + d[0] * STARFALL_RANGE,
                eye[1] + d[1] * STARFALL_RANGE,
                eye[2] + d[2] * STARFALL_RANGE,
            ];
            let t = w.map.strikes(eye, end).unwrap_or(1.0);
            let x = eye[0] + (end[0] - eye[0]) * t;
            let z = eye[2] + (end[2] - eye[2]) * t;
            let at = [x, w.map.height(x, z).max(SEA), z];
            w.zones.push(Zone {
                by,
                at,
                power: level_scale(level, pw),
                land: tick + STARFALL_DELAY,
            });
            ev.push(Event::Cast {
                by,
                spell: s.spell,
                stage: 0,
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
        spell::GUST => {
            let at = w.players[k].body.p;
            let dmg = level_scale(level, pw);
            let mut hit = Vec::new();
            for q in w.players.iter_mut().filter(|q| q.id != by && q.alive) {
                let (dx, dz) = (q.body.p[0] - at[0], q.body.p[2] - at[2]);
                let dist = (dx * dx + dz * dz).sqrt();
                if dist > GUST_RADIUS || (q.body.p[1] - at[1]).abs() > 3.0 {
                    continue;
                }
                let (nx, nz) = if dist > 0.01 {
                    (dx / dist, dz / dist)
                } else {
                    (1.0, 0.0)
                };
                q.body.v = [nx * GUST_PUSH, GUST_LIFT, nz * GUST_PUSH];
                q.body.ground = false;
                hit.push(q.id);
            }
            for id in hit {
                w.hurt(by, id, dmg, ev);
            }
        }
        spell::HASTE => {
            let p = &mut w.players[k];
            p.body.haste = p.body.haste.max(pw as u16);
        }
        _ => {}
    }
}

/// Step through the air toward where `k` looks, up to `reach` metres,
/// stopping short of anything standing in the way.
fn blink(w: &mut World, k: usize, reach: f32, ev: &mut Vec<Event>) {
    let p = &w.players[k];
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
        if y - start[1] > 3.0
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
    ev.push(Event::Cast {
        by,
        spell: spell::BLINK,
        stage: 1,
        at: [at[0], at[1] + EYE, at[2]],
    });
}

/// Everyone within `r` of `at` but `by`, hurt (less toward the edge).
fn burst(w: &mut World, by: u16, at: [f32; 3], r: f32, dmg: i32, ev: &mut Vec<Event>) {
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
        w.hurt(by, id, (dmg as f32 * k) as i32, ev);
    }
}

/// A bolt lands at `at`, on `who` (0: the ground or something standing).
pub fn impact(w: &mut World, b: &Bolt, at: [f32; 3], who: u16, ev: &mut Vec<Event>) {
    match b.kind {
        WAND | spell::LANCE => {
            if who != 0 {
                w.hurt(b.by, who, b.power, ev);
            }
        }
        spell::COMET => {
            ev.push(Event::Cast {
                by: b.by,
                spell: spell::COMET,
                stage: 1,
                at,
            });
            burst(
                w,
                b.by,
                at,
                COMET_RADIUS + 0.3 * (b.rank as f32 - 1.0),
                b.power,
                ev,
            );
        }
        spell::ROOT => {
            if who != 0 {
                if let Some(q) = w.find_mut(who) {
                    q.body.root = q.body.root.max(b.hold);
                }
                w.hurt(b.by, who, b.power, ev);
            }
        }
        spell::CHAIN => {
            if who == 0 {
                return;
            }
            w.hurt(b.by, who, b.power, ev);
            let mut struck = vec![b.by, who];
            let mut from = who;
            let mut dmg = b.power;
            let fight = w.phase == Phase::Fight;
            for _ in 0..CHAIN_LEAPS + (b.rank - 1) / 2 {
                let Some(f) = w.find(from).map(|p| p.body.p) else {
                    break;
                };
                let next = w
                    .players
                    .iter()
                    .filter(|q| q.alive && q.entrant == fight && !struck.contains(&q.id))
                    .map(|q| {
                        (
                            q.id,
                            (q.body.p[0] - f[0]).powi(2) + (q.body.p[2] - f[2]).powi(2),
                        )
                    })
                    .filter(|&(_, d2)| d2 < CHAIN_REACH * CHAIN_REACH)
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                let Some((to, _)) = next else {
                    break;
                };
                dmg = dmg * CHAIN_FADE / 100;
                ev.push(Event::Link { from, to });
                w.hurt(b.by, to, dmg, ev);
                struck.push(to);
                from = to;
            }
        }
        _ => {}
    }
}

/// What spells do over time: wards fade, mending heals, cooldowns run,
/// and starfall lands.
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
            spell: spell::STARFALL,
            stage: 1,
            at: z.at,
        });
        burst(
            w,
            z.by,
            [z.at[0], z.at[1] + 0.5, z.at[2]],
            STARFALL_RADIUS,
            z.power,
            ev,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{Phase, Slot};

    fn duel() -> (World, u16, u16) {
        let mut w = World::new(21);
        let a = w.join("a", 0);
        let b = w.join("b", 0);
        // Into the fight, then side by side on open ground.
        while w.phase != Phase::Fight {
            w.step();
        }
        w.players.retain(|p| !p.bot);
        let [x, z] = [0.0f32, 0.0];
        for (k, dx) in [(0usize, 0.0f32), (1, 4.0)] {
            let p = &mut w.players[k];
            p.body.glide = false;
            p.body.ground = true;
            p.body.p = [x + dx, w.map.height(x + dx, z), z];
            p.yaw = 0;
            p.pitch = 0;
        }
        (w, a, b)
    }

    #[test]
    fn a_ward_takes_the_hurt_first_and_mend_heals() {
        let (mut w, a, b) = duel();
        let mut ev = Vec::new();
        w.players[1].slots[2] = Some(Slot {
            spell: spell::WARD,
            rank: 1,
        });
        cast(&mut w, 1, 2, &mut ev);
        let shield = w.find(b).unwrap().shield;
        assert!(shield >= 35);
        w.hurt(a, b, 20, &mut ev);
        assert_eq!(w.find(b).unwrap().hp, HEALTH, "the ward took it");
        w.hurt(a, b, 40, &mut ev);
        let hp = w.find(b).unwrap().hp;
        assert!(hp < HEALTH);
        w.players[1].slots[3] = Some(Slot {
            spell: spell::MEND,
            rank: 1,
        });
        cast(&mut w, 1, 3, &mut ev);
        for _ in 0..MEND_TICKS {
            tick(&mut w, &mut ev);
            w.tick += 1;
        }
        assert!(w.find(b).unwrap().hp > hp, "mended");
    }

    #[test]
    fn gust_throws_back_and_blink_moves_you() {
        let (mut w, _, b) = duel();
        let mut ev = Vec::new();
        w.players[0].slots[2] = Some(Slot {
            spell: spell::GUST,
            rank: 1,
        });
        cast(&mut w, 0, 2, &mut ev);
        let v = w.find(b).unwrap().body.v;
        assert!(v[0] > 5.0 && v[1] > 0.0, "thrown away and up: {v:?}");
        let before = w.players[0].body.p;
        w.players[0].slots[3] = Some(Slot {
            spell: spell::BLINK,
            rank: 1,
        });
        cast(&mut w, 0, 3, &mut ev);
        let after = w.players[0].body.p;
        assert!(after != before, "blinked");
        assert!(w.players[0].cds[3] > 0, "and now it cools down");
        cast(&mut w, 0, 3, &mut ev);
        assert_eq!(w.players[0].body.p, after, "not twice");
    }
}
