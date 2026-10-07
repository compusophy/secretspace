//! Settling what Lumens swing at each other: who a strike picks, how hard
//! it lands (closing speed, the skid's carry, the launcher, Flow), how far
//! the target flies (the dimmer, the further), lances, heavies, thrown
//! motes and the glim they leave. Targets are judged where the striker saw
//! them: rewound by half its round trip and the 66 ms of interpolation, at
//! most 3 ticks.

use engine::fixed::{atan2, len, turn, unit, Fx};

use crate::combat::{heavy, kb_mul, stun, throw, Act, Swing};
use crate::island::Ring;
use crate::world::{Cause, Event, Mote, Pickup, World};

/// What kind of blow, for its numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Blow {
    Strike,
    Lance,
    Heavy,
    Mote,
}

fn milli(v: Fx, m: i32) -> Fx {
    Fx((v.0 as i64 * m as i64 / 1000) as i32)
}

/// Tiles a tick as thousandths of tiles a second.
fn per_second_milli(v: Fx) -> i64 {
    v.0 as i64 * crate::laws::HZ as i64 * 1000 / 65536
}

impl World {
    /// Ticks to rewind targets for a striker with this round trip.
    fn rewind(&self, rtt: u32) -> usize {
        let ms = (rtt / 2 + 66).min(100);
        ((ms * crate::laws::HZ + 500) / 1000).min(3) as usize
    }

    /// Others a striker at (x, y) facing `h` could reach, as it saw them:
    /// (index, distance, angle off its facing).
    fn reachable(&self, i: usize, reach: Fx, half_cone: u16) -> Vec<(usize, Fx, i32)> {
        let me = &self.lumens[i];
        let (x, y) = me.pos();
        let h = me.me.act.aim;
        let back = self.rewind(me.rtt);
        let mut out = Vec::new();
        for (j, o) in self.lumens.iter().enumerate() {
            if j == i || !o.alive() || o.me.body.mv == crate::motion::Move::Fallen {
                continue;
            }
            let (ox, oy) = self.was(o.id, back).unwrap_or(o.pos());
            let (dx, dy) = (ox.sub(x), oy.sub(y));
            let d = len(dx, dy);
            if d > reach.add(self.laws.body) {
                continue;
            }
            let off = turn(h, atan2(dy, dx));
            if off.abs() <= half_cone as i32 || d < self.laws.body {
                out.push((j, d, off));
            }
        }
        out
    }

