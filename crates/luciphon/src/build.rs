//! Building (§8), the wheel's acts (§4) and skills (§9). Build mode puts
//! a ghost on the tile in front of you; a tap there places the piece after
//! a 0.4 s channel that moving cancels, a hold on your own piece removes it
//! for half. A hearth goes in the Glow or the Dim, 16-48 tiles out, 10
//! from any other, never walling anyone in: a placement that would cut a
//! foreign hearth off from the Sanctum is refused.

use std::collections::VecDeque;

use engine::fixed::{unit, Fx};

use crate::island::Ring;
use crate::land::{core, on_hearth, Bag};
use crate::tiles::{ground, obj, Tile, Tiles, SIZE};
use crate::world::{Event, World};

/// The pieces, in the Build ring's order.
pub const PIECES: [u8; 6] = [
    obj::HEARTH,
    obj::WALL,
    obj::DOOR,
    obj::THORNS,
    obj::LANTERN,
    obj::PLANTER,
];

/// The wheel's slots, from north clockwise.
pub mod slot {
    pub const BUILD: u8 = 0;
    pub const WAVE: u8 = 1;
    pub const KINDLE: u8 = 2;
    pub const CHEER: u8 = 3;
    pub const REKINDLE: u8 = 4;
    pub const SIT: u8 = 5;
    pub const RECALL: u8 = 6;
    pub const BOW: u8 = 7;
}

/// Heart acts on the wire.
pub mod act {
    pub const CHIRP: u8 = 1;
    pub const WHEEL: u8 = 2;
    pub const PIECE: u8 = 3;
    pub const HOME: u8 = 4;
    pub const DONE: u8 = 5;
}

pub fn cost(piece: u8, l: &crate::laws::Laws) -> Bag {
    let b = |glim, wood, stone| Bag {
        glim,
        wood,
        stone,
        wheat: 0,
    };
    match piece {
        obj::HEARTH => b(l.hearth_glim, l.hearth_wood, l.hearth_stone),
        obj::WALL => b(0, l.wall_wood, 0),
        obj::DOOR => b(0, l.door_wood, 0),
        obj::THORNS => b(0, l.thorns_wood, l.thorns_stone),
        obj::LANTERN => b(l.lantern_glim, 0, 0),
        obj::PLANTER => b(0, l.planter_wood, 0),
        _ => Bag::default(),
    }
}

/// XP to reach a skill level: floor(60 (L-1)^2.3).
pub fn xp_for(level: u32) -> u32 {
    (60.0 * ((level.max(1) - 1) as f64).powf(2.3)) as u32
}

pub fn level(xp: u32) -> u32 {
    (1..=99)
        .take_while(|&lv| xp_for(lv) <= xp)
        .last()
        .unwrap_or(1)
}

/// The tile a Lumen's ghost sits on: one tile ahead of it.
pub fn ghost(x: Fx, y: Fx, facing: u16) -> (i32, i32) {
    let (ux, uy) = unit(facing);
    (x.add(ux).floor(), y.add(uy).floor())
}

impl World {
    /// Skill XP; a level gained is an event (the page draws a ring).
    pub fn gain(&mut self, i: usize, skill: usize, xp: u32) {
        let Some(lum) = self.lumens.get_mut(i) else {
            return;
        };
        if lum.bot.is_some() || xp == 0 {
            return;
        }
        let before = level(lum.xp[skill]);
        lum.xp[skill] = lum.xp[skill].saturating_add(xp);
        let after = level(lum.xp[skill]);
        if after > before {
            let id = lum.id;
            self.events.push(Event::Level {
                id,
                skill: skill as u8,
                level: after as u8,
            });
        }
    }

