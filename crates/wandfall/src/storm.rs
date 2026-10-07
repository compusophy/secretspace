//! The storm: a circle that waits, then closes to a smaller one inside
//! it, phase after phase, until nothing is left. Outside it you are hurt
//! every second, more each phase. Planned whole when the fight begins.

use engine::rng::Rng;

use crate::laws::{STORM, STORM_DRIFT, STORM_START, TICK_HZ};
use crate::map::Map;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Storm {
    /// The circles: where it starts, then where each phase closes to.
    pub circles: Vec<([f32; 2], f32)>,
}

/// The storm at a moment.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Now {
    pub centre: [f32; 2],
    pub r: f32,
    /// The circle it closes to next.
    pub next: ([f32; 2], f32),
    pub phase: usize,
    pub shrinking: bool,
    /// Seconds until it starts (or stops) closing.
    pub secs: u32,
    /// Damage a second outside.
    pub dps: i32,
}

impl Storm {
    pub fn plan(map: &Map, rng: &mut Rng) -> Storm {
        let mut circles = vec![([0.0, 0.0], STORM_START)];
        for &(_, _, r, _) in &STORM {
            let (c, prev) = *circles.last().unwrap();
            let room = (prev - r).max(0.0) * STORM_DRIFT;
            let mut best = c;
            for _ in 0..12 {
                let u = |rng: &mut Rng| (rng.next_u64() >> 40) as f32 / (1u64 << 24) as f32;
                let (dx, dz) = (u(rng) * 2.0 - 1.0, u(rng) * 2.0 - 1.0);
                let p = [c[0] + dx * room, c[1] + dz * room];
                // Inside the last circle, and over land.
                let off = ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt();
                if off + r <= prev && map.land(p[0], p[1]) {
                    best = p;
                    break;
                }
            }
            circles.push((best, r));
        }
        Storm { circles }
    }

    /// The storm `ticks` after the fight began.
    pub fn at(&self, ticks: u32) -> Now {
        let mut t = ticks as f32 / TICK_HZ as f32;
        for (k, &(wait, shrink, _, dps)) in STORM.iter().enumerate() {
            let (from, to) = (self.circles[k], self.circles[k + 1]);
            if t < wait as f32 {
                return Now {
                    centre: from.0,
                    r: from.1,
                    next: to,
                    phase: k,
                    shrinking: false,
                    secs: (wait as f32 - t).ceil() as u32,
                    dps,
                };
            }
            t -= wait as f32;
            if t < shrink as f32 {
                let k2 = t / shrink as f32;
                let lerp = |a: f32, b: f32| a + (b - a) * k2;
                return Now {
                    centre: [lerp(from.0[0], to.0[0]), lerp(from.0[1], to.0[1])],
                    r: lerp(from.1, to.1),
                    next: to,
                    phase: k,
                    shrinking: true,
                    secs: (shrink as f32 - t).ceil() as u32,
                    dps,
                };
            }
            t -= shrink as f32;
        }
        let last = *self.circles.last().unwrap();
        Now {
            centre: last.0,
            r: last.1,
            next: last,
            phase: STORM.len(),
            shrinking: false,
            secs: 0,
            dps: STORM[STORM.len() - 1].3 * 2,
        }
    }
}

impl Now {
    pub fn outside(&self, x: f32, z: f32) -> bool {
        (x - self.centre[0]).powi(2) + (z - self.centre[1]).powi(2) > self.r * self.r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_closes_to_nothing_each_circle_inside_the_last() {
        let map = Map::new(9);
        let s = Storm::plan(&map, &mut Rng::new(4));
        for w in s.circles.windows(2) {
            let ((a, ra), (b, rb)) = (w[0], w[1]);
            let off = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
            assert!(off + rb <= ra + 1e-3);
        }
        assert_eq!(s.at(0).r, STORM_START);
        assert!(!s.at(0).shrinking);
        let total: u32 = STORM.iter().map(|p| p.0 + p.1).sum();
        let end = s.at(total * TICK_HZ + 10);
        assert_eq!(end.r, 0.0);
        let mid = s.at((STORM[0].0 + STORM[0].1 / 2) * TICK_HZ);
        assert!(mid.shrinking && mid.r < STORM_START && mid.r > STORM[0].2);
    }
}
