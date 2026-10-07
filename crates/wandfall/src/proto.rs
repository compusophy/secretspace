//! The wire. Up (page → server): Join, and Inputs (a few at a time).
//! Down: Welcome, a Frame each tick, the Roster when names change, and
//! Events. Positions travel at 1/32 m (±1 km); your own body travels
//! exactly (its f32 bits), so prediction can match it to the bit.
//! Decoding never trusts the bytes (`hostile_bytes_never_panic`).

use engine::wire::{Reader, Writer};

use crate::motion::{Body, Input};

/// This protocol; older pages are told to reload.
pub const PROTO: u8 = 1;

pub mod tag {
    pub const JOIN: u8 = 1;
    pub const INPUT: u8 = 2;
    pub const WELCOME: u8 = 1;
    pub const FRAME: u8 = 2;
    pub const ROSTER: u8 = 3;
    pub const EVENTS: u8 = 4;
}

/// At most this many inputs in one message.
pub const MAX_INPUTS: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub enum Up {
    Join { proto: u8 },
    Inputs(Vec<Input>),
}

impl Up {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        match self {
            Up::Join { proto } => {
                w.u8(tag::JOIN).u8(*proto);
            }
            Up::Inputs(v) => {
                w.u8(tag::INPUT).u8(v.len().min(MAX_INPUTS) as u8);
                for i in v.iter().take(MAX_INPUTS) {
                    w.u16(i.seq).u16(i.yaw).i16(i.pitch).u8(i.keys);
                }
            }
        }
        w.0
    }

    pub fn decode(b: &[u8]) -> Option<Up> {
        let mut r = Reader::new(b);
        match r.u8()? {
            tag::JOIN => Some(Up::Join { proto: r.u8()? }),
            tag::INPUT => {
                let n = r.u8()? as usize;
                if n == 0 || n > MAX_INPUTS {
                    return None;
                }
                let mut v = Vec::with_capacity(n);
                for _ in 0..n {
                    v.push(Input {
                        seq: r.u16()?,
                        yaw: r.u16()?,
                        pitch: r.i16()?,
                        keys: r.u8()?,
                    });
                }
                Some(Up::Inputs(v))
            }
            _ => None,
        }
    }
}

/// Metres to the wire (1/32 m) and back.
pub fn q(v: f32) -> i16 {
    (v * 32.0).round().clamp(-32767.0, 32767.0) as i16
}

pub fn dq(v: i16) -> f32 {
    v as f32 / 32.0
}

/// Your own wizard, exactly.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Own {
    pub body: Body,
    /// The last input the server applied.
    pub seq: u16,
    pub hp: u8,
    pub kills: u8,
    /// Ticks until the wand is ready.
    pub cool: u8,
}

/// Another wizard (or you, as everyone sees you).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Seen {
    pub id: u16,
    pub p: [f32; 3],
    pub yaw: u16,
    pub pitch: i16,
    pub hp: u8,
    pub flags: u8,
}

pub mod flag {
    pub const ALIVE: u8 = 1;
    pub const GLIDE: u8 = 2;
    pub const GROUND: u8 = 4;
    pub const BOT: u8 = 8;
    pub const ENTRANT: u8 = 16;
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoltSeen {
    pub id: u16,
    pub by: u16,
    pub p: [f32; 3],
    pub v: [f32; 3],
}

/// A circle: centre and radius.
pub type Circle = ([f32; 2], f32);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub tick: u32,
    /// 0 lobby, 1 fight, 2 over.
    pub phase: u8,
    /// Seconds until the next thing happens (the fight, the storm).
    pub secs: u16,
    pub alive: u8,
    pub entrants: u8,
    pub storm: Circle,
    pub next: Circle,
    pub shrinking: bool,
    pub storm_phase: u8,
    pub winner: u16,
    pub you: Option<Own>,
    pub players: Vec<Seen>,
    pub bolts: Vec<BoltSeen>,
}

fn circle(w: &mut Writer, c: &Circle) {
    w.i16(q(c.0[0]))
        .i16(q(c.0[1]))
        .u16((c.1 * 8.0).round().clamp(0.0, 65535.0) as u16);
}

fn read_circle(r: &mut Reader) -> Option<Circle> {
    Some(([dq(r.i16()?), dq(r.i16()?)], r.u16()? as f32 / 8.0))
}

