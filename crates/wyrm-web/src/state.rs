//! What the page knows: the arena as it has been sent (`look::Live`, which
//! also draws its 20 frames a second at the screen's rate), the boards,
//! the feed, how you last died, and where the camera is.

use std::collections::VecDeque;

use look::Live;
use wyrm::proto::{Board, Down};

/// "you ate noodle": shown big for a moment.
pub struct Toast {
    pub text: String,
    pub at: f64,
}

pub struct Death {
    pub by: String,
    pub score: u32,
    pub kills: u32,
    /// Seconds it lived.
    pub secs: u32,
    pub at: f64,
}

/// How long feed lines and the toast stay (ms).
const FEED_MS: f64 = 7000.0;
const TOAST_MS: f64 = 2200.0;

pub struct State {
    pub live: Live,
    pub board: Board,
    pub feed: VecDeque<(f64, String)>,
    pub toast: Option<Toast>,
    pub death: Option<Death>,
    /// The longest this browser has been.
    pub best: u32,
    pub connected: bool,
    /// Where the camera looks and how close, eased; 0 zoom before the
    /// first picture.
    pub camera: (f32, f32),
    pub zoom: f32,
    /// When the last picture was drawn (ms), for easing by the time gone.
    pub drawn_at: f64,
}

impl State {
    pub fn new(best: u32) -> State {
        State {
            live: Live::default(),
            board: Board::default(),
            feed: VecDeque::new(),
            toast: None,
            death: None,
            best,
            connected: false,
            camera: (0.0, 0.0),
            zoom: 0.0,
            drawn_at: 0.0,
        }
    }

    pub fn playing(&self) -> bool {
        self.live.mirror.you != 0
    }

    pub fn score(&self) -> u32 {
        let m = &self.live.mirror;
        m.snakes.get(&m.you).map_or(0, |s| s.mass)
    }

    /// Take in one message from the server.
    pub fn receive(&mut self, now: f64, msg: Down) {
        match self.live.receive(now, msg) {
            None => self.best = self.best.max(self.score()),
            Some(Down::Board(b)) => self.board = b,
            Some(Down::Died {
                by,
                score,
                kills,
                secs,
            }) => {
                self.death = Some(Death {
                    by,
                    score,
                    kills,
                    secs,
                    at: now,
                });
            }
            Some(Down::Feed {
                killer_id,
                killer,
                victim,
                score,
            }) => {
                // Yours by its id: a name can be shared.
                if self.playing() && killer_id == self.live.mirror.you {
                    self.toast = Some(Toast {
                        text: format!("you ate {victim}!"),
                        at: now,
                    });
                }
                self.feed
                    .push_back((now, format!("{killer} ate {victim} ({score})")));
                while self.feed.len() > 4 {
                    self.feed.pop_front();
                }
            }
            Some(_) => {}
        }
        self.age(now);
    }

    /// Effects, feed lines and the toast fade on their own even when
    /// nothing new arrives.
    pub fn age(&mut self, now: f64) {
        self.live.age(now);
        if self.toast.as_ref().is_some_and(|t| now - t.at > TOAST_MS) {
            self.toast = None;
        }
        self.feed.retain(|f| now - f.0 < FEED_MS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_your_own_kill_is_toasted() {
        let mut st = State::new(0);
        st.live.mirror.you = 5;
        let feed = |id: u16| Down::Feed {
            killer_id: id,
            killer: "snake 5".into(),
            victim: "bean".into(),
            score: 40,
        };
        st.receive(1.0, feed(6));
        assert!(st.toast.is_none(), "another snake of the same name");
        st.receive(2.0, feed(5));
        assert_eq!(
            st.toast.as_ref().map(|t| t.text.as_str()),
            Some("you ate bean!")
        );
        assert_eq!(st.feed.len(), 2);
    }
}
