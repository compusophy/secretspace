//! Gathering and farming (§8). A node (birch, oak, rock, glow-moss,
//! crystal) gives on every strike until it is bare, then regrows. Each
//! rings every 30 ticks on its own phase: a strike landing within 2 ticks
//! of a ring is resonant and gives double, and three in a row ring it out
//! for a bonus. Staying planted by a node keeps striking it, slowly. A
//! Planter on lit land grows Sunwheat, ripe in 15 minutes, struck to
//! harvest.

use engine::fixed::Fx;

use crate::island::Ring;
use crate::laws::Laws;
use crate::tiles::{obj, Tile, Tiles};
use crate::world::{Event, World};

/// A node someone has struck since it was whole.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Node {
    /// Strikes it has left; when it grows back (0: not bare).
    pub left: u32,
    pub regrow: u32,
}

/// Sunwheat in a planter: when it was planted, by whose claim.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Crop {
    pub planted: u32,
    pub claim: u16,
}

/// What a node is: what it gives (0 wood, 1 stone, 2 glim), how much a
/// strike, how many strikes, how long it takes to regrow.
pub fn kind(o: u8, l: &Laws) -> Option<(usize, u32, u32, u32)> {
    Some(match o {
        obj::BIRCH => (0, l.birch_yield, l.birch_strikes, l.birch_regrow),
        obj::OAK => (0, l.oak_yield, l.oak_strikes, l.oak_regrow),
        obj::ROCK => (1, l.rock_yield, l.rock_strikes, l.rock_regrow),
        obj::GLOWMOSS => (2, l.moss_yield, l.moss_strikes, l.moss_regrow),
        obj::CRYSTAL => (2, l.crystal_yield, l.crystal_strikes, l.crystal_regrow),
        _ => return None,
    })
}

