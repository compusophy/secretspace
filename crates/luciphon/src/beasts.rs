//! Monsters (the MMO turn): hushlings and gloomhounds in the Dim, rim
//! wraiths out on the Rim (`laws::BEASTS`). They never set foot in the
//! Glow or the Sanctum. Each wanders near home until someone comes within
//! its notice, then hunts them, winds up where you can see it, and bites;
//! it gives up past its leash. Wands hurt them; the one who fells one
//! gains valor, and it leaves glim, wood, stone and sometimes gear. A
//! minute later another comes, somewhere no one stands.

use engine::fixed::{atan2, len, turn, unit, Fx};

use crate::island::Ring;
use crate::laws::{Beast as Kind, BEASTS, BEAST_CLEAR, BEAST_RESPAWN, GEAR};
use crate::world::{Cause, Event, Pickup, World};

#[derive(Clone, Debug, Default)]
pub struct Beast {
    pub id: u16,
    pub kind: u8,
    pub x: Fx,
    pub y: Fx,
    pub vx: Fx,
    pub vy: Fx,
    pub facing: u16,
    /// Health, thousandths.
    pub hp: i32,
    pub home: (Fx, Fx),
    /// Whom it hunts (0: no one), and where it wanders to.
    pub target: u16,
    pub goal: (Fx, Fx),
    /// Ticks left: until it may bite again, of its wind-up, of a stagger.
    pub cool: u32,
    pub wind: u32,
    pub stun: u32,
    /// Damage dealt to it, by Lumen.
    pub hurt_by: Vec<(u16, i32)>,
}

impl Beast {
    pub fn kind(&self) -> &'static Kind {
        &BEASTS[self.kind as usize % BEASTS.len()]
    }
}

impl World {
    /// Whether a monster of this kind may stand on tile (x, y).
    fn roams(&self, k: &Kind, x: Fx, y: Fx) -> bool {
        let t = self.tiles.under(x, y);
        if t.void() || t.solid() {
            return false;
        }
        match self.ring_at(x, y) {
            Ring::Dim => k.dim,
            Ring::Rim => true,
            _ => false,
        }
    }

    /// Every kind of monster, as many as it should have.
    pub fn populate(&mut self) {
        for (kind, k) in BEASTS.iter().enumerate() {
            let have = self
                .beasts
                .iter()
                .filter(|b| b.kind as usize == kind)
                .count();
            for _ in have..k.count as usize {
                self.spawn_beast(kind as u8);
            }
        }
    }

    /// One monster of this kind, somewhere in its ring away from anyone.
    pub fn spawn_beast(&mut self, kind: u8) -> bool {
        let k = &BEASTS[kind as usize % BEASTS.len()];
        let (r0, r1) = if k.dim {
            (self.laws.glow + 1, self.laws.dim - 1)
        } else {
            (self.laws.dim, self.laws.island - 2)
        };
        for _ in 0..40 {
            let a = self.rng.below(65536) as u16;
            let r = r0 + self.rng.below((r1 - r0).max(1) as u64) as i32;
            let (ux, uy) = unit(a);
            let (x, y) = (ux.mul_int(r).floor(), uy.mul_int(r).floor());
            let (fx, fy) = (Fx::int(x).add(Fx::HALF), Fx::int(y).add(Fx::HALF));
            let clear = self
                .lumens
                .iter()
                .all(|l| len(l.me.body.x.sub(fx), l.me.body.y.sub(fy)) > Fx::int(BEAST_CLEAR));
            if self.roams(k, fx, fy) && clear {
                let id = self.new_id();
                self.beasts.push(Beast {
                    id,
                    kind,
                    x: fx,
                    y: fy,
                    hp: k.hp,
                    home: (fx, fy),
                    goal: (fx, fy),
                    ..Beast::default()
                });
                return true;
            }
        }
        false
    }

    /// Who a monster would hunt: the nearest Lumen it can reach and harm.
    fn quarry(&self, b: &Beast) -> u16 {
        let k = b.kind();
        let mut best = (0u16, Fx::int(k.aggro));
        for l in &self.lumens {
            if !l.alive()
                || l.ghost > 0
                || l.down > 0
                || l.me.body.mv == crate::motion::Move::Fallen
            {
                continue;
            }
            let (x, y) = l.pos();
            if !matches!(self.ring_at(x, y), Ring::Dim | Ring::Rim) {
                continue;
            }
            let d = len(x.sub(b.x), y.sub(b.y));
            if d < best.1 {
                best = (l.id, d);
            }
        }
        best.0
    }

