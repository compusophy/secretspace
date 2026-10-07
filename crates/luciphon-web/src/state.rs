//! What the page knows, and when: the mirror of the world, its own Lumen
//! predicted ahead (`luciphon::predict`), everyone else drawn 66 ms behind
//! the newest frame between the two frames around that moment, and the
//! feel each frame's events start.

use std::collections::{HashMap, VecDeque};

use lucilook::{Body, Look};
use luciphon::laws::{Laws, LAWS};
use luciphon::mirror::Mirror;
use luciphon::predict::Predictor;
use luciphon::proto::{kind, Down};

use crate::laws::FEEL;

/// Where everyone was when a frame arrived.
type Snap = (f64, HashMap<u16, (f32, f32)>);

pub struct State {
    pub laws: Laws,
    pub mirror: Mirror,
    pub pred: Predictor,
    pub you: u16,
    pub joined: bool,
    pub connected: bool,
    pub people: u16,
    pub awake: u16,
    /// Where everyone was in recent frames, and when each arrived.
    snaps: VecDeque<Snap>,
    /// How far the drawn self is from the predicted one, fading.
    offset: (f32, f32),
    pub offset_at: f64,
    /// Your path since your last return (for the Underlight's thread).
    pub path: VecDeque<(f32, f32)>,
    path_at: f64,
    pub rtt: f64,
    /// The newest frame's own descent and killer.
    pub descent: u8,
    pub killer: u16,
    /// Things the page felt this frame (for haptics).
    pub felt: Vec<u32>,
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
            offset: (0.0, 0.0),
            offset_at: 0.0,
            path: VecDeque::new(),
            path_at: 0.0,
            rtt: 0.0,
            descent: 0,
            killer: 0,
            felt: Vec::new(),
        }
    }
}

