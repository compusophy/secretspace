//! A Lumen's body and how it moves, first person: you go where you push,
//! at once (quick to start, quick to stop), jump, and dash. Strikes face
//! where you look. In fixed point only, so the page predicts its own body
//! bit for bit. `step` is the whole of it:
//!
//! 1. timers and breath; 2. the ground underfoot; 3. hit-stun (no control,
//!    the flight decays); 4. a dash; 5. control (accelerate toward the
//!    stick's velocity, slower while striking or charging, little in the
//!    air); 6. the move, one axis at a time against solid tiles; 7. height:
//!    jumps and gravity, landing, or falling off the island into the Dark.

use engine::fixed::{len, unit, Fx};

use crate::laws::Laws;
use crate::tiles::{ground, Tiles};

/// What a person (or a resident) does this tick: the only way any Lumen
/// acts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Verb {
    #[default]
    None,
    /// A strike.
    Tap,
    /// A dash.
    Flick,
    /// A charge began `held_for` ticks ago.
    Hold {
        held_for: u8,
    },
    /// A charge let go: 0 for a heavy, else a throw's range (1-255: 4-9 tiles).
    Release {
        range: u8,
    },
    Cancel,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Intent {
    /// Which way to move (world heading), and how hard: 0 standing, 1-254
    /// a walk at that share of walk speed, 255 a run.
    pub heading: u16,
    pub throttle: u8,
    pub verb: Verb,
    /// Where you look (world heading): strikes, charges and throws go here.
    pub aim: u16,
    pub jump: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Move {
    #[default]
    Free = 0,
    Dash = 2,
    /// Hit-stun: no control while the flight decays.
    Stun = 3,
    /// Off the island, dropping into the Dark.
    Falling = 4,
    /// Gone into the Dark.
    Fallen = 5,
}

/// What an action leaves of control: everything (but slower while
/// striking or charging), or nothing (a lunge carries you).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Busy {
    #[default]
    No,
    Striking,
    Charging,
    Lunging,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Body {
    pub x: Fx,
    pub y: Fx,
    /// Height above the ground, and its speed (up is positive).
    pub z: Fx,
    pub vx: Fx,
    pub vy: Fx,
    pub vz: Fx,
    pub facing: u16,
    pub mv: Move,
    /// Ticks into a dash; ticks left of stun.
    pub t: u32,
    /// Breath in thousandths, and ticks until it regenerates.
    pub breath: i32,
    pub rest: u32,
    /// Breath regeneration, thousandths (Rekindled raises it).
    pub regen: i32,
    pub cooldown: u32,
    pub iframes: u32,
    /// The wall-kick window, whether this wall was kicked, and a dash held
    /// from a dash's last ticks.
    pub kick: u32,
    pub kicked: bool,
    pub kick_held: Option<u16>,
    pub dash_h: u16,
    pub dash_prior: Fx,
    /// The last place on ground.
    pub safe: (Fx, Fx),
    /// Materials carried (weight), and the claim whose land is its own.
    pub load: u32,
    pub claim: u16,
    pub busy: Busy,
}

impl Body {
    pub fn at(x: Fx, y: Fx, l: &Laws) -> Body {
        Body {
            x,
            y,
            breath: l.breath,
            regen: 1000,
            safe: (x, y),
            ..Body::default()
        }
    }

    pub fn speed(&self) -> Fx {
        len(self.vx, self.vy)
    }

    /// Whether nothing can hurt it this tick (a dash's first ticks).
    pub fn dodging(&self) -> bool {
        self.iframes > 0
    }

    pub fn grounded(&self) -> bool {
        self.z == Fx::ZERO && self.vz == Fx::ZERO
    }

    /// Thrown: this velocity, a little up, and no control for this many
    /// ticks.
    pub fn launch(&mut self, vx: Fx, vy: Fx, ticks: u32) {
        if matches!(self.mv, Move::Fallen | Move::Falling) {
            return;
        }
        self.vx = vx;
        self.vy = vy;
        self.vz = self.vz.max(len(vx, vy).div_int(3));
        self.mv = Move::Stun;
        self.t = ticks.max(1);
        self.busy = Busy::No;
    }
}

/// What happened in a step, for combat and Flow.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Moved {
    /// Speed when it hit a solid tile (zero if it did not).
    pub slam: Fx,
    pub dashed: bool,
    pub kicked: bool,
    pub jumped: bool,
    pub fell: bool,
}

