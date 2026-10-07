//! Luciphon's wire, protocol 1 (§14). Up: Join, Input (30 a second), Heart,
//! Device, Ping. Down: Welcome (with the server's laws), Frame, Chunk and
//! ChunkGone, Board, Pong, Elsewhere. Platform messages (`0xFE`) never
//! reach here. Decoding is bounded: hostile bytes are None, never a panic.

use engine::fixed::Fx;
use engine::wire::{Reader, Writer};

use crate::combat::{Act, Action, Me};
use crate::motion::{Body, Busy, Intent, Move, Verb};
use crate::tiles::{rle, unrle, Tile};

pub const PROTO: u16 = 4;

// Up.
pub const JOIN: u8 = 1;
pub const INPUT: u8 = 2;
pub const HEART: u8 = 3;
pub const DEVICE: u8 = 4;
pub const PING: u8 = 5;
// Down.
pub const WELCOME: u8 = 1;
pub const FRAME: u8 = 2;
pub const CHUNK: u8 = 3;
pub const CHUNK_GONE: u8 = 4;
pub const BOARD: u8 = 6;
pub const PONG: u8 = 7;
pub const ELSEWHERE: u8 = 8;
pub const CLAIMS: u8 = 9;

/// A claim as pages know it: its id, hue, name, and hearth (if it has one).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaimInfo {
    pub id: u16,
    pub hue: u8,
    pub name: String,
    pub hearth: Option<(i16, i16)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Up {
    Join { proto: u16 },
    Input { seq: u16, it: Intent },
    Heart { act: u8, arg: u8 },
    Device { w: u16, h: u16, touch: bool },
    Ping { t: u32, rtt: u8 },
}

fn verb_code(v: Verb) -> (u8, Option<u8>) {
    match v {
        Verb::None => (0, None),
        Verb::Tap => (1, None),
        Verb::Flick => (2, None),
        Verb::Hold { held_for } => (3, Some(held_for)),
        Verb::Release { range } => (4, Some(range)),
        Verb::Cancel => (5, None),
    }
}

impl Up {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        match self {
            Up::Join { proto } => w.u8(JOIN).u16(*proto),
            Up::Input { seq, it } => {
                // mag is the throttle, or the hold's age, or the release's range.
                let (code, arg) = verb_code(it.verb);
                let mag = arg.unwrap_or(it.throttle);
                let stick = ((it.throttle > 0) as u8) << 3;
                let jump = (it.jump as u8) << 4 | (it.sprint as u8) << 5;
                w.u8(INPUT)
                    .u16(*seq)
                    .u16(it.heading)
                    .u8(mag)
                    .u8(code | stick | jump)
                    .u16(it.aim)
            }
            Up::Heart { act, arg } => w.u8(HEART).u8(*act).u8(*arg),
            Up::Device { w: sw, h, touch } => w.u8(DEVICE).u16(*sw).u16(*h).u8(*touch as u8),
            Up::Ping { t, rtt } => w.u8(PING).u32(*t).u8(*rtt),
        };
        w.0
    }

    pub fn decode(b: &[u8]) -> Option<Up> {
        let mut r = Reader::new(b);
        let up = match r.u8()? {
            JOIN => Up::Join { proto: r.u16()? },
            INPUT => {
                let seq = r.u16()?;
                let heading = r.u16()?;
                let mag = r.u8()?;
                let v = r.u8()?;
                let aim = r.u16()?;
                let stick = v & 8 != 0;
                if v >> 6 != 0 {
                    return None;
                }
                let verb = match v & 7 {
                    0 => Verb::None,
                    1 => Verb::Tap,
                    2 => Verb::Flick,
                    3 => Verb::Hold { held_for: mag },
                    4 => Verb::Release { range: mag },
                    5 => Verb::Cancel,
                    _ => return None,
                };
                let throttle = match verb {
                    Verb::Hold { .. } | Verb::Release { .. } => 0,
                    _ if stick => mag.max(1),
                    _ => 0,
                };
                Up::Input {
                    seq,
                    it: Intent {
                        heading,
                        throttle,
                        verb,
                        aim,
                        jump: v & 16 != 0,
                        sprint: v & 32 != 0,
                    },
                }
            }
            HEART => Up::Heart {
                act: r.u8()?,
                arg: r.u8()?,
            },
            DEVICE => Up::Device {
                w: r.u16()?,
                h: r.u16()?,
                touch: r.u8()? != 0,
            },
            PING => Up::Ping {
                t: r.u32()?,
                rtt: r.u8()?,
            },
            _ => return None,
        };
        r.done().then_some(up)
    }
}

