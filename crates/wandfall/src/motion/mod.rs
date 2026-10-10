//! How a wizard moves, tick by tick: run and sprint (with momentum: speed
//! above your pace bleeds away, it does not vanish), slide (crouch at
//! speed; gravity carries it down a hill), walk slower up a slope and
//! faster down it, jump, glide down in the drop, wade in the shallows,
//! step up small rises, and never through trees, rocks or pillars; kick
//! off their sides (`wall`); a Tether hauls you (`tether`).
//! Arithmetic only, so the page predicts its own wizard to the bit.
//!
//! A tick runs in phases, each in its own file: how it stands and what
//! it spends (`run`), its jumps (`jump`), a launch rune or a ledge
//! underfoot (`ledge`), and gravity and the move itself (`collide`).

use crate::laws::*;
use crate::map::Map;
use crate::trig;

mod collide;
mod jump;
mod ledge;
mod run;
#[cfg(test)]
mod tests;

/// What a page says each tick: which input this is, where it looks, and
/// the keys held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Input {
    pub seq: u16,
    pub yaw: u16,
    pub pitch: i16,
    pub keys: u16,
    /// Spells cast this input (a bit a slot).
    pub cast: u8,
    /// The tick the page drew everyone else at, as it sent this (the low
    /// 16 bits): where it saw them, for the Lance (motion ignores it).
    pub view: u16,
}

impl Input {
    /// Whether it holds key `k`.
    pub fn has(&self, k: u16) -> bool {
        self.keys & k != 0
    }
}

pub mod cast {
    pub const SLOT: [u8; 4] = [1, 2, 4, 8];
}

pub mod keys {
    pub const FWD: u16 = 1;
    pub const BACK: u16 = 2;
    pub const LEFT: u16 = 4;
    pub const RIGHT: u16 = 8;
    pub const JUMP: u16 = 16;
    pub const FIRE: u16 = 32;
    /// Aiming down the wand.
    pub const AIM: u16 = 64;
    pub const CROUCH: u16 = 128;
    pub const SPRINT: u16 = 256;
}

/// A wizard's body: feet, velocity, and whether it stands.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    pub p: [f32; 3],
    pub v: [f32; 3],
    pub ground: bool,
    /// Falling slowly from the drop until it lands.
    pub glide: bool,
    /// Ticks left chilled (by Frost): slower.
    pub chill: u16,
    /// Crouching: lower, slower, harder to hit.
    pub crouch: bool,
    /// Ticks a jump still works since the ground was left; ticks an early
    /// jump waits for the ground.
    pub coyote: u8,
    pub buffer: u8,
    /// Sprinting; sliding; ticks before a slide boosts again.
    pub sprint: bool,
    pub slide: bool,
    pub slide_cd: u8,
    /// Stamina spent (0 fresh, `STAMINA` none left), ticks since it last
    /// sprinted, and winded (it cannot sprint till some comes back).
    pub spent: u16,
    pub breath: u8,
    pub winded: bool,
    /// Jump held last tick (a jump is a press); ticks on the ground since
    /// it landed; its air jump spent (till it lands).
    pub held: bool,
    pub landed: u8,
    pub air_jumped: bool,
    /// Ticks left of a climb onto a ledge (steering held till it is over).
    pub mantle: u8,
    /// Ticks left pulled by a Tether, toward where it caught.
    pub tether: u8,
    pub anchor: [f32; 3],
    /// Wall jumps since it last landed; ticks it may still kick off the
    /// wall it last touched, and which way that wall faces (x, z).
    pub walls: u8,
    pub wall: u8,
    pub wall_n: [f32; 2],
}

impl Body {
    /// How tall it stands, and where its eyes are: lower crouched on the
    /// ground or sliding (in the air, a crouch is only tucked legs).
    pub fn tall(&self) -> f32 {
        if self.low() {
            CROUCH_HEIGHT
        } else {
            HEIGHT
        }
    }

    pub fn eye(&self) -> f32 {
        if self.low() {
            CROUCH_EYE
        } else {
            EYE
        }
    }

    fn low(&self) -> bool {
        self.crouch && (self.ground || self.slide)
    }
}

/// How fast a velocity runs across the ground (its x and z).
pub fn flat(v: &[f32; 3]) -> f32 {
    (v[0] * v[0] + v[2] * v[2]).sqrt()
}

/// Where the keys steer, across the ground: a unit vector (x, z) from
/// where it looks, if any are held; and whether forward is.
#[derive(Clone, Copy, Debug)]
pub struct Wish {
    pub x: f32,
    pub z: f32,
    pub any: bool,
    pub forward: bool,
}

impl Wish {
    pub fn of(i: &Input) -> Wish {
        let (s, c) = trig::sin_cos(i.yaw);
        let f = i.has(keys::FWD) as i32 as f32 - i.has(keys::BACK) as i32 as f32;
        let r = i.has(keys::RIGHT) as i32 as f32 - i.has(keys::LEFT) as i32 as f32;
        // Forward is (c, s) on the ground; right is (-s, c).
        let (mut x, mut z) = (c * f - s * r, s * f + c * r);
        let len = (x * x + z * z).sqrt();
        if len > 1e-6 {
            x /= len;
            z /= len;
        }
        Wish {
            x,
            z,
            any: len > 1e-6,
            forward: f > 0.0,
        }
    }
}

/// What is underfoot as a tick starts: the sea (wading), and on bare
/// land (not a deck, not the sea floor) which way is up it, the rise a
/// metre east and a metre south.
#[derive(Clone, Copy, Debug)]
struct Under {
    wading: bool,
    gx: f32,
    gz: f32,
}

impl Under {
    fn of(b: &Body, map: &Map) -> Under {
        let under = map.height(b.p[0], b.p[2]);
        let wading = b.ground && under < SEA;
        let on_land = b.ground && !wading && map.floor(b.p[0], b.p[2], b.p[1]) <= under + 0.05;
        let (gx, gz) = if on_land {
            slope(map, b.p[0], b.p[2])
        } else {
            (0.0, 0.0)
        };
        Under { wading, gx, gz }
    }
}

/// The ground's rise a metre east and a metre south at (x, z).
fn slope(map: &Map, x: f32, z: f32) -> (f32, f32) {
    let e = 0.5;
    (
        (map.height(x + e, z) - map.height(x - e, z)) / (2.0 * e),
        (map.height(x, z + e) - map.height(x, z - e)) / (2.0 * e),
    )
}

/// One tick of a body under an input.
pub fn step(b: &mut Body, i: &Input, map: &Map) {
    let w = Wish::of(i);
    let u = Under::of(b, map);
    run::stance(b, i, &w, &u);
    run::stamina(b);
    run::steer(b, i, &w, &u);
    b.chill = b.chill.saturating_sub(1);
    jump::jump(b, i, &w, map);
    ledge::rune(b, map);
    ledge::mantle(b, map, &w);
    collide::fall(b, i);
    collide::advance(b, map);
}