    pub(crate) fn swing(&mut self, i: usize, s: Swing) {
        let id = self.lumens[i].id;
        match s {
            Swing::Strike { carry } => {
                self.events.push(Event::Strike { id });
                let l = &self.laws;
                let cands = self.reachable(i, l.reach, l.cone / 2);
                let tick = self.tick;
                let me = &self.lumens[i];
                let inner = (l.inner_cone / 2) as i32;
                let engaged = |c: &&(usize, Fx, i32)| {
                    let o = &self.lumens[c.0];
                    me.engaged_with(o.id, tick) || o.engaged_with(id, tick)
                };
                // First a Lumen you are fighting; then a node (or your own
                // empty planter); then anyone in the inner cone.
                let fighting = cands.iter().filter(engaged).min_by_key(|c| c.1).copied();
                if fighting.is_none() {
                    if let Some(idx) = self.node_in_reach(i) {
                        self.strike_node(i, idx, false);
                        return;
                    }
                    let b = self.lumens[i].me.body;
                    let (gx, gy) = crate::build::ghost(b.x, b.y, b.facing);
                    if let Some(idx) = crate::tiles::Tiles::index(gx, gy) {
                        if self.plant(i, idx as u16) {
                            return;
                        }
                    }
                }
                let pick = fighting.or_else(|| {
                    cands
                        .iter()
                        .filter(|c| c.2.abs() <= inner)
                        .min_by_key(|c| c.1)
                        .copied()
                });
                match pick {
                    Some((j, _, off)) => {
                        // Facing turns toward what it hits.
                        let most = self.laws.face_turn as i32;
                        let b = &mut self.lumens[i].me.body;
                        b.facing = b.facing.wrapping_add(off.clamp(-most, most) as u16);
                        self.land(i, j, Blow::Strike, carry, 0);
                    }
                    None => {
                        self.events.push(Event::Whiff { id });
                    }
                }
            }
            Swing::Lance => {
                let l = &self.laws;
                let reach = l.lance_reach;
                let body2 = l.body.mul_int(2);
                let me = &self.lumens[i];
                let (x, y) = me.pos();
                let (ux, uy) = unit(me.me.act.aim);
                let back = self.rewind(me.rtt);
                let mut best: Option<(usize, Fx)> = None;
                for (j, o) in self.lumens.iter().enumerate() {
                    if j == i || !o.alive() {
                        continue;
                    }
                    let (ox, oy) = self.was(o.id, back).unwrap_or(o.pos());
                    let (dx, dy) = (ox.sub(x), oy.sub(y));
                    let along = dx.mul(ux).add(dy.mul(uy));
                    let across = dx.mul(uy).sub(dy.mul(ux)).abs();
                    if along >= Fx::ZERO
                        && along <= reach
                        && across <= body2
                        && best.is_none_or(|b| along < b.1)
                    {
                        best = Some((j, along));
                    }
                }
                self.count("lance");
                match best {
                    Some((j, _)) => {
                        self.land(i, j, Blow::Lance, Fx::ZERO, 0);
                    }
                    None => {
                        self.events.push(Event::Whiff { id });
                        let a = &mut self.lumens[i].me.act;
                        a.act = Act::Recover;
                        a.t = self.laws.lance_whiff;
                    }
                }
            }
            Swing::Heavy { c } => {
                let cands = self.reachable(i, self.laws.reach, crate::laws::deg(30));
                if let Some(&(j, _, _)) = cands.iter().min_by_key(|c| c.1) {
                    self.lumens[i].me.act.landed = true;
                    self.land(i, j, Blow::Heavy, Fx::ZERO, c);
                } else if self.lumens[i].me.act.t <= 1 {
                    self.events.push(Event::Whiff { id });
                }
            }
            Swing::Throw { c, range } => {
                let l = &self.laws;
                let (dmg, pierce, reach) = throw(c, range, l);
                let me = &self.lumens[i];
                let (ux, uy) = unit(me.me.act.aim);
                let (x, y) = me.pos();
                let speed = l.mote_speed;
                let glim = l.throw_glim;
                self.lumens[i].glim = self.lumens[i].glim.saturating_sub(glim);
                let mid = self.new_id();
                self.motes.push(Mote {
                    id: mid,
                    x: x.add(ux.mul(Fx::HALF)),
                    y: y.add(uy.mul(Fx::HALF)),
                    vx: ux.mul(speed),
                    vy: uy.mul(speed),
                    left: reach,
                    owner: id,
                    damage: dmg,
                    pierce,
                    hit: Vec::new(),
                });
                self.count(if pierce { "perfect throw" } else { "throw" });
            }
        }
    }

