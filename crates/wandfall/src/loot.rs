//! Loot and levels. Chests stand across the island when a match begins;
//! walking into one opens it: XP, and scrolls of spells spill out around
//! it. Walking over a scroll takes it: into a free slot of its kind, or
//! ranking up the same spell if you have it; over a full set, only when
//! asked (the page's take key), and the spell it replaces is dropped. The
//! knocked out drop every spell they carried. XP from chests, damage and
//! knockouts raises a wizard's level: more health, more power.

use crate::laws::*;
use crate::world::{Event, Phase, Player, Slot, World};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chest {
    pub id: u16,
    pub p: [f32; 3],
    pub open: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scroll {
    pub id: u16,
    pub spell: u8,
    pub rank: u8,
    pub p: [f32; 3],
}

pub fn max_hp(level: u8) -> i32 {
    HEALTH + HEALTH_PER_LEVEL * (level.max(1) as i32 - 1)
}

/// Damage as a level deals it.
pub fn level_scale(level: u8, v: i32) -> i32 {
    v * (100 + POWER_PER_LEVEL * (level.max(1) as i32 - 1)) / 100
}

/// A spell's power at a rank.
pub fn power(spell: u8, rank: u8) -> i32 {
    let s = &SPELLS[spell as usize % SPELLS.len()];
    s.power * (100 + RANK_POWER * (rank.max(1) as i32 - 1)) / 100
}

/// A spell's cooldown at a rank (ticks).
pub fn cooldown(spell: u8, rank: u8) -> u32 {
    let s = &SPELLS[spell as usize % SPELLS.len()];
    s.cooldown * (100 - RANK_COOLDOWN * (rank.clamp(1, MAX_RANK) as u32 - 1)) / 100
}

/// The slots a kind of spell goes in.
pub fn slots_of(spell: u8) -> [usize; 2] {
    match SPELLS[spell as usize % SPELLS.len()].kind {
        Kind::Offense => [0, 1],
        Kind::Utility => [2, 3],
    }
}

/// XP for `who`: levels gained raise health with them.
pub fn gain(w: &mut World, who: u16, xp: u32, ev: &mut Vec<Event>) {
    let Some(p) = w.players.iter_mut().find(|p| p.id == who && p.alive) else {
        return;
    };
    if p.level >= MAX_LEVEL || w.phase != Phase::Fight {
        return;
    }
    p.xp += xp;
    while p.xp >= XP_PER_LEVEL && p.level < MAX_LEVEL {
        p.xp -= XP_PER_LEVEL;
        p.level += 1;
        p.hp += HEALTH_PER_LEVEL;
        ev.push(Event::Level {
            who,
            level: p.level,
        });
    }
    if p.level >= MAX_LEVEL {
        p.xp = 0;
    }
}

fn unit(w: &mut World) -> f32 {
    (w.rng.next_u64() >> 40) as f32 / (1u64 << 24) as f32
}

fn fresh_id(w: &mut World) -> u16 {
    let id = w.next_loot;
    w.next_loot = w.next_loot.wrapping_add(1).max(1);
    id
}

/// A scroll lying near `at`.
pub fn drop_scroll(w: &mut World, spell: u8, rank: u8, at: [f32; 3], spread: f32) {
    let a = unit(w) * std::f32::consts::TAU;
    let d = spread * (0.4 + 0.6 * unit(w));
    let (x, z) = (at[0] + a.cos() * d, at[2] + a.sin() * d);
    let lim = MAP_HALF - 2.0;
    let (x, z) = (x.clamp(-lim, lim), z.clamp(-lim, lim));
    let id = fresh_id(w);
    let y = w.map.height(x, z).max(SEA);
    w.scrolls.push(Scroll {
        id,
        spell,
        rank,
        p: [x, y, z],
    });
    w.loot_dirty = true;
}

/// Chests across the island, a third of them at the ruins.
pub fn scatter(w: &mut World) {
    w.chests.clear();
    w.scrolls.clear();
    let ruins: Vec<[f32; 3]> = w
        .map
        .props
        .iter()
        .filter(|p| p.kind == crate::map::Kind::Pillar)
        .map(|p| [p.x, p.y, p.z])
        .collect();
    let mut tries = 0;
    while w.chests.len() < CHESTS && tries < CHESTS * 30 {
        tries += 1;
        let [x, z] = if !ruins.is_empty() && w.chests.len().is_multiple_of(3) {
            let r = ruins[(w.rng.next_u64() % ruins.len() as u64) as usize];
            let a = unit(w) * std::f32::consts::TAU;
            [r[0] + a.cos() * 2.5, r[2] + a.sin() * 2.5]
        } else {
            w.map.spot(&mut w.rng)
        };
        if !w.map.land(x, z) || w.map.near(x, z, 1.0).next().is_some() {
            continue;
        }
        if w.chests
            .iter()
            .any(|c| (c.p[0] - x).powi(2) + (c.p[2] - z).powi(2) < 100.0)
        {
            continue;
        }
        let id = fresh_id(w);
        let y = w.map.height(x, z);
        w.chests.push(Chest {
            id,
            p: [x, y, z],
            open: false,
        });
    }
    w.loot_dirty = true;
}

/// A knocked-out wizard's spells, lying where it fell.
pub fn drop_spells(w: &mut World, k: usize) {
    let at = w.players[k].body.p;
    let held: Vec<Slot> = w.players[k].slots.iter().flatten().copied().collect();
    w.players[k].slots = [None; 4];
    for s in held {
        drop_scroll(w, s.spell, s.rank, at, 1.8);
    }
}

/// Take a scroll if it can be taken: what it replaced, if anything.
/// `force` takes it over the weakest spell of its kind.
pub fn take(p: &mut Player, spell: u8, rank: u8, force: bool) -> Option<Option<Slot>> {
    if let Some(s) = p.slots.iter_mut().flatten().find(|s| s.spell == spell) {
        s.rank = (s.rank + 1).max(rank).min(MAX_RANK);
        return Some(None);
    }
    let [a, b] = slots_of(spell);
    for k in [a, b] {
        if p.slots[k].is_none() {
            p.slots[k] = Some(Slot { spell, rank });
            p.cds[k] = 0;
            return Some(None);
        }
    }
    if !force {
        return None;
    }
    let weakest = if p.slots[a].map_or(0, |s| s.rank) <= p.slots[b].map_or(0, |s| s.rank) {
        a
    } else {
        b
    };
    let old = p.slots[weakest];
    p.slots[weakest] = Some(Slot { spell, rank });
    p.cds[weakest] = 0;
    Some(old)
}

/// Chests opened and scrolls taken by whoever stands at them.
pub fn touch(w: &mut World, ev: &mut Vec<Event>) {
    if w.phase != Phase::Fight {
        return;
    }
    for k in 0..w.players.len() {
        let p = &w.players[k];
        if !p.alive || !p.entrant || p.body.glide {
            continue;
        }
        let (at, id) = (p.body.p, p.id);
        let near = |q: [f32; 3], r: f32| {
            (q[0] - at[0]).powi(2) + (q[2] - at[2]).powi(2) < r * r && (q[1] - at[1]).abs() < 2.5
        };
        if let Some(c) = w
            .chests
            .iter()
            .position(|c| !c.open && near(c.p, CHEST_REACH))
        {
            w.chests[c].open = true;
            w.loot_dirty = true;
            let cp = w.chests[c].p;
            for _ in 0..SCROLLS_A_CHEST {
                let spell = (w.rng.next_u64() % SPELLS.len() as u64) as u8;
                let rank = if w.rng.next_u64().is_multiple_of(RARE_SCROLL) {
                    2
                } else {
                    1
                };
                drop_scroll(w, spell, rank, cp, 1.6);
            }
            gain(w, id, XP_CHEST, ev);
        }
        let Some(s) = w.scrolls.iter().position(|s| near(s.p, SCROLL_REACH)) else {
            continue;
        };
        let sc = w.scrolls[s];
        let p = &mut w.players[k];
        // Bots take what is better than their weakest of its kind.
        let force = p.take
            || (p.bot && {
                let [a, b] = slots_of(sc.spell);
                p.slots[a]
                    .map_or(0, |x| x.rank)
                    .min(p.slots[b].map_or(0, |x| x.rank))
                    < sc.rank
            });
        if let Some(old) = take(p, sc.spell, sc.rank, force) {
            w.scrolls.remove(s);
            w.loot_dirty = true;
            if let Some(o) = old {
                drop_scroll(w, o.spell, o.rank, at, 1.5);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;

    #[test]
    fn duplicates_rank_up_and_full_slots_need_asking() {
        let mut w = World::new(3);
        let id = w.join("t", 0);
        let p = w.find_mut(id).unwrap();
        // Out of the lobby's practice set.
        p.slots = [None; 4];
        assert_eq!(take(p, spell::LANCE, 1, false), Some(None));
        assert_eq!(take(p, spell::FIREBALL, 1, false), Some(None));
        assert_eq!(take(p, spell::LANCE, 1, false), Some(None));
        assert_eq!(
            p.slots[0],
            Some(Slot {
                spell: spell::LANCE,
                rank: 2
            })
        );
        assert_eq!(
            take(p, spell::FROST, 1, false),
            None,
            "both offensive slots are full"
        );
        let old = take(p, spell::FROST, 1, true).unwrap();
        assert_eq!(
            old,
            Some(Slot {
                spell: spell::FIREBALL,
                rank: 1
            }),
            "the weaker goes"
        );
        assert_eq!(take(p, spell::WARD, 3, false), Some(None));
        assert_eq!(
            p.slots[2],
            Some(Slot {
                spell: spell::WARD,
                rank: 3
            })
        );
        for _ in 0..9 {
            take(p, spell::WARD, 1, false);
        }
        assert_eq!(p.slots[2].unwrap().rank, MAX_RANK);
    }

    #[test]
    fn levels_raise_health_and_power() {
        assert_eq!(max_hp(1), HEALTH);
        assert!(max_hp(20) > max_hp(10));
        assert!(level_scale(20, 100) > level_scale(1, 100));
        assert!(power(spell::LANCE, MAX_RANK) > power(spell::LANCE, 1));
        assert!(cooldown(spell::LANCE, MAX_RANK) < cooldown(spell::LANCE, 1));
        assert_eq!(power(spell::WARD, 3), 60, "a rank is a quarter more");
    }
}