/// Things that are not tiles: Lumens, motes, glim on the ground.
pub mod kind {
    pub const LUMEN: u8 = 1;
    pub const MOTE: u8 = 2;
    pub const PICKUP: u8 = 3;
}

/// An entity as a page knows it: quantized, and exactly what the page
/// rebuilds. Positions are 1/256 tile.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ent {
    pub id: u16,
    pub kind: u8,
    pub x: i16,
    pub y: i16,
    /// Height above the ground, 1/256 tile.
    pub z: i16,
    pub facing: u16,
    /// For a Lumen: movement (low 3 bits), action (next 3), ghost, down.
    pub state: u8,
    pub flame: u8,
    /// Glim carried, in steps (0, 1-4, 5-14, 15-49, 50-149, 150+), or a
    /// pickup's glim.
    pub glim: u8,
    pub hue: u8,
    pub flow: u8,
    pub name: String,
}

/// Fields of an entity that can change, for a Moved's mask.
pub mod field {
    pub const POS: u8 = 1;
    pub const FACING: u8 = 2;
    pub const STATE: u8 = 4;
    pub const FLAME: u8 = 8;
    pub const GLIM: u8 = 16;
    pub const FLOW: u8 = 32;
    pub const Z: u8 = 64;
}

pub fn q(v: Fx) -> i16 {
    (v.0 >> 8).clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

pub fn unq(v: i16) -> Fx {
    Fx((v as i32) << 8)
}

/// Something that happened, for the page to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ev {
    pub kind: u8,
    pub a: u16,
    pub b: u16,
    pub n: u8,
}

/// Self: everything the page predicts, exactly, and what it carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Own {
    pub me: Me,
    pub flame: i32,
    pub glim: u32,
    pub wood: u32,
    pub stone: u32,
    pub flow: u8,
    pub ghost: u16,
    pub down: u8,
    /// In the Underlight: ticks until the return, and who did it.
    pub descent: u8,
    pub killer: u16,
    /// v0.2: Sunwheat carried; kindle mode; the piece in build mode (0:
    /// none) and the channel's ticks left; returning at the hearth; the
    /// level of every skill.
    pub wheat: u32,
    pub kindle: bool,
    pub build: u8,
    pub channel: u8,
    pub home: bool,
    pub levels: [u8; 7],
    /// Gear worn (wand, robe, charm) and carried (0: an empty place).
    pub gear: [u8; 3],
    pub bag: [u8; crate::laws::BAG],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub tick: u32,
    pub ack: u16,
    pub repeats: u8,
    pub queue: u8,
    pub own: Option<Own>,
    pub new: Vec<Ent>,
    /// id, mask, the entity's new values (only masked fields are sent).
    pub moved: Vec<(u8, Ent)>,
    pub gone: Vec<u16>,
    pub tiles: Vec<(u16, Tile)>,
    pub events: Vec<Ev>,
    /// Lit things beyond sight (Beacons): id and tile.
    pub far: Vec<(u16, i16, i16)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Down {
    Welcome {
        proto: u16,
        oldest: u16,
        build: u32,
        laws: Vec<u8>,
        tick: u32,
        hz: u8,
        seed: u64,
        you: u16,
    },
    Frame(Box<Frame>),
    Chunk {
        cx: u8,
        cy: u8,
        tiles: Vec<Tile>,
    },
    ChunkGone {
        cx: u8,
        cy: u8,
    },
    Board {
        people: u16,
        awake: u16,
    },
    Pong {
        t: u32,
    },
    Elsewhere,
    /// Every claim there is (sent at the start, and when one changes).
    Claims(Vec<ClaimInfo>),
}

