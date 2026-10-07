//! The page's own Lumen, ahead of the server: every Input it sends is
//! applied at once with the same `control` the server runs; each frame
//! resets to the server's exact state at `ack` and replays what the server
//! has not taken yet. Errors under a quarter tile are blended away; larger
//! ones snap.

use std::collections::VecDeque;

use engine::fixed::{len, Fx};

use crate::combat::{control, Me};
use crate::laws::Laws;
use crate::motion::Intent;
use crate::proto::Own;
use crate::tiles::Tiles;

/// Whether seq `a` comes after `b` (they wrap).
pub fn newer(a: u16, b: u16) -> bool {
    (a.wrapping_sub(b) as i16) > 0
}

#[derive(Clone, Debug, Default)]
pub struct Predictor {
    pub me: Me,
    pub glim: u32,
    pub flow: u8,
    pending: VecDeque<(u16, Intent)>,
    pub ready: bool,
}

impl Predictor {
    /// Apply an Input now, as the server will.
    pub fn push(&mut self, seq: u16, it: Intent, t: &Tiles, l: &Laws) {
        if !self.ready {
            return;
        }
        self.pending.push_back((seq, it));
        while self.pending.len() > 64 {
            self.pending.pop_front();
        }
        control(&mut self.me, &it, self.glim, self.flow, t, l);
    }

    /// The server's word on the Lumen, as of the Input `ack`: start from
    /// it and replay the rest. How far the prediction had drifted.
    pub fn reconcile(&mut self, own: &Own, ack: u16, t: &Tiles, l: &Laws) -> Fx {
        let before = (self.me.body.x, self.me.body.y);
        while self.pending.front().is_some_and(|&(s, _)| !newer(s, ack)) {
            self.pending.pop_front();
        }
        self.me = own.me;
        self.glim = own.glim;
        self.flow = own.flow;
        for (_, it) in &self.pending {
            control(&mut self.me, it, self.glim, self.flow, t, l);
        }
        let was_ready = self.ready;
        self.ready = true;
        if was_ready {
            len(self.me.body.x.sub(before.0), self.me.body.y.sub(before.1))
        } else {
            Fx::ZERO
        }
    }

    pub fn pending(&self) -> usize {
        self.pending.len()
    }
}
