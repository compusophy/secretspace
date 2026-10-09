//! The wire. Up (page → server): Join, and Inputs (a few at a time).
//! Down: Welcome, a Frame each tick, the Roster when names change, and
//! Events. Positions travel at 1/32 m (±1 km); your own body travels
//! exactly (its f32 bits), so prediction can match it to the bit.
//! Decoding never trusts the bytes (`hostile_bytes_never_panic`).

use engine::wire::{Reader, Writer};

use crate::laws::{MAX_RANK, SPELLS};
use crate::motion::{Body, Input};

/// This protocol; older pages are told to reload.
pub const PROTO: u8 = 10;

pub mod tag {
    pub const JOIN: u8 = 1;
    pub const INPUT: u8 = 2;
    pub const EQUIP: u8 = 3;
    pub const WELCOME: u8 = 1;
    pub const FRAME: u8 = 2;
    pub const ROSTER: u8 = 3;
    pub const EVENTS: u8 = 4;
    pub const LOOT: u8 = 5;
}

/// At most this many inputs in one message.
pub const MAX_INPUTS: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub enum Up {
    Join {
        proto: u8,
    },
    Inputs(Vec<Input>),
    /// The spellbook: put a spell you know in a slot.
    Equip {
        slot: u8,
        spell: u8,
    },
}

impl Up {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        match self {
            Up::Join { proto } => {
                w.u8(tag::JOIN).u8(*proto);
            }
            Up::Equip { slot, spell } => {
                w.u8(tag::EQUIP).u8(*slot).u8(*spell);
            }
            Up::Inputs(v) => {
                w.u8(tag::INPUT).u8(v.len().min(MAX_INPUTS) as u8);
                for i in v.iter().take(MAX_INPUTS) {
                    w.u16(i.seq).u16(i.yaw).i16(i.pitch).u16(i.keys).u8(i.cast);
                }
            }
        }
        w.0
    }

    pub fn decode(b: &[u8]) -> Option<Up> {
        let mut r = Reader::new(b);
        match r.u8()? {
            tag::JOIN => Some(Up::Join { proto: r.u8()? }),
            tag::EQUIP => Some(Up::Equip {
                slot: r.u8()?,
                spell: r.u8()?,
            }),
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
                        keys: r.u16()?,
                        cast: r.u8()?,
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
    pub hp: u16,
    pub kills: u8,
    /// Ticks until the wand is ready.
    pub cool: u8,
    pub level: u8,
    /// XP toward the next level (of XP_PER_LEVEL).
    pub xp: u8,
    pub shield: u16,
    /// Each slot: (spell, rank), or None; ticks until it is ready.
    pub slots: [Option<(u8, u8)>; 4],
    pub cds: [u16; 4],
    /// The spellbook: each spell's rank, 0 for one not known.
    pub book: [u8; SPELLS.len()],
}

/// Another wizard (or you, as everyone sees you).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Seen {
    pub id: u16,
    pub p: [f32; 3],
    pub yaw: u16,
    pub pitch: i16,
    pub hp: u16,
    pub flags: u8,
    pub level: u8,
    /// What spells are on it (`fx`).
    pub fx: u8,
}

pub mod fx {
    pub const SHIELD: u8 = 1;
    pub const CHILLED: u8 = 2;
    pub const MENDING: u8 = 4;
    pub const AIM: u8 = 8;
}

