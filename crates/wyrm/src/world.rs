//! The arena: snakes that steer, eat, grow and burst, and the food on the
//! ground. The server runs one `World` and calls `step` every tick.

mod save;

use std::collections::{HashMap, VecDeque};

use crate::bots::{self, Bot};
use crate::grid::Grid;
use crate::laws::*;
use engine::rng::Rng;

pub struct Snake {
    pub id: u16,
    /// Whose it is (0 for a bot or a guest).
    pub soul: u64,
    pub name: String,
    pub hue: u8,
    /// Body points, the head first, `STEP` apart.
    pub body: VecDeque<(f32, f32)>,
    pub angle: f32,
    /// Where its person (or its bot) wants it to head.
    pub want: f32,
    pub boost: bool,
    /// Whether it boosted on the last tick.
    pub boosting: bool,
    pub mass: f32,
    pub bot: Option<Bot>,
    /// Points added to the head on the last tick.
    pub moved: u8,
    pub kills: u32,
    /// The tick it was born at.
    pub born: u32,
    /// Until this tick it passes through everyone, and they through it.
    pub ghost_until: u32,
    /// Loaded from a save and waiting for its person: frozen and harmless
    /// until they come back, bursting at this tick if they do not. 0 when
    /// it is not waiting.
    pub held_until: u32,
    /// Food owed to the ground from boosting, not yet dropped.
    owed: f32,
    /// The box around its body: (x0, y0, x1, y1).
    pub bbox: (f32, f32, f32, f32),
}

impl Snake {
    pub fn head(&self) -> (f32, f32) {
        self.body.front().copied().unwrap_or((0.0, 0.0))
    }

    pub fn radius(&self) -> f32 {
        radius(self.mass)
    }

    pub fn ghost(&self, tick: u32) -> bool {
        tick < self.ghost_until || self.held_until != 0
    }

    /// Length as people see it: mass, rounded.
    pub fn score(&self) -> u32 {
        self.mass.max(0.0) as u32
    }
}

pub struct Food {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub value: u8,
    pub hue: u8,
    /// Tick it rots away at; 0 for food that grew here.
    pub until: u32,
}

/// A snake that burst.
pub struct Death {
    pub id: u16,
    pub name: String,
    pub score: u32,
    pub kills: u32,
    /// Ticks it lived.
    pub age: u32,
    /// Whose body it ran into; None for the arena's edge.
    pub killer: Option<(u16, String)>,
    pub human: bool,
}

pub struct World {
    pub tick: u32,
    pub snakes: Vec<Snake>,
    pub food: HashMap<u32, Food>,
    pub(crate) food_grid: Grid<u32>,
    /// Every body point: (snake index, point index). Rebuilt each tick.
    pub(crate) bodies: Grid<(u16, u16)>,
    rng: Rng,
    next_snake: u16,
    next_food: u32,
    /// Ticks at which a bot may next be born.
    bot_at: u32,
    /// Food eaten on the last tick, and by whom.
    pub eaten: Vec<(u32, u16)>,
    /// Snakes that burst on the last tick: where, their hue, their radius.
    pub bursts: Vec<(f32, f32, u8, f32)>,
    /// What people can see: the box each browser was last sent (x0, y0,
    /// x1, y1). The room fills it in; a bot leaves only from outside them.
    pub eyes: Vec<(f32, f32, f32, f32)>,
}

impl World {
    pub fn new(seed: u64) -> World {
        let mut w = World {
            tick: 0,
            snakes: Vec::new(),
            food: HashMap::new(),
            food_grid: Grid::new(64.0),
            bodies: Grid::new(48.0),
            rng: Rng::new(seed),
            next_snake: 1,
            next_food: 1,
            bot_at: 0,
            eaten: Vec::new(),
            bursts: Vec::new(),
            eyes: Vec::new(),
        };
        while w.food.len() < FOOD_TARGET {
            w.grow_food();
        }
        // A new arena starts full.
        for _ in 0..CROWD {
            let (name, bot) = bots::recruit(&mut w.rng, &w.snakes);
            w.spawn(&name, Some(bot));
        }
        w
    }

