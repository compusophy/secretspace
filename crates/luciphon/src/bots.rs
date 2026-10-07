//! Residents, v0: eight sparring Lumens with no homes. They run, drift,
//! chase, strike, charge heavies and throw, dodge with a fan of headings
//! clear of void, brambles and walls, and flee when they burn low. They
//! see the world 250-400 ms late (the history ring, 8-12 ticks back), aim
//! 6-12 degrees off, and act only through Intents a thumb could make: the
//! same grammar (`thumb`) and the same one-a-tick rate as a person.

use engine::fixed::{atan2, len, turn, unit, Fx};

use crate::combat::Act;
use crate::laws::deg;
use crate::motion::{Intent, Move, Verb};
use crate::tiles::obj;
use crate::world::World;

pub const RESIDENTS: usize = 8;

/// Names residents go by: they look typed, as people's do.
pub const NAMES: &[&str] = &[
    "moth", "ember", "wick", "lumi", "sol", "faye", "oro", "bram", "kiln", "nyx", "tallow", "ash",
    "vela", "pip", "corvo", "mira", "juno", "rook",
];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Brain {
    /// 0-255: how readily it fights.
    pub nerve: u8,
    /// How late it sees, in ticks, and how far off it aims.
    pub lag: u8,
    pub err: i16,
    pub target: u16,
    pub goal: (Fx, Fx),
    /// The stick was down last tick; a hold is under way, and its release.
    pub stick: bool,
    pub holding: bool,
    /// The charge, in ticks, it lets go at.
    pub charge_to: u32,
    pub throw_range: u8,
    /// A verb it means to make once the stick is up.
    pub next: Option<Verb>,
    pub rethink: u32,
    pub seq: u16,
}

impl Brain {
    pub fn new(rng: &mut engine::rng::Rng) -> Brain {
        Brain {
            nerve: 80 + rng.below(176) as u8,
            lag: 8 + rng.below(5) as u8,
            err: (deg(6) + rng.below(deg(6) as u64) as u16) as i16,
            ..Brain::default()
        }
    }
}

/// Whether a heading from (x, y) is clear for `ahead` tiles.
fn clear(w: &World, x: Fx, y: Fx, h: u16, ahead: i32) -> bool {
    let (ux, uy) = unit(h);
    (1..=ahead * 2).all(|k| {
        let d = Fx::HALF.mul_int(k);
        let t = w.tiles.under(x.add(ux.mul(d)), y.add(uy.mul(d)));
        !t.void() && !t.solid() && t.obj != obj::BRAMBLE
    })
}

/// The heading nearest `h` that is clear, fanning out both ways.
fn fan(w: &World, x: Fx, y: Fx, h: u16, ahead: i32) -> u16 {
    for k in 0..8 {
        for s in [1i32, -1] {
            let c = h.wrapping_add((s * k * deg(20) as i32) as u16);
            if clear(w, x, y, c, ahead) {
                return c;
            }
            if k == 0 {
                break;
            }
        }
    }
    h.wrapping_add(32768)
}

/// Every resident decides its Intent for this tick.
pub fn think(w: &mut World) {
    let tick = w.tick;
    for i in 0..w.lumens.len() {
        if w.lumens[i].bot.is_none() || !w.lumens[i].alive() {
            continue;
        }
        let it = decide(w, i, tick);
        let l = &mut w.lumens[i];
        if let Some(b) = l.bot.as_mut() {
            b.seq = b.seq.wrapping_add(1);
            b.stick = it.throttle > 0;
            match it.verb {
                Verb::Hold { .. } => b.holding = true,
                Verb::Release { .. } | Verb::Cancel => b.holding = false,
                _ => {}
            }
            let seq = b.seq;
            l.queue.push_back((seq, it));
        }
    }
}

