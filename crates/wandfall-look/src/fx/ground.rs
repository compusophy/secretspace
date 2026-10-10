//! The island's ground as effects need it: its height every couple of
//! metres, read smoothly between (the sea where it is under the sea), so
//! sparks come down on it and the storm's wall burns where it meets it.

use wandfall::laws::{MAP_HALF, SEA};
use wandfall::map::Map;

/// How far apart the heights are taken (m).
const STEP: f32 = 2.0;

#[derive(Default)]
pub struct Ground {
    n: usize,
    heights: Vec<f32>,
}

impl Ground {
    pub fn new(map: &Map) -> Ground {
        let n = (MAP_HALF * 2.0 / STEP) as usize + 1;
        let mut heights = Vec::with_capacity(n * n);
        for j in 0..n {
            for i in 0..n {
                let (x, z) = (i as f32 * STEP - MAP_HALF, j as f32 * STEP - MAP_HALF);
                heights.push(map.height(x, z).max(SEA));
            }
        }
        Ground { n, heights }
    }

    /// The ground (or the sea) under (`x`, `z`).
    pub fn at(&self, x: f32, z: f32) -> f32 {
        if self.n < 2 {
            return SEA;
        }
        let last = (self.n - 1) as f32;
        let u = ((x + MAP_HALF) / STEP).clamp(0.0, last);
        let v = ((z + MAP_HALF) / STEP).clamp(0.0, last);
        let (i, j) = ((u as usize).min(self.n - 2), (v as usize).min(self.n - 2));
        let (fu, fv) = (u - i as f32, v - j as f32);
        let h = |i: usize, j: usize| self.heights[j * self.n + i];
        let a = h(i, j) + (h(i + 1, j) - h(i, j)) * fu;
        let b = h(i, j + 1) + (h(i + 1, j + 1) - h(i, j + 1)) * fu;
        a + (b - a) * fv
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_reads_the_island_s_ground_closely() {
        let map = Map::new(3);
        let g = Ground::new(&map);
        let mut worst: f32 = 0.0;
        for k in 0..400 {
            let (x, z) = (
                (k % 20) as f32 * 13.7 - 130.0,
                (k / 20) as f32 * 13.1 - 125.0,
            );
            worst = worst.max((g.at(x, z) - map.height(x, z).max(SEA)).abs());
        }
        // Off by a little on the steepest slopes, no more.
        assert!(worst < 1.5, "{worst}");
        assert_eq!(Ground::default().at(0.0, 0.0), SEA);
    }
}
