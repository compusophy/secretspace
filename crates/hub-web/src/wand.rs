//! Wandfall's card. While a match is on, the hub watches it (as a
//! watcher: never one of the people there) and draws it from above as the
//! game's map does: the island, the storm's circles, every wizard in its
//! colour, bolts and beams in the colours of their spells, a burst where
//! one falls. Between matches it draws the idea of one instead: the storm
//! closing and opening again, wizards duelling.

use pixels::{Canvas, Rect, Rgba};
use wandfall::laws::{MAP_HALF, SEA};
use wandfall::map::Map;
use wandfall::proto::{self, flag, Ev, Frame};

/// The spells' colours (as the game draws them).
const SPELLS: [(u8, u8, u8); 8] = [
    (255, 120, 40),
    (255, 215, 90),
    (120, 225, 255),
    (165, 115, 255),
    (255, 100, 200),
    (80, 140, 255),
    (120, 235, 110),
    (215, 235, 245),
];

fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9e37_79b9) ^ 0x85eb_ca6b;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2c1b_3c6d);
    x ^= x >> 12;
    (x & 0xffff) as f32 / 65535.0
}

/// The wizards' colours (as the game draws them).
const HUES: [(u8, u8, u8); 8] = [
    (70, 110, 230),
    (200, 60, 70),
    (60, 160, 110),
    (150, 80, 200),
    (220, 140, 40),
    (40, 170, 190),
    (220, 90, 160),
    (120, 120, 130),
];

fn spell(k: u8) -> Rgba {
    let (r, g, b) = SPELLS.get(k as usize).copied().unwrap_or((255, 214, 128));
    Rgba::rgb(r, g, b)
}

pub struct WandWatch {
    link: Option<kit::Link>,
    seed: Option<u64>,
    map: Option<Map>,
    /// The island drawn at a size, and that size.
    island: Option<Canvas>,
    frame: Option<Frame>,
    frame_at: f64,
    /// Beams (when, from, to, spell) and falls (when, where).
    beams: Vec<(f64, [f32; 3], [f32; 3], u8)>,
    falls: Vec<(f64, [f32; 3])>,
    buf: Canvas,
}

impl Default for WandWatch {
    fn default() -> WandWatch {
        WandWatch::new()
    }
}

impl WandWatch {
    pub fn new() -> WandWatch {
        WandWatch {
            link: None,
            seed: None,
            map: None,
            island: None,
            frame: None,
            frame_at: -1e9,
            beams: Vec::new(),
            falls: Vec::new(),
            buf: Canvas::new(1, 1),
        }
    }

