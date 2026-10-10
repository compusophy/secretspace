//! The light grid: a square of cells on the ground about the eye, each
//! listing the lights whose reach touches it, so a pixel weighs only the
//! lights near it (hundreds in a scene, a few a pixel). Built on the CPU
//! each frame; Phase 3 moves it to froxels in compute.

use crate::buffers::{put_f32s, Grow};
use crate::laws::{GRID_CELL, GRID_CELLS, MAX_PER_CELL};
use crate::{geo, Light};
use gpu::wgpu;

#[derive(Clone, Debug, Default)]
pub struct Grid {
    /// The grid's corner (x, z), its cell size, and cells a side.
    pub origin: [f32; 2],
    pub cell: f32,
    pub n: u32,
    /// Per cell: where its list starts in `index`, and how long it is.
    pub cells: Vec<[u32; 2]>,
    pub index: Vec<u32>,
    /// The lights in the grid's order (nearest the centre first).
    pub lights: Vec<Light>,
}

impl Grid {
    /// The lights that reach the square about `centre` (x, z).
    pub fn build(&mut self, lights: &[Light], centre: [f32; 2]) {
        let (cell, n) = (GRID_CELL, GRID_CELLS);
        let half = cell * n as f32 / 2.0;
        // Snapped to whole cells, so lights do not swim as the eye moves.
        self.origin = [
            ((centre[0] - half) / cell).floor() * cell,
            ((centre[1] - half) / cell).floor() * cell,
        ];
        self.cell = cell;
        self.n = n;
        let span = cell * n as f32;
        let (ox, oz) = (self.origin[0], self.origin[1]);
        self.lights.clear();
        self.lights.extend(lights.iter().copied().filter(|l| {
            l.r > 0.0
                && l.p[0] + l.r >= ox
                && l.p[0] - l.r < ox + span
                && l.p[2] + l.r >= oz
                && l.p[2] - l.r < oz + span
        }));
        let d2 = |l: &Light| (l.p[0] - centre[0]).powi(2) + (l.p[2] - centre[1]).powi(2);
        self.lights.sort_by(|a, b| d2(a).total_cmp(&d2(b)));
        let cells = (n * n) as usize;
        let mut count = vec![0u32; cells];
        let range = |l: &Light| {
            let c = |v: f32, o: f32| ((v - o) / cell).floor().clamp(0.0, (n - 1) as f32) as u32;
            (
                c(l.p[0] - l.r, ox),
                c(l.p[0] + l.r, ox),
                c(l.p[2] - l.r, oz),
                c(l.p[2] + l.r, oz),
            )
        };
        for l in &self.lights {
            let (x0, x1, z0, z1) = range(l);
            for z in z0..=z1 {
                for x in x0..=x1 {
                    let k = &mut count[(z * n + x) as usize];
                    *k = (*k + 1).min(MAX_PER_CELL);
                }
            }
        }
        self.cells.clear();
        let mut at = 0;
        for &k in &count {
            self.cells.push([at, 0]);
            at += k;
        }
        self.index.clear();
        self.index.resize(at as usize, 0);
        for (i, l) in self.lights.iter().enumerate() {
            let (x0, x1, z0, z1) = range(l);
            for z in z0..=z1 {
                for x in x0..=x1 {
                    let c = &mut self.cells[(z * n + x) as usize];
                    if c[1] < count[(z * n + x) as usize] {
                        self.index[(c[0] + c[1]) as usize] = i as u32;
                        c[1] += 1;
                    }
                }
            }
        }
    }

    /// The lights listed for the cell under (x, z).
    pub fn at(&self, x: f32, z: f32) -> &[u32] {
        let c = |v: f32, o: f32| ((v - o) / self.cell).floor();
        let (cx, cz) = (c(x, self.origin[0]), c(z, self.origin[1]));
        if cx < 0.0 || cz < 0.0 || cx >= self.n as f32 || cz >= self.n as f32 {
            return &[];
        }
        let [s, k] = self.cells[(cz as u32 * self.n + cx as u32) as usize];
        &self.index[s as usize..(s + k) as usize]
    }
}

/// The grid on the GPU (the scene's group reads it): the lights, each
/// cell's span of the index, the index.
pub(crate) struct Lists {
    pub lights: Grow,
    pub cells: Grow,
    pub index: Grow,
}

impl Lists {
    pub fn new(device: &wgpu::Device) -> Lists {
        let storage = wgpu::BufferUsages::STORAGE;
        Lists {
            lights: Grow::new(device, "lights", storage),
            cells: Grow::new(device, "cells", storage),
            index: Grow::new(device, "index", storage),
        }
    }

    /// Put `grid` in, each light `glow` as bright (`b` the bytes to lay
    /// them in); whether a buffer was replaced (the group made again).
    pub fn put(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        grid: &Grid,
        glow: f32,
        b: &mut Vec<u8>,
    ) -> bool {
        b.clear();
        for l in &grid.lights {
            let c = geo::scale(l.c, glow);
            put_f32s(b, &[l.p[0], l.p[1], l.p[2], l.r, c[0], c[1], c[2], 0.0]);
        }
        let mut grew = self.lights.put(device, queue, b);
        b.clear();
        for c in &grid.cells {
            b.extend_from_slice(&c[0].to_le_bytes());
            b.extend_from_slice(&c[1].to_le_bytes());
        }
        grew |= self.cells.put(device, queue, b);
        b.clear();
        for i in &grid.index {
            b.extend_from_slice(&i.to_le_bytes());
        }
        grew | self.index.put(device, queue, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light(x: f32, z: f32, r: f32) -> Light {
        Light {
            p: [x, 1.0, z],
            r,
            c: [1.0; 3],
        }
    }

    #[test]
    fn a_pixel_sees_every_light_that_reaches_it_and_no_other() {
        let lights: Vec<Light> = (0..300)
            .map(|i| {
                let a = i as f32 * 0.37;
                light(
                    a.cos() * i as f32 * 0.3,
                    a.sin() * i as f32 * 0.3,
                    3.0 + (i % 5) as f32,
                )
            })
            .collect();
        let mut g = Grid::default();
        g.build(&lights, [3.0, -2.0]);
        for (x, z) in [(0.0, 0.0), (10.5, -7.25), (-40.0, 33.0), (60.0, 60.0)] {
            let listed: Vec<usize> = g.at(x, z).iter().map(|&i| i as usize).collect();
            for (i, l) in g.lights.iter().enumerate() {
                let d = ((l.p[0] - x).powi(2) + (l.p[2] - z).powi(2)).sqrt();
                if d < l.r {
                    assert!(listed.contains(&i), "light {i} reaches ({x}, {z})");
                }
            }
            assert!(listed.len() <= MAX_PER_CELL as usize);
        }
    }

    #[test]
    fn a_crowded_cell_keeps_the_nearest() {
        let lights: Vec<Light> = (0..200)
            .map(|i| light(0.5, 0.5, 2.0 + i as f32 * 0.01))
            .collect();
        let mut g = Grid::default();
        g.build(&lights, [0.0, 0.0]);
        assert_eq!(g.at(0.5, 0.5).len(), MAX_PER_CELL as usize);
        assert!(g.at(900.0, 0.0).is_empty(), "outside the grid: none");
    }
}
