//! The browser's copy of what it has been sent: frames applied in order
//! rebuild every visible snake's body exactly, a head step at a time.
//! Alongside, kept apart from the exact bodies, is what drawing between
//! frames needs: the points just cut off each tail, and how far behind its
//! newest head (and how much shorter than its body) each snake is to be
//! drawn as a frame lands.

use std::collections::{BTreeMap, HashMap, VecDeque};

use crate::proto::{angle_from_u16, unq, Frame};

/// The most points a snake is drawn behind its head. Frames that pile up
/// (a hidden tab coming back) are jumped, not raced through.
const MOST_LAG: f32 = 8.0;

pub struct Seen {
    pub id: u16,
    pub name: String,
    pub hue: u8,
    /// Head first.
    pub body: VecDeque<(f32, f32)>,
    pub mass: u32,
    pub boosting: bool,
    pub ghost: bool,
    pub angle: f32,
    /// For drawing only, never part of the body: the points last cut off
    /// its tail, nearest first, so the tail can glide on to where it ends.
    pub trail: VecDeque<(f32, f32)>,
    /// As the last frame landed: how many points behind its newest head,
    /// and how many shorter than its body (below 0, longer), the snake was
    /// drawn. Both ease to nothing by the next frame.
    pub lag: f32,
    pub short: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Pellet {
    pub x: f32,
    pub y: f32,
    pub value: u8,
    pub hue: u8,
}

#[derive(Default)]
pub struct Mirror {
    pub tick: u32,
    pub you: u16,
    pub centre: (f32, f32),
    pub snakes: BTreeMap<u16, Seen>,
    pub food: HashMap<u32, Pellet>,
}

impl Mirror {
    /// Apply one frame, after the last one was drawn all the way. Returns
    /// the food eaten in it: (pellet, eaten by).
    pub fn apply(&mut self, f: &Frame) -> Vec<(Pellet, u16)> {
        self.apply_after(f, 0.0)
    }

    /// Apply one frame that lands with `left` (0..=1) of the way from the
    /// last one still to be drawn: what was not drawn yet carries on into
    /// this one, so no snake jumps however the frames arrive.
    pub fn apply_after(&mut self, f: &Frame, left: f32) -> Vec<(Pellet, u16)> {
        let left = left.clamp(0.0, 1.0);
        self.tick = f.tick;
        self.you = f.you;
        self.centre = (unq(f.centre.0), unq(f.centre.1));
        for id in &f.gone {
            self.snakes.remove(id);
        }
        for u in &f.snakes {
            let points = u.points.iter().map(|&(x, y)| (unq(x), unq(y)));
            match &u.new {
                Some((name, hue)) => {
                    self.snakes.insert(
                        u.id,
                        Seen {
                            id: u.id,
                            name: name.clone(),
                            hue: *hue,
                            body: points.collect(),
                            mass: u.mass,
                            boosting: u.boosting,
                            ghost: u.ghost,
                            angle: angle_from_u16(u.angle),
                            trail: VecDeque::new(),
                            lag: 0.0,
                            short: 0.0,
                        },
                    );
                    if let Some(s) = self.snakes.get_mut(&u.id) {
                        s.body.truncate((u.len as usize).max(1));
                    }
                }
                None => {
                    let Some(s) = self.snakes.get_mut(&u.id) else {
                        continue;
                    };
                    let was = s.body.len();
                    let fresh: Vec<(f32, f32)> = points.collect();
                    for &p in fresh.iter().rev() {
                        s.body.push_front(p);
                    }
                    let len = (u.len as usize).max(1);
                    if s.body.len() > len {
                        let cut: Vec<(f32, f32)> = s.body.drain(len..).collect();
                        for p in cut.into_iter().rev() {
                            s.trail.push_front(p);
                        }
                    }
                    s.lag = (fresh.len() as f32 + s.lag * left).min(MOST_LAG);
                    let grew = s.body.len() as f32 - was as f32;
                    s.short = (grew + s.short * left).clamp(-MOST_LAG, MOST_LAG);
                    // Keep as much trail as the drawing can reach into.
                    s.trail
                        .truncate((s.lag - s.short).max(0.0).ceil() as usize + 2);
                    s.mass = u.mass;
                    s.boosting = u.boosting;
                    s.ghost = u.ghost;
                    s.angle = angle_from_u16(u.angle);
                }
            }
        }
        for fd in &f.food {
            self.food.insert(
                fd.id,
                Pellet {
                    x: unq(fd.x),
                    y: unq(fd.y),
                    value: fd.value,
                    hue: fd.hue,
                },
            );
        }
        f.eaten
            .iter()
            .filter_map(|&(id, by)| self.food.remove(&id).map(|p| (p, by)))
            .collect()
    }
}
