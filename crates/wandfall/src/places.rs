//! The island's places, from its seed: the Spire at the centre (a
//! wizard's tower on a raised plaza) and, on a ring about it, a stone
//! circle, a demon rift and a crystal grove. Each shapes the ground under
//! it and sets its own stones, spikes and crystals, which block as trees
//! do, and marks where its chests wait.

use engine::rng::Rng;

use crate::laws::*;
use crate::map::{smooth, Kind, Map, Prop};
use crate::trig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Spire,
    Circle,
    Rift,
    Grove,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Poi {
    pub place: Place,
    pub x: f32,
    pub z: f32,
    /// How wide (metres), and the ground's level there.
    pub r: f32,
    pub level: f32,
}

impl Poi {
    pub fn dist(&self, x: f32, z: f32) -> f32 {
        let (dx, dz) = (x - self.x, z - self.z);
        (dx * dx + dz * dz).sqrt()
    }

    /// How far out it shapes the ground.
    pub fn reach(&self) -> f32 {
        match self.place {
            Place::Spire => self.r + PLATEAU_FALL,
            Place::Circle => self.r * 1.5,
            Place::Rift => self.r * 1.6,
            Place::Grove => self.r * 1.4,
        }
    }

    /// The ground at (x, z) as this place shapes it, from `h`, the hills'.
    pub fn shape(&self, h: f32, x: f32, z: f32) -> f32 {
        let d = self.dist(x, z);
        if d >= self.reach() {
            return h;
        }
        let (want, f) = match self.place {
            // A flat plaza, falling smoothly to the hills.
            Place::Spire => (self.level, smooth((d - self.r) / PLATEAU_FALL)),
            // A raised green, level for the stones.
            Place::Circle => (
                self.level + 0.5,
                smooth((d - self.r * 0.7) / (self.r * 0.8)),
            ),
            // A scorched bowl inside a raised lip.
            Place::Rift => {
                let bowl = self.level - RIFT_DEPTH * (1.0 - smooth(d / (self.r * 0.85)));
                let t = (d - self.r) / (self.r * 0.3);
                let lip = (1.0 - t * t).max(0.0) * 1.4;
                (bowl + lip, smooth((d - self.r * 0.9) / (self.r * 0.7)))
            }
            // A knoll the crystals grow from.
            Place::Grove => (
                self.level + 1.2 * (1.0 - smooth(d / self.r)),
                smooth((d - self.r * 0.5) / (self.r * 0.9)),
            ),
        };
        want * (1.0 - f) + h * f
    }
}

fn unit(rng: &mut Rng) -> f32 {
    (rng.next_u64() >> 40) as f32 / (1u64 << 24) as f32
}

/// Where the places are, from the seed and the bare hills.
pub fn find(seed: u64, hills: impl Fn(f32, f32) -> f32) -> Vec<Poi> {
    let mut rng = Rng::new(seed ^ 0x9a1ce);
    let mut v = vec![Poi {
        place: Place::Spire,
        x: 0.0,
        z: 0.0,
        r: PLATEAU,
        level: PLATEAU_TOP,
    }];
    let first = (rng.next_u64() >> 48) as u16;
    for (k, place) in [Place::Circle, Place::Rift, Place::Grove]
        .into_iter()
        .enumerate()
    {
        let jitter = ((rng.next_u64() >> 52) as u16).wrapping_sub(2048);
        let turn = first
            .wrapping_add((k as u32 * 65536 / 3) as u16)
            .wrapping_add(jitter);
        let (s, c) = trig::sin_cos(turn);
        let ring = POI_RING + (unit(&mut rng) - 0.5) * 16.0;
        let (x, z) = (c * ring, s * ring);
        let floor = match place {
            Place::Rift => SEA + RIFT_DEPTH + 1.5,
            _ => SEA + 2.0,
        };
        v.push(Poi {
            place,
            x,
            z,
            r: POI_RADIUS,
            level: hills(x, z).max(floor),
        });
    }
    v
}

/// Around `p`, `d` out at `turn`.
fn about(p: &Poi, turn: u16, d: f32) -> (f32, f32) {
    let (s, c) = trig::sin_cos(turn);
    (p.x + c * d, p.z + s * d)
}

