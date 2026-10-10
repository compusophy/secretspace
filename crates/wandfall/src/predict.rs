//! The page's own wizard, a step ahead of the server: every input is
//! applied at once and kept until the server says it applied it too;
//! then the server's body is taken and the inputs it has not seen yet
//! are applied again. The same code runs on both ends, so nothing moves;
//! when something the page could not foresee does (a Gust, a chill, a
//! Tether caught), the wizard is drawn gliding to where it truly is,
//! not jumping there (`offset`).

use std::collections::VecDeque;

use crate::laws::{SMOOTH_MS, SMOOTH_SNAP};
use crate::map::Map;
use crate::motion::{step, Body, Input};

#[derive(Default)]
pub struct Predict {
    pub body: Body,
    pending: VecDeque<Input>,
    /// Where to draw the wizard, less where it is: what the last
    /// corrections moved it by, easing away (`settle`). Drawing only:
    /// the body itself is always exact.
    pub offset: [f32; 3],
}

/// Whether `a` comes at or before `b`, sequence numbers wrapping.
pub fn at_or_before(a: u16, b: u16) -> bool {
    (b.wrapping_sub(a) as i16) >= 0
}

impl Predict {
    pub fn push(&mut self, i: Input, map: &Map) {
        step(&mut self.body, &i, map);
        self.pending.push_back(i);
        if self.pending.len() > 240 {
            self.pending.pop_front();
        }
    }

    /// The server's body after input `seq`: how far the prediction was
    /// off (metres). A correction far enough to be a leap (a Blink, a
    /// respawn) is drawn at once; a nearer one eased.
    pub fn confirm(&mut self, server: Body, seq: u16, map: &Map) -> f32 {
        while self
            .pending
            .front()
            .is_some_and(|i| at_or_before(i.seq, seq))
        {
            self.pending.pop_front();
        }
        let before = self.body.p;
        self.body = server;
        for i in &self.pending {
            step(&mut self.body, i, map);
        }
        let d: f32 = (0..3).map(|k| (self.body.p[k] - before[k]).powi(2)).sum();
        let off = [0, 1, 2].map(|k| self.offset[k] + before[k] - self.body.p[k]);
        let far = off.iter().map(|x| x * x).sum::<f32>() > SMOOTH_SNAP * SMOOTH_SNAP;
        self.offset = if far { [0.0; 3] } else { off };
        d.sqrt()
    }

    /// `ms` on: the drawn wizard eased that much nearer the true one.
    pub fn settle(&mut self, ms: f64) {
        let k = (1.0 / (1.0 + ms.max(0.0) / SMOOTH_MS)) as f32;
        self.offset = self.offset.map(|x| x * k);
    }

    /// Start over from where the server put us.
    pub fn reset(&mut self, b: Body) {
        self.body = b;
        self.pending.clear();
        self.offset = [0.0; 3];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_correction_is_drawn_eased_and_a_leap_at_once() {
        let map = Map::new(3);
        let mut p = Predict::default();
        let at = |x: f32| Body {
            p: [x, map.height(x, -9.0), -9.0],
            ground: true,
            ..Body::default()
        };
        p.reset(at(-17.0));
        p.push(Input::default(), &map);
        // The server had it a metre east: it is there, drawn where it was.
        let off = p.confirm(at(-16.0), 0, &map);
        assert!((off - 1.0).abs() < 0.01 && (p.body.p[0] + 16.0).abs() < 0.01);
        assert!((p.offset[0] + 1.0).abs() < 0.01, "{:?}", p.offset);
        // Eased away over a few tenths of a second.
        p.settle(16.0);
        assert!(p.offset[0] < -0.5);
        for _ in 0..30 {
            p.settle(16.0);
        }
        assert!(p.offset[0].abs() < 0.05, "{:?}", p.offset);
        // A leap is drawn at once.
        p.confirm(at(-5.0), 1, &map);
        assert_eq!(p.offset, [0.0; 3]);
    }
}