    /// The Heart: a chirp, a wheel slot, a piece, where to return.
    pub fn heart(&mut self, id: u16, a: u8, arg: u8) {
        let Some(i) = self.index(id) else { return };
        if !self.lumens[i].alive() {
            if a == act::HOME {
                self.lumens[i].home = arg != 0;
            }
            return;
        }
        let l = self.laws.clone();
        match (a, arg) {
            (act::CHIRP, _) => self.events.push(Event::Emote { id, what: 0 }),
            (act::WHEEL, slot::BUILD) => {
                self.lumens[i].build = Some(0);
                self.lumens[i].build_idle = 0;
            }
            (act::WHEEL, slot::KINDLE) => {
                let k = !self.lumens[i].kindle;
                self.lumens[i].kindle = k;
                if !k {
                    self.drop_wick(i);
                }
                self.events.push(Event::Kindle { id, on: k });
            }
            (act::WHEEL, slot::REKINDLE) => {
                let lum = &mut self.lumens[i];
                if lum.rekindle == 0 && lum.flame < l.flame {
                    if lum.wheat > 0 {
                        lum.wheat -= 1;
                        lum.rekindle = l.rekindle_ticks;
                    } else if lum.glim >= l.rekindle_glim {
                        lum.glim -= l.rekindle_glim;
                        lum.rekindle = l.rekindle_ticks;
                    }
                    lum.me.body.load = lum.materials();
                }
            }
            (act::WHEEL, slot::RECALL) => {
                let has = self
                    .claim(self.lumens[i].claim)
                    .and_then(|c| c.hearth)
                    .is_some();
                if has {
                    self.lumens[i].recall = l.recall_ticks;
                }
            }
            (act::WHEEL, s) if s < 8 => self.events.push(Event::Emote { id, what: s }),
            (act::PIECE, p) if (p as usize) < PIECES.len() => {
                self.lumens[i].build = Some(PIECES[p as usize]);
                self.lumens[i].build_idle = 0;
            }
            (act::HOME, h) => self.lumens[i].home = h != 0,
            (act::DONE, _) => {
                self.lumens[i].build = None;
                self.lumens[i].channel = None;
            }
            _ => {}
        }
    }

    /// Each tick: Rekindle's heal, Recall's channel, build mode's channel.
    pub(crate) fn acts(&mut self, i: usize, verb: crate::motion::Verb, stick: bool) {
        let l = self.laws.clone();
        let lum = &mut self.lumens[i];
        if lum.rekindle > 0 {
            lum.rekindle -= 1;
            lum.flame =
                (lum.flame + l.rekindle_flame / l.rekindle_ticks.max(1) as i32).min(l.flame);
        }
        if lum.recall > 0 {
            if stick || lum.me.body.speed() > Fx::milli(50) {
                lum.recall = 0;
            } else {
                lum.recall -= 1;
                let (done, claim) = (lum.recall == 0, lum.claim);
                if done {
                    if let Some(h) = self.claim(claim).and_then(|c| c.hearth) {
                        let (x, y) = (Fx::int(h.0).add(Fx::HALF), Fx::int(h.1 + 2).add(Fx::HALF));
                        let lum = &mut self.lumens[i];
                        lum.me.body = crate::motion::Body::at(x, y, &l);
                        lum.me.body.claim = lum.claim;
                        let id = lum.id;
                        self.events.push(Event::Return { id });
                    }
                }
            }
        }
        let lum = &mut self.lumens[i];
        let Some(piece) = lum.build else { return };
        lum.build_idle += 1;
        if lum.build_idle > l.build_idle {
            lum.build = None;
            lum.channel = None;
            return;
        }
        if let Some((idx, p, left, remove)) = lum.channel {
            if stick || lum.me.body.speed() > Fx::milli(50) {
                lum.channel = None;
            } else if left <= 1 {
                lum.channel = None;
                lum.build_idle = 0;
                if remove {
                    self.remove_piece(i, idx);
                } else {
                    self.place(i, idx, p);
                }
            } else {
                lum.channel = Some((idx, p, left - 1, remove));
            }
            return;
        }
        let b = lum.me.body;
        let (gx, gy) = ghost(b.x, b.y, b.facing);
        let Some(idx) = Tiles::index(gx, gy) else {
            return;
        };
        match verb {
            crate::motion::Verb::Tap if piece != 0 => {
                lum.channel = Some((idx as u16, piece, l.build_channel, false));
            }
            crate::motion::Verb::Hold { .. } => {
                let t = self.tiles.t[idx];
                if obj::piece(t.obj) && t.owner() == self.lumens[i].claim && t.owner() != 0 {
                    self.lumens[i].channel = Some((idx as u16, t.obj, l.build_channel, true));
                }
            }
            _ => {}
        }
    }