/// Set each place's stones, spikes and crystals; mark its chests.
pub fn set(m: &mut Map) {
    let mut rng = Rng::new(m.seed ^ 0x5e7);
    for p in m.pois.clone() {
        let stand = |m: &mut Map, kind: Kind, (x, z): (f32, f32), r: f32, h: f32, yaw: f32| {
            let y = m.height(x, z) - 0.3;
            m.put(Prop {
                kind,
                x,
                z,
                y,
                r,
                h,
                yaw,
                scale: h,
            });
        };
        let quarter = |k: u32| (k * 16384) as u16;
        match p.place {
            Place::Spire => {
                stand(m, Kind::Tower, (p.x, p.z), TOWER_RADIUS, TOWER_HEIGHT, 0.0);
                // Lamps about the plaza, and a chest between each pair.
                for k in 0..8u32 {
                    let turn = (k * 8192 + 4096) as u16;
                    stand(m, Kind::Lamp, about(&p, turn, p.r - 4.0), 0.3, 3.2, 0.0);
                }
                for k in 0..4 {
                    let at = about(&p, quarter(k), TOWER_RADIUS + 6.0);
                    m.caches.push([at.0, at.1]);
                }
            }
            Place::Circle => {
                // Standing stones facing the altar; one or two have gone.
                let n = 9u32;
                for k in 0..n {
                    if unit(&mut rng) < 0.12 {
                        continue;
                    }
                    let turn = (k * 65536 / n) as u16;
                    let h = 4.0 + unit(&mut rng) * 1.8;
                    let yaw = trig::radians(turn);
                    stand(m, Kind::Stone, about(&p, turn, p.r * 0.55), 0.9, h, yaw);
                }
                stand(m, Kind::Altar, (p.x, p.z), 1.1, 1.1, 0.0);
                for k in [0, 2] {
                    let at = about(&p, quarter(k) + 8192, 5.5);
                    m.caches.push([at.0, at.1]);
                }
            }
            Place::Rift => {
                stand(
                    m,
                    Kind::Portal,
                    (p.x, p.z),
                    1.6,
                    6.5,
                    unit(&mut rng) * std::f32::consts::PI,
                );
                // Obsidian thrust up about the gate.
                let mut n = 0;
                let mut tries = 0;
                while n < 13 && tries < 200 {
                    tries += 1;
                    let turn = (rng.next_u64() >> 48) as u16;
                    let at = about(&p, turn, p.r * (0.3 + 0.75 * unit(&mut rng)));
                    let r = 0.5 + 0.6 * unit(&mut rng);
                    if m.near(at.0, at.1, r + 1.6).next().is_some() {
                        continue;
                    }
                    let h = 2.5 + 4.5 * unit(&mut rng);
                    stand(m, Kind::Spike, at, r, h, trig::radians(turn));
                    n += 1;
                }
                for k in [1, 3] {
                    let at = about(&p, quarter(k), 7.0);
                    m.caches.push([at.0, at.1]);
                }
            }
            Place::Grove => {
                let mut n = 0;
                let mut tries = 0;
                while n < 14 && tries < 200 {
                    tries += 1;
                    let turn = (rng.next_u64() >> 48) as u16;
                    let at = about(&p, turn, p.r * 0.95 * unit(&mut rng).sqrt());
                    let r = 0.5 + 0.7 * unit(&mut rng);
                    if m.near(at.0, at.1, r + 1.4).next().is_some() {
                        continue;
                    }
                    let h = 2.0 + 4.0 * unit(&mut rng);
                    stand(m, Kind::Crystal, at, r, h, trig::radians(turn));
                    n += 1;
                }
                for k in 0..6u32 {
                    let turn = (k * 65536 / 6) as u16 ^ (rng.next_u64() >> 52) as u16;
                    let at = about(&p, turn, p.r * 1.1);
                    if m.near(at.0, at.1, 1.5).next().is_none() {
                        let h = 3.0 + 2.0 * unit(&mut rng);
                        stand(m, Kind::Shroom, at, 0.4, h, trig::radians(turn));
                    }
                }
                for k in [0, 2] {
                    let at = about(&p, quarter(k) + 4096, 6.0);
                    m.caches.push([at.0, at.1]);
                }
            }
        }
    }
}
