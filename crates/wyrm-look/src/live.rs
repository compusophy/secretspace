//! What a page keeps of the arena it is sent, and the clock that draws the
//! server's 20 frames a second at whatever rate the screen runs: the
//! mirror, the bursts and gulps the frames set off, and how far between
//! two frames it is now. wyrm's page keeps one; so can the hub's preview.

use wyrm::mirror::Mirror;
use wyrm::proto::{unq, Down};

use crate::{Burst, Gulp, Scene};

/// How long food takes to fly into a mouth, and a burst to play (ms).
pub const GULP_MS: f64 = 250.0;
pub const BURST_MS: f64 = 700.0;
/// A camera following a snake it does not play catches up over about this
/// long (ms); a change of zoom over this.
pub const CAMERA_MS: f64 = 130.0;
pub const ZOOM_MS: f64 = 330.0;

/// How much of the way to its target something easing over about `tau` ms
/// moves in `dt` ms: the same feel at 30 frames a second or 144.
pub fn ease(dt: f64, tau: f64) -> f32 {
    (1.0 - (-dt.max(0.0) / tau.max(1.0)).exp()) as f32
}

pub struct Live {
    pub mirror: Mirror,
    pub arena: f32,
    /// ms from one server frame to the next, as its Hello says.
    pub period: f64,
    /// When the last frame landed (ms); 0 before the first.
    pub frame_at: f64,
    pub gulps: Vec<Gulp>,
    pub bursts: Vec<Burst>,
    /// A Hello came (a new connection): the next frame starts the mirror
    /// afresh, and the old picture stays until it does.
    renew: bool,
}

impl Default for Live {
    fn default() -> Live {
        Live {
            mirror: Mirror::default(),
            arena: wyrm::laws::ARENA,
            period: 1000.0 / wyrm::laws::TICK_HZ as f64,
            frame_at: 0.0,
            gulps: Vec::new(),
            bursts: Vec::new(),
            renew: false,
        }
    }
}

impl Live {
    /// Take in one message from the server. The picture's own (Hello, a
    /// frame) are used up; anything else is handed back.
    pub fn receive(&mut self, now: f64, msg: Down) -> Option<Down> {
        match msg {
            Down::Hello { arena, hz } => {
                self.arena = arena as f32;
                self.period = 1000.0 / hz.max(1) as f64;
                self.renew = true;
                None
            }
            Down::Frame(f) => {
                let left = if self.renew || self.frame_at <= 0.0 {
                    0.0
                } else {
                    1.0 - self.alpha(now)
                };
                if self.renew {
                    self.renew = false;
                    self.mirror = Mirror::default();
                    self.gulps.clear();
                }
                self.frame_at = now;
                for &(x, y, hue, r) in &f.bursts {
                    self.bursts.push(Burst {
                        x: unq(x),
                        y: unq(y),
                        hue,
                        r: r as f32,
                        at: now,
                    });
                }
                for (pellet, by) in self.mirror.apply_after(&f, left) {
                    if by != 0 {
                        self.gulps.push(Gulp {
                            pellet,
                            by,
                            at: now,
                        });
                    }
                }
                None
            }
            other => Some(other),
        }
    }

    /// Effects that have played out go.
    pub fn age(&mut self, now: f64) {
        self.gulps.retain(|g| now - g.at < GULP_MS);
        self.bursts.retain(|b| now - b.at < BURST_MS);
    }

    /// How far from the last frame to the next it is now: 0..=1.
    pub fn alpha(&self, now: f64) -> f32 {
        ((now - self.frame_at) / self.period).clamp(0.0, 1.0) as f32
    }

    /// The scene to draw now.
    pub fn scene(&self, now: f64, steer: Option<f32>, names: Option<i32>) -> Scene<'_> {
        Scene {
            mirror: &self.mirror,
            alpha: self.alpha(now),
            arena: self.arena,
            gulps: &self.gulps,
            bursts: &self.bursts,
            steer,
            names,
        }
    }
}