    /// Whether tiles are walkable from the Sanctum to every foreign
    /// hearth's core, with `extra` tiles counted solid. The hearths cut off.
    fn cut_off(&self, extra: &[(i32, i32)], except: u16) -> Vec<u16> {
        let n = SIZE as usize;
        let half = SIZE / 2;
        let mut seen = vec![false; n * n];
        let mut q = VecDeque::new();
        let open = |x: i32, y: i32| {
            let t = self.tiles.get(x, y);
            !t.void() && !t.solid() && !extra.contains(&(x, y))
        };
        for (x, y) in [(0, 3), (3, 0), (0, -3), (-3, 0)] {
            if let Some(i) = Tiles::index(x, y) {
                seen[i] = true;
                q.push_back((x, y));
            }
        }
        while let Some((x, y)) = q.pop_front() {
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if let Some(i) = Tiles::index(nx, ny) {
                    if !seen[i] && open(nx, ny) {
                        seen[i] = true;
                        q.push_back((nx, ny));
                    }
                }
            }
        }
        let _ = half;
        self.claims
            .iter()
            .filter(|c| c.id != except)
            .filter_map(|c| c.hearth.map(|h| (c.id, h)))
            .filter(|&(_, h)| {
                !core(h)
                    .filter(|&(x, y)| !on_hearth(h, x, y))
                    .any(|(x, y)| Tiles::index(x, y).is_some_and(|i| seen[i]))
            })
            .map(|(id, _)| id)
            .collect()
    }

    /// Whether Lumen i may put this piece at (x, y), and the tiles it needs.
    fn may_place(&self, i: usize, x: i32, y: i32, piece: u8) -> Option<Vec<(i32, i32)>> {
        let l = &self.laws;
        let lum = &self.lumens[i];
        let mine = lum.claim;
        let r2 = x * x + y * y;
        if r2 < l.no_build * l.no_build {
            return None;
        }
        // Not within 2 tiles of anyone else's land.
        let crowded = |tiles: &[(i32, i32)]| {
            tiles.iter().any(|&(tx, ty)| {
                (-l.claim_gap..=l.claim_gap).any(|dy| {
                    (-l.claim_gap..=l.claim_gap).any(|dx| {
                        let o = self.tiles.get(tx + dx, ty + dy).owner();
                        o != 0 && o != mine
                    })
                })
            })
        };
        let clear = |t: Tile| {
            !t.void() && (t.obj == obj::NONE || t.obj == obj::GLOWMOSS) && t.kind() != ground::WATER
        };
        if piece == obj::HEARTH {
            if self.claim(mine).and_then(|c| c.hearth).is_some() {
                return None;
            }
            let ring = crate::island::ring(l, x, y);
            let r = (r2 as f64).sqrt();
            if !matches!(ring, Ring::Glow | Ring::Dim)
                || r < l.hearth_near as f64
                || r > l.hearth_far as f64
            {
                return None;
            }
            let apart = l.hearth_apart * l.hearth_apart;
            if self
                .claims
                .iter()
                .filter_map(|c| c.hearth)
                .any(|h| (h.0 - x).pow(2) + (h.1 - y).pow(2) < apart)
            {
                return None;
            }
            let tiles: Vec<(i32, i32)> = core((x, y)).collect();
            if tiles.iter().any(|&(tx, ty)| {
                let t = self.tiles.get(tx, ty);
                !clear(t) || t.claim != 0 || tx * tx + ty * ty < l.no_build * l.no_build
            }) || crowded(&tiles)
            {
                return None;
            }
            let walls: Vec<(i32, i32)> = tiles
                .iter()
                .copied()
                .filter(|&(tx, ty)| on_hearth((x, y), tx, ty))
                .collect();
            if !self.cut_off(&walls, mine).is_empty()
                && self.cut_off(&walls, mine) != self.cut_off(&[], mine)
            {
                return None;
            }
            return Some(tiles);
        }
        let t = self.tiles.get(x, y);
        let home = self.claim(mine).and_then(|c| c.hearth);
        if !t.land_of(mine)
            || !clear(t)
            || home.is_some_and(|h| on_hearth(h, x, y))
            || crowded(&[(x, y)])
        {
            return None;
        }
        if piece == obj::LANTERN && level(lum.xp[2]) < l.lantern_level {
            return None;
        }
        let solid = !matches!(piece, obj::THORNS);
        if solid {
            let before = self.cut_off(&[], mine);
            let after = self.cut_off(&[(x, y)], mine);
            if after.iter().any(|c| !before.contains(c)) {
                return None;
            }
        }
        Some(vec![(x, y)])
    }

    /// Place a piece (paid from the bag, then the vault). Whether it went.
    pub fn place(&mut self, i: usize, idx: u16, piece: u8) -> bool {
        let (x, y) = Tiles::at_index(idx as usize);
        let Some(tiles) = self.may_place(i, x, y, piece) else {
            self.events.push(Event::Refused {
                id: self.lumens[i].id,
            });
            return false;
        };
        let price = cost(piece, &self.laws);
        // A hearth is paid before there is a vault (but a lodging has one).
        if piece == obj::HEARTH && self.claim_of(self.lumens[i].soul).is_none() {
            let (soul, name, hue) = (
                self.lumens[i].soul,
                self.lumens[i].name.clone(),
                self.lumens[i].hue,
            );
            let id = self.new_claim(soul, &name, hue);
            self.lumens[i].claim = id;
        }
        if !self.pay(i, price) {
            self.events.push(Event::Refused {
                id: self.lumens[i].id,
            });
            return false;
        }
        let claim = self.lumens[i].claim;
        if piece == obj::HEARTH {
            for &(tx, ty) in &tiles {
                let Some(k) = Tiles::index(tx, ty) else {
                    continue;
                };
                let t = self.tiles.t[k];
                let o = if on_hearth((x, y), tx, ty) {
                    obj::HEARTH
                } else {
                    t.obj
                };
                self.set_tile(k as u16, Tile { obj: o, claim, ..t });
            }
            let unix = self.unix;
            if let Some(c) = self.claim_mut(claim) {
                c.hearth = Some((x, y));
                c.seen = unix;
                c.cold = false;
            }
            self.lumens[i].me.body.claim = claim;
            let xp = self.laws.xp_hearth;
            self.gain(i, 2, xp);
            self.lumens[i].build = None;
            self.count("hearth");
        } else {
            let t = self.tile(idx);
            let o = if piece == obj::PLANTER {
                obj::SPROUT
            } else {
                piece
            };
            self.set_tile(idx, Tile { obj: o, ..t });
            if piece == obj::PLANTER {
                self.crops.insert(
                    idx,
                    crate::gather::Crop {
                        planted: self.tick,
                        claim,
                    },
                );
            }
            let xp = self.laws.xp_piece;
            self.gain(i, 2, xp);
            self.count("piece");
        }
        let id = self.lumens[i].id;
        self.events.push(Event::Placed { id, idx, piece });
        true
    }

    /// Take down your own piece for half its cost back.
    pub fn remove_piece(&mut self, i: usize, idx: u16) {
        let t = self.tile(idx);
        let claim = self.lumens[i].claim;
        if t.owner() != claim || claim == 0 || !obj::piece(t.obj) {
            return;
        }
        let piece = match t.obj {
            obj::SPROUT | obj::RIPE | obj::WILTED => obj::PLANTER,
            o => o,
        };
        let back = cost(piece, &self.laws);
        let half = |v: u32| (v as i64 * self.laws.refund as i64 / 1000) as u32;
        let refund = Bag {
            glim: half(back.glim),
            wood: half(back.wood),
            stone: half(back.stone),
            wheat: 0,
        };
        if piece == obj::HEARTH {
            // The land goes back to the commons (outlines stay); the vault
            // becomes a lodging's.
            let Some(h) = self.claim(claim).and_then(|c| c.hearth) else {
                return;
            };
            let until = self.unix + self.laws.outline_days as u64 * 86_400;
            for k in 0..self.tiles.t.len() {
                let tt = self.tiles.t[k];
                if tt.owner() == claim {
                    let (tx, ty) = Tiles::at_index(k);
                    let o = if on_hearth(h, tx, ty) || obj::piece(tt.obj) {
                        obj::NONE
                    } else {
                        tt.obj
                    };
                    self.set_tile(
                        k as u16,
                        Tile {
                            obj: o,
                            claim: 0,
                            ..tt
                        },
                    );
                    self.outlines.insert(k as u16, (claim, until));
                    self.crops.remove(&(k as u16));
                }
            }
            if let Some(c) = self.claim_mut(claim) {
                c.hearth = None;
                c.tiles = 0;
            }
            self.lumens[i].me.body.claim = 0;
        } else {
            self.set_tile(
                idx,
                Tile {
                    obj: obj::NONE,
                    ..t
                },
            );
            self.crops.remove(&idx);
        }
        let lum = &mut self.lumens[i];
        lum.glim += refund.glim;
        lum.wood += refund.wood;
        lum.stone += refund.stone;
        lum.me.body.load = lum.materials();
        let id = lum.id;
        self.events.push(Event::Removed { id, idx });
    }

    /// Strike an empty planter on your land: Sunwheat goes in.
    pub fn plant(&mut self, i: usize, idx: u16) -> bool {
        let t = self.tile(idx);
        let claim = self.lumens[i].claim;
        if t.obj != obj::PLANTER || t.owner() != claim || claim == 0 {
            return false;
        }
        self.set_tile(
            idx,
            Tile {
                obj: obj::SPROUT,
                ..t
            },
        );
        self.crops.insert(
            idx,
            crate::gather::Crop {
                planted: self.tick,
                claim,
            },
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_levels_follow_the_curve() {
        assert_eq!(level(0), 1);
        assert_eq!(xp_for(2), 60);
        assert_eq!(level(xp_for(5)), 5);
        assert_eq!(level(xp_for(5) - 1), 4);
        assert!(xp_for(99) > 2_000_000 && xp_for(99) < 2_600_000);
    }
}
