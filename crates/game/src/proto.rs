//! The wire between the server and a browser: small binary messages,
//! little-endian. Decoding never panics and never trusts a count.

use crate::laws::MAX_NAME;

/// World units are sent in quarters, as i16.
pub const Q: f32 = 4.0;

pub fn q(v: f32) -> i16 {
    (v * Q).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16
}

pub fn unq(v: i16) -> f32 {
    v as f32 / Q
}

/// Angles travel as u16 turns.
pub fn angle_to_u16(a: f32) -> u16 {
    let t = a.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
    (t * 65536.0) as u32 as u16
}

pub fn angle_from_u16(v: u16) -> f32 {
    v as f32 / 65536.0 * std::f32::consts::TAU
}

#[derive(Default)]
pub struct Writer(pub Vec<u8>);

impl Writer {
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn i16(&mut self, v: i16) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn str(&mut self, s: &str) -> &mut Self {
        let b: Vec<u8> = s.bytes().take(MAX_NAME * 4).collect();
        // Never cut a character in half.
        let mut n = b.len();
        while n > 0 && std::str::from_utf8(&b[..n]).is_err() {
            n -= 1;
        }
        self.u8(n as u8);
        self.0.extend_from_slice(&b[..n]);
        self
    }
}

pub struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub fn new(b: &'a [u8]) -> Reader<'a> {
        Reader { b, at: 0 }
    }
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(s)
    }
    pub fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    pub fn i16(&mut self) -> Option<i16> {
        Some(i16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    pub fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    pub fn str(&mut self) -> Option<String> {
        let n = self.u8()? as usize;
        Some(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    /// Room for `count` items of at least `size` bytes each, or None: a
    /// count is never trusted past the bytes that are actually here.
    fn room(&self, count: usize, size: usize) -> Option<usize> {
        (count.checked_mul(size)? <= self.b.len() - self.at).then_some(count)
    }
}

// ---- browser to server -----------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum Up {
    /// Start playing (again) under this name.
    Join { name: String },
    /// Head this way; boost or not.
    Steer { angle: u16, boost: bool },
    /// The browser's screen, in CSS pixels.
    Screen { w: u16, h: u16 },
}

impl Up {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        match self {
            Up::Join { name } => w.u8(1).str(name),
            Up::Steer { angle, boost } => w.u8(2).u16(*angle).u8(*boost as u8),
            Up::Screen { w: sw, h } => w.u8(3).u16(*sw).u16(*h),
        };
        w.0
    }

    pub fn decode(b: &[u8]) -> Option<Up> {
        let mut r = Reader::new(b);
        Some(match r.u8()? {
            1 => Up::Join { name: r.str()? },
            2 => Up::Steer {
                angle: r.u16()?,
                boost: r.u8()? != 0,
            },
            3 => Up::Screen {
                w: r.u16()?,
                h: r.u16()?,
            },
            _ => return None,
        })
    }
}

// ---- server to browser -----------------------------------------------------

pub const HELLO: u8 = 10;
pub const FRAME: u8 = 11;
pub const BOARD: u8 = 12;
pub const DIED: u8 = 13;
pub const FEED: u8 = 14;

#[derive(Clone, Debug, PartialEq)]
pub struct SnakeUpdate {
    pub id: u16,
    pub boosting: bool,
    /// Newly arrived: nothing can hurt it yet, and it hurts nothing.
    pub ghost: bool,
    pub mass: u32,
    /// Body length in points; the browser trims to it.
    pub len: u16,
    pub angle: u16,
    /// Name and hue, for a snake the browser has not seen yet; its points
    /// are then the whole body.
    pub new: Option<(String, u8)>,
    /// Points, head first: the whole body when new, else the head's new
    /// points since the last frame.
    pub points: Vec<(i16, i16)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FoodInfo {
    pub id: u32,
    pub x: i16,
    pub y: i16,
    pub value: u8,
    pub hue: u8,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Frame {
    pub tick: u32,
    /// The snake this browser plays; 0 when it is watching.
    pub you: u16,
    /// Where the view is centred.
    pub centre: (i16, i16),
    pub snakes: Vec<SnakeUpdate>,
    /// Snakes that died or left the view.
    pub gone: Vec<u16>,
    pub food: Vec<FoodInfo>,
    /// Food that was eaten (by that snake; 0 if it rotted or left view).
    pub eaten: Vec<(u32, u16)>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Leader {
    pub name: String,
    pub score: u32,
    pub hue: u8,
    pub human: bool,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Board {
    /// People playing, and snakes in the arena.
    pub people: u16,
    pub snakes: u16,
    pub top: Vec<Leader>,
    /// This browser's rank, 0 when not playing.
    pub rank: u16,
    /// Every snake's head on the minimap: x, y scaled to -127..=127 of the
    /// arena, and its size.
    pub dots: Vec<(i8, i8, u8)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Down {
    Hello {
        arena: u16,
        hz: u8,
    },
    Frame(Frame),
    Board(Board),
    /// This browser's snake burst: who it ran into ("" for the edge), and
    /// how long it got.
    Died {
        by: String,
        score: u32,
    },
    /// Someone ate someone, for the feed.
    Feed {
        killer: String,
        victim: String,
        score: u32,
    },
}

pub fn hello(arena: u16, hz: u8) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(HELLO).u16(arena).u8(hz);
    w.0
}

pub fn encode_frame(f: &Frame) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(FRAME)
        .u32(f.tick)
        .u16(f.you)
        .i16(f.centre.0)
        .i16(f.centre.1);
    w.u16(f.snakes.len() as u16);
    for s in &f.snakes {
        let flags = s.new.is_some() as u8 | (s.boosting as u8) << 1 | (s.ghost as u8) << 2;
        w.u16(s.id).u8(flags).u32(s.mass).u16(s.len).u16(s.angle);
        let points = match &s.new {
            Some((name, hue)) => {
                let n = s.points.len().min(u16::MAX as usize);
                w.u8(*hue).str(name).u16(n as u16);
                &s.points[..n]
            }
            None => {
                let n = s.points.len().min(255);
                w.u8(n as u8);
                &s.points[..n]
            }
        };
        for &(x, y) in points {
            w.i16(x).i16(y);
        }
    }
    w.u16(f.gone.len() as u16);
    for &id in &f.gone {
        w.u16(id);
    }
    w.u16(f.food.len() as u16);
    for fd in &f.food {
        w.u32(fd.id).i16(fd.x).i16(fd.y).u8(fd.value).u8(fd.hue);
    }
    w.u16(f.eaten.len() as u16);
    for &(id, by) in &f.eaten {
        w.u32(id).u16(by);
    }
    w.0
}

pub fn encode_board(b: &Board) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(BOARD).u16(b.people).u16(b.snakes).u16(b.rank);
    w.u8(b.top.len() as u8);
    for l in &b.top {
        w.str(&l.name).u32(l.score).u8(l.hue).u8(l.human as u8);
    }
    w.u16(b.dots.len() as u16);
    for &(x, y, s) in &b.dots {
        w.u8(x as u8).u8(y as u8).u8(s);
    }
    w.0
}

pub fn died(by: &str, score: u32) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(DIED).str(by).u32(score);
    w.0
}

pub fn feed(killer: &str, victim: &str, score: u32) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(FEED).str(killer).str(victim).u32(score);
    w.0
}

impl Down {
    pub fn decode(b: &[u8]) -> Option<Down> {
        let mut r = Reader::new(b);
        Some(match r.u8()? {
            HELLO => Down::Hello {
                arena: r.u16()?,
                hz: r.u8()?,
            },
            FRAME => Down::Frame(decode_frame(&mut r)?),
            BOARD => {
                let (people, snakes, rank) = (r.u16()?, r.u16()?, r.u16()?);
                let n = r.u8()? as usize;
                let mut top = Vec::with_capacity(r.room(n, 7)?);
                for _ in 0..n {
                    top.push(Leader {
                        name: r.str()?,
                        score: r.u32()?,
                        hue: r.u8()?,
                        human: r.u8()? != 0,
                    });
                }
                let n = r.u16()? as usize;
                let mut dots = Vec::with_capacity(r.room(n, 3)?);
                for _ in 0..n {
                    dots.push((r.u8()? as i8, r.u8()? as i8, r.u8()?));
                }
                Down::Board(Board {
                    people,
                    snakes,
                    top,
                    rank,
                    dots,
                })
            }
            DIED => Down::Died {
                by: r.str()?,
                score: r.u32()?,
            },
            FEED => Down::Feed {
                killer: r.str()?,
                victim: r.str()?,
                score: r.u32()?,
            },
            _ => return None,
        })
    }
}

fn decode_frame(r: &mut Reader) -> Option<Frame> {
    let mut f = Frame {
        tick: r.u32()?,
        you: r.u16()?,
        centre: (r.i16()?, r.i16()?),
        ..Frame::default()
    };
    let n = r.u16()? as usize;
    f.snakes.reserve(r.room(n, 12)?);
    for _ in 0..n {
        let id = r.u16()?;
        let flags = r.u8()?;
        let (mass, len, angle) = (r.u32()?, r.u16()?, r.u16()?);
        let (new, count) = if flags & 1 != 0 {
            let hue = r.u8()?;
            let name = r.str()?;
            (Some((name, hue)), r.u16()? as usize)
        } else {
            (None, r.u8()? as usize)
        };
        let mut points = Vec::with_capacity(r.room(count, 4)?);
        for _ in 0..count {
            points.push((r.i16()?, r.i16()?));
        }
        f.snakes.push(SnakeUpdate {
            id,
            boosting: flags & 2 != 0,
            ghost: flags & 4 != 0,
            mass,
            len,
            angle,
            new,
            points,
        });
    }
    let n = r.u16()? as usize;
    f.gone.reserve(r.room(n, 2)?);
    for _ in 0..n {
        f.gone.push(r.u16()?);
    }
    let n = r.u16()? as usize;
    f.food.reserve(r.room(n, 10)?);
    for _ in 0..n {
        f.food.push(FoodInfo {
            id: r.u32()?,
            x: r.i16()?,
            y: r.i16()?,
            value: r.u8()?,
            hue: r.u8()?,
        });
    }
    let n = r.u16()? as usize;
    f.eaten.reserve(r.room(n, 6)?);
    for _ in 0..n {
        f.eaten.push((r.u32()?, r.u16()?));
    }
    Some(f)
}
