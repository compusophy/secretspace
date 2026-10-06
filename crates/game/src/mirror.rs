//! The browser's copy of what it has been sent: frames applied in order
//! rebuild every visible snake's body exactly, a head step at a time.

use std::collections::{BTreeMap, HashMap, VecDeque};

use crate::proto::{angle_from_u16, unq, Frame};

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
    /// Points the head gained in the last frame (for smooth drawing).
    pub moved: usize,
    /// Points the head has gained since it was first seen: point k of the
    /// body is number `seq - k`, so stripes stay put as it moves.
    pub seq: u32,
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
    /// Apply one frame. Returns the food eaten in it: (pellet, eaten by).
    pub fn apply(&mut self, f: &Frame) -> Vec<(Pellet, u16)> {
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
                            moved: 0,
                            seq: 1 << 30,
                        },
                    );
                }
                None => {
                    let Some(s) = self.snakes.get_mut(&u.id) else {
                        continue;
                    };
                    let fresh: Vec<(f32, f32)> = points.collect();
                    for &p in fresh.iter().rev() {
                        s.body.push_front(p);
                    }
                    s.moved = fresh.len();
                    s.seq = s.seq.wrapping_add(fresh.len() as u32);
                    s.mass = u.mass;
                    s.boosting = u.boosting;
                    s.ghost = u.ghost;
                    s.angle = angle_from_u16(u.angle);
                }
            }
            if let Some(s) = self.snakes.get_mut(&u.id) {
                s.body.truncate((u.len as usize).max(1));
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
