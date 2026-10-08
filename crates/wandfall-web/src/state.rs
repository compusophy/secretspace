//! What the page knows: who is who, the latest frame, a few frames back
//! to draw everyone else smoothly (a tenth of a second in the past), the
//! server's clock, and what just happened (the feed, hits, your end).

use std::collections::{HashMap, VecDeque};

use wandfall::laws::TICK_HZ;
use wandfall::proto::{self, Ev, Frame, Loot, Seen};

/// How far behind the newest frame others are drawn (ms).
const BEHIND: f64 = 100.0;
const MS_A_TICK: f64 = 1000.0 / TICK_HZ as f64;

#[derive(Default)]
pub struct State {
    pub you: u16,
    pub seed: Option<u64>,
    pub names: HashMap<u16, (String, bool)>,
    pub frame: Option<Frame>,
    /// Server ms minus page ms, smoothed.
    offset: Option<f64>,
    snaps: VecDeque<(u32, Vec<Seen>)>,
    /// (when, line).
    pub feed: VecDeque<(f64, String)>,
    /// When your bolt last hit someone; when you were last hurt.
    pub hit_at: f64,
    pub hurt_at: f64,
    /// Bursts of light where bolts struck someone: (when, who).
    pub bursts: Vec<(f64, u16)>,
    /// You are out: by whom, your place.
    pub out: Option<(u16, u16)>,
    pub joined: bool,
    pub loot: Loot,
    /// Spells cast and landing, leaps and levels, for their effects:
    /// (when, what).
    pub shows: Vec<(f64, Ev)>,
    /// When you last levelled, and to what.
    pub levelled: (f64, u8),
    /// Your hits, to show their numbers: (when, on whom, how much).
    pub numbers: Vec<(f64, u16, u16)>,
}

impl State {
    pub fn name(&self, id: u16) -> String {
        match id {
            0 => "the storm".to_string(),
            _ => self
                .names
                .get(&id)
                .map_or_else(|| "someone".to_string(), |n| n.0.clone()),
        }
    }

    /// The server's time now, in ms.
    pub fn server_ms(&self, now: f64) -> f64 {
        now + self.offset.unwrap_or(0.0)
    }

    pub fn take(&mut self, f: Frame, now: f64) {
        let srv = f.tick as f64 * MS_A_TICK;
        let guess = srv - now;
        self.offset = Some(match self.offset {
            Some(o) if (guess - o).abs() < 250.0 => o + (guess - o) * 0.05,
            _ => guess,
        });
        self.snaps.push_back((f.tick, f.players.clone()));
        while self.snaps.len() > 12 {
            self.snaps.pop_front();
        }
        self.frame = Some(f);
    }

    pub fn events(&mut self, list: Vec<Ev>, now: f64) {
        for e in list {
            match e {
                Ev::Hit { by, to, amount } => {
                    if by == self.you {
                        self.hit_at = now;
                        self.numbers.push((now, to, amount));
                    }
                    if to == self.you {
                        self.hurt_at = now;
                    }
                    self.bursts.push((now, to));
                }
                Ev::Out { who, by, place } => {
                    let line = if by == 0 {
                        format!("{} fell to the storm", self.name(who))
                    } else {
                        format!("{} > {}", self.name(by), self.name(who))
                    };
                    self.feed.push_back((now, line));
                    if who == self.you {
                        self.out = Some((by, place));
                    }
                }
                Ev::Win { who } => {
                    let line = format!("{} wins the match", self.name(who));
                    self.feed.push_back((now, line));
                }
                Ev::Begin => {
                    self.out = None;
                    self.feed
                        .push_back((now, "the match begins: drop!".to_string()));
                }
                Ev::Lobby => self.out = None,
                Ev::Level { who, level } => {
                    if who == self.you {
                        self.levelled = (now, level);
                    }
                    self.shows.push((now, e));
                }
                Ev::Cast { .. } | Ev::Link { .. } => self.shows.push((now, e)),
            }
        }
        while self.feed.len() > 6 {
            self.feed.pop_front();
        }
        self.bursts.retain(|b| now - b.0 < 600.0);
        self.shows.retain(|s| now - s.0 < 1600.0);
        self.numbers.retain(|n| now - n.0 < 900.0);
    }

    /// Everyone as they were a moment ago, smoothly between frames.
    pub fn others(&self, now: f64) -> Vec<Seen> {
        let t = (self.server_ms(now) - BEHIND) / MS_A_TICK;
        let Some(last) = self.snaps.back() else {
            return Vec::new();
        };
        let mut a = last;
        let mut b = last;
        for w in self.snaps.iter().collect::<Vec<_>>().windows(2) {
            if w[0].0 as f64 <= t && t < w[1].0 as f64 {
                a = w[0];
                b = w[1];
            }
        }
        let span = (b.0 as f64 - a.0 as f64).max(1.0);
        let k = ((t - a.0 as f64) / span).clamp(0.0, 1.0) as f32;
        b.1.iter()
            .map(|s| match a.1.iter().find(|o| o.id == s.id) {
                Some(o) => {
                    let lerp = |x: f32, y: f32| x + (y - x) * k;
                    let turn = s.yaw.wrapping_sub(o.yaw) as i16 as f32;
                    Seen {
                        p: [
                            lerp(o.p[0], s.p[0]),
                            lerp(o.p[1], s.p[1]),
                            lerp(o.p[2], s.p[2]),
                        ],
                        yaw: o.yaw.wrapping_add((turn * k) as i16 as u16),
                        ..*s
                    }
                }
                None => *s,
            })
            .collect()
    }

    /// Bolts where they are now (flown on from the last frame).
    pub fn bolts(&self, now: f64) -> Vec<proto::BoltSeen> {
        let Some(f) = &self.frame else {
            return Vec::new();
        };
        let ahead =
            ((self.server_ms(now) - f.tick as f64 * MS_A_TICK) / 1000.0).clamp(0.0, 0.1) as f32;
        f.bolts
            .iter()
            .map(|b| proto::BoltSeen {
                p: [
                    b.p[0] + b.v[0] * ahead,
                    b.p[1] + b.v[1] * ahead,
                    b.p[2] + b.v[2] * ahead,
                ],
                ..*b
            })
            .collect()
    }
}