    /// Every monster's tick: think, move, bite; and the fallen come back.
    pub(crate) fn beasts_step(&mut self) {
        let tick = self.tick;
        for n in 0..self.beasts.len() {
            let mut b = std::mem::take(&mut self.beasts[n]);
            let k = b.kind();
            b.cool = b.cool.saturating_sub(1);
            if b.stun > 0 {
                b.stun -= 1;
                b.vx = b.vx.mul(Fx::ratio(7, 8));
                b.vy = b.vy.mul(Fx::ratio(7, 8));
                self.walk_beast(&mut b, k);
                self.beasts[n] = b;
                continue;
            }
            // Keep after its quarry while it is near home; else let go.
            let far = len(b.x.sub(b.home.0), b.y.sub(b.home.1)) > Fx::int(k.leash);
            let held = self.find(b.target).filter(|l| {
                l.alive()
                    && l.ghost == 0
                    && l.down == 0
                    && matches!(self.ring_at(l.pos().0, l.pos().1), Ring::Dim | Ring::Rim)
            });
            if held.is_none() || far {
                b.target = if far { 0 } else { self.quarry(&b) };
            }
            let prey = self.find(b.target).map(|l| (l.pos(), self.index(l.id)));
            if b.wind > 0 {
                // Winding up: rooted, turning a little toward it; then the bite.
                b.vx = Fx::ZERO;
                b.vy = Fx::ZERO;
                if let Some(((px, py), _)) = prey {
                    let want = atan2(py.sub(b.y), px.sub(b.x));
                    let off = turn(b.facing, want).clamp(-1200, 1200);
                    b.facing = b.facing.wrapping_add(off as u16);
                }
                b.wind -= 1;
                if b.wind == 0 {
                    b.cool = k.cooldown;
                    if let Some((_, Some(j))) = prey {
                        self.bite(&b, k, j);
                    }
                }
            } else if let Some(((px, py), _)) = prey {
                let (dx, dy) = (px.sub(b.x), py.sub(b.y));
                let d = len(dx, dy);
                b.facing = atan2(dy, dx);
                if d <= k.reach.add(self.laws.body) {
                    b.vx = Fx::ZERO;
                    b.vy = Fx::ZERO;
                    if b.cool == 0 {
                        b.wind = k.windup;
                    }
                } else {
                    let (ux, uy) = unit(b.facing);
                    b.vx = ux.mul(k.speed);
                    b.vy = uy.mul(k.speed);
                }
            } else {
                // Wander about home, slowly.
                if tick % 120 == (b.id as u32 % 120) {
                    let a = self.rng.below(65536) as u16;
                    let (ux, uy) = unit(a);
                    let r = Fx::int(2 + self.rng.below(5) as i32);
                    b.goal = (b.home.0.add(ux.mul(r)), b.home.1.add(uy.mul(r)));
                }
                let (dx, dy) = (b.goal.0.sub(b.x), b.goal.1.sub(b.y));
                if len(dx, dy) > Fx::HALF {
                    b.facing = atan2(dy, dx);
                    let (ux, uy) = unit(b.facing);
                    b.vx = ux.mul(k.speed.div_int(3));
                    b.vy = uy.mul(k.speed.div_int(3));
                } else {
                    b.vx = Fx::ZERO;
                    b.vy = Fx::ZERO;
                }
            }
            self.walk_beast(&mut b, k);
            self.beasts[n] = b;
        }
        // The fallen come back, a minute on.
        let due: Vec<u8> = self
            .beast_queue
            .iter()
            .filter(|q| q.1 <= tick)
            .map(|q| q.0)
            .collect();
        self.beast_queue.retain(|q| q.1 > tick);
        for kind in due {
            if !self.spawn_beast(kind) {
                self.beast_queue.push((kind, tick + 30));
            }
        }
    }

