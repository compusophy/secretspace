//! What the page knows: who is who, the latest frame, a few frames back
//! to draw everyone else smoothly (a tenth of a second in the past), the
//! server's clock, and what just happened (the feed, hits, your end).

use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};

use wandfall::laws::{spell, CAST_AHEAD, CROUCH_EYE, EYE, TICK_HZ};
use wandfall::proto::{self, flag, Ev, Frame, Loot, Seen};
use wandfall::trig;
use wandfall::world::STORM;

use crate::fx::scar_life;

/// How far behind the newest frame others are drawn (ms).
const BEHIND: f64 = 100.0;
const MS_A_TICK: f64 = 1000.0 / TICK_HZ as f64;
/// How many spells' scars lie on the island at most.
const SCARS: usize = 64;
/// How far a frame may say the server's clock is from the one kept
/// (ms) before it counts as off, and how many frames running must be
/// off before the clock is taken afresh (a new world counts its ticks
/// from nought): one late frame is a hiccup, not a new clock.
const STRAY: f64 = 250.0;
const OFF_FRAMES: u32 = 15;
/// Someone else's bolt starts on the clock wizards are drawn on (from
/// its caster's wand as drawn), and catches up with the newest frame
/// over this long (ms), so it lands when the hit does.
const CATCH_UP: f64 = 150.0;
/// Further than this between two frames a tick apart (m) is a leap (a
/// blink, a respawn, the drop), not a run: drawn there, not slid there.
const LEAP: f32 = 4.0;

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
    /// Server ms minus page ms, smoothed; how many frames running have
    /// said otherwise.
    offset: Option<f64>,
    off: u32,
    snaps: VecDeque<(u32, Vec<Seen>)>,
    /// (when, line).
    pub feed: VecDeque<(f64, Line)>,
    /// When your bolt last hit someone; when you were last hurt.
    pub hit_at: f64,
    pub hurt_at: f64,
    /// Where what hurt you came from, and when (the last few seconds).
    pub hurt_from: Vec<(f64, [f32; 3])>,
    /// Bursts of light where something struck someone: (when, who,
    /// what, by whom).
    pub bursts: Vec<(f64, u16, u8, u16)>,
    /// You are out: by whom, your place, and with what.
    pub out: Option<(u16, u16)>,
    pub out_with: u8,
    /// Yourself as you last were alive (for the card when you are out).
    pub last_own: Option<proto::Own>,
    /// Dust where wizards landed: (when, where, how hard).
    pub dust: Vec<(f64, [f32; 3], f32)>,
    /// Where wizards fell: (when, where, who, facing which way, and
    /// whether it has been put where the wizard was drawn yet).
    pub falls: Vec<(f64, [f32; 3], u16, u16, bool)>,
    /// Where spells left their mark on the island (a fireball's burn,
    /// lightning's, the Lance's, frost's rime): (when, where, which).
    pub scars: Vec<(f64, [f32; 3], u8)>,
    /// What last hurt each wizard.
    last_hit: HashMap<u16, u8>,
    pub joined: bool,
    pub loot: Loot,
    /// The hall of wizards' best: (name, wins, knockouts).
    pub hall: Vec<(String, u32, u32)>,
    /// Spells cast and landing, leaps and levels, for their effects:
    /// (when, what).
    pub shows: Vec<(f64, Ev)>,
    /// How each caster stood as it cast, as the frame before said (its
    /// spell's flash and frost's fan go the way it looked, a blink leaves
    /// from its feet): (when, who, its look, its feet).
    pub stood: Vec<(f64, u16, [f32; 3], [f32; 3])>,
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
    /// Each bolt in flight: when it was first seen, and the tick it was.
    born: HashMap<u16, (f64, u32)>,
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
        self.clock(f.tick as f64 * MS_A_TICK - now);
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
            if let Entry::Vacant(e) = self.born.entry(b.id) {
                e.insert((now, f.tick));
                self.fired.insert(b.by, now);
            }
        }
        self.born
            .retain(|id, _| f.bolts.iter().any(|b| b.id == *id));
        self.snaps.push_back((f.tick, f.players.clone()));
        while self.snaps.len() > 12 {
            self.snaps.pop_front();
        }
        self.frame = Some(f);
    }

    /// The server's clock as a frame says it (server ms minus page ms):
    /// a frame sooner than the clock kept says the link is quicker than
    /// thought, and is taken up quickly; a later one only slowly (a frame
    /// held up is not the link); one far off is let be, unless frame
    /// after frame says so.
    fn clock(&mut self, guess: f64) {
        let Some(o) = self.offset else {
            self.offset = Some(guess);
            return;
        };
        if (guess - o).abs() >= STRAY {
            self.off += 1;
            if self.off >= OFF_FRAMES {
                self.offset = Some(guess);
                self.off = 0;
            }
            return;
        }
        self.off = 0;
        let k = if guess > o { 0.2 } else { 0.02 };
        self.offset = Some(o + (guess - o) * k);
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
                    self.bursts.push((now, to, what, by));
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
                        self.falls.push((now, at, who, yaw, false));
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
                Ev::Cast {
                    spell: s,
                    stage: 1,
                    at,
                    ..
                } if matches!(s, spell::FIREBALL | spell::LIGHTNING | spell::FROST) => {
                    // Frost's shards land close together: one rime where
                    // they land on one another's.
                    let rimed = s == spell::FROST
                        && self.scars.iter().any(|c| {
                            c.2 == spell::FROST
                                && now - c.0 < 1000.0
                                && (c.1[0] - at[0]).powi(2) + (c.1[2] - at[2]).powi(2) < 4.0
                        });
                    if !rimed {
                        self.scars.push((now, at, s));
                    }
                    self.shows.push((now, e));
                }
                Ev::Beam {
                    spell: spell::LANCE,
                    to,
                    ..
                } => {
                    self.scars.push((now, to, spell::LANCE));
                    self.shows.push((now, e));
                }
                Ev::Cast {
                    by, stage: 0, at, ..
                } => {
                    if let Some(s) = self.player(by) {
                        let look = trig::look(s.yaw, s.pitch);
                        let eye = if s.flags & flag::CROUCH != 0 {
                            CROUCH_EYE
                        } else {
                            EYE
                        };
                        let feet = [
                            at[0] - look[0] * CAST_AHEAD,
                            at[1] - look[1] * CAST_AHEAD - eye,
                            at[2] - look[2] * CAST_AHEAD,
                        ];
                        self.stood.push((now, by, look, feet));
                    }
                    self.shows.push((now, e));
                }
                Ev::Cast { .. } | Ev::Beam { .. } => self.shows.push((now, e)),
            }
        }
        while self.feed.len() > 6 {
            self.feed.pop_front();
        }
        self.prune(now);
    }

    /// A wizard as the newest frame has it.
    fn player(&self, id: u16) -> Option<&Seen> {
        self.frame.as_ref()?.players.iter().find(|s| s.id == id)
    }

    /// What is over by `now` let go: bursts, shows, numbers, the fallen,
    /// dust, scars (each as long as it lasts, the oldest gone past the
    /// most). Every frame drawn as well as when events come: dust is
    /// raised while none do.
    pub fn prune(&mut self, now: f64) {
        self.scars.retain(|s| now - s.0 < scar_life(s.2));
        if self.scars.len() > SCARS {
            self.scars.drain(..self.scars.len() - SCARS);
        }
        self.bursts.retain(|b| now - b.0 < 600.0);
        self.shows.retain(|s| now - s.0 < 4000.0);
        self.stood.retain(|s| now - s.0 < 4000.0);
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
        let leap = (LEAP * span as f32).powi(2);
        b.1.iter()
            .map(|s| match a.1.iter().find(|o| o.id == s.id) {
                Some(o) => {
                    let d = [s.p[0] - o.p[0], s.p[1] - o.p[1], s.p[2] - o.p[2]];
                    let k = if d[0] * d[0] + d[1] * d[1] + d[2] * d[2] > leap {
                        1.0
                    } else {
                        k
                    };
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

    /// Bolts where they are now (flown on from the last frame). Yours
    /// as the newest frame has them; someone else's first as it was
    /// when the wizards are drawn (leaving its caster's wand as drawn,
    /// and not there before it left), catching up over `CATCH_UP`.
    pub fn bolts(&self, now: f64) -> Vec<proto::BoltSeen> {
        let Some(f) = &self.frame else {
            return Vec::new();
        };
        let srv = self.server_ms(now);
        let newest = f.tick as f64 * MS_A_TICK;
        f.bolts
            .iter()
            .filter_map(|b| {
                let (seen, tick) = self.born.get(&b.id).copied().unwrap_or((now, f.tick));
                let late = if b.by == self.you {
                    0.0
                } else {
                    BEHIND * (1.0 - ((now - seen) / CATCH_UP).clamp(0.0, 1.0))
                };
                let at = srv - late;
                // It left its wand a tick before it was first seen.
                if at < (tick as f64 - 1.0) * MS_A_TICK {
                    return None;
                }
                let ahead = ((at - newest) / 1000.0).min(0.1) as f32;
                Some(proto::BoltSeen {
                    p: [
                        b.p[0] + b.v[0] * ahead,
                        b.p[1] + b.v[1] * ahead,
                        b.p[2] + b.v[2] * ahead,
                    ],
                    ..*b
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wandfall::world::WAND;

    fn frame(tick: u32, players: Vec<Seen>, bolts: Vec<proto::BoltSeen>) -> Frame {
        Frame {
            tick,
            players,
            bolts,
            ..Frame::default()
        }
    }

    fn wizard(id: u16, p: [f32; 3]) -> Seen {
        Seen {
            id,
            p,
            flags: flag::ALIVE,
            ..Seen::default()
        }
    }

    #[test]
    fn a_hiccup_in_the_link_does_not_rubber_band_everyone() {
        let mut st = State::default();
        // Frames a tick apart, each arriving 50 ms after it was sent.
        let sent = |tick: u32| tick as f64 * MS_A_TICK;
        for tick in 1..=60 {
            st.take(frame(tick, Vec::new(), Vec::new()), sent(tick) + 50.0);
        }
        // The link stalls 300 ms; the frames held up come all at once,
        // and the clock the wizards are drawn on runs on through it.
        let late = sent(70) + 50.0 + 300.0;
        let mut last = st.server_ms(late);
        for tick in 61..=70 {
            st.take(frame(tick, Vec::new(), Vec::new()), late);
            let now = st.server_ms(late);
            assert!(now > last - 20.0, "the clock went back {} ms", last - now);
            last = now;
        }
        // On time again: the clock where it was.
        for tick in 71..=90 {
            let at = sent(tick) + 50.0;
            st.take(frame(tick, Vec::new(), Vec::new()), at);
            assert!((st.server_ms(at) - sent(tick)).abs() < 30.0);
        }
    }

    #[test]
    fn a_new_world_s_clock_is_taken_up() {
        let mut st = State::default();
        for tick in 1..=60 {
            let now = tick as f64 * MS_A_TICK;
            st.take(frame(5000 + tick, Vec::new(), Vec::new()), now);
        }
        // The world built again: its ticks start from nought.
        let start = 61.0 * MS_A_TICK;
        for tick in 0..30 {
            let now = start + tick as f64 * MS_A_TICK;
            st.take(frame(tick, Vec::new(), Vec::new()), now);
        }
        let now = start + 29.0 * MS_A_TICK;
        assert!((st.server_ms(now) - 29.0 * MS_A_TICK).abs() < 40.0);
    }

    #[test]
    fn a_blink_is_drawn_there_not_slid_there() {
        let mut st = State::default();
        let at = |x| vec![wizard(2, [x, 0.0, 0.0])];
        st.take(frame(1, at(0.0), Vec::new()), 0.0);
        st.take(frame(2, at(12.0), Vec::new()), MS_A_TICK);
        st.take(frame(3, at(12.3), Vec::new()), 2.0 * MS_A_TICK);
        let o = st.offset.unwrap();
        // Halfway between the first two frames as drawn: there already.
        assert_eq!(st.others(1.5 * MS_A_TICK - o + BEHIND)[0].p[0], 12.0);
        // A run between the next two is slid along.
        let x = st.others(2.5 * MS_A_TICK - o + BEHIND)[0].p[0];
        assert!(x > 12.05 && x < 12.25, "{x}");
    }

    #[test]
    fn someone_else_s_bolt_leaves_their_wand_as_drawn_and_catches_up() {
        let mut st = State {
            you: 1,
            ..State::default()
        };
        let bolt = |id, by| proto::BoltSeen {
            id,
            by,
            kind: WAND,
            p: [1.0, 1.5, 0.0],
            v: [30.0, 0.0, 0.0],
        };
        for tick in 1..=30 {
            st.take(frame(tick, Vec::new(), Vec::new()), tick as f64 * MS_A_TICK);
        }
        let now = 31.0 * MS_A_TICK;
        st.take(frame(31, Vec::new(), vec![bolt(7, 2), bolt(8, 1)]), now);
        // Yours at once; theirs not yet (its caster is drawn as it was
        // before it cast).
        let ids: Vec<u16> = st.bolts(now).iter().map(|b| b.id).collect();
        assert_eq!(ids, vec![8]);
        // A little later theirs too, behind yours; caught up in time.
        let later = st.bolts(now + 80.0);
        let x = |id| later.iter().find(|b| b.id == id).map(|b| b.p[0]);
        assert!(x(7).unwrap() < x(8).unwrap());
        let caught = st.bolts(now + CATCH_UP + 1.0);
        assert_eq!(caught[0].p, caught[1].p);
    }

    #[test]
    fn a_cast_remembers_how_its_caster_stood() {
        let mut st = State::default();
        let mut w = wizard(2, [5.0, 1.0, 3.0]);
        w.flags |= flag::CROUCH;
        w.yaw = 16384;
        w.pitch = -2000;
        st.take(frame(1, vec![w], Vec::new()), 0.0);
        let look = trig::look(w.yaw, w.pitch);
        let at = [
            5.0 + look[0] * CAST_AHEAD,
            1.0 + CROUCH_EYE + look[1] * CAST_AHEAD,
            3.0 + look[2] * CAST_AHEAD,
        ];
        let cast = Ev::Cast {
            by: 2,
            spell: spell::FROST,
            stage: 0,
            at,
        };
        st.events(vec![cast], 10.0);
        let (when, who, l, feet) = st.stood[0];
        assert_eq!((when, who, l), (10.0, 2, look));
        for k in 0..3 {
            assert!((feet[k] - w.p[k]).abs() < 1e-4, "{feet:?}");
        }
    }

    #[test]
    fn what_is_over_goes_though_no_events_come() {
        let mut st = State::default();
        st.dust.push((0.0, [0.0; 3], 1.0));
        st.prune(500.0);
        assert_eq!(st.dust.len(), 1);
        st.prune(900.0);
        assert!(st.dust.is_empty());
    }

    #[test]
    fn frost_s_shards_leave_one_rime_and_it_goes_when_it_fades() {
        let mut st = State::default();
        let shard = |x| Ev::Cast {
            by: 2,
            spell: spell::FROST,
            stage: 1,
            at: [x, 0.0, 0.0],
        };
        st.events((0..7).map(|k| shard(k as f32 * 0.3)).collect(), 0.0);
        st.events(vec![shard(30.0)], 100.0);
        assert_eq!(st.scars.len(), 2);
        st.prune(scar_life(spell::FROST) + 200.0);
        assert!(st.scars.is_empty());
    }
}