impl State {
    /// Take in one message from the room.
    pub fn receive(&mut self, bytes: &[u8], now: f64, look: &mut Look) {
        let Some(d) = Down::decode(bytes) else {
            return;
        };
        match &d {
            Down::Welcome { laws, you, .. } => {
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
                    look.ground.bake(&self.mirror.tiles, cx, cy);
                    // The chunk below may have a cliff it could not see.
                    if look.ground.has(cx, cy + 1) {
                        look.ground.bake(&self.mirror.tiles, cx, cy + 1);
                    }
                }
                Down::ChunkGone { cx, cy } => look.ground.forget(cx as i32, cy as i32),
                _ => {}
            }
            return;
        }
        // A frame.
        let Down::Frame(f) = d else { return };
        for &(i, _) in &f.tiles {
            let (x, y) = luciphon::tiles::Tiles::at_index(i as usize);
            look.ground.retile(&self.mirror.tiles, x, y);
        }
        let pos: HashMap<u16, (f32, f32)> = self
            .mirror
            .ents
            .values()
            .map(|e| (e.id, (e.x as f32 / 256.0, e.y as f32 / 256.0)))
            .collect();
        self.snaps.push_back((now, pos));
        while self.snaps.len() > 6 {
            self.snaps.pop_front();
        }
        if let Some(own) = f.own {
            let before = self.drawn_self();
            self.pred
                .reconcile(&own, f.ack, &self.mirror.tiles, &self.laws);
            let after = (self.pred.me.body.x.to_f32(), self.pred.me.body.y.to_f32());
            let off = (before.0 - after.0, before.1 - after.1);
            let far = (off.0 * off.0 + off.1 * off.1).sqrt() > FEEL.blend_tiles;
            self.offset = if far || before == (0.0, 0.0) {
                (0.0, 0.0)
            } else {
                off
            };
            self.offset_at = now;
            if self.descent == 0 && own.descent > 0 {
                self.felt.push(40);
            }
            if self.descent > 0 && own.descent == 0 {
                self.path.clear();
            }
            self.descent = own.descent;
            self.killer = own.killer;
        }
        self.feel(&f.events, look, now);
    }

    fn at(&self, id: u16) -> Option<(f32, f32)> {
        if id == self.you && self.pred.ready {
            return Some(self.drawn_self());
        }
        self.snaps.back().and_then(|s| s.1.get(&id).copied())
    }

    fn feel(&mut self, events: &[luciphon::proto::Ev], look: &mut Look, now: f64) {
        let fx = &mut look.fx;
        for e in events {
            let Some((x, y)) = self.at(e.a) else { continue };
            match e.kind {
                2 => {
                    let big = e.n & 128 != 0;
                    let (tx, ty) = self.at(e.b).unwrap_or((x, y));
                    fx.hit((x + tx) / 2.0, (y + ty) / 2.0 - 0.5, big, now);
                    if e.a == self.you {
                        self.felt.push(if big { 25 } else { 10 });
                    }
                }
                4 => fx.sling(x, y - 0.4, e.n != 0, now),
                6 | 7 => {
                    fx.burst(x, y - 0.3, 6, 3.0, lucilook::palette::RIM, now);
                    if e.a == self.you {
                        self.felt.push(15);
                    }
                }
                8 => fx.ring(x, y, 1.0, lucilook::palette::RIM, now),
                9 => fx.ring(x, y - 0.4, 0.7, lucilook::palette::INK, now),
                10 => fx.burst(x, y - 0.3, 10, 4.0, lucilook::palette::EMBER, now),
                12 => fx.burst(x, y - 0.3, 24, 6.0, lucilook::palette::GOLD, now),
                13 => fx.ring(x, y - 0.4, 1.6, lucilook::palette::GOLD, now),
                _ => {}
            }
        }
    }

    /// Where your own Lumen is drawn: predicted, with a small error eased.
    pub fn drawn_self(&self) -> (f32, f32) {
        let b = &self.pred.me.body;
        let (x, y) = (b.x.to_f32(), b.y.to_f32());
        (x + self.offset.0, y + self.offset.1)
    }

    /// Ease the drawn self toward the prediction; record the path.
    pub fn age(&mut self, now: f64, dt: f64) {
        let k = (-dt / FEEL.blend_ms * 3.0).exp() as f32;
        self.offset = (self.offset.0 * k, self.offset.1 * k);
        if self.pred.ready && self.descent == 0 && now - self.path_at > 200.0 {
            self.path_at = now;
            self.path.push_back(self.drawn_self());
            while self.path.len() > 1500 {
                self.path.pop_front();
            }
        }
    }

    /// Everything to draw now: others 66 ms behind, yourself predicted.
    pub fn bodies(&self, now: f64) -> Vec<Body> {
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
            let you = e.id == self.you && self.pred.ready && self.descent == 0;
            let (x, y, moving) = if you {
                let (x, y) = self.drawn_self();
                (x, y, self.pred.me.body.speed().0 > 2000)
            } else {
                let p1 =
                    b.1.get(&e.id)
                        .copied()
                        .unwrap_or((e.x as f32 / 256.0, e.y as f32 / 256.0));
                let p0 = a.1.get(&e.id).copied().unwrap_or(p1);
                let moving = (p1.0 - p0.0).abs() + (p1.1 - p0.1).abs() > 0.01;
                (
                    p0.0 + (p1.0 - p0.0) * alpha,
                    p0.1 + (p1.1 - p0.1) * alpha,
                    moving,
                )
            };
            if e.id == self.you && self.descent > 0 {
                continue;
            }
            let mut body = Body {
                id: e.id,
                kind: e.kind,
                x,
                y,
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
                body.facing = me.body.facing;
                body.state =
                    (body.state & 0b1100_0000) | me.body.mv as u8 | (me.act.act.code() & 7) << 3;
            }
            if e.kind == kind::LUMEN || e.kind == kind::MOTE || e.kind == kind::PICKUP {
                out.push(body);
            }
        }
        out
    }
}