    /// Move a monster by its velocity, one axis at a time, where it roams;
    /// turning aside when the way ahead is closed.
    fn walk_beast(&self, b: &mut Beast, k: &Kind) {
        let nx = b.x.add(b.vx);
        if self.roams(k, nx, b.y) {
            b.x = nx;
        } else {
            b.vx = Fx::ZERO;
        }
        let ny = b.y.add(b.vy);
        if self.roams(k, b.x, ny) {
            b.y = ny;
        } else {
            b.vy = Fx::ZERO;
        }
    }

    /// A monster's bite lands on Lumen j, if it is still in reach before it.
    fn bite(&mut self, b: &Beast, k: &Kind, j: usize) {
        let l = &self.lumens[j];
        let (x, y) = l.pos();
        let (dx, dy) = (x.sub(b.x), y.sub(b.y));
        let in_reach = len(dx, dy) <= k.reach.add(self.laws.body).add(Fx::milli(300));
        let ahead = turn(b.facing, atan2(dy, dx)).abs() < crate::laws::deg(70) as i32;
        let on = l.id;
        if !in_reach || !ahead || !l.alive() || l.ghost > 0 {
            return;
        }
        if l.me.body.dodging() {
            self.events.push(Event::Dodge { id: on });
            self.technique(j, "dodge");
            return;
        }
        self.events.push(Event::Hit {
            by: b.id,
            on,
            damage: (k.bite / 1000) as u8,
            big: k.bite >= 15_000,
        });
        self.count("bitten");
        self.hurt(j, k.bite, 0, Cause::Beast);
        let l = &mut self.lumens[j];
        if l.alive() {
            let (ux, uy) = unit(atan2(dy, dx));
            let kb = Fx(
                (k.kb.0 as i64 * crate::combat::kb_mul(l.flame, &self.laws) as i64 / 1000) as i32,
            );
            l.me.body
                .launch(ux.mul(kb), uy.mul(kb), crate::combat::stun(kb));
            l.me.act = Default::default();
        }
    }

    /// Monsters on a beam from Lumen i along its aim, short of `end`:
    /// (index, distance along it).
    pub(crate) fn beasts_on_beam(&self, i: usize, end: Fx) -> Vec<(usize, Fx)> {
        let me = &self.lumens[i];
        let (x, y) = me.pos();
        let (ux, uy) = unit(me.me.act.aim);
        let mut out = Vec::new();
        for (n, b) in self.beasts.iter().enumerate() {
            let (dx, dy) = (b.x.sub(x), b.y.sub(y));
            let along = dx.mul(ux).add(dy.mul(uy));
            let across = dx.mul(uy).sub(dy.mul(ux)).abs();
            let k = b.kind();
            if along > Fx::ZERO && along <= end.add(k.size) && across <= k.size.add(Fx::milli(150))
            {
                out.push((n, along));
            }
        }
        out.sort_by_key(|o| o.1);
        out
    }

    /// Lumen i's light lands on monster n: hurt, pushed and staggered; it
    /// turns on whoever hurt it; at nothing left, it falls.
    pub(crate) fn hurt_beast(&mut self, i: usize, n: usize, dmg: i32, kb: Fx) {
        let (ax, ay) = self.lumens[i].pos();
        let by = self.lumens[i].id;
        let b = &mut self.beasts[n];
        let (ux, uy) = unit(atan2(b.y.sub(ay), b.x.sub(ax)));
        b.hp -= dmg;
        b.vx = ux.mul(kb);
        b.vy = uy.mul(kb);
        b.stun = b.stun.max(4);
        b.wind = 0;
        b.target = by;
        match b.hurt_by.iter_mut().find(|h| h.0 == by) {
            Some(h) => h.1 += dmg,
            None => b.hurt_by.push((by, dmg)),
        }
        let on = b.id;
        self.events.push(Event::Hit {
            by,
            on,
            damage: (dmg / 1000).clamp(0, 255) as u8,
            big: dmg >= 15_000,
        });
        self.technique(i, "landed strike");
        if self.beasts[n].hp <= 0 {
            self.fell_beast(n, by);
        }
    }

