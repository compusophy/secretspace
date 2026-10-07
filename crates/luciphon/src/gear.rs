//! Gear: a wand, a robe and a charm worn, up to twelve pieces carried
//! (`laws::BAG`), crafted from what you gather within reach of a light (the
//! Luciphon or your hearth), or found where monsters fall. What is worn
//! adds up (`Worn`), and the world applies it: damage, reach and haste for
//! the wand; armor, Flame and breath for the rest. The table is
//! `laws::GEAR`.

use engine::fixed::{len, Fx};

use crate::laws::{Gear, BAG, CRAFT_NEAR, GEAR};
use crate::world::{Event, Pickup, World};

/// The gear with this id (0 is none).
pub fn get(id: u8) -> Option<&'static Gear> {
    GEAR.get((id as usize).checked_sub(1)?)
}

/// What everything worn adds up to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Worn {
    pub bolt: i32,
    pub reach: i32,
    pub haste: u8,
    pub armor: i32,
    pub flame: i32,
    pub breath: i32,
}

pub fn worn(gear: [u8; 3]) -> Worn {
    let mut w = Worn::default();
    for g in gear.iter().filter_map(|&id| get(id)) {
        w.bolt += g.bolt;
        w.reach += g.reach;
        w.haste = w.haste.saturating_add(g.haste);
        w.armor += g.armor;
        w.flame += g.flame;
        w.breath += g.breath;
    }
    w.armor = w.armor.min(600);
    w
}

impl World {
    /// Whether Lumen i stands near a light to craft by: the Luciphon, or
    /// its own hearth.
    pub fn by_a_light(&self, i: usize) -> bool {
        let lum = &self.lumens[i];
        let (x, y) = lum.pos();
        let near = |cx: Fx, cy: Fx| len(x.sub(cx), y.sub(cy)) <= Fx::int(CRAFT_NEAR);
        let half = Fx::HALF;
        near(half, half)
            || self
                .claim(lum.claim)
                .and_then(|c| c.hearth)
                .is_some_and(|(hx, hy)| near(Fx::int(hx).add(half), Fx::int(hy).add(half)))
    }

    /// Craft gear `id`: its materials spent, and the skill it needs met,
    /// near a light; it goes in the bag. Whether it was made.
    pub fn craft(&mut self, i: usize, id: u8) -> bool {
        let Some(g) = get(id) else { return false };
        let Some((wood, stone, glim, skill, level)) = g.craft else {
            return false;
        };
        let lum = &self.lumens[i];
        let have = crate::build::level(lum.xp[skill as usize % 7]);
        let ok = lum.wood >= wood
            && lum.stone >= stone
            && lum.glim >= glim
            && have >= level as u32
            && lum.bag.len() < BAG
            && self.by_a_light(i);
        let who = lum.id;
        if !ok {
            self.events.push(Event::Refused { id: who });
            return false;
        }
        let lum = &mut self.lumens[i];
        lum.wood -= wood;
        lum.stone -= stone;
        lum.glim -= glim;
        lum.me.body.load = lum.materials();
        lum.bag.push(id);
        self.events.push(Event::Got {
            id: who,
            item: id,
            made: true,
        });
        self.count("crafted");
        true
    }

    /// Wear the bag's piece `k`, in place of whatever was in its slot.
    pub fn equip(&mut self, i: usize, k: usize) -> bool {
        let lum = &mut self.lumens[i];
        let Some(g) = lum.bag.get(k).and_then(|&id| get(id)) else {
            return false;
        };
        let slot = g.slot as usize % 3;
        let was = lum.gear[slot];
        lum.gear[slot] = lum.bag[k];
        if was == 0 {
            lum.bag.remove(k);
        } else {
            lum.bag[k] = was;
        }
        true
    }

    /// Drop the bag's piece `k` at your feet, for anyone to take.
    pub fn discard(&mut self, i: usize, k: usize) -> bool {
        if k >= self.lumens[i].bag.len() {
            return false;
        }
        let item = self.lumens[i].bag.remove(k);
        let (x, y) = self.lumens[i].pos();
        let id = self.new_id();
        self.pickups.push(Pickup {
            id,
            x,
            y,
            item,
            ..Pickup::default()
        });
        true
    }

    /// Each tick: what is worn shapes the wand and the breath.
    pub(crate) fn wear(&mut self) {
        for lum in &mut self.lumens {
            let w = worn(lum.gear);
            lum.me.act.haste = w.haste;
            lum.me.body.regen = lum.regen_base + w.breath;
        }
    }
}

impl crate::world::Lumen {
    /// The most Flame it can hold, with what it wears.
    pub fn max_flame(&self, l: &crate::laws::Laws) -> i32 {
        l.flame + worn(self.gear).flame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::laws::{LAWS, STARTING_GEAR};

    #[test]
    fn gear_is_crafted_by_a_light_worn_and_dropped() {
        let mut w = World::new(LAWS, 4);
        let id = w.spawn("smith", 5, None);
        let i = w.index(id).unwrap();
        assert_eq!(w.lumens[i].gear, STARTING_GEAR);
        // An oak wand: 40 wood, 5 glim, hewing 2.
        let oak = 2;
        assert!(!w.craft(i, oak), "nothing to make it with");
        w.lumens[i].wood = 50;
        w.lumens[i].glim = 10;
        w.lumens[i].xp[0] = 1_000_000;
        assert!(w.craft(i, oak), "at the Luciphon, with enough");
        assert_eq!((w.lumens[i].wood, w.lumens[i].glim), (10, 5));
        assert_eq!(w.lumens[i].bag, vec![oak]);
        // Far from any light: no.
        w.lumens[i].me.body.x = Fx::int(30);
        w.lumens[i].wood = 50;
        assert!(!w.craft(i, oak));
        // Worn: the birch wand goes back in the bag.
        assert!(w.equip(i, 0));
        assert_eq!(
            (w.lumens[i].gear[0], w.lumens[i].bag.clone()),
            (oak, vec![1])
        );
        assert_eq!(worn(w.lumens[i].gear).bolt, 2_000);
        // Dropped, and taken back by walking over it.
        assert!(w.discard(i, 0));
        assert!(w.lumens[i].bag.is_empty());
        w.step();
        assert_eq!(w.lumens[i].bag, vec![1]);
        // Kept across a save, and so is a piece on the ground.
        w.lumens[i].bag.push(14);
        assert!(w.discard(i, 1));
        let mut back = World::new(LAWS, 4);
        back.load(&w.save()).unwrap();
        let d = &back.dreamers[&5];
        assert_eq!((d.gear[0], d.bag.clone()), (oak, vec![1]));
        assert!(back.pickups.iter().any(|p| p.item == 14));
    }

    #[test]
    fn every_piece_is_well_formed() {
        for (k, g) in GEAR.iter().enumerate() {
            assert!(g.slot < 3 && g.rarity < 4, "{}", g.name);
            assert!(!g.name.is_empty() && g.name.len() <= 18);
            assert_eq!(get(k as u8 + 1), Some(g));
        }
        assert!(get(0).is_none() && get(GEAR.len() as u8 + 1).is_none());
    }
}
