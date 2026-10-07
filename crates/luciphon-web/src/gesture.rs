//! The recogniser: one pointer's presses, moves and lifts in, gestures out.
//! A pure function of the trace, tested natively. Every press away from
//! the Heart is one of three things: moved 10 px before 300 ms, a drag (the
//! stick); let go sooner, unmoved, a tap; still at 300 ms, a hold. A drag
//! ends in a flick (fast, and only on release) or a slow lift; a mouse's
//! slow lift from a run latches the stick instead. A hold ends in a
//! release (a heavy, or a throw along the aim) or a free cancel.

use std::collections::VecDeque;

use kit::pointer::{Kind, Press};

use crate::laws::Feel;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gesture {
    Tap,
    Flick {
        heading: u16,
    },
    /// The stick came up without a flick.
    Lift,
    /// A hold began; the press was `ms` ago.
    Hold {
        ms: f64,
    },
    /// A hold let go along an aim `len` px long.
    Release {
        heading: u16,
        len: f64,
    },
    Cancel,
    Heart,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Idle,
    Pressed {
        x: f64,
        y: f64,
        t: f64,
    },
    Drag {
        ox: f64,
        oy: f64,
    },
    Hold {
        ox: f64,
        oy: f64,
        t: f64,
        left: bool,
    },
    Heart,
}

/// A heading from a screen delta (y down, as the world's).
pub fn heading(dx: f64, dy: f64) -> u16 {
    let a = dy.atan2(dx) / std::f64::consts::TAU;
    ((a.rem_euclid(1.0) * 65536.0).round() as u32 & 0xffff) as u16
}

pub struct Recogniser {
    state: State,
    /// The latched stick's origin (a mouse or pen only).
    latched: Option<(f64, f64)>,
    /// Where the pointer is, and has lately been.
    at: (f64, f64),
    trail: VecDeque<(f64, f64, f64)>,
    /// When the pointer last swung back close to the stick's origin, and
    /// whether it has been out at a run since the drag began.
    near_at: f64,
    out: bool,
}

impl Default for Recogniser {
    fn default() -> Recogniser {
        Recogniser {
            state: State::Idle,
            latched: None,
            at: (0.0, 0.0),
            trail: VecDeque::new(),
            near_at: f64::NEG_INFINITY,
            out: false,
        }
    }
}

fn dist(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt()
}

impl Recogniser {
    /// The stick, if one is down: its heading and how far it is pushed.
    pub fn stick(&self) -> Option<(u16, f64)> {
        let (ox, oy) = match self.state {
            State::Drag { ox, oy } => (ox, oy),
            _ => self.latched?,
        };
        let (dx, dy) = (self.at.0 - ox, self.at.1 - oy);
        Some((heading(dx, dy), (dx * dx + dy * dy).sqrt()))
    }

    /// The stick's origin, for drawing it.
    pub fn origin(&self) -> Option<(f64, f64)> {
        match self.state {
            State::Drag { ox, oy } | State::Hold { ox, oy, .. } => Some((ox, oy)),
            _ => self.latched,
        }
    }

    /// While a hold is under way: the aim's heading and length.
    pub fn aim(&self) -> Option<(u16, f64)> {
        match self.state {
            State::Hold { ox, oy, .. } => {
                let (dx, dy) = (self.at.0 - ox, self.at.1 - oy);
                Some((heading(dx, dy), (dx * dx + dy * dy).sqrt()))
            }
            _ => None,
        }
    }

    pub fn holding(&self) -> bool {
        matches!(self.state, State::Hold { .. })
    }

    pub fn pressing(&self) -> bool {
        !matches!(self.state, State::Idle)
    }

    /// Time passing with nothing new: a still press becomes a hold.
    pub fn tick(&mut self, now: f64, f: &Feel, out: &mut Vec<Gesture>) {
        if let State::Pressed { x, y, t } = self.state {
            if now - t >= f.hold_ms {
                self.state = State::Hold {
                    ox: x,
                    oy: y,
                    t,
                    left: false,
                };
                out.push(Gesture::Hold { ms: now - t });
            }
        }
    }

