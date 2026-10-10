//! Loot and levels. Spell cubes lie across the island when a match begins,
//! loose and in pairs at the caches (by each place, at the ruins). Running
//! over a cube learns its spell into your spellbook, or ranks it up if you
//! know it, and gives XP; a spell new to you goes into a free slot of its
//! kind, and the spellbook (the page's B) puts any spell you know in any
//! slot of its kind. The knocked out drop every spell they knew, each at
//! its rank, and their XP goes to whoever felled them. XP from cubes,
//! damage and knockouts raises a wizard's level: more health, more power.

use crate::laws::*;
use crate::world::{Event, Phase, Player, Slot, World};

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

/// The health a level gained heals: its share of the rise (to the
/// nearest whole).
pub fn level_heal() -> i32 {
    (HEALTH_PER_LEVEL * LEVEL_HEAL + 50) / 100
}

/// Damage as a level deals it (to the nearest whole).
pub fn level_scale(level: u8, v: i32) -> i32 {
    (v * (100 + POWER_PER_LEVEL * (level.max(1) as i32 - 1)) + 50) / 100
}

/// A spell's power at a rank (to the nearest whole).
pub fn power(spell: u8, rank: u8) -> i32 {
    let s = &SPELLS[spell as usize % SPELLS.len()];
    (s.power * (100 + RANK_POWER * (rank.max(1) as i32 - 1)) + 50) / 100
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

/// XP for `who`: a level gained raises its health, and heals it by a
/// share of the rise (a practice dummy never levels).
pub fn gain(w: &mut World, who: u16, xp: u32, ev: &mut Vec<Event>) {
    let range = w.practice.is_some();
    let Some(p) = w
        .players
        .iter_mut()
        .find(|p| p.id == who && p.alive && !(range && p.bot))
    else {
        return;
    };
    if p.level >= MAX_LEVEL || w.phase != Phase::Fight {
        return;
    }
    p.xp += xp;
    while p.xp >= XP_PER_LEVEL && p.level < MAX_LEVEL {
        p.xp -= XP_PER_LEVEL;
        p.level += 1;
        p.hp += level_heal();
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
    let y = w.map.floor(x, z, at[1] + 0.5).max(SEA);
    w.scrolls.push(Scroll {
        id,
        spell,
        rank,
        p: [x, y, z],
    });
    w.loot_dirty = true;
}

/// Cubes across the island: a pair at each cache (those the places keep
/// first, then a third of the rest at the ruins), and more lying loose.
pub fn scatter(w: &mut World) {
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
    let mut cached: Vec<[f32; 2]> = Vec::new();
    while cached.len() < CACHES && tries < CACHES * 30 {
        tries += 1;
        let given = caches.get(tries - 1).copied();
        let [x, z] = if let Some(at) = given {
            at
        } else if !ruins.is_empty() && cached.len().is_multiple_of(3) {
            let r = ruins[(w.rng.next_u64() % ruins.len() as u64) as usize];
            let a = unit(w) * std::f32::consts::TAU;
            [r[0] + a.cos() * CACHE_RUIN, r[2] + a.sin() * CACHE_RUIN]
        } else {
            w.map.spot(&mut w.rng)
        };
        // A place's own cache may stand on something (the causeway's
        // crown); others lie clear on the ground.
        if !w.map.land(x, z) || (given.is_none() && w.map.near(x, z, 1.0).next().is_some()) {
            continue;
        }
        if cached
            .iter()
            .any(|c| (c[0] - x).powi(2) + (c[1] - z).powi(2) < CACHE_APART * CACHE_APART)
        {
            continue;
        }
        cached.push([x, z]);
        let y = match given {
            Some(_) => w.map.floor(x, z, f32::MAX),
            None => w.map.height(x, z),
        };
        for _ in 0..CUBES_A_CACHE {
            let (spell, rank) = any_cube(w);
            drop_scroll(w, spell, rank, [x, y, z], 1.2);
        }
    }
    // And cubes lying loose.
    for _ in 0..LOOSE_CUBES {
        let [x, z] = w.map.spot(&mut w.rng);
        let (spell, rank) = any_cube(w);
        drop_scroll(w, spell, rank, [x, 0.0, z], 0.0);
    }
    w.loot_dirty = true;
}

/// A cube's spell, and its rank (now and then the second).
fn any_cube(w: &mut World) -> (u8, u8) {
    let spell = (w.rng.next_u64() % SPELLS.len() as u64) as u8;
    let rank = if w.rng.next_u64().is_multiple_of(RARE_SCROLL) {
        2
    } else {
        1
    };
    (spell, rank)
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
        p.cds[k] = p.spell_cds[i];
    } else if p.bot {
        let rank_of = |k: usize| p.slots[k].map_or(0, |s| s.rank);
        let k = if rank_of(a) <= rank_of(b) { a } else { b };
        if rank_of(k) < now {
            p.slots[k] = Some(Slot { spell, rank: now });
            p.cds[k] = p.spell_cds[i];
        }
    }
}

/// The spellbook: put a spell you know in a slot of its kind (if it is
/// in the other slot, the two trade places). A spell put in waits a
/// moment before it can be cast, or out its own cooldown if that is
/// longer (it keeps it from slot to slot).
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
        p.cds[slot] = p.spell_cds[i].max(EQUIP_COOLDOWN);
    }
    true
}