fn put_fx(w: &mut Writer, v: Fx) {
    w.i32(v.0);
}

fn get_fx(r: &mut Reader) -> Option<Fx> {
    r.i32().map(Fx)
}

fn put_body(w: &mut Writer, b: &Body) {
    for v in [
        b.x,
        b.y,
        b.z,
        b.vx,
        b.vy,
        b.vz,
        b.dash_prior,
        b.safe.0,
        b.safe.1,
    ] {
        put_fx(w, v);
    }
    w.u16(b.facing)
        .u8(b.mv as u8)
        .u32(b.t)
        .i32(b.breath)
        .u32(b.rest);
    w.i32(b.regen).u32(b.cooldown).u32(b.iframes);
    w.u8(b.winded as u8 | (b.sprinting as u8) << 1);
    w.u32(b.kick).u8(b.kicked as u8);
    w.u8(b.kick_held.is_some() as u8)
        .u16(b.kick_held.unwrap_or(0));
    w.u16(b.dash_h).u32(b.load).u16(b.claim).u8(b.busy as u8);
}

fn get_body(r: &mut Reader) -> Option<Body> {
    let mut f = [Fx::ZERO; 9];
    for v in &mut f {
        *v = get_fx(r)?;
    }
    let facing = r.u16()?;
    let mv = match r.u8()? {
        0 => Move::Free,
        2 => Move::Dash,
        3 => Move::Stun,
        4 => Move::Falling,
        5 => Move::Fallen,
        _ => return None,
    };
    let (t, breath, rest, regen) = (r.u32()?, r.i32()?, r.u32()?, r.i32()?);
    let (cooldown, iframes) = (r.u32()?, r.u32()?);
    let wind = r.u8()?;
    let (kick, kicked) = (r.u32()?, r.u8()? != 0);
    let held = r.u8()? != 0;
    let held_h = r.u16()?;
    let (dash_h, load, claim) = (r.u16()?, r.u32()?, r.u16()?);
    let busy = match r.u8()? {
        0 => Busy::No,
        1 => Busy::Striking,
        2 => Busy::Charging,
        3 => Busy::Lunging,
        _ => return None,
    };
    Some(Body {
        x: f[0],
        y: f[1],
        z: f[2],
        vx: f[3],
        vy: f[4],
        vz: f[5],
        dash_prior: f[6],
        safe: (f[7], f[8]),
        facing,
        mv,
        t,
        breath,
        rest,
        regen,
        winded: wind & 1 != 0,
        sprinting: wind & 2 != 0,
        cooldown,
        iframes,
        kick,
        kicked,
        kick_held: held.then_some(held_h),
        dash_h,
        load,
        claim,
        busy,
    })
}

fn put_action(w: &mut Writer, a: &Action) {
    w.u8(a.act.code())
        .u32(a.t)
        .u32(a.c)
        .u16(a.aim)
        .u8(a.landed as u8);
    w.u8(a.build as u8).i32(a.carry.0).u32(a.late).u8(a.haste);
}

fn get_action(r: &mut Reader) -> Option<Action> {
    let act = Act::from_code(r.u8()?)?;
    let (t, c, aim, landed) = (r.u32()?, r.u32()?, r.u16()?, r.u8()? != 0);
    let (build, carry, late, haste) = (r.u8()? != 0, Fx(r.i32()?), r.u32()?, r.u8()?);
    Some(Action {
        act,
        t,
        c,
        aim,
        landed,
        carry,
        late,
        haste,
        build,
    })
}

