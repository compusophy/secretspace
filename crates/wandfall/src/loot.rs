//! Loot and levels. Spell cubes lie loose across the island when a match
//! begins, and chests stand about it; walking into a chest opens it: XP,
//! and cubes spill out around it. Running over a cube learns its spell
//! into your spellbook, or ranks it up if you know it; a spell new to you
//! goes into a free slot of its kind, and the spellbook (the page's B)
//! puts any spell you know in any slot of its kind. The knocked out drop
//! every spell they knew, each at its rank, and their XP goes to whoever
//! felled them. XP from chests, damage and knockouts raises a wizard's
//! level: more health, more power.

use crate::laws::*;
use crate::world::{Event, Phase, Player, Slot, World};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chest {
    pub id: u16,
    pub p: [f32; 3],
    pub open: bool,
}

/// A spell cube lying on the island.
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

/// Chests across the island: those the places keep first, then a third
/// of the rest at the ruins.
pub fn scatter(w: &mut World) {
    w.chests.clear();
    w.scrolls.clear();
    let caches = w.map.caches.clone();
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
        let [x, z] = if let Some(&at) = caches.get(tries - 1) {
            at
        } else if !ruins.is_empty() && w.chests.len().is_multiple_of(3) {
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
    // And cubes lying loose, away from the chests.
    for _ in 0..LOOSE_CUBES {
        let [x, z] = w.map.spot(&mut w.rng);
        let spell = (w.rng.next_u64() % SPELLS.len() as u64) as u8;
        let rank = if w.rng.next_u64().is_multiple_of(RARE_SCROLL) {
            2
        } else {
            1
        };
        drop_scroll(w, spell, rank, [x, 0.0, z], 0.0);
    }
    w.loot_dirty = true;
}

/// A knocked-out wizard's spells, every one it knew at its rank, lying
/// where it fell.
pub fn drop_spells(w: &mut World, k: usize) {
    let at = w.players[k].body.p;
    let book = std::mem::replace(&mut w.players[k].book, [0; SPELLS.len()]);
    w.players[k].slots = [None; 4];
    for (spell, &rank) in book.iter().enumerate() {
        if rank > 0 {
            drop_scroll(w, spell as u8, rank, at, 2.2);
        }
    }
}

/// Learn a spell from a cube, or rank it up if it is known: a spell new
/// to you goes into a free slot of its kind. Bots keep the best of each
/// kind in their slots.
pub fn learn(p: &mut Player, spell: u8, rank: u8) {
    let i = spell as usize % SPELLS.len();
    let spell = i as u8;
    let was = p.book[i];
    p.book[i] = if was == 0 {
        rank.clamp(1, MAX_RANK)
    } else {
        (was + 1).max(rank).min(MAX_RANK)
    };
    let now = p.book[i];
    if let Some(s) = p.slots.iter_mut().flatten().find(|s| s.spell == spell) {
        s.rank = now;
        return;
    }
    let [a, b] = slots_of(spell);
    if let Some(k) = [a, b].into_iter().find(|&k| p.slots[k].is_none()) {
        p.slots[k] = Some(Slot { spell, rank: now });
        p.cds[k] = 0;
    } else if p.bot {
        let rank_of = |k: usize| p.slots[k].map_or(0, |s| s.rank);
        let k = if rank_of(a) <= rank_of(b) { a } else { b };
        if rank_of(k) < now {
            p.slots[k] = Some(Slot { spell, rank: now });
        }
    }
}

/// The spellbook: put a spell you know in a slot of its kind (if it is
/// in the other slot, the two trade places). A spell put in waits a
/// moment before it can be cast.
pub fn equip(p: &mut Player, slot: usize, spell: u8) -> bool {
    let i = spell as usize;
    if slot > 3 || i >= SPELLS.len() || p.book[i] == 0 || !slots_of(spell).contains(&slot) {
        return false;
    }
    if p.slots[slot].is_some_and(|s| s.spell == spell) {
        return true;
    }
    let held = Some(Slot {
        spell,
        rank: p.book[i],
    });
    if let Some(k) = (0..4).find(|&k| k != slot && p.slots[k].is_some_and(|s| s.spell == spell)) {
        p.slots.swap(k, slot);
        p.cds.swap(k, slot);
    } else {
        p.slots[slot] = held;
        p.cds[slot] = p.cds[slot].max(EQUIP_COOLDOWN);
    }
    true
}

/// Chests opened and cubes picked up by whoever stands at them.
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
        let sc = w.scrolls.remove(s);
        learn(&mut w.players[k], sc.spell, sc.rank);
        w.loot_dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;

    #[test]
    fn running_over_cubes_learns_and_ranks_up_and_the_book_equips() {
        let mut w = World::new(3);
        let id = w.join("t", 0);
        let p = w.find_mut(id).unwrap();
        p.book = [0; SPELLS.len()];
        p.slots = [None; 4];
        learn(p, spell::LANCE, 1);
        learn(p, spell::FIREBALL, 1);
        learn(p, spell::LANCE, 1);
        let lance = Some(Slot {
            spell: spell::LANCE,
            rank: 2,
        });
        assert_eq!(p.slots[0], lance, "known: ranked up, in its slot");
        learn(p, spell::FROST, 2);
        assert_eq!(
            p.book[spell::FROST as usize],
            2,
            "learned though the slots are full"
        );
        assert!(!p.slots.iter().flatten().any(|s| s.spell == spell::FROST));
        assert!(equip(p, 1, spell::FROST), "the book puts it in");
        assert_eq!(p.slots[1].unwrap().spell, spell::FROST);
        assert!(p.cds[1] >= EQUIP_COOLDOWN, "and it waits a moment");
        assert!(!equip(p, 2, spell::FROST), "not in a utility slot");
        assert!(!equip(p, 3, spell::WARD), "not a spell it knows");
        assert!(equip(p, 0, spell::FROST), "in the other slot: they trade");
        assert_eq!(p.slots[1], lance);
        for _ in 0..9 {
            learn(p, spell::LANCE, 1);
        }
        assert_eq!(p.book[spell::LANCE as usize], MAX_RANK);
    }

    #[test]
    fn the_fallen_drop_every_spell_they_knew() {
        let mut w = World::new(3);
        let id = w.join("t", 0);
        let k = w.players.iter().position(|p| p.id == id).unwrap();
        w.players[k].book = [0; SPELLS.len()];
        w.players[k].book[spell::LANCE as usize] = 3;
        w.players[k].book[spell::WARD as usize] = 1;
        w.scrolls.clear();
        drop_spells(&mut w, k);
        let mut dropped: Vec<(u8, u8)> = w.scrolls.iter().map(|s| (s.spell, s.rank)).collect();
        dropped.sort();
        assert_eq!(dropped, vec![(spell::LANCE, 3), (spell::WARD, 1)]);
        assert_eq!(w.players[k].book, [0; SPELLS.len()]);
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