/// The ground's multipliers, thousandths.
struct Underfoot {
    accel: i32,
    speed: i32,
    dash: bool,
    ice: bool,
}

fn underfoot(b: &Body, t: &Tiles, l: &Laws) -> Underfoot {
    let tile = t.under(b.x, b.y);
    let mut u = Underfoot {
        accel: 1000,
        speed: 1000,
        dash: true,
        ice: false,
    };
    match tile.kind() {
        ground::ICE => {
            u.accel = l.ice_grip;
            u.ice = true;
        }
        ground::MUD => u.speed = l.mud_speed,
        ground::WATER => {
            u.speed = l.water_speed;
            u.dash = false;
        }
        _ => {}
    }
    if tile.land_of(b.claim) {
        u.speed = u.speed * l.own_land / 1000;
    }
    u
}

fn milli(v: Fx, m: i32) -> Fx {
    Fx((v.0 as i64 * m as i64 / 1000) as i32)
}

/// Move `v` toward `want` by at most `step`.
fn approach(v: Fx, want: Fx, step: Fx) -> Fx {
    if v < want {
        v.add(step).min(want)
    } else {
        v.sub(step).max(want)
    }
}

fn start_dash(b: &mut Body, h: u16, free: bool, l: &Laws, m: &mut Moved) {
    b.dash_prior = b.speed();
    b.mv = Move::Dash;
    b.t = 0;
    b.dash_h = h;
    b.iframes = l.iframes;
    b.kick_held = None;
    b.busy = Busy::No;
    if !free {
        b.breath -= l.dash_breath;
        b.rest = l.breath_rest;
    }
    m.dashed = true;
}

/// One tick of a body.
pub fn step(b: &mut Body, it: &Intent, t: &Tiles, l: &Laws) -> Moved {
    let mut m = Moved::default();
    if b.mv == Move::Fallen {
        return m;
    }
    // 1. Timers and breath.
    for c in [&mut b.cooldown, &mut b.iframes, &mut b.kick] {
        *c = c.saturating_sub(1);
    }
    if b.rest > 0 {
        b.rest -= 1;
    } else {
        b.breath = (b.breath + l.breath_regen * b.regen / 1000).min(l.breath);
    }
    // 2. The ground.
    let g = underfoot(b, t, l);
    let flick = it.verb == Verb::Flick;
    // A dash goes the way you move, or the way you look standing still.
    let dash_h = if it.throttle > 0 { it.heading } else { it.aim };

    match b.mv {
        Move::Dash if flick && b.t + l.kick_hold >= l.dash_ticks => b.kick_held = Some(dash_h),
        Move::Free if flick && b.busy != Busy::Charging => {
            if b.kick > 0 && !b.kicked {
                b.kicked = true;
                start_dash(b, dash_h, true, l, &mut m);
                m.kicked = true;
            } else if b.cooldown == 0 && b.breath >= l.dash_breath && g.dash {
                start_dash(b, dash_h, false, l, &mut m);
            }
        }
        _ => {}
    }

    let air = !b.grounded();
    match b.mv {
        // 3. No control while the flight decays.
        Move::Stun => {
            let decay = milli(l.flight_decay, if g.ice { 150 } else { 1000 });
            let s = b.speed();
            if s > Fx::ZERO {
                let k = s.sub(decay).max(Fx::ZERO);
                b.vx = b.vx.mul(k).div(s);
                b.vy = b.vy.mul(k).div(s);
            }
            b.t = b.t.saturating_sub(1);
            if b.t == 0 {
                b.mv = Move::Free;
            }
        }
        // 4. A dash is its own velocity.
        Move::Dash => {
            let (ux, uy) = unit(b.dash_h);
            b.vx = ux.mul(l.dash_speed);
            b.vy = uy.mul(l.dash_speed);
            b.facing = it.aim;
        }
        Move::Falling | Move::Fallen => {}
        // 5. Control.
        Move::Free if b.busy == Busy::Lunging => {}
        Move::Free => {
            b.facing = it.aim;
            let mut top = match it.throttle {
                0 => Fx::ZERO,
                255 => l.run,
                n => Fx((l.walk.0 as i64 * n as i64 / 254) as i32),
            };
            let cut = (b.load as i32 / l.weight_step) * l.weight_cut;
            top = milli(top, (1000 - cut).max(l.weight_floor));
            top = milli(top, g.speed);
            top = match b.busy {
                Busy::Striking => milli(top, l.strike_slow),
                Busy::Charging => milli(top, l.charge_slow),
                _ => top,
            };
            let (ux, uy) = unit(it.heading);
            let (wx, wy) = (ux.mul(top), uy.mul(top));
            let mut rate = if top > Fx::ZERO { l.accel } else { l.stop };
            rate = milli(rate, g.accel);
            if air {
                rate = milli(rate, l.air_control);
            }
            b.vx = approach(b.vx, wx, rate);
            b.vy = approach(b.vy, wy, rate);
            if it.jump && !air {
                b.vz = l.jump;
                m.jumped = true;
            }
        }
    }

    // 6. The move, x then y, against solid tiles.
    let before = b.speed();
    if slide(b, t, l) {
        m.slam = before;
        if b.mv == Move::Dash {
            b.vx = Fx::ZERO;
            b.vy = Fx::ZERO;
            b.mv = Move::Free;
            b.cooldown = l.dash_cooldown;
            b.kick = l.kick_window;
            b.kicked = false;
            if let Some(h) = b.kick_held.take() {
                b.kicked = true;
                start_dash(b, h, true, l, &mut m);
                m.kicked = true;
            }
        }
    }
    if b.mv == Move::Dash {
        b.t += 1;
        if b.t >= l.dash_ticks {
            b.mv = Move::Free;
            b.cooldown = l.dash_cooldown;
            b.kick_held = None;
            let s = b.dash_prior.min(l.run);
            let (ux, uy) = unit(b.dash_h);
            b.vx = ux.mul(s);
            b.vy = uy.mul(s);
        }
    }

    // 7. Height: jumps and gravity; ground to land on, or the Dark.
    let over_void = t.under(b.x, b.y).void();
    if b.mv == Move::Falling || b.vz != Fx::ZERO || b.z != Fx::ZERO || over_void {
        b.vz = b.vz.sub(l.gravity);
        b.z = b.z.add(b.vz);
        if b.z <= Fx::ZERO && !over_void && b.mv != Move::Falling {
            b.z = Fx::ZERO;
            b.vz = Fx::ZERO;
        } else if b.z <= Fx::ZERO && over_void && b.mv != Move::Falling {
            // Off the edge: nothing below but the Dark.
            b.mv = Move::Falling;
            b.busy = Busy::No;
        }
        if b.mv == Move::Falling && b.z < l.fall_depth.neg() {
            b.mv = Move::Fallen;
            m.fell = true;
        }
    }
    if b.grounded() && !over_void && b.mv != Move::Falling {
        b.safe = (b.x, b.y);
    }
    m
}

