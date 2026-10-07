//! A Lumen's own actions, the part the page predicts: a tap's strike (wind
//! up, land, recover), a lance out of a dash, a hold's charge and its
//! release (a heavy lunge, or a throw), the skid strike after a slow lift.
//! `control` turns one Intent into the action's next tick and the body's
//! step; what it would hit is a `Swing` for the world to settle (`hits`).

use engine::fixed::{unit, Fx};

use crate::laws::Laws;
use crate::motion::{self, Body, Busy, Intent, Move, Moved, Verb};
use crate::tiles::Tiles;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Act {
    #[default]
    Idle,
    Windup,
    Active,
    Recover,
    Charge,
    Lunge,
}

impl Act {
    pub fn code(self) -> u8 {
        self as u8
    }

    pub fn from_code(c: u8) -> Option<Act> {
        Some(match c {
            0 => Act::Idle,
            1 => Act::Windup,
            2 => Act::Active,
            3 => Act::Recover,
            4 => Act::Charge,
            5 => Act::Lunge,
            _ => return None,
        })
    }
}

/// The action under way.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Action {
    pub act: Act,
    /// Ticks left in this phase.
    pub t: u32,
    /// The charge, in ticks from the press.
    pub c: u32,
    /// Where the action points.
    pub aim: u16,
    /// It already landed (a lunge lands once).
    pub landed: bool,
    /// A slow lift this many ticks ago, at this speed (the skid strike).
    pub lift: Option<(u32, Fx)>,
    /// The speed a strike carries: the lift's, if it came soon after one.
    pub carry: Fx,
    /// The stick was down last tick.
    pub stick: bool,
    /// Ticks left in which a tap after a dash is still a lance.
    pub late: u32,
}

/// What a Lumen's own tick would do to others, for the world to settle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Swing {
    /// The strike's active tick.
    Strike {
        carry: Fx,
    },
    Lance,
    /// A lunge tick, at this charge, until it lands.
    Heavy {
        c: u32,
    },
    /// A mote let go at this charge and range (1-255).
    Throw {
        c: u32,
        range: u8,
    },
}

/// Everything about a Lumen the page predicts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Me {
    pub body: Body,
    pub act: Action,
}

