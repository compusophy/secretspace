//! The lifecycle of a Lumen (§12). Leaving out of combat, or 20 s with no
//! input, is an instant Dream: the body fades and its state is kept by
//! soul, to wake where it began to dream. Leaving in a fight Lingers ten
//! seconds, still there and still vulnerable. Guests are never kept.
//! Falling drops what you carry by ring, half the glim to the Dark.

use engine::fixed::{len, Fx};

use crate::combat::Me;
use crate::island::Ring;
use crate::motion::Body;
use crate::world::{Event, Lumen, Pickup, World};

/// A soul's Lumen while it is away.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dreamer {
    pub name: String,
    pub hue: u8,
    pub x: Fx,
    pub y: Fx,
    pub flame: i32,
    pub glim: u32,
    pub wood: u32,
    pub stone: u32,
    pub wheat: u32,
    pub claim: u16,
    pub xp: [u32; 7],
    pub played: u32,
    pub sparks_off: bool,
    pub home: bool,
    pub deaths: Vec<u32>,
}

impl Dreamer {
    pub fn of(l: &Lumen) -> Dreamer {
        Dreamer {
            name: l.name.clone(),
            hue: l.hue,
            x: l.me.body.x,
            y: l.me.body.y,
            flame: l.flame.max(1),
            glim: l.glim,
            wood: l.wood,
            stone: l.stone,
            wheat: l.wheat,
            claim: l.claim,
            xp: l.xp,
            played: l.played,
            sparks_off: l.sparks_off,
            home: l.home,
            deaths: l.deaths.clone(),
        }
    }
}

impl World {
    /// Whether leaving now must Linger: hit in the last 10 s, or someone
    /// who may hurt it is in sight.
    pub fn in_a_fight(&self, i: usize) -> bool {
        let me = &self.lumens[i];
        let recent =
            me.last_hit.1 != 0 && self.tick.wrapping_sub(me.last_hit.1) <= self.laws.engaged;
        let sight = Fx::int(self.laws.sight);
        let (x, y) = me.pos();
        let threat = self.lumens.iter().any(|o| {
            o.id != me.id
                && o.alive()
                && o.ghost == 0
                && len(o.me.body.x.sub(x), o.me.body.y.sub(y)) <= sight
        }) && !me.spark(&self.laws)
            && self.ring_at(x, y) != Ring::Sanctum;
        recent || threat
    }

    /// The page left: Linger in a fight, else Dream at once.
    pub fn leave(&mut self, id: u16) {
        let Some(i) = self.index(id) else { return };
        if self.lumens[i].alive() && self.in_a_fight(i) {
            let l = &mut self.lumens[i];
            l.linger = self.laws.linger.max(1);
            l.queue.clear();
            l.last = Default::default();
        } else {
            self.dream(id);
        }
    }

    /// Asleep: kept by soul (a guest is simply gone), the body fading.
    pub fn dream(&mut self, id: u16) {
        let Some(i) = self.index(id) else { return };
        self.drop_wick(i);
        // Fallen: it wakes where it would have returned.
        let back = (!self.lumens[i].alive()).then(|| self.return_spot(i));
        let l = &self.lumens[i];
        if l.soul != 0 && l.bot.is_none() {
            let mut d = Dreamer::of(l);
            if let Some((x, y)) = back {
                (d.x, d.y, d.flame) = (x, y, self.laws.flame);
            }
            self.dreamers.insert(l.soul, d);
        }
        self.events.push(Event::Dream { id });
        self.lumens.remove(i);
    }

    /// A soul comes back: its dreamer wakes where it slept (or a new Lumen
    /// is lit in the Sanctum). Its id.
    pub fn wake(&mut self, soul: u64, name: &str) -> u16 {
        let d = self.dreamers.remove(&soul);
        let id = self.spawn(name, soul, None);
        let i = self.lumens.len() - 1;
        let l = self.laws.clone();
        let claim = self.claim_of(soul).map(|c| c.id).unwrap_or(0);
        let lum = &mut self.lumens[i];
        lum.claim = claim;
        lum.me.body.claim = claim;
        if let Some(d) = d {
            lum.hue = d.hue;
            lum.flame = d.flame.clamp(1, l.flame);
            (lum.glim, lum.wood, lum.stone, lum.wheat) = (d.glim, d.wood, d.stone, d.wheat);
            lum.xp = d.xp;
            lum.played = d.played;
            lum.sparks_off = d.sparks_off;
            lum.home = d.home;
            lum.deaths = d.deaths;
            lum.ghost = l.wake_ghost;
            let spot = self.nearest_walkable(d.x, d.y);
            let lum = &mut self.lumens[i];
            lum.me = Me {
                body: Body::at(spot.0, spot.1, &l),
                ..Me::default()
            };
            lum.me.body.claim = claim;
            lum.me.body.load = lum.materials();
        } else if let Some(h) = self.claim(claim).and_then(|c| c.hearth) {
            let (x, y) = (Fx::int(h.0).add(Fx::HALF), Fx::int(h.1 + 2).add(Fx::HALF));
            let lum = &mut self.lumens[i];
            lum.me.body = Body::at(x, y, &l);
            lum.me.body.claim = claim;
        }
        self.events.push(Event::Return { id });
        id
    }

