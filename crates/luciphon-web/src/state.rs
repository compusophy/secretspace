//! What the page knows, and when: the mirror of the world, its own Lumen
//! predicted ahead (`luciphon::predict`) and drawn smoothly between ticks,
//! everyone else drawn 66 ms behind the newest frame between the two
//! frames around that moment, the chunks the renderer must (re)build, and
//! the feel each frame's events start.

use std::collections::{HashMap, HashSet, VecDeque};

use luciphon::combat::Swing;
use luciphon::laws::{Laws, LAWS};
use luciphon::mirror::Mirror;
use luciphon::motion::Intent;
use luciphon::predict::Predictor;
use luciphon::proto::{kind, ClaimInfo, Down, Ev, PROTO};
use luciphon::tiles::Tiles;
use pixels::Rgba;

use crate::controls::TICK_MS;
use crate::fx::Fx;
use crate::laws::FEEL;

/// Where everyone was when a frame arrived (x, y, height).
type Snap = (f64, HashMap<u16, [f32; 3]>);

/// A beam of light to draw: from, to (x, height, y), when it was fired,
/// whether it pierced, and its shooter's hue.
#[derive(Clone, Copy, Debug)]
pub struct Beam {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub at: f64,
    pub big: bool,
    pub hue: u8,
}

/// How long a beam shows, ms.
pub const BEAM_MS: f64 = 160.0;

/// A thing to draw, where the page decided it is now.
#[derive(Clone, Debug, Default)]
pub struct Thing {
    pub id: u16,
    pub kind: u8,
    pub x: f32,
    pub y: f32,
    /// Height above the ground.
    pub z: f32,
    pub facing: u16,
    /// Movement in the low 3 bits, action in the next 3, ghost, down.
    pub state: u8,
    pub flame: u8,
    pub glim: u8,
    pub hue: u8,
    pub flow: u8,
    pub name: String,
    pub moving: bool,
    pub you: bool,
}

impl Thing {
    pub fn mv(&self) -> u8 {
        self.state & 7
    }
    pub fn act(&self) -> u8 {
        (self.state >> 3) & 7
    }
    pub fn ghost(&self) -> bool {
        self.state & 64 != 0
    }
    pub fn down(&self) -> bool {
        self.state & 128 != 0
    }
}

pub struct State {
    pub laws: Laws,
    pub mirror: Mirror,
    pub pred: Predictor,
    pub you: u16,
    pub joined: bool,
    pub connected: bool,
    pub people: u16,
    pub awake: u16,
    snaps: VecDeque<Snap>,
    /// Your Lumen before the last Input, and when that Input went.
    prev: [f32; 3],
    pushed_at: f64,
    /// How far the drawn self is from the predicted one, fading.
    offset: [f32; 3],
    /// Your path since your last return (for the Underlight's thread).
    pub path: VecDeque<(f32, f32)>,
    path_at: f64,
    pub rtt: f64,
    /// The newest frame's own descent and killer.
    pub descent: u8,
    pub killer: u16,
    /// Things the page felt this frame (for haptics).
    pub felt: Vec<u32>,
    /// Every claim, by id; the world's seed; when the newest frame came.
    pub claims: HashMap<u16, ClaimInfo>,
    pub seed: u64,
    frame_at: f64,
    /// A word that shows for a moment (a level, a refusal).
    pub toast: Option<(String, f64)>,
    /// Chunks to (re)build, and chunks gone.
    pub stale: HashSet<(i32, i32)>,
    pub gone: Vec<(i32, i32)>,
    /// When you last fired (for the wand's kick), and beams in the air.
    pub struck_at: f64,
    pub beams: Vec<Beam>,
    /// The server no longer speaks this page's protocol.
    pub outdated: bool,
}

impl Default for State {
    fn default() -> State {
        State {
            laws: LAWS,
            mirror: Mirror::default(),
            pred: Predictor::default(),
            you: 0,
            joined: false,
            connected: false,
            people: 0,
            awake: 0,
            snaps: VecDeque::new(),
            prev: [0.0; 3],
            pushed_at: 0.0,
            offset: [0.0; 3],
            path: VecDeque::new(),
            path_at: 0.0,
            rtt: 0.0,
            descent: 0,
            killer: 0,
            felt: Vec::new(),
            claims: HashMap::new(),
            seed: 0,
            frame_at: 0.0,
            toast: None,
            stale: HashSet::new(),
            gone: Vec::new(),
            struck_at: 0.0,
            beams: Vec::new(),
            outdated: false,
        }
    }
}