/// Cubes picked up by whoever runs over them.
pub fn touch(w: &mut World, ev: &mut Vec<Event>) {
    if w.phase != Phase::Fight {
        return;
    }
    let range = w.practice.is_some();
    for k in 0..w.players.len() {
        let p = &w.players[k];
        // Practice dummies leave the cubes to you.
        if !p.alive || !p.entrant || p.body.glide || (range && p.bot) {
            continue;
        }
        let (at, id) = (p.body.p, p.id);
        let near = |q: [f32; 3], r: f32| {
            (q[0] - at[0]).powi(2) + (q[2] - at[2]).powi(2) < r * r && (q[1] - at[1]).abs() < 2.5
        };
        let Some(s) = w.scrolls.iter().position(|s| near(s.p, SCROLL_REACH)) else {
            continue;
        };
        let sc = w.scrolls.remove(s);
        learn(&mut w.players[k], sc.spell, sc.rank);
        w.loot_dirty = true;
        gain(w, id, XP_CUBE, ev);
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
    fn a_spell_keeps_its_cooldown_from_slot_to_slot() {
        let mut w = World::new(3);
        let id = w.join("t", 0);
        let k = w.players.iter().position(|p| p.id == id).unwrap();
        let p = &mut w.players[k];
        p.book = [0; SPELLS.len()];
        p.slots = [None; 4];
        learn(p, spell::MEND, 1);
        learn(p, spell::BLINK, 1);
        learn(p, spell::WARD, 1);
        assert_eq!(p.slots[2].map(|s| s.spell), Some(spell::MEND));
        let aim = crate::spells::Aim::of(p);
        crate::spells::cast(&mut w, k, 2, aim, &mut Vec::new());
        let p = &mut w.players[k];
        let mend = cooldown(spell::MEND, 1);
        assert_eq!(p.cds[2], mend);
        // Ward into Mend's slot, then Mend into the other.
        assert!(equip(p, 2, spell::WARD));
        assert_eq!(p.cds[2], EQUIP_COOLDOWN, "Ward is not charged Mend's");
        assert!(equip(p, 3, spell::MEND));
        assert_eq!(p.cds[3], mend, "and Mend still cools down");
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
        // Small numbers grow too: rounded, not cut off.
        let shard = power(spell::FROST, 1);
        assert!(level_scale(4, shard) > level_scale(1, shard));
        // A level gained heals its share of the rise, rounded.
        assert!(
            (level_heal() * 100 - HEALTH_PER_LEVEL * LEVEL_HEAL).abs() <= 50,
            "{} of {HEALTH_PER_LEVEL} at {LEVEL_HEAL}%",
            level_heal()
        );
        let mut w = World::new(3);
        let id = w.join("t", 0);
        w.phase = Phase::Fight;
        let k = w.players.iter().position(|p| p.id == id).unwrap();
        w.players[k].hp = HEALTH / 2;
        gain(&mut w, id, XP_PER_LEVEL, &mut Vec::new());
        let p = &w.players[k];
        assert_eq!((p.level, p.max_hp()), (2, max_hp(2)));
        assert_eq!(p.hp, HEALTH / 2 + level_heal(), "healed by the level");
    }
}