    /// The walkable tile centre nearest a point (for waking on land that
    /// was built on meanwhile).
    pub fn nearest_walkable(&self, x: Fx, y: Fx) -> (Fx, Fx) {
        let (cx, cy) = (x.floor(), y.floor());
        for r in 0..12i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    let t = self.tiles.get(cx + dx, cy + dy);
                    if !t.void() && !t.solid() {
                        if r == 0 {
                            return (x, y);
                        }
                        return (
                            Fx::int(cx + dx).add(Fx::HALF),
                            Fx::int(cy + dy).add(Fx::HALF),
                        );
                    }
                }
            }
        }
        (Fx::int(4), Fx::HALF)
    }

    /// Where a fallen Lumen returns: its hearth if it chose it, else the
    /// Luciphon.
    pub fn return_spot(&mut self, i: usize) -> (Fx, Fx) {
        let lum = &self.lumens[i];
        let hearth = self
            .claim(lum.claim)
            .and_then(|c| c.hearth)
            .filter(|_| lum.home);
        match hearth {
            Some(h) => {
                self.nearest_walkable(Fx::int(h.0).add(Fx::HALF), Fx::int(h.1 + 2).add(Fx::HALF))
            }
            None => self.spawn_spot(),
        }
    }

    /// Idle too long, or done Lingering: Dream.
    pub(crate) fn sleepers(&mut self) {
        let l = self.laws.clone();
        let mut gone = Vec::new();
        for lum in &mut self.lumens {
            if lum.bot.is_some() || !lum.alive() {
                continue;
            }
            if lum.linger > 0 {
                lum.linger -= 1;
                if lum.linger == 0 {
                    gone.push(lum.id);
                }
            } else if lum.idle >= l.idle_dream && lum.soul != 0 {
                gone.push(lum.id);
            }
        }
        for id in gone {
            self.dream(id);
        }
    }

    /// What a fall drops: by ring, half the glim taken by the Dark, the
    /// rest scattered within reach of where it last stood.
    pub(crate) fn drop_bag(&mut self, i: usize) -> u32 {
        let l = self.laws.clone();
        let lum = &self.lumens[i];
        let (sx, sy) = lum.me.body.safe;
        let share = match self.ring_at(sx, sy) {
            Ring::Rim => l.rim_drop,
            Ring::Dim => l.dim_drop,
            _ => 0,
        };
        let part = |v: u32| (v as i64 * share as i64 / 1000) as u32;
        let (glim, wood, stone, wheat) = (
            part(lum.glim),
            part(lum.wood),
            part(lum.stone),
            part(lum.wheat),
        );
        {
            let lum = &mut self.lumens[i];
            lum.glim -= glim;
            lum.wood -= wood;
            lum.stone -= stone;
            lum.wheat -= wheat;
            lum.me.body.load = lum.materials();
        }
        let mut left = (glim - glim / 2, wood, stone, wheat);
        let total = glim + wood + stone + wheat;
        while left != (0, 0, 0, 0) {
            let g = left.0.min(5);
            let (w, s, h) = (left.1.min(10), left.2.min(10), left.3.min(5));
            left = (left.0 - g, left.1 - w, left.2 - s, left.3 - h);
            let ang = self.rng.below(65536) as u16;
            let r = Fx((l.scatter.0 as i64 * self.rng.below(1000) as i64 / 1000) as i32);
            let (ux, uy) = engine::fixed::unit(ang);
            let (px, py) = (sx.add(ux.mul(r)), sy.add(uy.mul(r)));
            let t = self.tiles.under(px, py);
            let spot = if t.void() || t.solid() {
                (sx, sy)
            } else {
                (px, py)
            };
            let id = self.new_id();
            self.pickups.push(Pickup {
                id,
                x: spot.0,
                y: spot.1,
                glim: g,
                wood: w,
                stone: s,
                wheat: h,
            });
        }
        total
    }
}