fn decide(w: &mut World, i: usize, tick: u32) -> Intent {
    let me = &w.lumens[i];
    let brain = me.bot.clone().unwrap_or_default();
    let (x, y) = me.pos();
    let body = me.me.body;
    let lag = brain.lag as usize;
    let low = me.flame < w.laws.flame * 3 / 10;
    let mut rng_roll = || w.rng.below(1000) as u32;
    let roll = rng_roll();
    let roll2 = rng_roll();

    // Charging: let go at the planned tick, aimed at where it saw them.
    if brain.holding {
        let aim = seen_heading(w, i, brain.target, lag).unwrap_or(body.facing);
        let aim = aim.wrapping_add(brain.err as u16);
        if me.me.act.act != Act::Charge {
            return Intent {
                verb: Verb::Cancel,
                aim,
                ..Intent::default()
            };
        }
        if me.me.act.c >= brain.charge_to {
            return Intent {
                verb: Verb::Release {
                    range: brain.throw_range,
                },
                aim,
                ..Intent::default()
            };
        }
        return Intent {
            aim,
            ..Intent::default()
        };
    }
    if matches!(body.mv, Move::Stun | Move::Fallen) || me.down > 0 {
        return Intent::default();
    }
    // On the edge: flick back.
    if body.mv == Move::Teeter {
        mind(w, i).next = None;
        return if brain.stick {
            Intent {
                verb: Verb::Flick,
                aim: body.back,
                ..Intent::default()
            }
        } else {
            Intent {
                heading: body.back,
                throttle: 255,
                ..Intent::default()
            }
        };
    }
    // A verb waiting for the stick to come up.
    if let Some(v) = brain.next {
        if v == Verb::Flick && !brain.stick {
            // A flick ends a drag: drag first.
            return Intent {
                heading: body.facing,
                throttle: 255,
                ..Intent::default()
            };
        }
        if !brain.stick {
            mind(w, i).next = None;
            return Intent {
                verb: v,
                aim: body.facing,
                ..Intent::default()
            };
        }
        if v == Verb::Flick {
            mind(w, i).next = None;
            return Intent {
                verb: Verb::Flick,
                aim: body.facing,
                ..Intent::default()
            };
        }
        // Lift the stick first (a skid), then the verb.
        return Intent::default();
    }

    // Who is near, as it saw them.
    let mut nearest: Option<(u16, Fx, Fx, Fx)> = None;
    for o in &w.lumens {
        if o.id == me.id || !o.alive() || o.ghost > 0 || o.down > 0 {
            continue;
        }
        let Some((ox, oy)) = w.was(o.id, lag) else {
            continue;
        };
        let d = len(ox.sub(x), oy.sub(y));
        let engaged = me.engaged_with(o.id, tick);
        let d = if engaged { d.sub(Fx::int(3)) } else { d };
        if d < Fx::int(8) && nearest.is_none_or(|n| d < n.1) {
            nearest = Some((o.id, d, ox, oy));
        }
    }

    // Danger: someone close winding up toward it. Dodge sideways.
    for o in &w.lumens {
        if o.id == me.id || !o.alive() {
            continue;
        }
        let (ox, oy) = o.pos();
        let d = len(ox.sub(x), oy.sub(y));
        let winding = matches!(o.me.act.act, Act::Windup | Act::Charge);
        let toward = turn(o.me.act.aim, atan2(y.sub(oy), x.sub(ox))).abs() < deg(40) as i32;
        if winding
            && toward
            && d < Fx::milli(1_700)
            && body.breath >= w.laws.dash_breath
            && roll < 120 + brain.nerve as u32
        {
            let side =
                atan2(y.sub(oy), x.sub(ox)).wrapping_add(if roll2 < 500 { 16384 } else { 49152 });
            let h = fan(w, x, y, side, 3);
            if brain.stick {
                return Intent {
                    verb: Verb::Flick,
                    aim: h,
                    ..Intent::default()
                };
            }
            mind(w, i).next = Some(Verb::Flick);
            return Intent {
                heading: h,
                throttle: 255,
                ..Intent::default()
            };
        }
    }

    // Burning low: run for the Sanctum.
    if low {
        let home = atan2(y.neg(), x.neg());
        let h = fan(w, x, y, home, 3);
        return Intent {
            heading: h,
            throttle: 255,
            ..Intent::default()
        };
    }

    if let Some((tid, d, ox, oy)) = nearest.filter(|_| brain.nerve as u32 * 4 > roll / 4) {
        mind(w, i).target = tid;
        let toward = atan2(oy.sub(y), ox.sub(x)).wrapping_add(brain.err as u16);
        let glim = w.lumens[i].glim;
        // Close: strike (lift the stick, then tap), or charge a heavy.
        if d < Fx::milli(1_150) {
            if roll < 25 + brain.nerve as u32 / 4 {
                // Nearly half its heavies are let go in the perfect window.
                let to = if roll2 < 450 {
                    19 + roll2 % 3
                } else {
                    12 + roll2 % 20
                };
                {
                    let b = mind(w, i);
                    b.next = Some(Verb::Hold { held_for: 9 });
                    b.charge_to = to;
                    b.throw_range = 0;
                }
            } else if roll < 400 {
                mind(w, i).next = Some(Verb::Tap);
            }
            return Intent {
                heading: toward,
                throttle: if brain.stick { 0 } else { 60 },
                ..Intent::default()
            };
        }
        // In range for a throw.
        if glim >= w.laws.throw_glim && d > Fx::int(4) && d < Fx::int(8) && roll < 12 {
            let span = (d.sub(Fx::int(4)).0 as i64 * 254 / Fx::int(5).0 as i64) as u8;
            {
                let b = mind(w, i);
                b.next = Some(Verb::Hold { held_for: 9 });
                b.charge_to = 14 + roll2 % 10;
                b.throw_range = span.max(1);
            }
            return Intent::default();
        }
        // Chase.
        let h = fan(w, x, y, toward, 2);
        return Intent {
            heading: h,
            throttle: 255,
            ..Intent::default()
        };
    }

    // Wander toward a goal on the island.
    let reached = len(brain.goal.0.sub(x), brain.goal.1.sub(y)) < Fx::int(2);
    if reached || tick >= brain.rethink {
        let h = w.rng.below(65536) as u16;
        // Mostly the Glow and the Dim; now and then out to the Rim.
        let r = Fx::int(6 + w.rng.below(50) as i32);
        let (ux, uy) = unit(h);
        let goal = (ux.mul(r), uy.mul(r));
        let rethink = tick + 150 + w.rng.below(300) as u32;
        let b = mind(w, i);
        b.goal = goal;
        b.rethink = rethink;
    }
    let goal = w.lumens[i].bot.as_ref().map_or((x, y), |b| b.goal);
    let h = fan(w, x, y, atan2(goal.1.sub(y), goal.0.sub(x)), 3);
    Intent {
        heading: h,
        throttle: if roll < 300 { 160 } else { 255 },
        ..Intent::default()
    }
}

/// A resident's mind (every resident has one).
fn mind(w: &mut World, i: usize) -> &mut Brain {
    w.lumens[i].bot.get_or_insert_with(Brain::default)
}

/// The heading toward another Lumen as this one saw it `lag` ticks ago.
fn seen_heading(w: &World, i: usize, target: u16, lag: usize) -> Option<u16> {
    let (x, y) = w.lumens[i].pos();
    let (ox, oy) = w.was(target, lag)?;
    Some(atan2(oy.sub(y), ox.sub(x)))
}