/// One tick of a Lumen's own control: the Intent's verb, the action's
/// timers, and the body's step. `glim` decides whether a throw can fly;
/// at Flow 3 a charge starts full.
pub fn control(
    me: &mut Me,
    it: &Intent,
    glim: u32,
    flow: u8,
    t: &Tiles,
    l: &Laws,
) -> (Moved, Option<Swing>) {
    let (b, a) = (&mut me.body, &mut me.act);
    let mut swing = None;
    if matches!(b.mv, Move::Stun | Move::Teeter | Move::Fallen) {
        // Struck, or on the edge: whatever was under way is lost.
        *a = Action {
            stick: it.throttle > 0,
            ..Action::default()
        };
    }
    // The skid strike: a slow lift remembers its speed for a moment.
    let stick = it.throttle > 0;
    if let Some((n, _)) = &mut a.lift {
        *n += 1;
    }
    if a.lift.is_some_and(|(n, _)| n > l.skid_carry) {
        a.lift = None;
    }
    if a.stick && !stick && it.verb != Verb::Flick {
        a.lift = Some((0, b.speed()));
    }
    a.stick = stick;
    if b.mv == Move::Dash {
        a.late = l.lance_late + 1;
    } else {
        a.late = a.late.saturating_sub(1);
    }

    match it.verb {
        Verb::Tap if b.mv == Move::Dash || (a.late > 0 && a.act == Act::Idle) => {
            swing = Some(Swing::Lance);
            a.act = Act::Recover;
            a.t = l.recovery;
            a.aim = if b.mv == Move::Dash {
                b.dash_h
            } else {
                b.facing
            };
        }
        Verb::Tap if a.act == Act::Idle => {
            a.act = Act::Windup;
            a.t = l.windup;
            a.aim = b.facing;
            a.carry = a.lift.map_or(b.speed(), |(_, s)| s);
            a.lift = None;
        }
        Verb::Hold { held_for } if a.act == Act::Idle && b.mv != Move::Dash => {
            a.act = Act::Charge;
            a.c = if flow >= 3 {
                l.charge_full
            } else {
                held_for as u32
            };
            a.aim = it.aim;
        }
        Verb::Release { range } if a.act == Act::Charge => {
            let c = a.c;
            a.aim = it.aim;
            if c < l.charge_min {
                a.act = Act::Idle;
            } else if c > l.charge_max {
                a.act = Act::Windup;
                a.t = l.windup;
                a.carry = Fx::ZERO;
            } else if range > 0 && glim >= l.throw_glim {
                swing = Some(Swing::Throw { c, range });
                a.act = Act::Recover;
                a.t = l.recovery;
            } else {
                a.act = Act::Lunge;
                a.t = l.lunge_ticks;
                a.landed = false;
            }
        }
        Verb::Cancel if a.act == Act::Charge => a.act = Act::Idle,
        _ => {}
    }

    // The action's own clock.
    match a.act {
        Act::Idle => {}
        Act::Charge => {
            a.c = a.c.saturating_add(1);
            if it.verb == Verb::None && stick {
                a.aim = it.aim;
            }
        }
        Act::Windup => {
            a.t = a.t.saturating_sub(1);
            if a.t == 0 {
                a.act = Act::Active;
                a.t = l.active;
                swing = Some(Swing::Strike { carry: a.carry });
            }
        }
        Act::Active => {
            a.t = a.t.saturating_sub(1);
            if a.t == 0 {
                a.act = Act::Recover;
                a.t = l.recovery;
            }
        }
        Act::Recover => {
            a.t = a.t.saturating_sub(1);
            if a.t == 0 {
                a.act = Act::Idle;
            }
        }
        Act::Lunge => {
            if !a.landed && swing.is_none() {
                swing = Some(Swing::Heavy { c: a.c });
            }
            let (ux, uy) = unit(a.aim);
            let v = l.lunge.div_int(l.lunge_ticks.max(1) as i32);
            b.vx = ux.mul(v);
            b.vy = uy.mul(v);
            b.facing = a.aim;
            a.t = a.t.saturating_sub(1);
            if a.t == 0 {
                a.act = Act::Recover;
                a.t = l.heavy_recovery;
            }
        }
    }
    b.busy = match a.act {
        Act::Idle => Busy::No,
        Act::Charge => Busy::Charging,
        Act::Lunge => Busy::Lunging,
        _ => Busy::Striking,
    };
    let moved = motion::step(b, it, t, l);
    if moved.dashed {
        // A dash cancels a strike's recovery (and anything else).
        a.act = Act::Idle;
        a.t = 0;
    }
    (moved, swing)
}

/// Damage in thousandths and knockback speed for a hit, before the
/// target's own multiplier.
pub fn heavy(c: u32, l: &Laws) -> (i32, Fx, bool) {
    let perfect = (l.charge_full..=l.perfect_to).contains(&c);
    let k = (c.clamp(l.charge_min, l.charge_full) - l.charge_min) as i32;
    let span = (l.charge_full - l.charge_min).max(1) as i32;
    let dmg = l.heavy_low + (l.heavy_full - l.heavy_low) * k / span;
    let kb = l
        .heavy_kb_low
        .add(l.heavy_kb_full.sub(l.heavy_kb_low).mul_int(k).div_int(span));
    if perfect {
        (
            l.perfect,
            Fx((kb.0 as i64 * l.perfect_kb as i64 / 1000) as i32),
            true,
        )
    } else {
        (dmg, kb, false)
    }
}

/// A throw's damage in thousandths, whether it pierces, and its range.
pub fn throw(c: u32, range: u8, l: &Laws) -> (i32, bool, Fx) {
    let perfect = (l.charge_full..=l.perfect_to).contains(&c);
    let k = (c.clamp(l.charge_min, l.charge_full) - l.charge_min) as i32;
    let span = (l.charge_full - l.charge_min).max(1) as i32;
    let dmg = if perfect {
        l.throw_perfect
    } else {
        l.throw_low + (l.throw_full - l.throw_low) * k / span
    };
    let reach = l.throw_near.add(
        l.throw_far
            .sub(l.throw_near)
            .mul_int(range.max(1) as i32 - 1)
            .div_int(254),
    );
    (dmg, perfect, reach)
}

