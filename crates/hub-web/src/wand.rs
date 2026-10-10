//! Wandfall's card: the room itself, live. The hub watches it (as a
//! watcher: never one of the people there) and shows the match as it is
//! played: drawn by the engine in 3D over a fighter's shoulder (off
//! screen, on WebGPU, and read back into the card), or, where there is no
//! WebGPU, from above as the game's map draws it. Either way, how it
//! stands: how many of how many are left in the fight, the lobby's
//! countdown, or who won. Matches always run (bots fight while no one is
//! there), so there is always one to show.

use pixels::{Canvas, Rect, Rgba};
use wandfall::laws::{MAP_HALF, SEA};
use wandfall::map::Map;
use wandfall::places::Place;
use wandfall::proto::{self, flag, Ev, Frame};
use wandfall_look::spectate::Spectator;

/// The match in 3D: the engine drawing off screen, the spectator it
/// draws, and the last picture read back.
struct Three {
    off: gpu::Offscreen,
    r: render::Renderer,
    spec: Spectator,
    pic: Canvas,
    has: bool,
}

/// A colour of the game's (0 to 1) as the card's.
fn rgba(c: [f32; 3]) -> Rgba {
    let k = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Rgba::rgb(k(c[0]), k(c[1]), k(c[2]))
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
    /// Everyone's names, and who won the last match.
    names: Vec<(u16, bool, String)>,
    winner: u16,
    /// The hall of wizards' first, if anyone has won.
    champion: Option<String>,
    buf: Canvas,
    three: Option<Three>,
    /// The room's last roster and loot, for a spectator that starts late.
    said: Vec<Vec<u8>>,
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
            names: Vec::new(),
            winner: 0,
            champion: None,
            buf: Canvas::new(1, 1),
            three: None,
            said: Vec::new(),
        }
    }

    /// A device to draw the match in 3D with (WebGPU, off screen).
    pub fn give(&mut self, off: gpu::Offscreen) {
        let q = render::Quality::pick("q=medium", off.caps.software, false);
        let r = render::Renderer::new(&off.device, &off.queue, gpu::Offscreen::FORMAT, q);
        let mut spec = Spectator::new();
        spec.st.seed = self.seed;
        for b in &self.said {
            spec.message(b, kit::now());
        }
        self.three = Some(Three {
            off,
            r,
            spec,
            pic: Canvas::new(1, 1),
            has: false,
        });
    }

    /// Stop watching (a game is open over the page); `poll` opens again.
    pub fn rest(&mut self) {
        if let Some(link) = self.link.take() {
            link.close();
        }
    }

    fn poll(&mut self, now: f64) {
        let link = self
            .link
            .get_or_insert_with(|| kit::Link::open("wandfall", Vec::new(), true));
        for ev in link.poll(now) {
            let kit::Net::Message(b) = ev else { continue };
            if let Some(t) = self.three.as_mut() {
                t.spec.message(&b, now);
            }
            if proto::read_roster(&b).is_some() || proto::Loot::decode(&b).is_some() {
                let roster = proto::read_roster(&b).is_some();
                self.said
                    .retain(|o| proto::read_roster(o).is_some() != roster);
                self.said.push(b.clone());
            }
            if let Some(h) = proto::read_hall(&b) {
                self.champion = h.into_iter().find(|r| r.1 > 0).map(|r| r.0);
            }
            if let Some((_, _, seed, _)) = proto::read_welcome(&b) {
                if self.seed != Some(seed) {
                    self.seed = Some(seed);
                    self.map = Some(Map::new(seed));
                    self.island = None;
                }
            } else if let Some(f) = Frame::decode(&b) {
                self.frame = Some(f);
                self.frame_at = now;
            } else if let Some(list) = proto::read_roster(&b) {
                self.names = list;
            } else if let Some(list) = proto::read_events(&b) {
                for e in list {
                    match e {
                        Ev::Beam {
                            from, to, spell, ..
                        } => self.beams.push((now, from, to, spell)),
                        Ev::Win { who } => self.winner = who,
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

    /// The island from above, `s` pixels a side, drawn into `island`
    /// once for the size.
    fn island(&mut self, s: i32) {
        let Some(map) = self.map.as_ref() else {
            return;
        };
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
            // The places: the Spire, the circle, the rift, the grove, the
            // causeway.
            let k = s as f32 / 96.0;
            let px = |v: f32| (v + MAP_HALF) / (2.0 * MAP_HALF) * s as f32;
            for p in &map.pois {
                let (x, y) = (px(p.x), px(p.z));
                match p.place {
                    Place::Spire => {
                        c.circle(x, y, 4.0 * k, Rgba::rgb(196, 188, 176));
                        c.circle(x, y, 1.8 * k, Rgba::rgb(120, 80, 220));
                    }
                    Place::Circle => c.ring(x, y, 3.0 * k, 1.2 * k, Rgba::rgb(120, 225, 255)),
                    Place::Rift => {
                        c.circle(x, y, 3.6 * k, Rgba::rgb(40, 22, 22));
                        c.circle(x, y, 1.6 * k, Rgba::rgb(255, 110, 40));
                    }
                    Place::Grove => c.circle(x, y, 2.6 * k, Rgba::rgb(170, 120, 255)),
                    Place::Causeway => {
                        c.circle(x, y, 3.2 * k, Rgba::rgb(64, 64, 72));
                        c.circle(x, y, 1.2 * k, Rgba::rgb(150, 255, 214));
                    }
                }
            }
            self.island = Some(c);
        }
    }

    /// The card's picture in `b`: the room, live.
    pub fn draw(&mut self, c: &mut Canvas, b: Rect, u: i32, now: f64) {
        self.poll(now);
        let uf = u as f32;
        if self.three(b, now) {
            self.tags(u, now);
            c.blit(&self.buf, b.x as i32, b.y as i32, 6.0 * uf);
            return;
        }
        self.buf.resize(b.w as i32, b.h as i32);
        self.buf.clear(Rgba::rgb(22, 50, 84));
        let side = (b.h.min(b.w) * 1.15) as i32;
        let (ox, oy) = ((b.w as i32 - side) / 2, (b.h as i32 - side) / 2);
        self.island(side);
        if let Some(isl) = &self.island {
            self.buf.blit(isl, ox, oy, 0.0);
        }
        let to = |x: f32, z: f32| {
            (
                ox as f32 + (x + MAP_HALF) / (2.0 * MAP_HALF) * side as f32,
                oy as f32 + (z + MAP_HALF) / (2.0 * MAP_HALF) * side as f32,
            )
        };
        let k = side as f32 / (2.0 * MAP_HALF);
        let Some(f) = self.frame.as_ref() else {
            self.tags(u, now);
            c.blit(&self.buf, b.x as i32, b.y as i32, 6.0 * uf);
            return;
        };
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
            let col = rgba(wandfall_look::fx::colour(bo.kind));
            self.buf.line(x0, y0, x, y, 1.2 * uf, col);
            self.buf.glow(x, y, 3.0 * uf, col.fade(0.7));
        }
        for &(when, from, to_, sp) in &self.beams {
            let a = (1.0 - (now - when) / 400.0) as f32;
            let (x0, y0) = to(from[0], from[2]);
            let (x1, y1) = to(to_[0], to_[2]);
            let col = rgba(wandfall_look::fx::colour(sp));
            self.buf.line(x0, y0, x1, y1, 1.5 * uf, col.fade(a));
        }
        for s in f.players.iter().filter(|s| s.flags & flag::ALIVE != 0) {
            let (x, y) = to(s.p[0], s.p[2]);
            let hue = rgba(wandfall_look::rig::hue(s.id));
            self.buf.circle(x, y, 2.6 * uf, Rgba(7, 10, 18, 200));
            self.buf
                .circle(x, y, 2.0 * uf, hue.mix(Rgba::rgb(255, 255, 255), 0.25));
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
        self.tags(u, now);
        c.blit(&self.buf, b.x as i32, b.y as i32, 6.0 * uf);
    }

    /// The match drawn in 3D into the card's buffer, if there is a device
    /// and a match, and a picture has come back.
    fn three(&mut self, b: Rect, now: f64) -> bool {
        let Some(t) = self.three.as_mut() else {
            return false;
        };
        if !t.spec.live(now) {
            return false;
        }
        let size = (b.w.max(1.0) as u32, b.h.max(1.0) as u32);
        if let Some((view, mut enc)) = t.off.begin(size) {
            t.spec.draw(&mut t.r, &mut enc, &view, size, now);
            t.off.end(enc);
        }
        if t.off.read(&mut t.pic) {
            // Opaque, whatever the picture's alpha says.
            for a in t.pic.data.iter_mut().skip(3).step_by(4) {
                *a = 255;
            }
            t.has = true;
        }
        if !t.has {
            return false;
        }
        self.buf.resize(b.w as i32, b.h as i32);
        self.buf.clear(Rgba::rgb(14, 12, 28));
        self.buf.blit(&t.pic, 0, 0, 0.0);
        true
    }

    /// How it stands, a tag a line: how many of how many are left, the
    /// lobby's countdown (and the champion, under it), or who won; and
    /// whether it is live.
    fn standing(&self, now: f64) -> Vec<(String, bool)> {
        let Some(f) = self.frame.as_ref() else {
            return vec![("connecting to the island".to_string(), false)];
        };
        let line = match f.phase {
            _ if now - self.frame_at > 3000.0 => ("reconnecting".to_string(), false),
            1 => (format!("LIVE  {} of {} left", f.alive, f.entrants), true),
            0 => {
                let next = (format!("next match in {}s", f.secs), false);
                return match &self.champion {
                    Some(c) => vec![next, (format!("champion {c}"), false)],
                    None => vec![next],
                };
            }
            _ => {
                let name = self
                    .names
                    .iter()
                    .find(|n| n.0 == self.winner)
                    .map(|n| n.2.clone());
                (
                    name.map_or("match over".to_string(), |n| format!("{n} won")),
                    false,
                )
            }
        };
        vec![line]
    }

    /// How it stands, in the card's corner.
    fn tags(&mut self, u: i32, now: f64) {
        for (row, (text, live)) in self.standing(now).into_iter().enumerate() {
            crate::tag::tag(&mut self.buf, row as i32, &text, live, u, now);
        }
    }
}