fn f32s(w: &mut Writer, v: [f32; 3]) {
    for x in v {
        w.u32(x.to_bits());
    }
}

fn read_f32s(r: &mut Reader) -> Option<[f32; 3]> {
    Some([
        f32::from_bits(r.u32()?),
        f32::from_bits(r.u32()?),
        f32::from_bits(r.u32()?),
    ])
}

impl Frame {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.u8(tag::FRAME)
            .u32(self.tick)
            .u8(self.phase)
            .u16(self.secs)
            .u8(self.alive)
            .u8(self.entrants);
        circle(&mut w, &self.storm);
        circle(&mut w, &self.next);
        w.u8(self.shrinking as u8)
            .u8(self.storm_phase)
            .u16(self.winner);
        match &self.you {
            None => {
                w.u8(0);
            }
            Some(o) => {
                w.u8(1);
                f32s(&mut w, o.body.p);
                f32s(&mut w, o.body.v);
                w.u8(o.body.ground as u8 | (o.body.glide as u8) << 1)
                    .u16(o.seq)
                    .u8(o.hp)
                    .u8(o.kills)
                    .u8(o.cool);
            }
        }
        let n = self.players.len().min(255);
        w.u8(n as u8);
        for s in &self.players[..n] {
            w.u16(s.id)
                .i16(q(s.p[0]))
                .i16(q(s.p[1]))
                .i16(q(s.p[2]))
                .u16(s.yaw)
                .i16(s.pitch)
                .u8(s.hp)
                .u8(s.flags);
        }
        let n = self.bolts.len().min(255);
        w.u8(n as u8);
        for b in &self.bolts[..n] {
            w.u16(b.id).u16(b.by);
            for x in b.p {
                w.i16(q(x));
            }
            for x in b.v {
                w.i16((x * 8.0).round().clamp(-32767.0, 32767.0) as i16);
            }
        }
        w.0
    }

    pub fn decode(b: &[u8]) -> Option<Frame> {
        let mut r = Reader::new(b);
        if r.u8()? != tag::FRAME {
            return None;
        }
        let mut f = Frame {
            tick: r.u32()?,
            phase: r.u8()?,
            secs: r.u16()?,
            alive: r.u8()?,
            entrants: r.u8()?,
            storm: read_circle(&mut r)?,
            next: read_circle(&mut r)?,
            shrinking: r.u8()? != 0,
            storm_phase: r.u8()?,
            winner: r.u16()?,
            ..Frame::default()
        };
        if r.u8()? == 1 {
            let p = read_f32s(&mut r)?;
            let v = read_f32s(&mut r)?;
            let g = r.u8()?;
            f.you = Some(Own {
                body: Body {
                    p,
                    v,
                    ground: g & 1 != 0,
                    glide: g & 2 != 0,
                },
                seq: r.u16()?,
                hp: r.u8()?,
                kills: r.u8()?,
                cool: r.u8()?,
            });
        }
        let n = r.u8()? as usize;
        r.room(n, 14)?;
        for _ in 0..n {
            f.players.push(Seen {
                id: r.u16()?,
                p: [dq(r.i16()?), dq(r.i16()?), dq(r.i16()?)],
                yaw: r.u16()?,
                pitch: r.i16()?,
                hp: r.u8()?,
                flags: r.u8()?,
            });
        }
        let n = r.u8()? as usize;
        r.room(n, 16)?;
        for _ in 0..n {
            let id = r.u16()?;
            let by = r.u16()?;
            let p = [dq(r.i16()?), dq(r.i16()?), dq(r.i16()?)];
            let mut v = [0.0; 3];
            for x in v.iter_mut() {
                *x = r.i16()? as f32 / 8.0;
            }
            f.bolts.push(BoltSeen { id, by, p, v });
        }
        Some(f)
    }
}

pub fn welcome(you: u16, seed: u64, hz: u8) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(tag::WELCOME).u8(PROTO).u16(you).u64(seed).u8(hz);
    w.0
}

/// (protocol, you, seed, ticks a second).
pub fn read_welcome(b: &[u8]) -> Option<(u8, u16, u64, u8)> {
    let mut r = Reader::new(b);
    if r.u8()? != tag::WELCOME {
        return None;
    }
    Some((r.u8()?, r.u16()?, r.u64()?, r.u8()?))
}

