//! A Lumen's body and how it moves: every movement rule of §7, in its
//! order, in fixed point only, so the page predicts its own body bit for
//! bit. `step` is the whole of it:
//!
//! 1. timers and breath; 2. the ground underfoot; 3. hit-stun or a
//!    spin-out (no control, flight decays); 4. a dash; 5. control (carve,
//!    accelerate, skid, drift and its slingshots); 6. the move, one axis at
//!    a time against solid tiles; 7. the void: teeter, or fall.

use engine::fixed::{atan2, len, turn, unit, Fx};

use crate::laws::Laws;
use crate::tiles::{ground, Tiles};

/// What a person (or a resident) does this tick: the only way any Lumen
/// acts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Verb {
    #[default]
    None,
    Tap,
    Flick,
    /// A hold began `held_for` ticks ago.
    Hold {
        held_for: u8,
    },
    /// A hold let go: 0 for a heavy, else a throw's range (1-255: 4-9 tiles).
    Release {
        range: u8,
    },
    Cancel,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Intent {
    /// The stick's heading, and how far it is pushed: 0 the stick is up,
    /// 1-254 a walk at that share of walk speed, 255 a run.
    pub heading: u16,
    pub throttle: u8,
    pub verb: Verb,
    /// The flick's or the aim's heading.
    pub aim: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Move {
    #[default]
    Free,
    Drift,
    Dash,
    /// Hit-stun, or a spin-out: no control while the flight decays.
    Stun,
    /// On the edge of the void, a moment from falling.
    Teeter,
    /// Gone into the Dark.
    Fallen,
}

/// What an action leaves of control: nothing, the stick (a strike: a dash
/// still cancels it), everything but a cancel (a charge roots you), or
/// nothing at all (a lunge carries you).
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
    pub vx: Fx,
    pub vy: Fx,
    pub facing: u16,
    pub mv: Move,
    /// Ticks into a drift or a dash; ticks left of stun or a teeter.
    pub t: u32,
    /// Breath in thousandths, and ticks until it regenerates.
    pub breath: i32,
    pub rest: u32,
    /// Breath regeneration, thousandths (Rekindled raises it).
    pub regen: i32,
    pub cooldown: u32,
    pub iframes: u32,
    /// A slingshot: ticks left, and its top-speed multiplier.
    pub sling: u32,
    pub sling_mul: i32,
    /// A drift with the stick up coasts this many ticks more.
    pub coast: u32,
    /// The wall-kick window, whether this wall was kicked, and a flick held
    /// from a dash's last ticks.
    pub kick: u32,
    pub kicked: bool,
    pub kick_held: Option<u16>,
    pub dash_h: u16,
    pub dash_prior: Fx,
    pub dash_void: Fx,
    /// Teetering: the way back to ground. The last place on ground.
    pub back: u16,
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

    /// Thrown: this velocity, and no control for this many ticks.
    pub fn launch(&mut self, vx: Fx, vy: Fx, ticks: u32) {
        if matches!(self.mv, Move::Fallen | Move::Teeter) {
            return;
        }
        self.vx = vx;
        self.vy = vy;
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
    /// A slingshot out of a drift: 1 blue, 2 gold.
    pub sling: u8,
    pub spun: bool,
    pub saved: bool,
    pub fell: bool,
}

/// The ground's multipliers, thousandths.
struct Underfoot {
    accel: i32,
    turn: i32,
    skid: i32,
    speed: i32,
    drift: bool,
    dash: bool,
    ice: bool,
}

fn underfoot(b: &Body, t: &Tiles, l: &Laws) -> Underfoot {
    let tile = t.under(b.x, b.y);
    let mut u = Underfoot {
        accel: 1000,
        turn: 1000,
        skid: 1000,
        speed: 1000,
        drift: true,
        dash: true,
        ice: false,
    };
    match tile.kind() {
        ground::ICE => {
            u.accel = l.ice_grip;
            u.turn = l.ice_grip;
            u.skid = l.ice_skid;
            u.ice = true;
        }
        ground::MUD => {
            u.speed = l.mud_speed;
            u.drift = false;
        }
        ground::WATER => {
            u.speed = l.water_speed;
            u.dash = false;
        }
        _ => {}
    }
    if b.claim != 0 && tile.claim & 0x7fff == b.claim {
        u.speed = u.speed * l.own_land / 1000;
    }
    u
}

fn milli(v: Fx, m: i32) -> Fx {
    Fx((v.0 as i64 * m as i64 / 1000) as i32)
}

fn rotate(from: u16, to: u16, most: i32) -> u16 {
    let d = turn(from, to).clamp(-most, most);
    from.wrapping_add(d as u16)
}

/// How fast a body at this speed may turn, heading units a tick.
fn turn_rate(speed: Fx, l: &Laws) -> i32 {
    let (slow, fast) = (l.turn_slow as i32, l.turn_fast as i32);
    if speed.0 <= l.walk.0 {
        slow
    } else if speed.0 >= l.run.0 {
        fast
    } else {
        let k = (speed.0 - l.walk.0) as i64 * 1000 / (l.run.0 - l.walk.0) as i64;
        slow - ((slow - fast) as i64 * k / 1000) as i32
    }
}

fn start_dash(b: &mut Body, h: u16, free: bool, l: &Laws, m: &mut Moved) {
    b.dash_prior = b.speed();
    b.mv = Move::Dash;
    b.t = 0;
    b.dash_h = h;
    b.dash_void = Fx::ZERO;
    b.facing = h;
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
    for c in [&mut b.cooldown, &mut b.iframes, &mut b.kick, &mut b.sling] {
        *c = c.saturating_sub(1);
    }
    if b.rest > 0 {
        b.rest -= 1;
    } else {
        b.breath = (b.breath + l.breath_regen * b.regen / 1000).min(l.breath);
    }
    // 2. The ground.
    let g = underfoot(b, t, l);
    let stick = it.throttle > 0;
    let flick = it.verb == Verb::Flick;

    // Flicks: a save, a held kick, a rebound, or a dash.
    match b.mv {
        Move::Teeter if flick => {
            if turn(it.aim, b.back).abs() <= l.save_angle as i32 && b.breath >= l.save_breath {
                let (ux, uy) = unit(b.back);
                b.x = b.x.add(ux);
                b.y = b.y.add(uy);
                b.vx = Fx::ZERO;
                b.vy = Fx::ZERO;
                b.breath -= l.save_breath;
                b.rest = l.breath_rest;
                b.mv = Move::Free;
                m.saved = true;
            }
        }
        Move::Dash if flick && b.t + l.kick_hold >= l.dash_ticks => b.kick_held = Some(it.aim),
        Move::Free | Move::Drift if flick && b.busy != Busy::Charging => {
            if b.kick > 0 && !b.kicked {
                b.kicked = true;
                start_dash(b, it.aim, true, l, &mut m);
                m.kicked = true;
            } else if b.cooldown == 0 && b.breath >= l.dash_breath && g.dash {
                start_dash(b, it.aim, false, l, &mut m);
            }
        }
        _ => {}
    }

    let mut speed = b.speed();
    let mut vh = if speed.0 == 0 {
        b.facing
    } else {
        atan2(b.vy, b.vx)
    };
    match b.mv {
        // 3. No control while the flight decays.
        Move::Stun => {
            let decay = milli(l.flight_decay, if g.ice { 150 } else { 1000 });
            speed = speed.sub(decay).max(Fx::ZERO);
            b.t = b.t.saturating_sub(1);
            if b.t == 0 {
                b.mv = Move::Free;
            }
        }
        // 4. A dash is its own velocity.
        Move::Dash => {
            vh = b.dash_h;
            speed = l.dash_speed;
        }
        Move::Teeter => {
            speed = Fx::ZERO;
            b.t = b.t.saturating_sub(1);
            if b.t == 0 {
                b.mv = Move::Fallen;
                m.fell = true;
                return m;
            }
        }
        // 5. Control.
        Move::Free | Move::Drift if b.busy == Busy::Lunging => b.mv = Move::Free,
        Move::Free | Move::Drift if b.busy != Busy::No => {
            speed = speed.sub(milli(l.skid, g.skid)).max(Fx::ZERO);
            b.mv = Move::Free;
        }
        Move::Drift => {
            if stick {
                b.facing = it.heading;
                b.coast = l.drift_coast;
            } else {
                b.coast = b.coast.saturating_sub(1);
            }
            vh = rotate(vh, b.facing, l.grip as i32 * g.turn / 1000);
            b.t += 1;
            if b.t > l.spin_at {
                b.mv = Move::Stun;
                b.t = l.spin_ticks;
                speed = Fx(speed.0 / 2);
                m.spun = true;
            } else if turn(vh, b.facing).abs() < l.drift_out as i32 {
                b.mv = Move::Free;
                if b.t >= l.gold_at {
                    (b.sling, b.sling_mul, m.sling) = (l.sling_ticks, l.gold, 2);
                } else if b.t >= l.blue_at {
                    (b.sling, b.sling_mul, m.sling) = (l.sling_ticks, l.blue, 1);
                }
            } else if b.coast == 0 {
                b.mv = Move::Free;
            }
        }
        Move::Free => {
            let h = it.heading;
            if stick && g.drift && speed >= l.drift_speed && turn(vh, h).abs() > l.drift_in as i32 {
                b.mv = Move::Drift;
                b.t = 0;
                b.coast = l.drift_coast;
                b.facing = h;
            } else {
                if stick {
                    vh = if speed < l.snap {
                        h
                    } else {
                        rotate(vh, h, turn_rate(speed, l) * g.turn / 1000)
                    };
                    b.facing = h;
                }
                let mut top = match it.throttle {
                    0 => Fx::ZERO,
                    255 => l.run,
                    n => Fx((l.walk.0 as i64 * n as i64 / 254) as i32),
                };
                let cut = (b.load as i32 / l.weight_step) * l.weight_cut;
                top = milli(top, (1000 - cut).max(l.weight_floor));
                top = milli(top, g.speed);
                if b.sling > 0 {
                    top = milli(top, b.sling_mul);
                }
                speed = if speed < top {
                    speed.add(milli(l.accel, g.accel)).min(top)
                } else {
                    speed.sub(milli(l.skid, g.skid)).max(top)
                };
            }
        }
        Move::Fallen => {}
    }
    let (ux, uy) = unit(vh);
    b.vx = ux.mul(speed);
    b.vy = uy.mul(speed);

    // 6. The move, x then y, against solid tiles.
    let before = b.speed();
    let hit = slide(b, t, l);
    if hit {
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
    let mut ended = false;
    if b.mv == Move::Dash {
        b.t += 1;
        if t.under(b.x, b.y).void() {
            b.dash_void = b.dash_void.add(l.dash_speed);
        }
        if b.t >= l.dash_ticks {
            ended = true;
            b.mv = Move::Free;
            b.cooldown = l.dash_cooldown;
            b.kick_held = None;
            let s = b.dash_prior.min(l.run);
            let (ux, uy) = unit(b.dash_h);
            b.vx = ux.mul(s);
            b.vy = uy.mul(s);
        }
    }

    // 7. The void.
    if t.under(b.x, b.y).void() {
        if b.mv == Move::Dash && b.dash_void <= l.dash_void {
            // Crossing a gap; it must end on ground.
        } else if b.mv == Move::Dash || ended {
            b.mv = Move::Fallen;
            m.fell = true;
        } else if b.speed() <= l.teeter_speed && b.mv != Move::Teeter {
            b.back = atan2(b.safe.1.sub(b.y), b.safe.0.sub(b.x));
            b.x = b.safe.0;
            b.y = b.safe.1;
            b.vx = Fx::ZERO;
            b.vy = Fx::ZERO;
            b.mv = Move::Teeter;
            b.t = l.teeter_ticks;
            b.busy = Busy::No;
        } else if b.mv != Move::Teeter {
            b.mv = Move::Fallen;
            m.fell = true;
        }
    } else if b.mv != Move::Teeter {
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
    // x
    let nx = b.x.add(b.vx);
    if b.vx.0 != 0 {
        let lead = if b.vx.0 > 0 { nx.add(r) } else { nx.sub(r) };
        let col = lead.floor();
        let rows = b.y.sub(r).add(eps).floor()..=b.y.add(r).sub(eps).floor();
        if rows.clone().any(|ty| t.solid(col, ty)) {
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
    // y
    let ny = b.y.add(b.vy);
    if b.vy.0 != 0 {
        let lead = if b.vy.0 > 0 { ny.add(r) } else { ny.sub(r) };
        let row = lead.floor();
        let cols = b.x.sub(r).add(eps).floor()..=b.x.add(r).sub(eps).floor();
        if cols.clone().any(|tx| t.solid(tx, row)) {
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
            ..Intent::default()
        }
    }

    #[test]
    fn a_run_reaches_top_speed_and_a_lift_skids_to_a_stop() {
        let l = &LAWS;
        let t = floor(40);
        let mut b = Body::at(Fx::ZERO, Fx::ZERO, l);
        for _ in 0..30 {
            step(&mut b, &run(0), &t, l);
        }
        assert_eq!(b.speed().0 / 64, l.run.0 / 64);
        let mut n = 0;
        while b.speed().0 > 0 {
            step(&mut b, &Intent::default(), &t, l);
            n += 1;
        }
        // 6 tiles/s at 15 tiles/s² stops in 0.4 s: 12 ticks.
        assert!((11..=13).contains(&n), "{n}");
    }

    #[test]
    fn a_hard_swing_drifts_and_comes_out_slingshot() {
        let l = &LAWS;
        let t = floor(40);
        let mut b = Body::at(Fx::int(-20), Fx::ZERO, l);
        for _ in 0..30 {
            step(&mut b, &run(0), &t, l);
        }
        // Swing 150 degrees.
        let h = crate::laws::deg(150);
        step(&mut b, &run(h), &t, l);
        assert_eq!(b.mv, Move::Drift);
        let mut sling = 0;
        for _ in 0..40 {
            sling = sling.max(step(&mut b, &run(h), &t, l).sling);
        }
        assert_eq!(sling, 1, "about 11 ticks of drift is a blue slingshot");
    }

    #[test]
    fn a_dash_goes_three_tiles_and_costs_breath() {
        let l = &LAWS;
        let t = floor(40);
        let mut b = Body::at(Fx::ZERO, Fx::ZERO, l);
        let flick = Intent {
            verb: Verb::Flick,
            aim: 0,
            ..Intent::default()
        };
        let m = step(&mut b, &flick, &t, l);
        assert!(m.dashed);
        assert!(b.dodging());
        for _ in 0..4 {
            step(&mut b, &Intent::default(), &t, l);
        }
        assert_eq!(b.mv, Move::Free);
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
        let flick = |h| Intent {
            verb: Verb::Flick,
            aim: h,
            ..Intent::default()
        };
        step(&mut b, &flick(0), &t, l);
        let mut slam = Fx::ZERO;
        for _ in 0..5 {
            slam = slam.max(step(&mut b, &Intent::default(), &t, l).slam);
        }
        assert!(b.x < Fx::int(2).sub(l.body).add(Fx(2)));
        assert_eq!(slam, l.dash_speed);
        assert!(b.kick > 0);
        let m = step(&mut b, &flick(32768), &t, l);
        assert!(m.kicked && m.dashed, "a free rebound");
    }

    #[test]
    fn the_edge_teeters_a_walker_and_takes_a_launch() {
        let l = &LAWS;
        let t = floor(5);
        let mut b = Body::at(Fx::int(4), Fx::HALF, l);
        let walk = Intent {
            heading: 0,
            throttle: 200,
            ..Intent::default()
        };
        let mut n = 0;
        while b.mv != Move::Teeter && n < 60 {
            step(&mut b, &walk, &t, l);
            n += 1;
        }
        assert_eq!(b.mv, Move::Teeter);
        // Flick back toward the ground: saved.
        let save = Intent {
            verb: Verb::Flick,
            aim: 32768,
            ..Intent::default()
        };
        assert!(step(&mut b, &save, &t, l).saved);
        assert_eq!(b.mv, Move::Free);
        // Launched off at speed: gone.
        b.launch(Fx::int(1), Fx::ZERO, 30);
        let mut fell = false;
        for _ in 0..10 {
            fell |= step(&mut b, &Intent::default(), &t, l).fell;
        }
        assert!(fell && b.mv == Move::Fallen);
    }
}