pub mod flag {
    pub const ALIVE: u8 = 1;
    pub const GLIDE: u8 = 2;
    pub const GROUND: u8 = 4;
    pub const BOT: u8 = 8;
    pub const ENTRANT: u8 = 16;
    pub const CROUCH: u8 = 32;
    pub const SPRINT: u8 = 64;
    pub const SLIDE: u8 = 128;
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoltSeen {
    pub id: u16,
    pub by: u16,
    /// `world::WAND`, or a spell.
    pub kind: u8,
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
                let b = &o.body;
                w.u8(b.ground as u8
                    | (b.glide as u8) << 1
                    | (b.crouch as u8) << 2
                    | (b.sprint as u8) << 3
                    | (b.slide as u8) << 4
                    | (b.winded as u8) << 5
                    | (b.held as u8) << 6
                    | (b.air_jumped as u8) << 7)
                    .u8(b.coyote.min(15) | b.buffer.min(15) << 4)
                    .u8(b.slide_cd)
                    .u16(b.spent)
                    .u8(b.breath)
                    .u8(b.landed)
                    .u8(b.mantle)
                    .u16(o.body.chill)
                    .u16(o.seq)
                    .u16(o.hp)
                    .u8(o.kills)
                    .u8(o.cool)
                    .u8(o.level)
                    .u8(o.xp)
                    .u16(o.shield);
                for (s, cd) in o.slots.iter().zip(o.cds) {
                    let (spell, rank) = s.unwrap_or((255, 0));
                    w.u8(spell).u8(rank).u16(cd);
                }
                // Two ranks a byte.
                for pair in o.book.chunks(2) {
                    w.u8(pair[0] & 15 | (pair.get(1).copied().unwrap_or(0) & 15) << 4);
                }
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
                .u16(s.hp)
                .u8(s.flags)
                .u8(s.level)
                .u8(s.fx);
        }
        let n = self.bolts.len().min(255);
        w.u8(n as u8);
        for b in &self.bolts[..n] {
            w.u16(b.id).u16(b.by).u8(b.kind);
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
            let jump = r.u8()?;
            let slide_cd = r.u8()?;
            let (spent, breath, landed, mantle) = (r.u16()?, r.u8()?, r.u8()?, r.u8()?);
            let mut o = Own {
                body: Body {
                    p,
                    v,
                    ground: g & 1 != 0,
                    glide: g & 2 != 0,
                    crouch: g & 4 != 0,
                    sprint: g & 8 != 0,
                    slide: g & 16 != 0,
                    winded: g & 32 != 0,
                    held: g & 64 != 0,
                    air_jumped: g & 128 != 0,
                    landed,
                    mantle,
                    slide_cd,
                    spent,
                    breath,
                    coyote: jump & 15,
                    buffer: jump >> 4,
                    chill: r.u16()?,
                },
                seq: r.u16()?,
                hp: r.u16()?,
                kills: r.u8()?,
                cool: r.u8()?,
                level: r.u8()?,
                xp: r.u8()?,
                shield: r.u16()?,
                ..Own::default()
            };
            for k in 0..4 {
                let (spell, rank) = (r.u8()?, r.u8()?);
                o.slots[k] = (spell != 255).then_some((spell, rank));
                o.cds[k] = r.u16()?;
            }
            for k in (0..o.book.len()).step_by(2) {
                let b = r.u8()?;
                o.book[k] = (b & 15).min(MAX_RANK);
                if let Some(next) = o.book.get_mut(k + 1) {
                    *next = (b >> 4).min(MAX_RANK);
                }
            }
            f.you = Some(o);
        }
        let n = r.u8()? as usize;
        r.room(n, 17)?;
        for _ in 0..n {
            f.players.push(Seen {
                id: r.u16()?,
                p: [dq(r.i16()?), dq(r.i16()?), dq(r.i16()?)],
                yaw: r.u16()?,
                pitch: r.i16()?,
                hp: r.u16()?,
                flags: r.u8()?,
                level: r.u8()?,
                fx: r.u8()?,
            });
        }
        let n = r.u8()? as usize;
        r.room(n, 17)?;
        for _ in 0..n {
            let id = r.u16()?;
            let by = r.u16()?;
            let kind = r.u8()?;
            let p = [dq(r.i16()?), dq(r.i16()?), dq(r.i16()?)];
            let mut v = [0.0; 3];
            for x in v.iter_mut() {
                *x = r.i16()? as f32 / 8.0;
            }
            f.bolts.push(BoltSeen { id, by, kind, p, v });
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
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ev {
    /// `what`: `world::WAND`, a spell, or `world::STORM`.
    Hit {
        by: u16,
        to: u16,
        amount: u16,
        what: u8,
    },
    Out {
        who: u16,
        by: u16,
        place: u16,
    },
    Win {
        who: u16,
    },
    Begin,
    Lobby,
    Cast {
        by: u16,
        spell: u8,
        stage: u8,
        at: [f32; 3],
    },
    Beam {
        by: u16,
        spell: u8,
        from: [f32; 3],
        to: [f32; 3],
    },
    Level {
        who: u16,
        level: u8,
    },
}

pub fn events(list: &[Ev]) -> Vec<u8> {
    let mut w = Writer::default();
    let n = list.len().min(255);
    w.u8(tag::EVENTS).u8(n as u8);
    for e in &list[..n] {
        match *e {
            Ev::Hit {
                by,
                to,
                amount,
                what,
            } => w.u8(1).u16(by).u16(to).u16(amount).u8(what),
            Ev::Out { who, by, place } => w.u8(2).u16(who).u16(by).u16(place),
            Ev::Win { who } => w.u8(3).u16(who),
            Ev::Begin => w.u8(4),
            Ev::Lobby => w.u8(5),
            Ev::Cast {
                by,
                spell,
                stage,
                at,
            } => w
                .u8(6)
                .u16(by)
                .u8(spell)
                .u8(stage)
                .i16(q(at[0]))
                .i16(q(at[1]))
                .i16(q(at[2])),
            Ev::Beam {
                by,
                spell,
                from,
                to,
            } => {
                w.u8(7).u16(by).u8(spell);
                for x in from.into_iter().chain(to) {
                    w.i16(q(x));
                }
                &mut w
            }
            Ev::Level { who, level } => w.u8(8).u16(who).u8(level),
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
                what: r.u8()?,
            },
            2 => Ev::Out {
                who: r.u16()?,
                by: r.u16()?,
                place: r.u16()?,
            },
            3 => Ev::Win { who: r.u16()? },
            4 => Ev::Begin,
            5 => Ev::Lobby,
            6 => Ev::Cast {
                by: r.u16()?,
                spell: r.u8()?,
                stage: r.u8()?,
                at: [dq(r.i16()?), dq(r.i16()?), dq(r.i16()?)],
            },
            7 => Ev::Beam {
                by: r.u16()?,
                spell: r.u8()?,
                from: [dq(r.i16()?), dq(r.i16()?), dq(r.i16()?)],
                to: [dq(r.i16()?), dq(r.i16()?), dq(r.i16()?)],
            },
            8 => Ev::Level {
                who: r.u16()?,
                level: r.u8()?,
            },
            _ => return None,
        });
    }
    Some(v)
}