    /// One pointer event. `heart` says whether a point is on the Heart.
    pub fn feed(
        &mut self,
        p: &Press,
        f: &Feel,
        heart: &dyn Fn(f64, f64) -> bool,
        out: &mut Vec<Gesture>,
    ) {
        self.tick(p.t, f, out);
        self.at = (p.x, p.y);
        self.trail.push_back((p.t, p.x, p.y));
        while self.trail.front().is_some_and(|s| p.t - s.0 > 200.0) {
            self.trail.pop_front();
        }
        // A swing back through the origin (not the press itself).
        if let Some((ox, oy)) = self.origin() {
            let d = dist(p.x, p.y, ox, oy);
            if d >= f.run_px {
                self.out = true;
            } else if d < f.swing_px && self.out {
                self.near_at = p.t;
            }
        }
        match (p.kind, self.state) {
            (Kind::Down, _) => {
                // A press lets a latched stick go, as a lift would.
                if self.latched.take().is_some() {
                    out.push(Gesture::Lift);
                }
                self.trail.clear();
                self.trail.push_back((p.t, p.x, p.y));
                if p.second || heart(p.x, p.y) {
                    self.state = State::Heart;
                    if p.second {
                        out.push(Gesture::Heart);
                    }
                } else {
                    self.state = State::Pressed {
                        x: p.x,
                        y: p.y,
                        t: p.t,
                    };
                }
            }
            (Kind::Hover, _) => {
                if let Some((ox, oy)) = self.latched {
                    if dist(p.x, p.y, ox, oy) < f.drag_px {
                        self.latched = None;
                        out.push(Gesture::Lift);
                    }
                }
            }
            (Kind::Move, State::Pressed { x, y, .. }) => {
                if dist(p.x, p.y, x, y) >= f.drag_px {
                    self.state = State::Drag { ox: x, oy: y };
                    self.near_at = f64::NEG_INFINITY;
                    self.out = false;
                }
            }
            (Kind::Move, State::Drag { ox, oy }) => {
                // Past the trail distance, the origin follows.
                let d = dist(p.x, p.y, ox, oy);
                if d > f.trail_px {
                    let k = (d - f.trail_px) / d;
                    self.state = State::Drag {
                        ox: ox + (p.x - ox) * k,
                        oy: oy + (p.y - oy) * k,
                    };
                }
            }
            (Kind::Move, State::Hold { ox, oy, t, left }) => {
                self.state = State::Hold {
                    ox,
                    oy,
                    t,
                    left: left || dist(p.x, p.y, ox, oy) > f.ring_px,
                };
            }
            (Kind::Up, State::Pressed { .. }) => {
                self.state = State::Idle;
                out.push(Gesture::Tap);
            }
            (Kind::Up, State::Drag { ox, oy }) => {
                self.state = State::Idle;
                match self.flick(p, f) {
                    Some(h) => out.push(Gesture::Flick { heading: h }),
                    None => {
                        let run = dist(p.x, p.y, ox, oy) >= f.run_px;
                        if run && !p.touch {
                            self.latched = Some((ox, oy));
                        } else {
                            out.push(Gesture::Lift);
                        }
                    }
                }
            }
            (Kind::Up, State::Hold { ox, oy, t, left }) => {
                self.state = State::Idle;
                let d = dist(p.x, p.y, ox, oy);
                if p.t - t < f.release_ms || (left && d < f.cancel_px) {
                    out.push(Gesture::Cancel);
                } else {
                    out.push(Gesture::Release {
                        heading: heading(p.x - ox, p.y - oy),
                        len: d,
                    });
                }
            }
            (Kind::Up, State::Heart) => {
                self.state = State::Idle;
                if !p.second {
                    out.push(Gesture::Heart);
                }
            }
            (Kind::Cancel, State::Drag { .. }) => {
                self.state = State::Idle;
                out.push(Gesture::Lift);
            }
            (Kind::Cancel, State::Hold { .. }) => {
                self.state = State::Idle;
                out.push(Gesture::Cancel);
            }
            (Kind::Cancel, _) => self.state = State::Idle,
            _ => {}
        }
    }

    /// Whether a release is a flick, and which way.
    fn flick(&self, p: &Press, f: &Feel) -> Option<u16> {
        if p.t - self.near_at < f.swing_ms {
            return None;
        }
        let at = |ago: f64| {
            self.trail
                .iter()
                .find(|s| p.t - s.0 <= ago)
                .copied()
                .unwrap_or((p.t, p.x, p.y))
        };
        let (t0, x0, y0) = at(f.flick_window);
        let dt = (p.t - t0).max(1.0);
        let speed = dist(p.x, p.y, x0, y0) / dt;
        let (_, x1, y1) = at(f.flick_span);
        let travelled = dist(p.x, p.y, x1, y1);
        (speed >= f.flick_speed && travelled >= f.flick_px).then(|| heading(p.x - x0, p.y - y0))
    }
}