    /// A monster falls: valor to whoever hurt it most, and what it leaves.
    fn fell_beast(&mut self, n: usize, by: u16) {
        let b = self.beasts.remove(n);
        let k = b.kind();
        let most = b.hurt_by.iter().max_by_key(|h| h.1).map_or(by, |h| h.0);
        if let Some(i) = self.index(most) {
            self.gain(i, 4, k.xp);
        }
        self.events.push(Event::Felled {
            id: b.id,
            by: most,
            kind: b.kind,
        });
        self.count(match b.kind {
            0 => "hushling felled",
            1 => "gloomhound felled",
            _ => "rim wraith felled",
        });
        let glim = k.glim.0 + self.rng.below((k.glim.1 - k.glim.0 + 1) as u64) as u32;
        let wood = self.rng.below(k.wood as u64 + 1) as u32;
        let stone = self.rng.below(k.stone as u64 + 1) as u32;
        let id = self.new_id();
        self.pickups.push(Pickup {
            id,
            x: b.x,
            y: b.y,
            glim,
            wood,
            stone,
            ..Pickup::default()
        });
        // Now and then, a piece of gear: radiant, rare or fine.
        let roll = self.rng.below(1000) as u32;
        let [fine, rare, radiant] = k.drops;
        let rarity = if roll < radiant {
            Some(3)
        } else if roll < radiant + rare {
            Some(2)
        } else if roll < radiant + rare + fine {
            Some(1)
        } else {
            None
        };
        if let Some(r) = rarity {
            let pool: Vec<u8> = GEAR
                .iter()
                .enumerate()
                .filter(|g| g.1.rarity == r)
                .map(|g| g.0 as u8 + 1)
                .collect();
            if !pool.is_empty() {
                let item = pool[self.rng.below(pool.len() as u64) as usize];
                let id = self.new_id();
                self.pickups.push(Pickup {
                    id,
                    x: b.x.add(Fx::HALF),
                    y: b.y,
                    item,
                    ..Pickup::default()
                });
                self.count("gear dropped");
            }
        }
        self.beast_queue.push((b.kind, self.tick + BEAST_RESPAWN));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::laws::LAWS;

    #[test]
    fn monsters_keep_to_the_dark_rings_and_hunt() {
        let mut w = World::new(LAWS, 9);
        w.populate();
        let total: usize = BEASTS.iter().map(|k| k.count as usize).sum();
        assert_eq!(w.beasts.len(), total);
        for b in &w.beasts {
            assert!(matches!(w.ring_at(b.x, b.y), Ring::Dim | Ring::Rim));
        }
        // Someone walks up to a hushling: it notices, comes and bites.
        let h = w.beasts.iter().position(|b| b.kind == 0).unwrap();
        let (bx, by) = (w.beasts[h].x, w.beasts[h].y);
        let id = w.spawn("prey", 3, None);
        let i = w.index(id).unwrap();
        w.lumens[i].ghost = 0;
        w.lumens[i].me.body.x = bx.add(Fx::int(3));
        w.lumens[i].me.body.y = by;
        let flame = w.lumens[i].flame;
        for _ in 0..90 {
            w.step();
            if w.lumens[i].flame < flame {
                break;
            }
        }
        assert!(w.lumens[i].flame < flame, "bitten");
        for _ in 0..600 {
            w.step();
        }
        for b in &w.beasts {
            assert!(
                matches!(w.ring_at(b.x, b.y), Ring::Dim | Ring::Rim),
                "{}",
                b.kind().name
            );
        }
    }

    #[test]
    fn a_felled_monster_leaves_loot_and_comes_back() {
        let mut w = World::new(LAWS, 10);
        w.populate();
        let id = w.spawn("hunter", 4, None);
        let i = w.index(id).unwrap();
        let n = w.beasts.iter().position(|b| b.kind == 2).unwrap();
        let before = w.pickups.len();
        let xp = w.lumens[i].xp[4];
        let hp = w.beasts[n].hp;
        w.hurt_beast(i, n, hp, Fx::ZERO);
        assert_eq!(
            w.beasts.len(),
            BEASTS.iter().map(|k| k.count as usize).sum::<usize>() - 1
        );
        assert!(w.pickups.len() > before, "it leaves something");
        assert!(w.lumens[i].xp[4] > xp, "valor");
        w.tick += BEAST_RESPAWN;
        w.step();
        assert_eq!(
            w.beasts.iter().filter(|b| b.kind == 2).count(),
            BEASTS[2].count as usize
        );
    }
}
