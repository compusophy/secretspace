//! Gestures to Inputs, 30 a second: the stick's heading and push, and at
//! most one verb each, in an order a thumb could make (a flick only after
//! the stick was down, a tap or a hold only after it came up). With
//! `?trace=1` the raw pointer stream is kept for download.

use std::collections::VecDeque;

use kit::pointer::Press;
use luciphon::motion::{Intent, Verb};

use crate::gesture::{Gesture, Recogniser};
use crate::laws::Feel;

/// Ticks a second the page sends Inputs at.
pub const TICK_MS: f64 = 1000.0 / 30.0;

#[derive(Default)]
pub struct Input {
    pub rec: Recogniser,
    verbs: VecDeque<(Verb, u16)>,
    pub seq: u16,
    pub next_at: f64,
    last_stick: bool,
    heading: u16,
    /// Heart taps (None) and wheel slots picked, to act on.
    pub hearts: Vec<Option<u8>>,
    /// Gestures seen (bits: drag, tap, flick, hold), for the hints.
    pub done: u8,
    pub trace: Option<Vec<String>>,
}

impl Input {
    /// One pointer event (CSS pixels).
    pub fn feed(&mut self, p: &Press, f: &Feel, heart: &dyn Fn(f64, f64) -> bool, facing: u16) {
        if let Some(t) = &mut self.trace {
            let k = match p.kind {
                kit::pointer::Kind::Down => "d",
                kit::pointer::Kind::Move => "m",
                kit::pointer::Kind::Up => "u",
                kit::pointer::Kind::Hover => "h",
                kit::pointer::Kind::Cancel => "c",
            };
            t.push(format!(
                "{k} {:.1} {:.1} {:.1} {}",
                p.x, p.y, p.t, p.touch as u8
            ));
        }
        let mut out = Vec::new();
        self.rec.feed(p, f, heart, &mut out);
        self.take(out, facing);
    }

    /// Time passing (a press becomes a hold).
    pub fn tick(&mut self, now: f64, f: &Feel, facing: u16) {
        let mut out = Vec::new();
        self.rec.tick(now, f, &mut out);
        self.take(out, facing);
    }

    fn take(&mut self, gestures: Vec<Gesture>, facing: u16) {
        for g in gestures {
            match g {
                Gesture::Tap => {
                    self.done |= 2;
                    self.verbs.push_back((Verb::Tap, facing));
                }
                Gesture::Flick { heading } => {
                    self.done |= 4;
                    self.verbs.push_back((Verb::Flick, heading));
                }
                Gesture::Hold { ms } => {
                    self.done |= 8;
                    let held_for = (ms / TICK_MS).round().clamp(0.0, 255.0) as u8;
                    self.verbs.push_back((Verb::Hold { held_for }, facing));
                }
                Gesture::Release { heading, len } => {
                    let range = range(len, &crate::laws::FEEL);
                    let aim = if range == 0 && len < 12.0 {
                        facing
                    } else {
                        heading
                    };
                    self.verbs.push_back((Verb::Release { range }, aim));
                }
                Gesture::Cancel => self.verbs.push_back((Verb::Cancel, facing)),
                Gesture::Heart => self.hearts.push(None),
                Gesture::Wheel(s) => self.hearts.push(Some(s)),
                Gesture::Lift => {}
            }
        }
        while self.verbs.len() > 6 {
            self.verbs.pop_front();
        }
    }

    /// The Input for the next tick.
    pub fn sample(&mut self, f: &Feel, facing: u16) -> (u16, Intent) {
        let (mut heading, mut throttle) = (self.heading, 0u8);
        if let Some((h, d)) = self.rec.stick() {
            self.done |= 1;
            heading = h;
            throttle = if d >= f.run_px {
                255
            } else if d <= f.walk_px {
                1
            } else {
                (((d - f.walk_px) / (f.run_px - f.walk_px)) * 253.0 + 1.0) as u8
            };
        }
        let mut aim = facing;
        if let Some((h, _)) = self.rec.aim() {
            aim = h;
            throttle = 0;
        }
        let mut verb = Verb::None;
        if let Some(&(v, a)) = self.verbs.front() {
            match v {
                // A flick ends a drag: if no stick went out, send one first.
                Verb::Flick if !self.last_stick => {
                    heading = a;
                    throttle = 255;
                }
                Verb::Flick => {
                    self.verbs.pop_front();
                    verb = v;
                    aim = a;
                    throttle = 0;
                }
                // A tap or a hold starts with the stick up.
                Verb::Tap | Verb::Hold { .. } if self.last_stick => throttle = 0,
                _ => {
                    self.verbs.pop_front();
                    verb = v;
                    aim = a;
                    if !matches!(v, Verb::None) {
                        throttle = 0;
                    }
                }
            }
        }
        self.heading = heading;
        self.last_stick = throttle > 0;
        self.seq = self.seq.wrapping_add(1);
        (
            self.seq,
            Intent {
                heading,
                throttle,
                verb,
                aim,
            },
        )
    }
}

/// A throw's range from its arrow: 0 (a heavy) under `throw_px`, else
/// 1-255 across the arrow's span.
pub fn range(len: f64, f: &Feel) -> u8 {
    if len < f.throw_px {
        0
    } else {
        let k = ((len - f.throw_px) / (f.throw_far_px - f.throw_px)).clamp(0.0, 1.0);
        (1.0 + k * 254.0) as u8
    }
}