fn put_own(w: &mut Writer, o: &Own) {
    put_body(w, &o.me.body);
    put_action(w, &o.me.act);
    w.i32(o.flame)
        .u32(o.glim)
        .u32(o.wood)
        .u32(o.stone)
        .u8(o.flow);
    w.u16(o.ghost).u8(o.down).u8(o.descent).u16(o.killer);
    w.u32(o.wheat)
        .u8(o.kindle as u8)
        .u8(o.build)
        .u8(o.channel)
        .u8(o.home as u8);
    for l in o.levels {
        w.u8(l);
    }
    for g in o.gear.iter().chain(o.bag.iter()) {
        w.u8(*g);
    }
}

fn get_own(r: &mut Reader) -> Option<Own> {
    Some(Own {
        me: Me {
            body: get_body(r)?,
            act: get_action(r)?,
        },
        flame: r.i32()?,
        glim: r.u32()?,
        wood: r.u32()?,
        stone: r.u32()?,
        flow: r.u8()?,
        ghost: r.u16()?,
        down: r.u8()?,
        descent: r.u8()?,
        killer: r.u16()?,
        wheat: r.u32()?,
        kindle: r.u8()? != 0,
        build: r.u8()?,
        channel: r.u8()?,
        home: r.u8()? != 0,
        levels: {
            let mut l = [0u8; 7];
            for v in &mut l {
                *v = r.u8()?;
            }
            l
        },
        gear: [r.u8()?, r.u8()?, r.u8()?],
        bag: {
            let mut b = [0u8; crate::laws::BAG];
            for v in &mut b {
                *v = r.u8()?;
            }
            b
        },
    })
}

fn put_ent(w: &mut Writer, e: &Ent, mask: u8) {
    use field::*;
    if mask & POS != 0 {
        w.i16(e.x).i16(e.y);
    }
    if mask & FACING != 0 {
        w.u16(e.facing);
    }
    if mask & STATE != 0 {
        w.u8(e.state);
    }
    if mask & FLAME != 0 {
        w.u8(e.flame);
    }
    if mask & GLIM != 0 {
        w.u8(e.glim);
    }
    if mask & FLOW != 0 {
        w.u8(e.flow);
    }
    if mask & Z != 0 {
        w.i16(e.z);
    }
}

fn get_ent(r: &mut Reader, e: &mut Ent, mask: u8) -> Option<()> {
    use field::*;
    if mask & POS != 0 {
        e.x = r.i16()?;
        e.y = r.i16()?;
    }
    if mask & FACING != 0 {
        e.facing = r.u16()?;
    }
    if mask & STATE != 0 {
        e.state = r.u8()?;
    }
    if mask & FLAME != 0 {
        e.flame = r.u8()?;
    }
    if mask & GLIM != 0 {
        e.glim = r.u8()?;
    }
    if mask & FLOW != 0 {
        e.flow = r.u8()?;
    }
    if mask & Z != 0 {
        e.z = r.i16()?;
    }
    Some(())
}

const ALL: u8 = 127;