    fn poll(&mut self, now: f64) {
        let link = self
            .link
            .get_or_insert_with(|| kit::Link::open("wandfall", Vec::new(), true));
        for ev in link.poll(now) {
            let kit::Net::Message(b) = ev else { continue };
            if let Some((_, _, seed, _)) = proto::read_welcome(&b) {
                if self.seed != Some(seed) {
                    self.seed = Some(seed);
                    self.map = Some(Map::new(seed));
                    self.island = None;
                }
            } else if let Some(f) = Frame::decode(&b) {
                self.frame = Some(f);
                self.frame_at = now;
            } else if let Some(list) = proto::read_events(&b) {
                for e in list {
                    match e {
                        Ev::Beam {
                            from, to, spell, ..
                        } => self.beams.push((now, from, to, spell)),
                        Ev::Out { who, .. } => {
                            let at = self
                                .frame
                                .as_ref()
                                .and_then(|f| f.players.iter().find(|p| p.id == who).map(|p| p.p));
                            if let Some(at) = at {
                                self.falls.push((now, at));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        self.beams.retain(|b| now - b.0 < 400.0);
        self.falls.retain(|f| now - f.0 < 900.0);
    }

    /// The island from above, `s` pixels a side.
    fn island(&mut self, s: i32) -> Option<&Canvas> {
        let map = self.map.as_ref()?;
        if self.island.as_ref().is_none_or(|c| c.w != s) {
            let mut c = Canvas::new(s, s);
            for j in 0..s {
                for i in 0..s {
                    let at = |k: i32| (k as f32 + 0.5) / s as f32 * 2.0 * MAP_HALF - MAP_HALF;
                    let h = map.height(at(i), at(j));
                    let col = if h < SEA {
                        Rgba::rgb(22, 50, 84)
                    } else if h < SEA + 0.6 {
                        Rgba::rgb(190, 172, 120)
                    } else {
                        let k = ((h - SEA) / 12.0).clamp(0.0, 1.0);
                        Rgba::rgb(64, 116, 58).mix(Rgba::rgb(136, 148, 88), k)
                    };
                    c.pixel(i, j, col);
                }
            }
            self.island = Some(c);
        }
        self.island.as_ref()
    }

    /// The card's picture in `b`: the match, live, if one is on.
    pub fn draw(&mut self, c: &mut Canvas, b: Rect, t: f32, u: i32, now: f64) {
        self.poll(now);
        let fight =
            self.frame.as_ref().is_some_and(|f| f.phase == 1) && now - self.frame_at < 2000.0;
        if !fight || self.map.is_none() {
            return idea(c, b, t, u as f32);
        }
        let uf = u as f32;
        self.buf.resize(b.w as i32, b.h as i32);
        self.buf.clear(Rgba::rgb(22, 50, 84));
        let side = (b.h.min(b.w) * 1.15) as i32;
        let (ox, oy) = ((b.w as i32 - side) / 2, (b.h as i32 - side) / 2);
        if let Some(isl) = self.island(side) {
            let isl = isl.clone();
            self.buf.blit(&isl, ox, oy, 0.0);
        }
        let to = |x: f32, z: f32| {
            (
                ox as f32 + (x + MAP_HALF) / (2.0 * MAP_HALF) * side as f32,
                oy as f32 + (z + MAP_HALF) / (2.0 * MAP_HALF) * side as f32,
            )
        };
        let k = side as f32 / (2.0 * MAP_HALF);
        let f = self.frame.as_ref().expect("a frame");
        let violet = Rgba::rgb(190, 110, 255);
        if f.storm.1 < MAP_HALF * 1.2 {
            let (x, y) = to(f.storm.0[0], f.storm.0[1]);
            self.buf.ring(x, y, f.storm.1 * k, 2.0 * uf, violet);
            self.buf
                .ring(x, y, f.storm.1 * k + 3.0 * uf, 3.0 * uf, violet.fade(0.25));
        }
        if f.next.1 < MAP_HALF * 1.2 {
            let (x, y) = to(f.next.0[0], f.next.0[1]);
            self.buf
                .ring(x, y, f.next.1 * k, uf, Rgba(244, 241, 255, 150));
        }
        for bo in &f.bolts {
            let (x, y) = to(bo.p[0], bo.p[2]);
            let (x0, y0) = to(bo.p[0] - bo.v[0] * 0.08, bo.p[2] - bo.v[2] * 0.08);
            let col = spell(bo.kind);
            self.buf.line(x0, y0, x, y, 1.2 * uf, col);
            self.buf.glow(x, y, 3.0 * uf, col.fade(0.7));
        }
        for &(when, from, to_, sp) in &self.beams {
            let a = (1.0 - (now - when) / 400.0) as f32;
            let (x0, y0) = to(from[0], from[2]);
            let (x1, y1) = to(to_[0], to_[2]);
            self.buf.line(x0, y0, x1, y1, 1.5 * uf, spell(sp).fade(a));
        }
        for s in f.players.iter().filter(|s| s.flags & flag::ALIVE != 0) {
            let (x, y) = to(s.p[0], s.p[2]);
            let (r, g, bl) = HUES[s.id as usize % HUES.len()];
            self.buf.circle(x, y, 2.6 * uf, Rgba(7, 10, 18, 200));
            self.buf.circle(
                x,
                y,
                2.0 * uf,
                Rgba::rgb(r, g, bl).mix(Rgba::rgb(255, 255, 255), 0.25),
            );
        }
        for &(when, at) in &self.falls {
            let a = (1.0 - (now - when) / 900.0) as f32;
            let (x, y) = to(at[0], at[2]);
            self.buf.ring(
                x,
                y,
                (3.0 + 10.0 * (1.0 - a)) * uf,
                uf,
                Rgba(255, 214, 128, 255).fade(a),
            );
        }
        // LIVE, with a beating dot, and how many are left.
        let p = 4.0 * uf;
        let beat = 0.55 + 0.45 * ((now / 400.0) as f32).sin().abs();
        let tag = format!("LIVE  {} left", f.alive);
        let tw = pixels::text_width(&tag, u) as f32;
        self.buf.round_rect(
            Rect::new(p, p, tw + 14.0 * uf, 11.0 * uf),
            3.0 * uf,
            Rgba(7, 10, 18, 170),
        );
        self.buf.circle(
            p + 5.5 * uf,
            p + 5.5 * uf,
            2.5 * uf,
            Rgba(240, 70, 70, 255).fade(beat),
        );
        self.buf.text(
            (p as i32) + 10 * u,
            (p as i32) + 2 * u,
            &tag,
            u,
            Rgba(244, 241, 255, 230),
        );
        c.blit(&self.buf, b.x as i32, b.y as i32, 6.0 * uf);
    }
}

/// The idea of a match, between matches.
fn idea(c: &mut Canvas, b: Rect, t: f32, u: f32) {
    c.round_rect(b, 6.0 * u, Rgba::rgb(14, 30, 52));
    let (cx, cy) = (b.x + b.w / 2.0, b.y + b.h / 2.0);
    let r = b.h.min(b.w) * 0.42;
    c.glow(cx, cy, r * 1.6, Rgba::rgb(40, 90, 130).fade(0.5));
    // The island: sand, then grass, a hill or two.
    c.circle(cx, cy, r + 2.0 * u, Rgba::rgb(190, 172, 120));
    c.circle(cx, cy, r, Rgba::rgb(70, 124, 64));
    c.circle(cx - r * 0.3, cy - r * 0.2, r * 0.35, Rgba::rgb(96, 140, 76));
    c.circle(
        cx + r * 0.35,
        cy + r * 0.25,
        r * 0.25,
        Rgba::rgb(110, 146, 84),
    );
    // The storm: closing over twelve seconds, then again.
    let k = (t / 12.0).fract();
    let sr = r * (1.25 - 0.95 * k);
    c.ring(cx, cy, sr, 2.0 * u, Rgba::rgb(190, 110, 255));
    c.ring(cx, cy, sr + 3.0 * u, 3.0 * u, Rgba(190, 110, 255, 60));
    // Wizards wandering inside the storm.
    let n = 6;
    let at = |i: u32| {
        let a = hash(i) * std::f32::consts::TAU + t * (0.2 + 0.2 * hash(i + 9));
        let d = sr.min(r) * (0.25 + 0.6 * hash(i + 3));
        (cx + a.cos() * d, cy + a.sin() * d * 0.8)
    };
    for i in 0..n {
        let (x, y) = at(i);
        c.circle(x, y, 2.2 * u, Rgba::rgb(250, 246, 236));
    }
    // Bolts: one duel at a time, a spell's colour each.
    let beat = (t / 0.9) as u32;
    let f = (t / 0.9).fract();
    let (a, z) = (beat % n, (beat * 7 + 3) % n);
    if a != z {
        let ((x0, y0), (x1, y1)) = (at(a), at(z));
        let (r0, g0, b0) = SPELLS[(beat % 8) as usize];
        let col = Rgba::rgb(r0, g0, b0);
        let p = (f * 1.6).min(1.0);
        let (x, y) = (x0 + (x1 - x0) * p, y0 + (y1 - y0) * p);
        let tail = (p - 0.25).max(0.0);
        let (tx, ty) = (x0 + (x1 - x0) * tail, y0 + (y1 - y0) * tail);
        if p < 1.0 {
            c.line(tx, ty, x, y, 1.5 * u, col);
            c.glow(x, y, 6.0 * u, col.fade(0.8));
        } else {
            c.glow(x1, y1, (4.0 + 10.0 * (f - 0.625)) * u, col.fade(1.0 - f));
        }
    }
}
