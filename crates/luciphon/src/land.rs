//! Land (§8): a hearth and its 5x5 core are your first claim, with a
//! vault no one can raid; you bank by standing on the core, keeping a lamp
//! of 30 glim. In kindle mode every commons tile you cross near your
//! hearth becomes your wick, a glim staked on each; step back onto your
//! land to close the loop and everything enclosed is yours. Land earns a
//! little and costs upkeep, settled every minute; an empty vault lets the
//! furthest land fade, leaving its outline to re-kindle at half price.

use std::collections::VecDeque;

use engine::fixed::Fx;

use crate::tiles::{obj, Tile, Tiles, WICK};
use crate::world::{Event, World};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bag {
    pub glim: u32,
    pub wood: u32,
    pub stone: u32,
    pub wheat: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Claim {
    pub id: u16,
    pub soul: u64,
    pub name: String,
    pub hue: u8,
    /// The hearth's centre tile; None for a lodging (a vault, no land).
    pub hearth: Option<(i32, i32)>,
    pub vault: Bag,
    /// Kindled tiles (the core aside).
    pub tiles: u32,
    /// When its soul was last awake (seconds since 1970).
    pub seen: u64,
    pub cold: bool,
    /// Thousandths of glim earned or owed and not yet settled.
    pub acc: i64,
    pub fade_at: u32,
}

/// The 5x5 core around a hearth's centre.
pub fn core(c: (i32, i32)) -> impl Iterator<Item = (i32, i32)> {
    (-2..=2).flat_map(move |dy| (-2..=2).map(move |dx| (c.0 + dx, c.1 + dy)))
}

/// Within the 3x3 of a hearth centred at `c`.
pub fn on_hearth(c: (i32, i32), x: i32, y: i32) -> bool {
    (x - c.0).abs() <= 1 && (y - c.1).abs() <= 1
}

impl World {
    pub fn claim(&self, id: u16) -> Option<&crate::land::Claim> {
        self.claims.iter().find(|c| c.id == id && id != 0)
    }

    pub fn claim_mut(&mut self, id: u16) -> Option<&mut Claim> {
        self.claims.iter_mut().find(|c| c.id == id && id != 0)
    }

    pub fn claim_of(&self, soul: u64) -> Option<&Claim> {
        self.claims.iter().find(|c| c.soul == soul && soul != 0)
    }

    /// Whether a claim's vault holds glim (its land is lit and safe).
    pub fn fed(&self, id: u16) -> bool {
        self.claim(id).is_some_and(|c| c.vault.glim > 0 && !c.cold)
    }

    /// A new claim (a lodging until it has a hearth). Its id.
    pub fn new_claim(&mut self, soul: u64, name: &str, hue: u8) -> u16 {
        let id = self.claims.iter().map(|c| c.id).max().unwrap_or(0) + 1;
        self.claims.push(Claim {
            id: id.min(0x7fff),
            soul,
            name: name.to_string(),
            hue,
            seen: self.unix,
            ..Claim::default()
        });
        id
    }

    /// The most kindled tiles Lumen i may hold: 64, and 32 more for each
    /// hour played, up to 600.
    pub fn cap(&self, i: usize) -> u32 {
        let l = &self.laws;
        let hours = self.lumens[i].played / (3600 * crate::laws::HZ);
        (l.cap_base + l.cap_per_hour * hours).min(l.cap_max)
    }

    /// Standing on your core (or, with no hearth, by the Luciphon):
    /// everything carried pours into the vault, but for the lamp.
    pub(crate) fn bank(&mut self, i: usize) {
        let lum = &self.lumens[i];
        if lum.soul == 0 || !lum.alive() {
            return;
        }
        let (x, y) = (lum.me.body.x.floor(), lum.me.body.y.floor());
        let soul = lum.soul;
        // A soul with no hearth yet keeps a lodging: a vault by the
        // Luciphon, and no land.
        if self.claim_of(soul).is_none()
            && x * x + y * y <= 25
            && lum.glim + lum.materials() > self.laws.lamp
        {
            let (name, hue) = (lum.name.clone(), lum.hue);
            let id = self.new_claim(soul, &name, hue);
            self.lumens[i].claim = id;
        }
        let Some(c) = self.claim_of(soul) else {
            return;
        };
        let at_home = match c.hearth {
            Some(h) => core(h).any(|t| t == (x, y)),
            None => x * x + y * y <= 25,
        };
        if !at_home {
            return;
        }
        let lamp = self.laws.lamp;
        let id = c.id;
        let lum = &mut self.lumens[i];
        let pour = Bag {
            glim: lum.glim.saturating_sub(lamp),
            wood: lum.wood,
            stone: lum.stone,
            wheat: lum.wheat,
        };
        lum.glim -= pour.glim;
        (lum.wood, lum.stone, lum.wheat) = (0, 0, 0);
        lum.me.body.load = 0;
        let lid = lum.id;
        let need = lamp.saturating_sub(lum.glim);
        let Some(c) = self.claim_mut(id) else { return };
        c.vault.glim += pour.glim;
        c.vault.wood += pour.wood;
        c.vault.stone += pour.stone;
        c.vault.wheat += pour.wheat;
        let top = need.min(c.vault.glim);
        c.vault.glim -= top;
        let moved = pour != Bag::default() || top > 0;
        self.lumens[i].glim += top;
        if moved {
            self.events.push(Event::Banked { id: lid });
        }
    }

    /// Pay from what Lumen i carries, then from its vault. Whether it could.
    pub fn pay(&mut self, i: usize, cost: Bag) -> bool {
        let soul = self.lumens[i].soul;
        let vault = self.claim_of(soul).map(|c| c.vault).unwrap_or_default();
        let lum = &self.lumens[i];
        let can = |have: u32, v: u32, need: u32| have + v >= need;
        if !(can(lum.glim, vault.glim, cost.glim)
            && can(lum.wood, vault.wood, cost.wood)
            && can(lum.stone, vault.stone, cost.stone)
            && can(lum.wheat, vault.wheat, cost.wheat))
        {
            return false;
        }
        let mut owed = Bag::default();
        let lum = &mut self.lumens[i];
        for (have, need, rest) in [
            (&mut lum.glim, cost.glim, &mut owed.glim),
            (&mut lum.wood, cost.wood, &mut owed.wood),
            (&mut lum.stone, cost.stone, &mut owed.stone),
            (&mut lum.wheat, cost.wheat, &mut owed.wheat),
        ] {
            let take = (*have).min(need);
            *have -= take;
            *rest = need - take;
        }
        lum.me.body.load = lum.materials();
        if let Some(c) = self.claims.iter_mut().find(|c| c.soul == soul && soul != 0) {
            c.vault.glim -= owed.glim;
            c.vault.wood -= owed.wood;
            c.vault.stone -= owed.stone;
            c.vault.wheat -= owed.wheat;
        }
        true
    }

    /// Kindle mode, each tick: grow the wick, close the loop, or let a
    /// wick left open too long gutter.
    pub(crate) fn kindle(&mut self, i: usize) {
        let l = self.laws.clone();
        let lum = &self.lumens[i];
        if !lum.wick.is_empty() && self.tick.wrapping_sub(lum.wick_since) > l.wick_life {
            self.drop_wick(i);
            return;
        }
        let claim = lum.claim;
        let Some(home) = self.claim(claim).and_then(|c| c.hearth) else {
            return;
        };
        if !lum.kindle || !lum.alive() {
            return;
        }
        let (x, y) = (lum.me.body.x.floor(), lum.me.body.y.floor());
        let Some(idx) = Tiles::index(x, y) else {
            return;
        };
        let t = self.tiles.t[idx];
        if t.land_of(claim) {
            if !lum.wick.is_empty() {
                self.close_loop(i);
            }
            return;
        }
        let near = (x - home.0).pow(2) + (y - home.1).pow(2) <= l.kindle_reach.pow(2);
        let outside = x * x + y * y >= l.no_build * l.no_build;
        let commons = t.claim == 0 && !t.void();
        if near && outside && commons && lum.glim >= 1 && (lum.wick.len() as u32) < l.wick_max {
            let lum = &mut self.lumens[i];
            if lum.wick.is_empty() {
                lum.wick_since = self.tick;
            }
            lum.wick.push(idx as u16);
            lum.glim -= 1;
            self.set_tile(
                idx as u16,
                Tile {
                    claim: claim | WICK,
                    ..t
                },
            );
        }
    }

    /// The open wick goes out; its stakes are lost to the Dark.
    pub fn drop_wick(&mut self, i: usize) {
        let wick = std::mem::take(&mut self.lumens[i].wick);
        for idx in wick {
            let t = self.tile(idx);
            if t.wick().is_some() {
                self.set_tile(idx, Tile { claim: 0, ..t });
            }
        }
    }

    /// Back on your land with an open wick: the wick and what it encloses
    /// become yours (within the 64x64 box, 300 tiles, and your cap).
    fn close_loop(&mut self, i: usize) {
        let l = self.laws.clone();
        let claim = self.lumens[i].claim;
        let wick = std::mem::take(&mut self.lumens[i].wick);
        let pos: Vec<(i32, i32)> = wick.iter().map(|&w| Tiles::at_index(w as usize)).collect();
        let (x0, x1) = (
            pos.iter().map(|p| p.0).min().unwrap_or(0) - 1,
            pos.iter().map(|p| p.0).max().unwrap_or(0) + 1,
        );
        let (y0, y1) = (
            pos.iter().map(|p| p.1).min().unwrap_or(0) - 1,
            pos.iter().map(|p| p.1).max().unwrap_or(0) + 1,
        );
        let mut enclosed: Vec<u16> = Vec::new();
        if x1 - x0 < l.fill_box && y1 - y0 < l.fill_box {
            let (w, h) = ((x1 - x0 + 1) as usize, (y1 - y0 + 1) as usize);
            let mine = |x: i32, y: i32| {
                let t = self.tiles.get(x, y);
                t.land_of(claim) || t.wick() == Some(claim)
            };
            // Flood the outside in from the box's border.
            let mut out = vec![false; w * h];
            let mut q = VecDeque::new();
            for y in y0..=y1 {
                for x in x0..=x1 {
                    if (x == x0 || x == x1 || y == y0 || y == y1) && !mine(x, y) {
                        let k = (y - y0) as usize * w + (x - x0) as usize;
                        out[k] = true;
                        q.push_back((x, y));
                    }
                }
            }
            while let Some((x, y)) = q.pop_front() {
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < x0 || nx > x1 || ny < y0 || ny > y1 {
                        continue;
                    }
                    let k = (ny - y0) as usize * w + (nx - x0) as usize;
                    if !out[k] && !mine(nx, ny) {
                        out[k] = true;
                        q.push_back((nx, ny));
                    }
                }
            }
            let mut foreign = false;
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let k = (y - y0) as usize * w + (x - x0) as usize;
                    if out[k] || mine(x, y) {
                        continue;
                    }
                    let t = self.tiles.get(x, y);
                    if t.owner() != 0 {
                        foreign = true;
                    } else if !t.void() && x * x + y * y >= l.no_build * l.no_build {
                        if let Some(idx) = Tiles::index(x, y) {
                            enclosed.push(idx as u16);
                        }
                    }
                }
            }
            if foreign || enclosed.len() as u32 > l.fill_max {
                enclosed.clear();
            }
        }
        let have = self.claim(claim).map_or(0, |c| c.tiles);
        let cap = self.cap(i);
        let room = cap.saturating_sub(have) as usize;
        let mut wick = wick;
        if wick.len() + enclosed.len() > room {
            enclosed.clear();
            // Only the wick, and only up to the cap; the rest goes out.
            while wick.len() > room {
                let idx = wick.pop().unwrap_or(0);
                let t = self.tile(idx);
                self.set_tile(idx, Tile { claim: 0, ..t });
            }
        }
        // Each enclosed tile costs a glim (half for your own outlines).
        let cost: u32 = enclosed
            .iter()
            .map(|idx| {
                if self.outlines.get(idx).is_some_and(|o| o.0 == claim) {
                    1
                } else {
                    2
                }
            })
            .sum::<u32>()
            .div_ceil(2);
        if !self.pay(
            i,
            Bag {
                glim: cost,
                ..Bag::default()
            },
        ) {
            enclosed.clear();
        }
        let n = wick.len() + enclosed.len();
        for idx in wick.into_iter().chain(enclosed) {
            let t = self.tile(idx);
            self.set_tile(idx, Tile { claim, ..t });
            self.outlines.remove(&idx);
        }
        if let Some(c) = self.claim_mut(claim) {
            c.tiles += n as u32;
        }
        let xp = self.laws.xp_tile * n as u32;
        self.gain(i, 2, xp);
        let id = self.lumens[i].id;
        self.events.push(Event::Loop {
            id,
            n: n.min(u16::MAX as usize) as u16,
        });
        self.count("loop closed");
    }

    /// Anyone who may hurt a wick's owner, touching the wick, snuffs it:
    /// gone, the stake lost, and the owner hurt and stunned a moment.
    pub(crate) fn snuff(&mut self) {
        let mut hits = Vec::new();
        for (j, o) in self.lumens.iter().enumerate() {
            if !o.alive() || o.ghost > 0 {
                continue;
            }
            let t = self.tiles.under(o.me.body.x, o.me.body.y);
            if let Some(c) = t.wick().filter(|&c| c != o.claim) {
                if let Some(i) = self
                    .lumens
                    .iter()
                    .position(|l| l.claim == c && !l.wick.is_empty())
                {
                    let (x, y) = self.lumens[i].pos();
                    let ring = self.ring_at(x, y);
                    let spared = self.lumens[i].spark(&self.laws)
                        && !matches!(
                            ring,
                            crate::island::Ring::Sanctum | crate::island::Ring::Rim
                        );
                    if !spared && i != j {
                        hits.push((i, o.id));
                    }
                }
            }
        }
        for (i, by) in hits {
            if self.lumens[i].wick.is_empty() {
                continue;
            }
            self.drop_wick(i);
            let (d, stun) = (self.laws.snuff_damage, self.laws.snuff_stun);
            self.lumens[i].me.body.launch(Fx::ZERO, Fx::ZERO, stun);
            self.hurt(i, d, by, crate::world::Cause::Strike);
            let id = self.lumens[i].id;
            self.events.push(Event::Snuffed { id });
            self.count("snuffed");
        }
    }

    /// Every minute: land earns and costs, a fed claim's crops grow, an
    /// empty vault lets land fade, a long-gone soul's hearth goes cold.
    pub(crate) fn tend_land(&mut self) {
        let l = self.laws.clone();
        let awake: Vec<u64> = self
            .lumens
            .iter()
            .filter(|x| x.alive() && x.soul != 0)
            .map(|x| x.soul)
            .collect();
        let (tick, unix) = (self.tick, self.unix);
        let mut fades = Vec::new();
        for c in &mut self.claims {
            if c.hearth.is_none() {
                continue;
            }
            let dreaming = !awake.contains(&c.soul);
            if !dreaming {
                c.seen = unix;
            }
            if unix > c.seen + l.cold_days as u64 * 86_400 {
                c.cold = true;
            }
            if tick.is_multiple_of(l.land_every.max(1)) {
                let n = c.tiles as f64;
                let earn = n * l.land_yield as f64 / 60.0;
                let cost = l.upkeep as f64 * n.powf(l.upkeep_power as f64 / 1000.0) / 60.0;
                let k = if dreaming { 0.5 } else { 1.0 };
                c.acc += ((earn - cost) * k).round() as i64;
                let whole = c.acc / 1000;
                if whole > 0 {
                    c.vault.glim += whole as u32;
                } else if whole < 0 {
                    c.vault.glim = c.vault.glim.saturating_sub((-whole) as u32);
                }
                c.acc -= whole * 1000;
            }
            if (c.vault.glim == 0 || c.cold) && c.tiles > 0 && tick >= c.fade_at {
                c.fade_at = tick + l.fade_every;
                fades.push(c.id);
            }
        }
        for id in fades {
            self.fade(id);
        }
    }

    /// The claim's kindled tile furthest from its hearth fades to the
    /// commons, leaving an outline.
    fn fade(&mut self, id: u16) {
        let Some(h) = self.claim(id).and_then(|c| c.hearth) else {
            return;
        };
        let far = self
            .tiles
            .t
            .iter()
            .enumerate()
            .filter(|(i, t)| {
                let (x, y) = Tiles::at_index(*i);
                t.land_of(id) && !core(h).any(|c| c == (x, y)) && !obj::piece(t.obj)
            })
            .max_by_key(|(i, _)| {
                let (x, y) = Tiles::at_index(*i);
                (x - h.0).pow(2) + (y - h.1).pow(2)
            })
            .map(|(i, _)| i as u16);
        let Some(idx) = far else { return };
        let t = self.tile(idx);
        self.set_tile(idx, Tile { claim: 0, ..t });
        let until = self.unix + self.laws.outline_days as u64 * 86_400;
        self.outlines.insert(idx, (id, until));
        if let Some(c) = self.claim_mut(id) {
            c.tiles = c.tiles.saturating_sub(1);
        }
    }
}