/// A node's phase: the tick offset of its rings (from the world's seed).
pub fn phase(seed: u64, idx: u16) -> u32 {
    let h = engine::rng::splitmix(seed ^ (idx as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
    (h % 30) as u32
}

/// Whether a node at `idx` rings within `window` ticks of `tick`.
pub fn resonant(seed: u64, idx: u16, tick: u32, every: u32, window: u32) -> bool {
    let p = (tick + phase(seed, idx)) % every.max(1);
    p <= window || every - p <= window
}

impl World {
    /// Change a tile, and remember it changed (for the views).
    pub fn set_tile(&mut self, idx: u16, t: Tile) {
        if let Some(cur) = self.tiles.t.get_mut(idx as usize) {
            if *cur != t {
                *cur = t;
                self.dirty.push(idx);
            }
        }
    }

    pub fn tile(&self, idx: u16) -> Tile {
        self.tiles.t.get(idx as usize).copied().unwrap_or_default()
    }

    /// A node or a crop within reach in front of Lumen i, nearest first.
    pub fn node_in_reach(&self, i: usize) -> Option<u16> {
        let me = &self.lumens[i];
        let (x, y) = me.pos();
        let (ux, uy) = engine::fixed::unit(me.me.act.aim);
        let reach = self.laws.gather_reach;
        let mut best: Option<(Fx, u16)> = None;
        let (cx, cy) = (x.floor(), y.floor());
        for ty in cy - 2..=cy + 2 {
            for tx in cx - 2..=cx + 2 {
                let Some(idx) = Tiles::index(tx, ty) else {
                    continue;
                };
                let t = self.tiles.t[idx];
                let harvest = t.obj == obj::RIPE || t.obj == obj::WILTED;
                if !obj::node(t.obj) && !harvest {
                    continue;
                }
                let (dx, dy) = (
                    Fx::int(tx).add(Fx::HALF).sub(x),
                    Fx::int(ty).add(Fx::HALF).sub(y),
                );
                let d = engine::fixed::len(dx, dy);
                // In front: the tile's centre is ahead of the body.
                let ahead = dx.mul(ux).add(dy.mul(uy));
                if d <= reach && ahead > Fx::milli(-200) && best.is_none_or(|b| d < b.0) {
                    best = Some((d, idx as u16));
                }
            }
        }
        best.map(|b| b.1)
    }

    /// Lumen i strikes the node (or crop) at idx. Whether it gave.
    pub fn strike_node(&mut self, i: usize, idx: u16, dwelling: bool) -> bool {
        let t = self.tile(idx);
        if t.obj == obj::RIPE || t.obj == obj::WILTED {
            return self.harvest(i, idx);
        }
        let l = self.laws.clone();
        let Some((what, base, strikes, regrow)) = kind(t.obj, &l) else {
            return false;
        };
        let (tx, ty) = Tiles::at_index(idx as usize);
        let mult = match crate::island::ring(&l, tx, ty) {
            Ring::Dim => l.dim_mult,
            Ring::Rim => l.rim_mult,
            _ => 1000,
        };
        let res = !dwelling && resonant(self.seed, idx, self.tick, l.ring_every, l.ring_window);
        let id = self.lumens[i].id;
        let lum = &mut self.lumens[i];
        let mut amount = base as i32 * mult * if res { 2 } else { 1 };
        let mut out = false;
        if res {
            if lum.rings.0 == idx {
                lum.rings.1 += 1;
            } else {
                lum.rings = (idx, 1);
            }
            if lum.rings.1 >= 3 {
                lum.rings = (idx, 0);
                out = true;
                let bonus = if what == 2 {
                    l.ring_out_glim
                } else {
                    l.ring_out_material
                };
                amount += bonus as i32 * 1000;
            }
        } else {
            lum.rings = (0, 0);
        }
        lum.frac[what] += amount;
        let whole = (lum.frac[what] / 1000).max(0) as u32;
        lum.frac[what] -= whole as i32 * 1000;
        let room = match what {
            2 => l.glim_max.saturating_sub(lum.glim),
            _ => l.carry_max.saturating_sub(lum.materials()),
        };
        let got = whole.min(room);
        match what {
            0 => lum.wood += got,
            1 => lum.stone += got,
            _ => lum.glim += got,
        }
        lum.me.body.load = lum.materials();
        lum.dwell = Some((idx, self.tick + l.dwell_every));
        let skill = match what {
            0 => 0,
            1 => 1,
            _ => {
                if t.obj == obj::CRYSTAL {
                    1
                } else {
                    0
                }
            }
        };
        let xp = if what == 2 { l.xp_glim } else { l.xp_material } * got * if res { 2 } else { 1 };
        self.gain(i, skill, xp);
        // One strike fewer; bare at none.
        let n = self.nodes.entry(idx).or_insert(Node {
            left: strikes,
            regrow: 0,
        });
        n.left = n.left.saturating_sub(1);
        if n.left == 0 {
            let time = if out {
                regrow as i64 * l.ring_out_regrow as i64 / 1000
            } else {
                regrow as i64
            };
            n.regrow = self.tick + time as u32;
            self.set_tile(
                idx,
                Tile {
                    obj: t.obj | obj::BARE,
                    ..t
                },
            );
        }
        self.events.push(Event::Gather {
            id,
            idx,
            resonant: res,
            out,
        });
        self.count(if res { "resonant strike" } else { "gather" });
        if out {
            self.count("ring-out");
        }
        true
    }

    /// Strike a ripe crop: Sunwheat into the bag, the planter empty again.
    fn harvest(&mut self, i: usize, idx: u16) -> bool {
        let t = self.tile(idx);
        let mut n = self.laws.wheat_harvest;
        if t.obj == obj::WILTED {
            n /= 2;
        }
        let lum = &mut self.lumens[i];
        let room = self.laws.carry_max.saturating_sub(lum.materials());
        lum.wheat += n.min(room);
        lum.me.body.load = lum.materials();
        self.crops.remove(&idx);
        self.set_tile(
            idx,
            Tile {
                obj: obj::PLANTER,
                ..t
            },
        );
        let xp = self.laws.xp_crop;
        self.gain(i, 3, xp);
        let id = self.lumens[i].id;
        self.events.push(Event::Gather {
            id,
            idx,
            resonant: false,
            out: false,
        });
        self.count("harvest");
        true
    }

    /// Dwelling: a Lumen planted beside the node it struck keeps striking
    /// it, slowly and never resonant. Any input stops it.
    pub(crate) fn dwell(&mut self) {
        for i in 0..self.lumens.len() {
            let Some((idx, at)) = self.lumens[i].dwell else {
                continue;
            };
            let still = self.lumens[i].idle > 0 && self.lumens[i].me.body.speed() == Fx::ZERO;
            if !still || !self.lumens[i].alive() {
                if self.lumens[i].idle == 0 {
                    self.lumens[i].dwell = None;
                }
                continue;
            }
            if self.tick >= at {
                let in_reach = self.node_in_reach(i) == Some(idx);
                if !in_reach || !self.strike_node(i, idx, true) {
                    self.lumens[i].dwell = None;
                }
            }
        }
    }

    /// Bare nodes grow back; crops ripen on lit land, and wilt.
    pub(crate) fn grow(&mut self) {
        let tick = self.tick;
        let back: Vec<u16> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.left == 0 && n.regrow <= tick)
            .map(|(&i, _)| i)
            .collect();
        for idx in back {
            self.nodes.remove(&idx);
            let t = self.tile(idx);
            self.set_tile(
                idx,
                Tile {
                    obj: t.obj & !obj::BARE,
                    ..t
                },
            );
        }
        let (ripe, full) = (self.laws.wheat_ripe, self.laws.wheat_full);
        let crops: Vec<(u16, Crop)> = self.crops.iter().map(|(&i, &c)| (i, c)).collect();
        for (idx, c) in crops {
            let t = self.tile(idx);
            // An unfed claim's crops stand still.
            if !self.fed(c.claim) {
                if let Some(c) = self.crops.get_mut(&idx) {
                    c.planted = c.planted.wrapping_add(1);
                }
                continue;
            }
            let age = tick.wrapping_sub(c.planted);
            let stage = if age >= ripe + full {
                obj::WILTED
            } else if age >= ripe {
                obj::RIPE
            } else {
                obj::SPROUT
            };
            if t.obj != stage {
                self.set_tile(idx, Tile { obj: stage, ..t });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_node_rings_once_in_thirty_ticks() {
        for idx in [1u16, 77, 9000] {
            let rings = (0..30).filter(|&t| resonant(5, idx, t, 30, 0)).count();
            assert_eq!(rings, 1);
            let near = (0..30).filter(|&t| resonant(5, idx, t, 30, 2)).count();
            assert_eq!(near, 5);
        }
    }
}
