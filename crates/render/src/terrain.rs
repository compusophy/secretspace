//! The ground as the engine draws it: a heightfield (a game samples its
//! own height function into one), meshed smooth for the world shader's
//! terrain material, and uploaded as a texture so the GPU knows the
//! ground too (grass grows on it, the sea sees its shallows).

use crate::geo::{self, Geo, V3};

pub struct Terrain {
    /// Where the first sample is (x, z), metres between samples, samples
    /// a side.
    pub origin: [f32; 2],
    pub cell: f32,
    pub n: usize,
    /// Heights, row by row (z), `n` by `n`; how much grass grows at
    /// each (0 to 1, all 1 unless a game says otherwise).
    pub heights: Vec<f32>,
    pub lush: Vec<f32>,
}

impl Terrain {
    /// Sample `height` over a square `size` metres across from `origin`,
    /// every `cell` metres.
    pub fn sample(
        origin: [f32; 2],
        size: f32,
        cell: f32,
        height: impl Fn(f32, f32) -> f32,
    ) -> Terrain {
        let n = (size / cell).round() as usize + 1;
        let mut heights = Vec::with_capacity(n * n);
        for j in 0..n {
            for i in 0..n {
                heights.push(height(
                    origin[0] + i as f32 * cell,
                    origin[1] + j as f32 * cell,
                ));
            }
        }
        Terrain {
            origin,
            cell,
            n,
            lush: vec![1.0; heights.len()],
            heights,
        }
    }

    /// Set how much grass grows at each sample, from (x, z).
    pub fn grow(&mut self, lush: impl Fn(f32, f32) -> f32) {
        for j in 0..self.n {
            for i in 0..self.n {
                let (x, z) = (
                    self.origin[0] + i as f32 * self.cell,
                    self.origin[1] + j as f32 * self.cell,
                );
                self.lush[j * self.n + i] = lush(x, z).clamp(0.0, 1.0);
            }
        }
    }

    fn at(&self, i: usize, j: usize) -> f32 {
        self.heights[j.min(self.n - 1) * self.n + i.min(self.n - 1)]
    }

    /// The height at (x, z), between samples.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        let fx = ((x - self.origin[0]) / self.cell).clamp(0.0, (self.n - 1) as f32 - 0.001);
        let fz = ((z - self.origin[1]) / self.cell).clamp(0.0, (self.n - 1) as f32 - 0.001);
        let (i, j) = (fx as usize, fz as usize);
        let (tx, tz) = (fx - i as f32, fz - j as f32);
        let a = self.at(i, j) + (self.at(i + 1, j) - self.at(i, j)) * tx;
        let b = self.at(i, j + 1) + (self.at(i + 1, j + 1) - self.at(i, j + 1)) * tx;
        a + (b - a) * tz
    }

    /// The mesh: a smooth grid, its normals from the heights about each
    /// sample; squares wholly below `floor` are left out. `paint` gives
    /// each sample (x, z, height) a colour and how much of it covers the
    /// ground the material would draw (0 leaves the ground as it is).
    pub fn mesh(&self, floor: f32, paint: impl Fn(f32, f32, f32) -> (V3, f32)) -> Geo {
        let n = self.n;
        let mut g = Geo::default();
        let mut ids = vec![u32::MAX; n * n];
        let low = |i: usize, j: usize| self.at(i, j) < floor;
        for j in 0..n {
            for i in 0..n {
                let x = self.origin[0] + i as f32 * self.cell;
                let z = self.origin[1] + j as f32 * self.cell;
                let h = self.at(i, j);
                let dx = self.at(i + 1, j) - self.at(i.saturating_sub(1), j);
                let dz = self.at(i, j + 1) - self.at(i, j.saturating_sub(1));
                let nrm = geo::norm([-dx, 2.0 * self.cell, -dz]);
                let (c, cover) = paint(x, z, h);
                ids[j * n + i] = g.vertex([x, h, z], nrm, c, cover);
            }
        }
        for j in 0..n - 1 {
            for i in 0..n - 1 {
                if low(i, j) && low(i + 1, j) && low(i, j + 1) && low(i + 1, j + 1) {
                    continue;
                }
                let (a, b) = (ids[j * n + i], ids[j * n + i + 1]);
                let (c, d) = (ids[(j + 1) * n + i], ids[(j + 1) * n + i + 1]);
                // Counter-clockwise from above.
                g.index(a, c, b);
                g.index(b, c, d);
            }
        }
        g
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heights_between_samples_and_a_mesh_that_faces_up() {
        let t = Terrain::sample([-10.0, -10.0], 20.0, 1.0, |x, z| x * 0.5 + z * 0.25);
        assert_eq!(t.n, 21);
        assert!((t.height(2.5, -3.0) - (1.25 - 0.75)).abs() < 1e-4);
        let g = t.mesh(-100.0, |_, _, _| ([1.0; 3], 0.0));
        assert_eq!(g.triangles(), 20 * 20 * 2);
        // Every triangle faces up.
        let p = |k: u32| {
            let o = k as usize * geo::STRIDE;
            [g.v[o], g.v[o + 1], g.v[o + 2]]
        };
        for tri in g.i.chunks(3) {
            let n = geo::cross(
                geo::sub(p(tri[1]), p(tri[0])),
                geo::sub(p(tri[2]), p(tri[0])),
            );
            assert!(n[1] > 0.0);
        }
    }
}