    /// Lumen i's blow lands on j.
    fn land(&mut self, i: usize, j: usize, blow: Blow, carry: Fx, c: u32) -> bool {
        let tick = self.tick;
        let l = self.laws.clone();
        let (a_id, t_id) = (self.lumens[i].id, self.lumens[j].id);
        let t = &self.lumens[j];
        if t.ghost > 0 {
            return false;
        }
        if t.me.body.dodging() {
            self.events.push(Event::Dodge { id: t_id });
            self.technique(j, "dodge");
            return false;
        }
        let (ax, ay) = self.lumens[i].pos();
        let (tx, ty) = t.pos();
        let dir = if tx == ax && ty == ay {
            self.lumens[i].me.act.aim
        } else {
            atan2(ty.sub(ay), tx.sub(ax))
        };
        let (ux, uy) = unit(dir);
        let a = &self.lumens[i];
        // Speeds along the blow.
        let va = carry.max(a.me.body.vx.mul(ux).add(a.me.body.vy.mul(uy)));
        let vt = t.me.body.vx.mul(ux).add(t.me.body.vy.mul(uy));
        let (mut dmg, mut kb) = match blow {
            Blow::Strike => {
                let closing = va.sub(vt).max(Fx::ZERO);
                let dmg = (l.strike_base as i64
                    + per_second_milli(closing) * l.strike_closing as i64 / 1000)
                    .min(l.strike_cap as i64) as i32;
                let kb = l.strike_kb.add(milli(va.max(Fx::ZERO), l.strike_carry));
                (dmg, kb)
            }
            Blow::Lance => (l.lance_damage, milli(l.strike_kb, l.lance_kb)),
            Blow::Heavy => {
                let (d, kb, perfect) = heavy(c, &l);
                if perfect {
                    self.count("perfect heavy");
                } else {
                    self.count("heavy");
                }
                (d, kb)
            }
            Blow::Mote => (0, l.throw_kb),
        };
        if blow == Blow::Mote {
            dmg = c as i32;
        }
        // The launcher: a third strike on one target in a row.
        if blow == Blow::Strike {
            let combo = &mut self.lumens[i].combo;
            if combo.0 == t_id && tick.wrapping_sub(combo.2) <= l.launcher_within {
                combo.1 += 1;
            } else {
                *combo = (t_id, 1, tick);
            }
            combo.2 = tick;
            if combo.1 >= 3 {
                combo.1 = 0;
                kb = milli(kb, l.launcher);
                self.count("launcher");
            }
            if carry > self.laws.walk {
                self.count("running strike");
            }
        }
        // Striking a Lumen ends your own ghost.
        self.lumens[i].ghost = 0;
        // Sparks: a newcomer outside the Sanctum and the Rim takes nothing
        // from a Lumen; a newcomer who strikes outside the Sanctum gives
        // its Sparks up, and that strike lands.
        let ring_t = self.ring_at(tx, ty);
        if self.lumens[i].spark(&l) && ring_t != Ring::Sanctum {
            self.lumens[i].sparks_off = true;
        }
        if self.lumens[j].spark(&l) && !matches!(ring_t, Ring::Sanctum | Ring::Rim) {
            self.events.push(Event::Hit {
                by: a_id,
                on: t_id,
                damage: 0,
                big: false,
            });
            return false;
        }
        let (ring, sanctum) = {
            let r = self.ring_at(tx, ty);
            (r, r == Ring::Sanctum)
        };
        if sanctum {
            // Strikes only shove here.
            dmg = 0;
            kb = l.strike_kb;
        } else {
            let t = &self.lumens[j];
            let spared =
                ring == Ring::Glow && t.spared.iter().any(|&(o, until)| o == a_id && until > tick);
            if t.down > 0 || spared {
                dmg = 0;
            }
        }
        if self.lumens[i].flow >= 3 {
            kb = milli(kb, l.flow_kb);
        }
        // Engaged, both ways; who hit whom last.
        for (x, y) in [(i, t_id), (j, a_id)] {
            let e = &mut self.lumens[x].engaged;
            e.retain(|e| e.0 != y);
            e.push((y, tick + l.engaged));
        }
        self.lumens[j].last_hit = (a_id, tick);
        let valor = (dmg / 1000).max(0) as u32 * l.xp_valor;
        self.gain(i, 4, valor);
        let cause = if blow == Blow::Mote {
            Cause::Mote
        } else {
            Cause::Strike
        };
        self.hurt(j, dmg, a_id, cause);
        let t = &mut self.lumens[j];
        if t.alive() && !sanctum {
            let m = kb_mul(t.flame, &l);
            kb = milli(kb, m);
        }
        if t.alive() {
            t.me.body.launch(ux.mul(kb), uy.mul(kb), stun(kb));
            t.me.act = Default::default();
        }
        self.events.push(Event::Hit {
            by: a_id,
            on: t_id,
            damage: (dmg / 1000).clamp(0, 255) as u8,
            big: blow != Blow::Strike || kb > l.heavy_kb_low,
        });
        self.technique(i, "landed strike");
        true
    }

