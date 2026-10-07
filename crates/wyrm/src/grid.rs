//! A uniform grid over the arena, for "what is near here" without looking
//! at everything.

use crate::laws::ARENA;

pub struct Grid<T> {
    cell: f32,
    n: usize,
    cells: Vec<Vec<T>>,
}

impl<T: Copy + PartialEq> Grid<T> {
    pub fn new(cell: f32) -> Grid<T> {
        let n = (2.0 * ARENA / cell).ceil() as usize + 2;
        Grid {
            cell,
            n,
            cells: (0..n * n).map(|_| Vec::new()).collect(),
        }
    }

    fn coord(&self, v: f32) -> usize {
        (((v + ARENA) / self.cell).floor().max(0.0) as usize).min(self.n - 1)
    }

    fn index(&self, x: f32, y: f32) -> usize {
        self.coord(y) * self.n + self.coord(x)
    }

    pub fn insert(&mut self, x: f32, y: f32, v: T) {
        let i = self.index(x, y);
        self.cells[i].push(v);
    }

    pub fn remove(&mut self, x: f32, y: f32, v: T) {
        let i = self.index(x, y);
        if let Some(k) = self.cells[i].iter().position(|&e| e == v) {
            self.cells[i].swap_remove(k);
        }
    }

    pub fn clear(&mut self) {
        for c in &mut self.cells {
            c.clear();
        }
    }

    /// Everything in the cells that touch the box `(x0, y0)`..`(x1, y1)`.
    pub fn within(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> impl Iterator<Item = T> + '_ {
        let (cx0, cx1) = (self.coord(x0), self.coord(x1));
        let (cy0, cy1) = (self.coord(y0), self.coord(y1));
        (cy0..=cy1).flat_map(move |cy| {
            (cx0..=cx1).flat_map(move |cx| self.cells[cy * self.n + cx].iter().copied())
        })
    }

    /// Everything in the cells within `r` of `(x, y)`.
    pub fn near(&self, x: f32, y: f32, r: f32) -> impl Iterator<Item = T> + '_ {
        self.within(x - r, y - r, x + r, y + r)
    }
}