fn pos(p: &Predictor) -> [f32; 3] {
    let b = &p.me.body;
    [b.x.to_f32(), b.y.to_f32(), b.z.to_f32()]
}

impl State {
    /// Take in one message from the room.
    pub fn receive(&mut self, bytes: &[u8], now: f64, fx: &mut Fx) {
        let Some(d) = Down::decode(bytes) else {
            return;
        };
        match &d {
            Down::Claims(list) => {
                self.claims = list.iter().map(|c| (c.id, c.clone())).collect();
                // Land takes its claim's hue: every chunk again.
                self.stale.extend(self.mirror.chunks.iter().copied());
            }
            Down::Welcome {
                laws,
                you,
                seed,
                oldest,
                ..
            } => {
                self.outdated |= *oldest > PROTO;
                self.seed = *seed;
                if let Some(l) = Laws::decode(laws) {
                    self.laws = l;
                }
                if *you != 0 {
                    self.you = *you;
                    self.joined = true;
                }
            }
            Down::Board { people, awake } => {
                self.people = *people;
                self.awake = *awake;
            }
            Down::Pong { t } => {
                let rtt = now - *t as f64;
                if rtt >= 0.0 {
                    self.rtt = if self.rtt == 0.0 {
                        rtt
                    } else {
                        self.rtt * 0.8 + rtt * 0.2
                    };
                }
            }
            Down::Elsewhere => {
                self.joined = false;
                self.you = 0;
            }
            _ => {}
        }
        if !self.mirror.apply(&d) {
            match d {
                Down::Chunk { cx, cy, .. } => {
                    let (cx, cy) = (cx as i32, cy as i32);
                    self.stale.insert((cx, cy));
                    // Neighbours' edges may face it.
                    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        if self.mirror.chunks.contains(&(cx + dx, cy + dy)) {
                            self.stale.insert((cx + dx, cy + dy));
                        }
                    }
                }
                Down::ChunkGone { cx, cy } => self.gone.push((cx as i32, cy as i32)),
                _ => {}
            }
            return;
        }
        // A frame.
        let Down::Frame(f) = d else { return };
        self.frame_at = now;
        for &(i, _) in &f.tiles {
            let (x, y) = Tiles::at_index(i as usize);
            self.stale.insert(Tiles::chunk_of(x, y));
        }
        let at: HashMap<u16, [f32; 3]> = self
            .mirror
            .ents
            .values()
            .map(|e| {
                let k = 1.0 / 256.0;
                (e.id, [e.x as f32 * k, e.y as f32 * k, e.z as f32 * k])
            })
            .collect();
        self.snaps.push_back((now, at));
        while self.snaps.len() > 6 {
            self.snaps.pop_front();
        }
        if let Some(own) = f.own {
            let before = self.drawn_self(now);
            self.pred
                .reconcile(&own, f.ack, &self.mirror.tiles, &self.laws);
            self.offset = [0.0; 3];
            let after = self.drawn_self(now);
            let off = [
                before[0] - after[0],
                before[1] - after[1],
                before[2] - after[2],
            ];
            let far = (off[0] * off[0] + off[1] * off[1]).sqrt() > FEEL.blend_tiles;
            if !far && before != [0.0; 3] {
                self.offset = off;
            } else {
                self.prev = pos(&self.pred);
            }
            if self.descent == 0 && own.descent > 0 {
                self.felt.push(40);
            }
            if self.descent > 0 && own.descent == 0 {
                self.path.clear();
                self.prev = pos(&self.pred);
            }
            self.descent = own.descent;
            self.killer = own.killer;
        }
        self.feel(&f.events, fx, now);
    }

    /// Apply an Input now, as the server will; whether the wand fired,
    /// and how (to draw at once).
    pub fn push(&mut self, seq: u16, it: Intent, now: f64) -> Option<Swing> {
        self.prev = pos(&self.pred);
        self.pushed_at = now;
        let s = self.pred.push(seq, it, &self.mirror.tiles, &self.laws);
        if s.is_some() {
            self.struck_at = now;
        }
        s
    }

    /// How far your own beam goes, along your aim from where you are: to
    /// the first solid tile, or (unless it pierces) the first body.
    pub fn own_beam(&self, now: f64, big: bool) -> f32 {
        let b = &self.pred.me.body;
        let aim = self.pred.me.act.aim;
        let l = &self.laws;
        let (end, _) = luciphon::combat::beam(&self.mirror.tiles, b.x, b.y, aim, l.beam_reach);
        let mut end = end.to_f32();
        if !big {
            let (x, y) = (b.x.to_f32(), b.y.to_f32());
            let a = aim as f32 / 65536.0 * std::f32::consts::TAU;
            let (ux, uy) = (a.cos(), a.sin());
            let width = l.beam_width.to_f32();
            for t in self.things(now) {
                if t.kind != kind::LUMEN || t.you {
                    continue;
                }
                let (dx, dy) = (t.x - x, t.y - y);
                let along = dx * ux + dy * uy;
                if along > 0.0 && along < end && (dx * uy - dy * ux).abs() <= width {
                    end = along;
                }
            }
        }
        end
    }

    /// Where something is now (x, y, height), as drawn.
    pub fn at(&self, id: u16, now: f64) -> Option<[f32; 3]> {
        if id == self.you && self.pred.ready {
            return Some(self.drawn_self(now));
        }
        self.snaps.back().and_then(|s| s.1.get(&id).copied())
    }

    fn feel(&mut self, events: &[Ev], fx: &mut Fx, now: f64) {
        use lucilook::palette::{EMBER, GOLD, INK, RIM};
        let g = |p: [f32; 3], up: f32| [p[0], p[2] + up, p[1]];
        for e in events {
            let Some(p) = self.at(e.a, now) else { continue };
            let mine = e.a == self.you;
            match e.kind {
                // Others' beams (your own were drawn when you fired).
                4 if !mine => {
                    let a = e.b as f32 / 65536.0 * std::f32::consts::TAU;
                    let (ux, uy) = (a.cos(), a.sin());
                    let len = (e.n & 127) as f32 / 8.0;
                    let from = [p[0] + ux * 0.35, p[2] + 0.95, p[1] + uy * 0.35];
                    let to = [p[0] + ux * len, p[2] + 0.95, p[1] + uy * len];
                    let hue = self.mirror.ents.get(&e.a).map_or(0, |e| e.hue);
                    self.beams.push(Beam {
                        from,
                        to,
                        at: now,
                        big: e.n & 128 != 0,
                        hue,
                    });
                    fx.burst(to, 5, 2.0, RIM, now);
                }
                2 => {
                    let big = e.n & 128 != 0;
                    let t = self.at(e.b, now).unwrap_or(p);
                    fx.hit(g(t, 0.9), big, now);
                    if mine {
                        self.felt.push(if big { 25 } else { 10 });
                    }
                    if e.b == self.you {
                        fx.hurt(big, now);
                        self.felt.push(if big { 40 } else { 20 });
                    }
                }
                6 | 7 if !mine => fx.burst(g(p, 0.5), 8, 3.0, RIM, now),
                7 => self.felt.push(15),
                9 => fx.ring(g(p, 0.0), 0.7, INK, now),
                10 => fx.burst(g(p, 0.6), 12, 4.0, EMBER, now),
                12 => fx.burst(g(p, 0.6), 30, 6.0, GOLD, now),
                13 => fx.ring(g(p, 0.0), 1.6, GOLD, now),
                14 => {
                    let (tx, ty) = Tiles::at_index(e.b as usize);
                    let at = [tx as f32 + 0.5, 0.8, ty as f32 + 0.5];
                    let c = if e.n & 1 != 0 {
                        RIM
                    } else {
                        Rgba::rgb(200, 180, 140)
                    };
                    fx.burst(at, if e.n & 1 != 0 { 14 } else { 6 }, 2.5, c, now);
                    if e.n & 1 != 0 {
                        fx.ring([at[0], 0.0, at[2]], 0.8, RIM, now);
                        if mine {
                            self.felt.push(10);
                        }
                    }
                }
                16 => {
                    fx.burst(g(p, 0.6), 36, 7.0, GOLD, now);
                    if mine {
                        self.toast = Some((format!("{} tiles kindled", e.b), now));
                    }
                }
                17 => fx.burst(g(p, 0.6), 18, 5.0, EMBER, now),
                20 if mine => {
                    const SKILLS: [&str; 7] = [
                        "hewing",
                        "delving",
                        "kindling",
                        "tending",
                        "valor",
                        "wayfaring",
                        "voice",
                    ];
                    fx.ring(g(p, 0.0), 2.4, GOLD, now);
                    let name = SKILLS.get(e.n as usize).unwrap_or(&"skill");
                    self.toast = Some((format!("{name} {}", e.b), now));
                }
                21 => {
                    let (tx, ty) = Tiles::at_index(e.b as usize);
                    fx.burst([tx as f32 + 0.5, 0.5, ty as f32 + 0.5], 10, 3.0, GOLD, now);
                }
                23 if mine => self.toast = Some(("not here".into(), now)),
                _ => {}
            }
        }
    }

    /// The world's tick now, between frames.
    pub fn tick_now(&self, now: f64) -> f64 {
        self.mirror.tick as f64 + ((now - self.frame_at) / TICK_MS).clamp(0.0, 3.0)
    }

    /// Where your own Lumen is drawn (x, y, height): between the last two
    /// predicted ticks, with a small correction eased away.
    pub fn drawn_self(&self, now: f64) -> [f32; 3] {
        let cur = pos(&self.pred);
        let a = ((now - self.pushed_at) / TICK_MS).clamp(0.0, 1.0) as f32;
        let mut p = [0.0; 3];
        for k in 0..3 {
            p[k] = self.prev[k] + (cur[k] - self.prev[k]) * a + self.offset[k];
        }
        p
    }

    /// Ease the drawn self toward the prediction; record the path.
    pub fn age(&mut self, now: f64, dt: f64) {
        self.beams
            .retain(|b| now - b.at < BEAM_MS * if b.big { 2.0 } else { 1.0 });
        let k = (-dt / FEEL.blend_ms * 3.0).exp() as f32;
        for v in &mut self.offset {
            *v *= k;
        }
        if self.pred.ready && self.descent == 0 && now - self.path_at > 200.0 {
            self.path_at = now;
            let p = self.drawn_self(now);
            self.path.push_back((p[0], p[1]));
            while self.path.len() > 1500 {
                self.path.pop_front();
            }
        }
    }

    /// Everything to draw now: others 66 ms behind, yourself predicted.
    pub fn things(&self, now: f64) -> Vec<Thing> {
        let t = now - FEEL.behind_ms;
        let (a, b) = match self.snaps.len() {
            0 => return Vec::new(),
            1 => (&self.snaps[0], &self.snaps[0]),
            n => {
                let k = self
                    .snaps
                    .iter()
                    .position(|s| s.0 > t)
                    .unwrap_or(n - 1)
                    .max(1);
                (&self.snaps[k - 1], &self.snaps[k])
            }
        };
        let span = (b.0 - a.0).max(1.0);
        let alpha = ((t - a.0) / span).clamp(0.0, 1.0) as f32;
        let mut out = Vec::new();
        for e in self.mirror.ents.values() {
            if !matches!(e.kind, kind::LUMEN | kind::MOTE | kind::PICKUP) {
                continue;
            }
            let you = e.id == self.you && self.pred.ready;
            if you && self.descent > 0 {
                continue;
            }
            let k = 1.0 / 256.0;
            let (p, moving) = if you {
                (self.drawn_self(now), self.pred.me.body.speed().0 > 2000)
            } else {
                let p1 = b.1.get(&e.id).copied().unwrap_or([
                    e.x as f32 * k,
                    e.y as f32 * k,
                    e.z as f32 * k,
                ]);
                let p0 = a.1.get(&e.id).copied().unwrap_or(p1);
                let moving = (p1[0] - p0[0]).abs() + (p1[1] - p0[1]).abs() > 0.01;
                let mut p = [0.0; 3];
                for i in 0..3 {
                    p[i] = p0[i] + (p1[i] - p0[i]) * alpha;
                }
                (p, moving)
            };
            let mut thing = Thing {
                id: e.id,
                kind: e.kind,
                x: p[0],
                y: p[1],
                z: p[2],
                facing: e.facing,
                state: e.state,
                flame: e.flame,
                glim: e.glim,
                hue: e.hue,
                flow: e.flow,
                name: e.name.clone(),
                moving,
                you,
            };
            if you {
                // Your own state is the prediction's.
                let me = &self.pred.me;
                thing.facing = me.body.facing;
                thing.state =
                    (thing.state & 0b1100_0000) | me.body.mv as u8 | (me.act.act.code() & 7) << 3;
            }
            out.push(thing);
        }
        out
    }
}
