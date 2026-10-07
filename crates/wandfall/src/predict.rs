//! The page's own wizard, a step ahead of the server: every input is
//! applied at once and kept until the server says it applied it too;
//! then the server's body is taken and the inputs it has not seen yet
//! are applied again. The same code runs on both ends, so nothing moves.

use std::collections::VecDeque;

use crate::map::Map;
use crate::motion::{step, Body, Input};

#[derive(Default)]
pub struct Predict {
    pub body: Body,
    pending: VecDeque<Input>,
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
    /// off (metres).
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
        d.sqrt()
    }

    /// Start over from where the server put us.
    pub fn reset(&mut self, b: Body) {
        self.body = b;
        self.pending.clear();
    }
}