impl Down {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        match self {
            Down::Welcome {
                proto,
                oldest,
                build,
                laws,
                tick,
                hz,
                seed,
                you,
            } => {
                w.u8(WELCOME).u16(*proto).u16(*oldest).u32(*build);
                w.u16(laws.len() as u16);
                w.0.extend_from_slice(laws);
                w.u32(*tick).u8(*hz).u64(*seed).u16(*you);
            }
            Down::Frame(f) => encode_frame(&mut w, f),
            Down::Chunk { cx, cy, tiles } => {
                w.u8(CHUNK).u8(0).u8(*cx).u8(*cy);
                rle(tiles, &mut w);
            }
            Down::ChunkGone { cx, cy } => {
                w.u8(CHUNK_GONE).u8(0).u8(*cx).u8(*cy);
            }
            Down::Board { people, awake } => {
                w.u8(BOARD).u16(*people).u16(*awake);
            }
            Down::Pong { t } => {
                w.u8(PONG).u32(*t);
            }
            Down::Elsewhere => {
                w.u8(ELSEWHERE);
            }
            Down::Claims(list) => {
                w.u8(CLAIMS).u16(list.len() as u16);
                for c in list {
                    let (hx, hy) = c.hearth.unwrap_or((0, 0));
                    w.u16(c.id)
                        .u8(c.hue)
                        .str(&c.name)
                        .u8(c.hearth.is_some() as u8)
                        .i16(hx)
                        .i16(hy);
                }
            }
        }
        w.0
    }

    pub fn decode(b: &[u8]) -> Option<Down> {
        let mut r = Reader::new(b);
        let d = match r.u8()? {
            WELCOME => {
                let (proto, oldest, build) = (r.u16()?, r.u16()?, r.u32()?);
                let n = r.u16()? as usize;
                let laws = r.bytes(n)?.to_vec();
                Down::Welcome {
                    proto,
                    oldest,
                    build,
                    laws,
                    tick: r.u32()?,
                    hz: r.u8()?,
                    seed: r.u64()?,
                    you: r.u16()?,
                }
            }
            FRAME => Down::Frame(Box::new(decode_frame(&mut r)?)),
            CHUNK => {
                let _zone = r.u8()?;
                let (cx, cy) = (r.u8()?, r.u8()?);
                Down::Chunk {
                    cx,
                    cy,
                    tiles: unrle(&mut r)?,
                }
            }
            CHUNK_GONE => {
                let _zone = r.u8()?;
                Down::ChunkGone {
                    cx: r.u8()?,
                    cy: r.u8()?,
                }
            }
            BOARD => Down::Board {
                people: r.u16()?,
                awake: r.u16()?,
            },
            PONG => Down::Pong { t: r.u32()? },
            ELSEWHERE => Down::Elsewhere,
            CLAIMS => {
                let n = r.u16()? as usize;
                r.room(n, 9)?;
                let mut list = Vec::with_capacity(n);
                for _ in 0..n {
                    let (id, hue, name) = (r.u16()?, r.u8()?, r.str()?);
                    let (has, hx, hy) = (r.u8()? != 0, r.i16()?, r.i16()?);
                    list.push(ClaimInfo {
                        id,
                        hue,
                        name,
                        hearth: has.then_some((hx, hy)),
                    });
                }
                Down::Claims(list)
            }
            _ => return None,
        };
        r.done().then_some(d)
    }
}

fn encode_frame(w: &mut Writer, f: &Frame) {
    w.u8(FRAME).u32(f.tick).u16(f.ack).u8(f.repeats).u8(f.queue);
    match &f.own {
        Some(o) => {
            w.u8(1);
            put_own(w, o);
        }
        None => {
            w.u8(0);
        }
    }
    w.u16(f.new.len() as u16);
    for e in &f.new {
        w.u16(e.id).u8(e.kind).u8(e.hue);
        put_ent(w, e, ALL);
        if e.kind == kind::LUMEN {
            w.str(&e.name);
        }
    }
    w.u16(f.moved.len() as u16);
    for (mask, e) in &f.moved {
        w.u16(e.id).u8(*mask);
        put_ent(w, e, *mask);
    }
    w.u16(f.gone.len() as u16);
    for id in &f.gone {
        w.u16(*id);
    }
    w.u16(f.tiles.len() as u16);
    for (i, t) in &f.tiles {
        w.u16(*i).u32(t.to_u32());
    }
    w.u8(f.events.len().min(255) as u8);
    for e in f.events.iter().take(255) {
        w.u8(e.kind).u16(e.a).u16(e.b).u8(e.n);
    }
    w.u8(f.far.len().min(255) as u8);
    for &(id, x, y) in f.far.iter().take(255) {
        w.u16(id).i16(x).i16(y);
    }
}