/// The knockback multiplier, thousandths, of a target at this Flame.
pub fn kb_mul(flame: i32, l: &Laws) -> i32 {
    1000 + 2 * 1000 * (l.flame - flame.clamp(0, l.flame)) / l.flame
}

/// Ticks of hit-stun for a knockback speed.
pub fn stun(kb: Fx) -> u32 {
    let tps = ((kb.0 as i64 * crate::laws::HZ as i64 + 32768) / 65536) as u32;
    4 + tps / 2
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::laws::LAWS;
    use crate::tiles::{ground, obj, Tile};

    fn floor() -> Tiles {
        let mut t = Tiles::default();
        for y in -20..20 {
            for x in -20..20 {
                t.set(x, y, Tile::new(ground::MEADOW, obj::NONE));
            }
        }
        t
    }

    fn verb(v: Verb, aim: u16) -> Intent {
        Intent {
            verb: v,
            aim,
            ..Intent::default()
        }
    }

    #[test]
    fn a_tap_winds_up_lands_and_recovers() {
        let (l, t) = (&LAWS, floor());
        let mut me = Me {
            body: Body::at(Fx::ZERO, Fx::ZERO, l),
            ..Me::default()
        };
        let mut swings = vec![];
        let (_, s) = control(&mut me, &verb(Verb::Tap, 0), 0, 0, &t, l);
        swings.extend(s);
        for _ in 0..20 {
            let (_, s) = control(&mut me, &Intent::default(), 0, 0, &t, l);
            swings.extend(s);
        }
        assert_eq!(swings.len(), 1);
        assert!(matches!(swings[0], Swing::Strike { .. }));
        assert_eq!(me.act.act, Act::Idle);
    }

    #[test]
    fn a_hold_released_in_time_lunges_and_early_does_nothing() {
        let (l, t) = (&LAWS, floor());
        let mut me = Me {
            body: Body::at(Fx::ZERO, Fx::ZERO, l),
            ..Me::default()
        };
        control(&mut me, &verb(Verb::Hold { held_for: 9 }, 0), 0, 0, &t, l);
        assert_eq!(me.body.busy, Busy::Charging);
        // Released at 10 ticks: too soon.
        control(&mut me, &verb(Verb::Release { range: 0 }, 0), 0, 0, &t, l);
        assert_eq!(me.act.act, Act::Idle);
        control(&mut me, &verb(Verb::Hold { held_for: 9 }, 0), 0, 0, &t, l);
        for _ in 0..9 {
            control(&mut me, &Intent::default(), 0, 0, &t, l);
        }
        let (_, s) = control(&mut me, &verb(Verb::Release { range: 0 }, 0), 0, 0, &t, l);
        assert!(matches!(s, Some(Swing::Heavy { c: 19 })), "{s:?}");
        let x0 = me.body.x;
        for _ in 0..4 {
            control(&mut me, &Intent::default(), 0, 0, &t, l);
        }
        assert!(me.body.x.sub(x0) > Fx::int(1), "a lunge carries forward");
        assert!(heavy(19, l).2, "c 19 is perfect");
        assert_eq!(heavy(12, l).0, l.heavy_low);
        // A throw needs glim; without, it is a heavy along the aim.
        control(&mut me, &Intent::default(), 0, 0, &t, l);
        for _ in 0..20 {
            control(&mut me, &Intent::default(), 0, 0, &t, l);
        }
        control(&mut me, &verb(Verb::Hold { held_for: 13 }, 0), 5, 0, &t, l);
        let (_, s) = control(&mut me, &verb(Verb::Release { range: 200 }, 0), 5, 0, &t, l);
        assert!(matches!(s, Some(Swing::Throw { range: 200, .. })), "{s:?}");
    }

    #[test]
    fn knockback_grows_as_flame_falls() {
        let l = &LAWS;
        assert_eq!(kb_mul(l.flame, l), 1000);
        assert_eq!(kb_mul(25_000, l), 2500);
        assert_eq!(kb_mul(0, l), 3000);
        assert_eq!(stun(crate::laws::tps(8_000)), 8);
    }
}