    fn unit(&mut self) -> f32 {
        (self.rng.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn find(&self, id: u16) -> Option<&Snake> {
        self.snakes.iter().find(|s| s.id == id)
    }

    pub fn humans(&self) -> usize {
        self.snakes.iter().filter(|s| s.bot.is_none()).count()
    }

    /// A new snake, somewhere clear. Returns its id.
    pub fn spawn(&mut self, name: &str, bot: Option<Bot>) -> u16 {
        let mut id = self.next_snake;
        while id == 0 || self.snakes.iter().any(|s| s.id == id) {
            id = id.wrapping_add(1);
        }
        self.next_snake = id.wrapping_add(1);
        let (x, y) = self.clear_spot();
        let angle = wrap(self.unit() * std::f32::consts::TAU);
        let mass = START_MASS;
        let body: VecDeque<(f32, f32)> = (0..body_len(mass))
            .map(|k| {
                let d = k as f32 * STEP;
                (x - angle.cos() * d, y - angle.sin() * d)
            })
            .collect();
        let hue = self.rng.below(256) as u8;
        let ghost_until = if bot.is_none() {
            self.tick + GHOST_TICKS
        } else {
            0
        };
        // Someone who gave no name still needs one the feed can tell apart.
        let name = engine::who::clean_name(name);
        self.snakes.push(Snake {
            id,
            soul: 0,
            name: if name.is_empty() {
                format!("snake {id}")
            } else {
                name
            },
            hue,
            body,
            angle,
            want: angle,
            boost: false,
            boosting: false,
            mass,
            bot,
            moved: 0,
            kills: 0,
            born: self.tick,
            ghost_until,
            held_until: 0,
            owed: 0.0,
            bbox: (x, y, x, y),
        });
        self.rebuild_bodies();
        id
    }

    /// A place well away from every body, inside the arena.
    fn clear_spot(&mut self) -> (f32, f32) {
        let mut best = (0.0, 0.0, -1.0f32);
        for _ in 0..SPAWN_TRIES {
            let a = self.unit() * std::f32::consts::TAU;
            let r = self.unit().sqrt() * ARENA * SPAWN_RING;
            let (x, y) = (a.cos() * r, a.sin() * r);
            let near = self
                .bodies
                .near(x, y, SPAWN_CLEAR)
                .map(|(s, k)| {
                    let p = self.snakes[s as usize].body[k as usize];
                    ((p.0 - x).powi(2) + (p.1 - y).powi(2)).sqrt()
                })
                .fold(f32::MAX, f32::min);
            if near > SPAWN_CLEAR {
                return (x, y);
            }
            if near > best.2 {
                best = (x, y, near);
            }
        }
        (best.0, best.1)
    }

    /// Steer a snake; the angle is where it should head.
    pub fn steer(&mut self, id: u16, angle: f32, boost: bool) {
        if let Some(s) = self.snakes.iter_mut().find(|s| s.id == id) {
            if angle.is_finite() {
                s.want = angle;
            }
            s.boost = boost;
        }
    }

    /// A person's snake waiting for them (after a restart), or theirs in
    /// another tab: it is theirs again, a ghost for a moment. Its id.
    pub fn claim(&mut self, soul: u64) -> Option<u16> {
        let tick = self.tick;
        let s = self
            .snakes
            .iter_mut()
            .find(|s| soul != 0 && s.soul == soul && s.bot.is_none())?;
        if s.held_until != 0 {
            s.held_until = 0;
            s.ghost_until = tick + RESUME_GHOST;
        }
        Some(s.id)
    }

    /// A person left: their snake bursts like any other.
    pub fn remove(&mut self, id: u16) -> Option<Death> {
        let i = self.snakes.iter().position(|s| s.id == id)?;
        Some(self.burst(i, None))
    }

    fn grow_food(&mut self) {
        let a = self.unit() * std::f32::consts::TAU;
        let r = self.unit().sqrt() * (ARENA - 30.0);
        let value = 1 + self.rng.below(NATURAL_FOOD_MAX as u64) as u8;
        let hue = self.rng.below(256) as u8;
        self.drop_food(a.cos() * r, a.sin() * r, value, hue, 0);
    }

    fn drop_food(&mut self, x: f32, y: f32, value: u8, hue: u8, until: u32) {
        let id = self.next_food;
        self.next_food = self.next_food.wrapping_add(1).max(1);
        self.food_grid.insert(x, y, id);
        self.food.insert(
            id,
            Food {
                id,
                x,
                y,
                value,
                hue,
                until,
            },
        );
    }

    fn eat(&mut self, id: u32) -> u8 {
        match self.food.remove(&id) {
            Some(f) => {
                self.food_grid.remove(f.x, f.y, id);
                f.value
            }
            None => 0,
        }
    }

    fn rebuild_bodies(&mut self) {
        self.bodies.clear();
        for (si, s) in self.snakes.iter().enumerate() {
            for (k, &(x, y)) in s.body.iter().enumerate() {
                self.bodies.insert(x, y, (si as u16, k as u16));
            }
        }
    }

    /// Bots keep the arena busy when few people are in it.
    fn populate(&mut self) {
        let humans = self.humans();
        let bots = self.snakes.len() - humans;
        let want = CROWD.saturating_sub(humans).max(MIN_BOTS);
        if bots < want && self.tick >= self.bot_at {
            let (name, bot) = bots::recruit(&mut self.rng, &self.snakes);
            self.spawn(&name, Some(bot));
            self.bot_at = self.tick + BOT_RESPAWN / 4;
        } else if bots > want {
            self.retire();
        }
    }

    /// Too crowded: a bot steps out quietly, with no burst and no food,
    /// where nobody sees it go: the smallest no browser was sent (players
    /// and watchers alike). While every bot is in someone's view, none
    /// goes yet.
    fn retire(&mut self) {
        let seen = |s: &Snake| {
            let (x0, y0, x1, y1) = s.bbox;
            self.eyes
                .iter()
                .any(|e| x1 >= e.0 && x0 <= e.2 && y1 >= e.1 && y0 <= e.3)
        };
        let pick = self
            .snakes
            .iter()
            .enumerate()
            .filter(|(_, s)| s.bot.is_some() && !seen(s))
            .min_by(|a, b| a.1.mass.total_cmp(&b.1.mass))
            .map(|(i, _)| i);
        if let Some(i) = pick {
            self.snakes.remove(i);
            self.rebuild_bodies();
        }
    }

    /// One tick of the arena. Returns the snakes that burst.
    pub fn step(&mut self) -> Vec<Death> {
        self.tick = self.tick.wrapping_add(1);
        self.eaten.clear();
        self.bursts.clear();

        // Bots decide where to go.
        let plans: Vec<(usize, f32, bool)> = (0..self.snakes.len())
            .filter(|&i| self.snakes[i].bot.is_some())
            .map(|i| {
                let (a, b) = bots::think(self, i);
                (i, a, b)
            })
            .collect();
        for (i, a, b) in plans {
            self.snakes[i].want = a;
            self.snakes[i].boost = b;
        }

        // Everyone moves.
        let mut dropped = Vec::new();
        for s in &mut self.snakes {
            if s.held_until != 0 {
                s.moved = 0;
                s.boosting = false;
                continue;
            }
            s.boosting = s.boost && s.mass >= MIN_BOOST_MASS;
            let steps = if s.boosting { BOOST_STEPS } else { STEPS };
            let r = radius(s.mass);
            for _ in 0..steps {
                let t = turn(r);
                s.angle = wrap(s.angle + wrap(s.want - s.angle).clamp(-t, t));
                let (x, y) = s.head();
                s.body
                    .push_front((x + s.angle.cos() * STEP, y + s.angle.sin() * STEP));
            }
            s.moved = steps;
            if s.boosting {
                let cost = boost_cost(s.mass);
                s.mass -= cost;
                s.owed += cost * BOOST_DROP;
                if s.owed >= 1.0 {
                    let v = (s.owed as u8).min(FOOD_MAX);
                    s.owed -= v as f32;
                    if let Some(&(x, y)) = s.body.back() {
                        dropped.push((x, y, v, s.hue));
                    }
                }
            }
            s.body.truncate(body_len(s.mass).max(2));
        }
        for (x, y, v, h) in dropped {
            self.drop_food(x, y, v, h, self.tick + ROT_TICKS);
        }

        // Everyone eats what is under their mouth.
        for i in 0..self.snakes.len() {
            if self.snakes[i].held_until != 0 {
                continue;
            }
            let (hx, hy) = self.snakes[i].head();
            let r = self.snakes[i].radius();
            let reach = r + MOUTH_REACH;
            let meals: Vec<u32> = self
                .food_grid
                .near(hx, hy, reach + 12.0)
                .filter(|id| {
                    self.food.get(id).is_some_and(|f| {
                        let d = (f.x - hx).powi(2) + (f.y - hy).powi(2);
                        d < (reach + food_radius(f.value)).powi(2)
                    })
                })
                .collect();
            for id in meals {
                let v = self.eat(id);
                self.snakes[i].mass += v as f32;
                self.eaten.push((id, self.snakes[i].id));
            }
        }

        // Who ran into whom.
        self.rebuild_bodies();
        let mut dead: Vec<(usize, Option<usize>)> = Vec::new();
        for (i, s) in self.snakes.iter().enumerate() {
            if s.held_until != 0 {
                // Nobody came back for it.
                if s.held_until <= self.tick {
                    dead.push((i, None));
                }
                continue;
            }
            let (hx, hy) = s.head();
            let r = s.radius();
            if hx * hx + hy * hy > (ARENA - r).powi(2) {
                dead.push((i, None));
                continue;
            }
            if s.ghost(self.tick) {
                continue;
            }
            let hit = self.bodies.near(hx, hy, r + 44.0).find(|&(o, k)| {
                let o = o as usize;
                if o == i {
                    return false;
                }
                let other = &self.snakes[o];
                if other.ghost(self.tick) {
                    return false;
                }
                let p = other.body[k as usize];
                let reach = (r + other.radius()) * HIT_FORGIVE;
                (p.0 - hx).powi(2) + (p.1 - hy).powi(2) < reach * reach
            });
            if let Some((o, _)) = hit {
                dead.push((i, Some(o as usize)));
            }
        }
        // Credit the killers first, then burst from the back so the
        // indices still to come stay good.
        let mut dead: Vec<(usize, Option<(u16, String)>)> = dead
            .into_iter()
            .map(|(i, killer)| {
                let k = killer.map(|o| {
                    self.snakes[o].kills += 1;
                    (self.snakes[o].id, self.snakes[o].name.clone())
                });
                (i, k)
            })
            .collect();
        dead.sort_by_key(|d| std::cmp::Reverse(d.0));
        let mut deaths = Vec::new();
        for (i, killer) in dead {
            deaths.push(self.burst_with(i, killer));
        }
        if !deaths.is_empty() {
            self.rebuild_bodies();
        }

        // Food grows back; dropped food rots.
        let mut n = 0;
        while self.food.len() < FOOD_TARGET && n < FOOD_REGROW {
            self.grow_food();
            n += 1;
        }
        if self.tick.is_multiple_of(TICK_HZ) {
            let rotten: Vec<u32> = self
                .food
                .values()
                .filter(|f| f.until != 0 && f.until <= self.tick)
                .map(|f| f.id)
                .collect();
            for id in rotten {
                self.eat(id);
            }
        }

        self.populate();
        self.rebuild_bodies();
        self.fit_boxes();
        deaths
    }

    fn fit_boxes(&mut self) {
        for s in &mut self.snakes {
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for &(x, y) in &s.body {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
            s.bbox = (x0, y0, x1, y1);
        }
    }

    fn burst(&mut self, i: usize, killer: Option<(u16, String)>) -> Death {
        let d = self.burst_with(i, killer);
        self.rebuild_bodies();
        d
    }

    /// Turn a snake into food along where its body lay: `DEATH_DROP` of
    /// its mass, in pellets of at most `FOOD_MAX`, about one for every
    /// other point of its body (more for a snake heavier than it is long).
    fn burst_with(&mut self, i: usize, killer: Option<(u16, String)>) -> Death {
        let s = self.snakes.remove(i);
        let (hx, hy) = s.head();
        let r = s.radius();
        self.bursts.push((hx, hy, s.hue, r));
        let total = (s.mass * DEATH_DROP).round().max(1.0) as usize;
        let points: Vec<(f32, f32)> = s.body.iter().step_by(2).copied().collect();
        let count = total
            .div_ceil(FOOD_MAX as usize)
            .max(points.len().min(total))
            .max(1);
        let (per, extra) = (total / count, total % count);
        for j in 0..count {
            let (x, y) = points
                .get(j * points.len() / count)
                .copied()
                .unwrap_or((hx, hy));
            let jx = (self.unit() - 0.5) * r;
            let jy = (self.unit() - 0.5) * r;
            let hue = s.hue.wrapping_add(self.rng.below(BURST_HUES as u64) as u8);
            let value = (per + (j < extra) as usize) as u8;
            self.drop_food(x + jx, y + jy, value, hue, self.tick + ROT_TICKS);
        }
        if s.bot.is_some() {
            self.bot_at = self.bot_at.max(self.tick + BOT_RESPAWN);
        }
        Death {
            id: s.id,
            name: s.name.clone(),
            score: s.score(),
            kills: s.kills,
            age: self.tick.wrapping_sub(s.born),
            killer,
            human: s.bot.is_none(),
        }
    }

    /// How far a snake at `from` heading `angle` can go, up to `look`,
    /// before it meets another snake's body or the edge.
    pub fn clearance(&self, me: usize, from: (f32, f32), angle: f32, look: f32) -> f32 {
        let r = self.snakes[me].radius();
        let (c, s) = (angle.cos(), angle.sin());
        let mut d = r;
        while d < look {
            let (x, y) = (from.0 + c * d, from.1 + s * d);
            if x * x + y * y > (ARENA - r - 20.0).powi(2) {
                return d;
            }
            let blocked = self.bodies.near(x, y, r + 46.0).any(|(o, k)| {
                if o as usize == me {
                    return false;
                }
                let other = &self.snakes[o as usize];
                let p = other.body[k as usize];
                let gap = r + other.radius() + 8.0;
                (p.0 - x).powi(2) + (p.1 - y).powi(2) < gap * gap
            });
            if blocked {
                return d;
            }
            d += r.max(12.0);
        }
        look
    }

    /// Food near a point: (id, x, y, value).
    pub fn food_near(&self, x: f32, y: f32, r: f32) -> impl Iterator<Item = &Food> + '_ {
        self.food_grid
            .near(x, y, r)
            .filter_map(move |id| self.food.get(&id))
    }

    /// Food inside a box.
    pub fn food_within(
        &self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    ) -> impl Iterator<Item = &Food> + '_ {
        self.food_grid
            .within(x0, y0, x1, y1)
            .filter_map(move |id| self.food.get(&id))
            .filter(move |f| f.x >= x0 && f.x <= x1 && f.y >= y0 && f.y <= y1)
    }
}