/// The spell cubes (id, spell, rank, where) on the island, sent when
/// they change.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Loot {
    pub scrolls: Vec<(u16, u8, u8, [f32; 3])>,
}

impl Loot {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        let n = self.scrolls.len().min(u16::MAX as usize);
        w.u8(tag::LOOT).u16(n as u16);
        for (id, spell, rank, p) in &self.scrolls[..n] {
            w.u16(*id)
                .u8(*spell)
                .u8(*rank)
                .i16(q(p[0]))
                .i16(q(p[1]))
                .i16(q(p[2]));
        }
        w.0
    }

    pub fn decode(b: &[u8]) -> Option<Loot> {
        let mut r = Reader::new(b);
        if r.u8()? != tag::LOOT {
            return None;
        }
        let mut l = Loot::default();
        let n = r.u16()? as usize;
        r.room(n, 10)?;
        for _ in 0..n {
            l.scrolls.push((
                r.u16()?,
                r.u8()?,
                r.u8()?,
                [dq(r.i16()?), dq(r.i16()?), dq(r.i16()?)],
            ));
        }
        Some(l)
    }
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
                let _ = Loot::decode(&b);
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
                    chill: 7,
                    crouch: true,
                    coyote: 3,
                    buffer: 2,
                    sprint: false,
                    slide: true,
                    slide_cd: 17,
                    spent: 1234,
                    breath: 9,
                    winded: true,
                    held: true,
                    landed: 2,
                    air_jumped: true,
                    mantle: 5,
                },
                seq: 65535,
                hp: 252,
                kills: 2,
                cool: 3,
                level: 20,
                xp: 99,
                shield: 40,
                slots: [Some((0, 3)), None, Some((7, 1)), None],
                cds: [0, 300, 12, 0],
                book: [3, 0, 1, 2, 0, 0, 0, 1],
            }),
            players: vec![Seen {
                id: 4,
                p: [10.0, 2.5, -7.25],
                yaw: 1000,
                pitch: -200,
                hp: 50,
                flags: flag::ALIVE | flag::BOT,
                level: 3,
                fx: fx::SHIELD,
            }],
            bolts: vec![BoltSeen {
                id: 9,
                by: 4,
                kind: 1,
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
            cast: 17,
        }]);
        assert_eq!(Up::decode(&up.encode()), Some(up));
        let ev = vec![
            Ev::Hit {
                by: 1,
                to: 2,
                amount: 12,
                what: 3,
            },
            Ev::Beam {
                by: 1,
                spell: 1,
                from: [1.0, 2.0, 3.0],
                to: [-4.0, 5.5, 60.25],
            },
            Ev::Win { who: 1 },
            Ev::Begin,
        ];
        assert_eq!(read_events(&events(&ev)), Some(ev));
    }
}