fn decode_frame(r: &mut Reader) -> Option<Frame> {
    let mut f = Frame {
        tick: r.u32()?,
        ack: r.u16()?,
        repeats: r.u8()?,
        queue: r.u8()?,
        ..Frame::default()
    };
    if r.u8()? != 0 {
        f.own = Some(get_own(r)?);
    }
    let n = r.u16()? as usize;
    r.room(n, 4)?;
    for _ in 0..n {
        let mut e = Ent {
            id: r.u16()?,
            kind: r.u8()?,
            hue: r.u8()?,
            ..Ent::default()
        };
        get_ent(r, &mut e, ALL)?;
        if e.kind == kind::LUMEN {
            e.name = r.str()?;
        }
        f.new.push(e);
    }
    let n = r.u16()? as usize;
    r.room(n, 3)?;
    for _ in 0..n {
        let mut e = Ent {
            id: r.u16()?,
            ..Ent::default()
        };
        let mask = r.u8()?;
        get_ent(r, &mut e, mask)?;
        f.moved.push((mask, e));
    }
    let n = r.u16()? as usize;
    r.room(n, 2)?;
    for _ in 0..n {
        f.gone.push(r.u16()?);
    }
    let n = r.u16()? as usize;
    r.room(n, 6)?;
    for _ in 0..n {
        f.tiles.push((r.u16()?, Tile::from_u32(r.u32()?)));
    }
    let n = r.u8()? as usize;
    r.room(n, 6)?;
    for _ in 0..n {
        f.events.push(Ev {
            kind: r.u8()?,
            a: r.u16()?,
            b: r.u16()?,
            n: r.u8()?,
        });
    }
    let n = r.u8()? as usize;
    r.room(n, 6)?;
    for _ in 0..n {
        f.far.push((r.u16()?, r.i16()?, r.i16()?));
    }
    Some(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::laws::LAWS;

    #[test]
    fn messages_round_trip() {
        for up in [
            Up::Join { proto: PROTO },
            Up::Input {
                seq: 7,
                it: Intent {
                    heading: 1234,
                    throttle: 200,
                    verb: Verb::Flick,
                    aim: 999,
                    jump: true,
                    sprint: true,
                },
            },
            Up::Input {
                seq: 8,
                it: Intent {
                    heading: 0,
                    throttle: 0,
                    verb: Verb::Hold { held_for: 9 },
                    aim: 5,
                    jump: false,
                    sprint: false,
                },
            },
            Up::Heart { act: 1, arg: 2 },
            Up::Device {
                w: 390,
                h: 844,
                touch: true,
            },
            Up::Ping { t: 5, rtt: 20 },
        ] {
            assert_eq!(Up::decode(&up.encode()), Some(up));
        }
        let own = Own {
            me: Me {
                body: Body::at(Fx::int(3), Fx::milli(-1500), &LAWS),
                ..Me::default()
            },
            flame: 50_000,
            glim: 30,
            ..Own::default()
        };
        let f = Frame {
            tick: 9,
            ack: 3,
            own: Some(own),
            new: vec![Ent {
                id: 4,
                kind: kind::LUMEN,
                x: -300,
                y: 200,
                z: 90,
                name: "moth".into(),
                ..Ent::default()
            }],
            moved: vec![(
                field::POS | field::Z,
                Ent {
                    id: 4,
                    x: 1,
                    y: 2,
                    z: 3,
                    ..Ent::default()
                },
            )],
            gone: vec![9],
            tiles: vec![(5, Tile::new(2, 1))],
            events: vec![Ev {
                kind: 1,
                a: 2,
                b: 3,
                n: 4,
            }],
            ..Frame::default()
        };
        let d = Down::Frame(Box::new(f));
        assert_eq!(Down::decode(&d.encode()), Some(d));
    }
}
