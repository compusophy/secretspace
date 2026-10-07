//! What one browser sees: the part of the arena around its snake (or the
//! snake it watches), sent as the changes since its last frame.

use std::collections::{HashMap, HashSet};

use crate::laws::{radius, view_scale, ARENA};
use crate::proto::{self, angle_to_u16, q, Board, FoodInfo, Frame, Leader, SnakeUpdate};
use crate::world::World;

/// World units past the screen's edge that are sent anyway.
const MARGIN: f32 = 120.0;

pub struct Viewer {
    /// The snake this browser plays, 0 when it is not playing.
    pub you: u16,
    /// The snake it watches when not playing.
    pub watching: u16,
    pub screen: (f32, f32),
    pub centre: (f32, f32),
    known: HashSet<u16>,
    food: HashSet<u32>,
}

impl Default for Viewer {
    fn default() -> Viewer {
        Viewer {
            you: 0,
            watching: 0,
            screen: (1280.0, 800.0),
            centre: (0.0, 0.0),
            known: HashSet::new(),
            food: HashSet::new(),
        }
    }
}

impl Viewer {
    /// The next frame for this browser, as bytes.
    pub fn frame(&mut self, w: &World) -> Vec<u8> {
        let followed = w
            .find(self.you)
            .or_else(|| w.find(self.watching))
            .or_else(|| w.snakes.iter().max_by(|a, b| a.mass.total_cmp(&b.mass)));
        if let Some(s) = followed {
            if self.you == 0 {
                self.watching = s.id;
            }
            self.centre = s.head();
        }
        let r = match w.find(self.you) {
            Some(s) => radius(s.mass),
            None => 18.0,
        };
        let scale = view_scale(r, self.screen.0, self.screen.1);
        let hw = self.screen.0.clamp(200.0, 3840.0) / 2.0 / scale + MARGIN;
        let hh = self.screen.1.clamp(200.0, 2160.0) / 2.0 / scale + MARGIN;
        let (x0, y0, x1, y1) = (
            self.centre.0 - hw,
            self.centre.1 - hh,
            self.centre.0 + hw,
            self.centre.1 + hh,
        );

        let mut f = Frame {
            tick: w.tick,
            you: if w.find(self.you).is_some() {
                self.you
            } else {
                0
            },
            centre: (q(self.centre.0), q(self.centre.1)),
            ..Frame::default()
        };
        let mut seen = HashSet::new();
        for s in &w.snakes {
            let (bx0, by0, bx1, by1) = s.bbox;
            let inside = bx1 >= x0 && bx0 <= x1 && by1 >= y0 && by0 <= y1;
            if !inside && s.id != self.you {
                continue;
            }
            seen.insert(s.id);
            let fresh = self.known.insert(s.id);
            let take = if fresh {
                s.body.len()
            } else {
                s.moved as usize
            };
            f.snakes.push(SnakeUpdate {
                id: s.id,
                boosting: s.boosting,
                ghost: s.ghost(w.tick),
                mass: s.score(),
                len: s.body.len() as u16,
                angle: angle_to_u16(s.angle),
                new: fresh.then(|| (s.name.clone(), s.hue)),
                points: s.body.iter().take(take).map(|p| (q(p.0), q(p.1))).collect(),
            });
        }
        f.gone = self.known.difference(&seen).copied().collect();
        f.gone.sort_unstable();
        self.known = seen;

        let eaten: HashMap<u32, u16> = w.eaten.iter().copied().collect();
        let mut here = HashSet::new();
        for fd in w.food_within(x0, y0, x1, y1) {
            here.insert(fd.id);
            if !self.food.contains(&fd.id) {
                f.food.push(FoodInfo {
                    id: fd.id,
                    x: q(fd.x),
                    y: q(fd.y),
                    value: fd.value,
                    hue: fd.hue,
                });
            }
        }
        f.eaten = self
            .food
            .difference(&here)
            .map(|id| (*id, eaten.get(id).copied().unwrap_or(0)))
            .collect();
        f.eaten.sort_unstable();
        self.food = here;
        f.bursts = w
            .bursts
            .iter()
            .filter(|b| b.0 >= x0 && b.0 <= x1 && b.1 >= y0 && b.1 <= y1)
            .map(|&(x, y, hue, r)| (q(x), q(y), hue, r as u8))
            .collect();
        proto::encode_frame(&f)
    }

    /// Forget everything sent, so the next frame starts afresh.
    pub fn reset(&mut self) {
        self.known.clear();
        self.food.clear();
    }
}

/// The leaderboard and minimap, as `you` should see them; `people` is
/// how many browsers are connected.
pub fn board(w: &World, you: u16, people: u16) -> Vec<u8> {
    let mut order: Vec<&crate::world::Snake> = w.snakes.iter().collect();
    order.sort_by(|a, b| b.mass.total_cmp(&a.mass).then(a.id.cmp(&b.id)));
    let rank = order
        .iter()
        .position(|s| s.id == you)
        .map_or(0, |p| p as u16 + 1);
    let b = Board {
        people,
        snakes: w.snakes.len() as u16,
        top: order
            .iter()
            .take(10)
            .map(|s| Leader {
                name: s.name.clone(),
                score: s.score(),
                hue: s.hue,
                human: s.bot.is_none(),
            })
            .collect(),
        rank,
        dots: w
            .snakes
            .iter()
            .map(|s| {
                let (x, y) = s.head();
                let k = |v: f32| (v / ARENA * 127.0).clamp(-127.0, 127.0) as i8;
                (k(x), k(y), s.radius() as u8)
            })
            .collect(),
    };
    proto::encode_board(&b)
}
