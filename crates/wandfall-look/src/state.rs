//! What the page knows: who is who, the latest frame, a few frames back
//! to draw everyone else smoothly (a tenth of a second in the past), the
//! server's clock, and what just happened (the feed, hits, your end).

use std::collections::{HashMap, VecDeque};

use wandfall::laws::TICK_HZ;
use wandfall::proto::{self, Ev, Frame, Loot, Seen};
use wandfall::world::STORM;

/// How far behind the newest frame others are drawn (ms).
const BEHIND: f64 = 100.0;
const MS_A_TICK: f64 = 1000.0 / TICK_HZ as f64;

/// A line in the feed: words, or a knockout (who, with what, whom).
#[derive(Clone, Debug)]
pub enum Line {
    Text(String),
    Out { by: u16, with: u8, who: u16 },
}

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
    pub feed: VecDeque<(f64, Line)>,
    /// When your bolt last hit someone; when you were last hurt.
    pub hit_at: f64,
    pub hurt_at: f64,
    /// Where what hurt you came from, and when (the last few seconds).
    pub hurt_from: Vec<(f64, [f32; 3])>,
    /// Bursts of light where something struck someone: (when, who,
    /// what).
    pub bursts: Vec<(f64, u16, u8)>,
    /// You are out: by whom, your place, and with what.
    pub out: Option<(u16, u16)>,
    pub out_with: u8,
    /// Yourself as you last were alive (for the card when you are out).
    pub last_own: Option<proto::Own>,
    /// Dust where wizards landed: (when, where, how hard).
    pub dust: Vec<(f64, [f32; 3], f32)>,
    /// Where wizards fell: (when, where, who, facing which way).
    pub falls: Vec<(f64, [f32; 3], u16, u16)>,
    /// What last hurt each wizard.
    last_hit: HashMap<u16, u8>,
    pub joined: bool,
    pub loot: Loot,
    /// Spells cast and landing, leaps and levels, for their effects:
    /// (when, what).
    pub shows: Vec<(f64, Ev)>,
    /// When you last levelled, and to what.
    pub levelled: (f64, u8),
    /// When you last learned a spell or ranked one up: the spell, its
    /// rank now, and whether it is in a slot.
    pub learned: (f64, u8, u8, bool),
    /// Your hits, to show their numbers: (when, on whom, how much, with
    /// what).
    pub numbers: Vec<(f64, u16, u16, u8)>,
    /// When each of your spells was last ready again (a flash on the
    /// bar), and its cooldown the frame before.
    pub ready: [f64; 4],
    cds: [u16; 4],
    /// When each wizard last loosed a bolt (to raise its arm).
    pub fired: HashMap<u16, f64>,
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
        if let Some(o) = &f.you {
            if let Some(old) = &self.last_own {
                for (k, (&a, &b)) in old.book.iter().zip(&o.book).enumerate() {
                    if b > a {
                        let slotted = o.slots.iter().flatten().any(|s| s.0 == k as u8);
                        self.learned = (now, k as u8, b, slotted);
                    }
                }
            }
            self.last_own = Some(*o);
            for k in 0..4 {
                if self.cds[k] > 0 && o.cds[k] == 0 {
                    self.ready[k] = now;
                }
            }
            self.cds = o.cds;
        }
        for b in &f.bolts {
            let known = self
                .frame
                .as_ref()
                .is_some_and(|o| o.bolts.iter().any(|x| x.id == b.id));
            if !known {
                self.fired.insert(b.by, now);
            }
        }
        self.snaps.push_back((f.tick, f.players.clone()));
        while self.snaps.len() > 12 {
            self.snaps.pop_front();
        }
        self.frame = Some(f);
    }

    pub fn events(&mut self, list: Vec<Ev>, now: f64) {
        for e in list {
            match e {
                Ev::Hit {
                    by,
                    to,
                    amount,
                    what,
                } => {
                    if by == self.you {
                        self.hit_at = now;
                        self.numbers.push((now, to, amount, what));
                    }
                    if to == self.you {
                        self.hurt_at = now;
                        // From where the one who hurt you stands (the
                        // storm comes from everywhere).
                        let from = self
                            .frame
                            .as_ref()
                            .and_then(|f| f.players.iter().find(|s| s.id == by && by != to));
                        if let Some(s) = from {
                            self.hurt_from.retain(|h| now - h.0 < 2000.0);
                            self.hurt_from.push((now, s.p));
                        }
                    }
                    self.bursts.push((now, to, what));
                    self.last_hit.insert(to, what);
                }
                Ev::Out { who, by, place } => {
                    let with = self.last_hit.get(&who).copied().unwrap_or(STORM);
                    let line = if by == 0 {
                        Line::Text(format!("{} fell to the storm", self.name(who)))
                    } else {
                        Line::Out { by, with, who }
                    };
                    self.feed.push_back((now, line));
                    if who == self.you {
                        self.out = Some((by, place));
                        self.out_with = with;
                    }
                    let at = self
                        .snaps
                        .back()
                        .and_then(|s| s.1.iter().find(|s| s.id == who).map(|s| (s.p, s.yaw)));
                    if let Some((at, yaw)) = at {
                        self.falls.push((now, at, who, yaw));
                    }
                }
                Ev::Win { who } => {
                    let line = format!("{} wins the match", self.name(who));
                    self.feed.push_back((now, Line::Text(line)));
                }
                Ev::Begin => {
                    self.out = None;
                    self.last_hit.clear();
                    let line = Line::Text("the match begins: drop!".to_string());
                    self.feed.push_back((now, line));
                }
                Ev::Lobby => self.out = None,
                Ev::Level { who, level } => {
                    if who == self.you {
                        self.levelled = (now, level);
                    }
                    self.shows.push((now, e));
                }
                Ev::Cast { .. } | Ev::Beam { .. } => self.shows.push((now, e)),
            }
        }
        while self.feed.len() > 6 {
            self.feed.pop_front();
        }
        self.bursts.retain(|b| now - b.0 < 600.0);
        self.shows.retain(|s| now - s.0 < 4000.0);
        self.numbers.retain(|n| now - n.0 < 900.0);
        self.falls.retain(|f| now - f.0 < 4000.0);
        self.dust.retain(|d| now - d.0 < 800.0);
    }

    /// Everyone as they were a moment ago, smoothly between frames.
    /// The tick everyone else is drawn at now (a little behind the
    /// newest frame).
    pub fn view_tick(&self, now: f64) -> u32 {
        ((self.server_ms(now) - BEHIND) / MS_A_TICK)
            .round()
            .max(0.0) as u32
    }

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

    /// How fast a wizard moves over the ground (m/s), from the last two
    /// frames.
    pub fn speed(&self, id: u16) -> f32 {
        let n = self.snaps.len();
        if n < 2 {
            return 0.0;
        }
        let (a, b) = (&self.snaps[n - 2], &self.snaps[n - 1]);
        let (Some(p), Some(q)) = (
            a.1.iter().find(|s| s.id == id),
            b.1.iter().find(|s| s.id == id),
        ) else {
            return 0.0;
        };
        let secs = (b.0.saturating_sub(a.0)).max(1) as f32 / TICK_HZ as f32;
        ((q.p[0] - p.p[0]).powi(2) + (q.p[2] - p.p[2]).powi(2)).sqrt() / secs
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