/// Everyone's id, whether a bot, and name.
pub fn roster(list: &[(u16, bool, String)]) -> Vec<u8> {
    let mut w = Writer::default();
    let n = list.len().min(255);
    w.u8(tag::ROSTER).u8(n as u8);
    for (id, bot, name) in &list[..n] {
        w.u16(*id).u8(*bot as u8).str(name);
    }
    w.0
}

pub fn read_roster(b: &[u8]) -> Option<Vec<(u16, bool, String)>> {
    let mut r = Reader::new(b);
    if r.u8()? != tag::ROSTER {
        return None;
    }
    let n = r.u8()? as usize;
    r.room(n, 4)?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push((r.u16()?, r.u8()? != 0, r.str()?));
    }
    Some(v)
}

/// Events as the wire carries them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ev {
    Hit { by: u16, to: u16, amount: u16 },
    Out { who: u16, by: u16, place: u16 },
    Win { who: u16 },
    Begin,
    Lobby,
}

pub fn events(list: &[Ev]) -> Vec<u8> {
    let mut w = Writer::default();
    let n = list.len().min(255);
    w.u8(tag::EVENTS).u8(n as u8);
    for e in &list[..n] {
        match *e {
            Ev::Hit { by, to, amount } => w.u8(1).u16(by).u16(to).u16(amount),
            Ev::Out { who, by, place } => w.u8(2).u16(who).u16(by).u16(place),
            Ev::Win { who } => w.u8(3).u16(who),
            Ev::Begin => w.u8(4),
            Ev::Lobby => w.u8(5),
        };
    }
    w.0
}

pub fn read_events(b: &[u8]) -> Option<Vec<Ev>> {
    let mut r = Reader::new(b);
    if r.u8()? != tag::EVENTS {
        return None;
    }
    let n = r.u8()? as usize;
    r.room(n, 1)?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push(match r.u8()? {
            1 => Ev::Hit {
                by: r.u16()?,
                to: r.u16()?,
                amount: r.u16()?,
            },
            2 => Ev::Out {
                who: r.u16()?,
                by: r.u16()?,
                place: r.u16()?,
            },
            3 => Ev::Win { who: r.u16()? },
            4 => Ev::Begin,
            5 => Ev::Lobby,
            _ => return None,
        });
    }
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_bytes_never_panic() {
        let mut rng = engine::rng::Rng::new(1);
        for len in 0..200 {
            for _ in 0..200 {
                let b: Vec<u8> = (0..len).map(|_| rng.next_u64() as u8).collect();
                let _ = Up::decode(&b);
                let _ = Frame::decode(&b);
                let _ = read_roster(&b);
                let _ = read_events(&b);
                let _ = read_welcome(&b);
            }
        }
    }

    #[test]
    fn a_frame_round_trips_and_your_body_exactly() {
        let f = Frame {
            tick: 77,
            phase: 1,
            secs: 30,
            alive: 9,
            entrants: 16,
            storm: ([1.5, -2.0], 200.0),
            next: ([3.0, 4.0], 110.0),
            shrinking: true,
            storm_phase: 1,
            winner: 0,
            you: Some(Own {
                body: Body {
                    p: [1.234_567_9, 2.0, -3.333_333],
                    v: [0.1, -9.8, 0.0],
                    ground: false,
                    glide: true,
                },
                seq: 65535,
                hp: 88,
                kills: 2,
                cool: 3,
            }),
            players: vec![Seen {
                id: 4,
                p: [10.0, 2.5, -7.25],
                yaw: 1000,
                pitch: -200,
                hp: 50,
                flags: flag::ALIVE | flag::BOT,
            }],
            bolts: vec![BoltSeen {
                id: 9,
                by: 4,
                p: [1.0, 2.0, 3.0],
                v: [75.0, 0.5, -1.0],
            }],
        };
        assert_eq!(Frame::decode(&f.encode()), Some(f));
        let up = Up::Inputs(vec![Input {
            seq: 5,
            yaw: 9,
            pitch: -3,
            keys: 33,
        }]);
        assert_eq!(Up::decode(&up.encode()), Some(up));
        let ev = vec![
            Ev::Hit {
                by: 1,
                to: 2,
                amount: 12,
            },
            Ev::Win { who: 1 },
            Ev::Begin,
        ];
        assert_eq!(read_events(&events(&ev)), Some(ev));
    }
}
