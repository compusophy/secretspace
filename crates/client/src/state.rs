//! What the page knows: its mirror of the arena, the boards, the feed, and
//! the timing that lets 20 frames a second be drawn at 60.

use std::collections::VecDeque;

use game::mirror::{Mirror, Pellet};
use game::proto::{Board, Down};

/// Food flying into the mouth that ate it.
pub struct Gulp {
    pub pellet: Pellet,
    pub by: u16,
    pub at: f64,
}

pub struct Death {
    pub by: String,
    pub score: u32,
    pub at: f64,
}

pub struct State {
    pub mirror: Mirror,
    pub arena: f32,
    /// When the last frame arrived, and the usual gap between frames (ms).
    pub frame_at: f64,
    pub gap: f64,
    pub board: Board,
    pub feed: VecDeque<(f64, String)>,
    pub gulps: Vec<Gulp>,
    pub death: Option<Death>,
    /// The longest this browser has been.
    pub best: u32,
    pub connected: bool,
    /// Where the camera looks and how close, eased.
    pub camera: (f32, f32),
    pub zoom: f32,
}

impl State {
    pub fn new(best: u32) -> State {
        State {
            mirror: Mirror::default(),
            arena: game::laws::ARENA,
            frame_at: 0.0,
            gap: 1000.0 / game::laws::TICK_HZ as f64,
            board: Board::default(),
            feed: VecDeque::new(),
            gulps: Vec::new(),
            death: None,
            best,
            connected: false,
            camera: (0.0, 0.0),
            zoom: 0.0,
        }
    }

    pub fn playing(&self) -> bool {
        self.mirror.you != 0
    }

    pub fn score(&self) -> u32 {
        self.mirror
            .snakes
            .get(&self.mirror.you)
            .map_or(0, |s| s.mass)
    }

    /// Take in one message from the server.
    pub fn receive(&mut self, now: f64, msg: Down) {
        match msg {
            Down::Hello { arena, .. } => {
                self.arena = arena as f32;
                self.mirror = Mirror::default();
            }
            Down::Frame(f) => {
                if self.frame_at > 0.0 {
                    let g = (now - self.frame_at).clamp(10.0, 250.0);
                    self.gap += (g - self.gap) * 0.1;
                }
                self.frame_at = now;
                for (pellet, by) in self.mirror.apply(&f) {
                    if by != 0 {
                        self.gulps.push(Gulp {
                            pellet,
                            by,
                            at: now,
                        });
                    }
                }
                let score = self.score();
                if score > self.best {
                    self.best = score;
                }
            }
            Down::Board(b) => self.board = b,
            Down::Died { by, score } => {
                self.death = Some(Death { by, score, at: now });
            }
            Down::Feed {
                killer,
                victim,
                score,
            } => {
                self.feed
                    .push_back((now, format!("{killer} ate {victim} ({score})")));
                while self.feed.len() > 4 {
                    self.feed.pop_front();
                }
            }
        }
        self.gulps.retain(|g| now - g.at < 250.0);
        self.feed.retain(|f| now - f.0 < 7000.0);
    }

    /// Feed lines fade on their own even when nothing new arrives.
    pub fn age(&mut self, now: f64) {
        self.gulps.retain(|g| now - g.at < 250.0);
        self.feed.retain(|f| now - f.0 < 7000.0);
    }

    /// How far between the last frame and the next we are now: 0..=1.
    pub fn alpha(&self, now: f64) -> f32 {
        ((now - self.frame_at) / self.gap).clamp(0.0, 1.0) as f32
    }
}