    /// Motes fly and land; Lumens pick up glim they touch.
    pub(crate) fn motes_and_pickups(&mut self) {
        let mut landed: Vec<(Fx, Fx)> = Vec::new();
        let mut k = 0;
        while k < self.motes.len() {
            let m = &mut self.motes[k];
            let (nx, ny) = (m.x.add(m.vx), m.y.add(m.vy));
            m.left = m.left.sub(len(m.vx, m.vy));
            let blocked = self.tiles.under(nx, ny).solid();
            let (mx, my, owner) = (m.x, m.y, m.owner);
            if !blocked {
                m.x = nx;
                m.y = ny;
            }
            // Who it touches.
            let reach = self.laws.body.add(Fx::milli(150));
            let hit = self.lumens.iter().position(|o| {
                o.id != owner
                    && o.alive()
                    && !self.motes[k].hit.contains(&o.id)
                    && len(
                        o.me.body.x.sub(self.motes[k].x),
                        o.me.body.y.sub(self.motes[k].y),
                    ) <= reach
            });
            let mut gone = false;
            if let Some(j) = hit {
                let tid = self.lumens[j].id;
                let dmg = self.motes[k].damage;
                if let Some(i) = self.index(owner) {
                    // Aim the blow along the mote's flight.
                    self.lumens[i].me.act.aim = atan2(self.motes[k].vy, self.motes[k].vx);
                    self.land(i, j, Blow::Mote, Fx::ZERO, dmg as u32);
                }
                self.motes[k].hit.push(tid);
                gone = !self.motes[k].pierce;
            }
            let m = &self.motes[k];
            if !gone && (blocked || m.left <= Fx::ZERO) {
                gone = true;
                // One that hit nobody lands as glim anyone may take.
                if m.hit.is_empty() {
                    let spot = if self.tiles.under(m.x, m.y).void() {
                        (mx, my)
                    } else {
                        (m.x, m.y)
                    };
                    if !self.tiles.under(spot.0, spot.1).void() {
                        landed.push(spot);
                    }
                }
            }
            if gone {
                self.motes.remove(k);
            } else {
                k += 1;
            }
        }
        for (x, y) in landed {
            let id = self.new_id();
            let glim = self.laws.throw_glim;
            self.pickups.push(Pickup {
                id,
                x,
                y,
                glim,
                ..Pickup::default()
            });
        }
        // Pickups: the first Lumen within reach takes what it can carry.
        let reach = Fx::milli(600);
        let (gmax, cmax) = (self.laws.glim_max, self.laws.carry_max);
        for k in (0..self.pickups.len()).rev() {
            let p = self.pickups[k].clone();
            let Some(i) = self.lumens.iter().position(|o| {
                o.alive() && o.down == 0 && len(o.me.body.x.sub(p.x), o.me.body.y.sub(p.y)) <= reach
            }) else {
                continue;
            };
            let o = &mut self.lumens[i];
            let g = p.glim.min(gmax.saturating_sub(o.glim));
            o.glim += g;
            let mut room = cmax.saturating_sub(o.materials());
            let mut take = |v: u32| {
                let t = v.min(room);
                room -= t;
                t
            };
            let (w, s, h) = (take(p.wood), take(p.stone), take(p.wheat));
            o.wood += w;
            o.stone += s;
            o.wheat += h;
            o.me.body.load = o.materials();
            let left = &mut self.pickups[k];
            left.glim -= g;
            left.wood -= w;
            left.stone -= s;
            left.wheat -= h;
            if left.glim + left.wood + left.stone + left.wheat == 0 {
                self.pickups.remove(k);
            }
        }
    }
}