/// Move by the velocity, one axis at a time, stopping at solid tiles.
/// Whether it touched one.
fn slide(b: &mut Body, t: &Tiles, l: &Laws) -> bool {
    let r = l.body;
    let eps = Fx(1);
    let mut hit = false;
    let nx = b.x.add(b.vx);
    if b.vx.0 != 0 {
        let lead = if b.vx.0 > 0 { nx.add(r) } else { nx.sub(r) };
        let col = lead.floor();
        let rows = b.y.sub(r).add(eps).floor()..=b.y.add(r).sub(eps).floor();
        if rows.clone().any(|ty| t.get(col, ty).solid_for(b.claim)) {
            b.x = if b.vx.0 > 0 {
                Fx::int(col).sub(r).sub(eps)
            } else {
                Fx::int(col + 1).add(r).add(eps)
            };
            b.vx = Fx::ZERO;
            hit = true;
        } else {
            b.x = nx;
        }
    }
    let ny = b.y.add(b.vy);
    if b.vy.0 != 0 {
        let lead = if b.vy.0 > 0 { ny.add(r) } else { ny.sub(r) };
        let row = lead.floor();
        let cols = b.x.sub(r).add(eps).floor()..=b.x.add(r).sub(eps).floor();
        if cols.clone().any(|tx| t.get(tx, row).solid_for(b.claim)) {
            b.y = if b.vy.0 > 0 {
                Fx::int(row).sub(r).sub(eps)
            } else {
                Fx::int(row + 1).add(r).add(eps)
            };
            b.vy = Fx::ZERO;
            hit = true;
        } else {
            b.y = ny;
        }
    }
    hit
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::laws::LAWS;
    use crate::tiles::{obj, Tile};

    fn floor(n: i32) -> Tiles {
        let mut t = Tiles::default();
        for y in -n..n {
            for x in -n..n {
                t.set(x, y, Tile::new(ground::MEADOW, obj::NONE));
            }
        }
        t
    }

    fn run(h: u16) -> Intent {
        Intent {
            heading: h,
            throttle: 255,
            aim: h,
            ..Intent::default()
        }
    }

    #[test]
    fn you_go_where_you_push_at_once_and_stop_at_once() {
        let l = &LAWS;
        let t = floor(40);
        let mut b = Body::at(Fx::ZERO, Fx::ZERO, l);
        let mut n = 0;
        while b.speed() < l.run && n < 30 {
            step(&mut b, &run(0), &t, l);
            n += 1;
        }
        assert!(n <= 6, "up to speed in {n} ticks");
        // A hard turn: straight away the other way, no drifting.
        for _ in 0..6 {
            step(&mut b, &run(32768), &t, l);
        }
        assert!(b.vx < Fx::ZERO, "already going back");
        let mut n = 0;
        while b.speed() > Fx::ZERO && n < 30 {
            step(&mut b, &Intent::default(), &t, l);
            n += 1;
        }
        assert!(n <= 6, "stopped in {n} ticks");
        // Strikes face where you look, not where you move.
        step(
            &mut b,
            &Intent {
                aim: 16384,
                ..run(0)
            },
            &t,
            l,
        );
        assert_eq!(b.facing, 16384);
    }

    #[test]
    fn a_jump_goes_up_and_comes_down() {
        let l = &LAWS;
        let t = floor(40);
        let mut b = Body::at(Fx::ZERO, Fx::ZERO, l);
        let m = step(
            &mut b,
            &Intent {
                jump: true,
                ..Intent::default()
            },
            &t,
            l,
        );
        assert!(m.jumped && b.z > Fx::ZERO);
        let mut top = Fx::ZERO;
        for _ in 0..60 {
            step(&mut b, &Intent::default(), &t, l);
            top = top.max(b.z);
        }
        assert!(
            top > Fx::HALF,
            "a jump clears half a tile: {}",
            top.to_f32()
        );
        assert!(b.grounded());
    }

    #[test]
    fn a_dash_goes_three_tiles_and_costs_breath() {
        let l = &LAWS;
        let t = floor(40);
        let mut b = Body::at(Fx::ZERO, Fx::ZERO, l);
        let dash = Intent {
            verb: Verb::Flick,
            ..Intent::default()
        };
        assert!(step(&mut b, &dash, &t, l).dashed);
        assert!(b.dodging());
        for _ in 0..4 {
            step(&mut b, &Intent::default(), &t, l);
        }
        assert!(b.x.sub(Fx::int(3)).abs() < Fx(64), "{}", b.x.to_f32());
        assert_eq!(b.breath, l.breath - l.dash_breath);
    }

    #[test]
    fn walls_stop_bodies_and_open_a_wall_kick() {
        let l = &LAWS;
        let mut t = floor(40);
        for y in -5..5 {
            t.set(2, y, Tile::new(ground::MEADOW, obj::ROCK));
        }
        let mut b = Body::at(Fx::ZERO, Fx::HALF, l);
        let dash = |h| Intent {
            verb: Verb::Flick,
            aim: h,
            ..Intent::default()
        };
        step(&mut b, &dash(0), &t, l);
        let mut slam = Fx::ZERO;
        for _ in 0..5 {
            slam = slam.max(step(&mut b, &Intent::default(), &t, l).slam);
        }
        assert_eq!(slam, l.dash_speed);
        assert!(b.kick > 0);
        let m = step(&mut b, &dash(32768), &t, l);
        assert!(m.kicked && m.dashed, "a free rebound");
    }

    #[test]
    fn walking_off_the_edge_drops_you_into_the_dark() {
        let l = &LAWS;
        let t = floor(5);
        let mut b = Body::at(Fx::int(4), Fx::HALF, l);
        let mut fell = false;
        for _ in 0..200 {
            fell |= step(&mut b, &run(0), &t, l).fell;
        }
        assert!(fell && b.mv == Move::Fallen);
        // And a jump carries you over a narrow gap.
        let mut t = floor(20);
        t.set(3, 0, Tile::default());
        let mut b = Body::at(Fx::ZERO, Fx::HALF, l);
        for k in 0..60 {
            let jump = k == 6;
            step(&mut b, &Intent { jump, ..run(0) }, &t, l);
        }
        assert!(b.mv == Move::Free && b.x > Fx::int(5), "over the gap");
    }
}
