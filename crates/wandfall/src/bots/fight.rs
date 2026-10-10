//! How a bot fights: which spells it casts, where it aims each (led as
//! that spell flies, a little off), and how it moves in a duel: strafing
//! and hopping about the distance it likes, and out from under Lightning.

use super::sense::Mark;
use super::walk::{self, Plan};
use super::{keys_toward, range, shake, turn, unit, Mind};
use crate::laws::*;
use crate::loot::power;
use crate::motion::{cast, keys, Input};
use crate::spells::{blink_to, Aim};
use crate::trig;
use crate::world::{Player, World};

/// The spells aimed at a mark, the first cast first when several are
/// ready in one tick (they share one aim).
const AIMED: [u8; 4] = [
    spell::LANCE,
    spell::FIREBALL,
    spell::FROST,
    spell::LIGHTNING,
];

/// The slot bits of `me`'s spells that are aimed at a mark.
fn aimed_bits(me: &Player) -> u8 {
    let mut bits = 0;
    for (k, s) in me.slots.iter().enumerate() {
        if s.is_some_and(|s| AIMED.contains(&s.spell)) {
            bits |= cast::SLOT[k];
        }
    }
    bits
}

/// The slot `me` holds `spell` in, among `bits`.
fn slot_of(me: &Player, bits: u8, spell: u8) -> Option<usize> {
    (0..4).find(|&k| bits & cast::SLOT[k] != 0 && me.slots[k].is_some_and(|s| s.spell == spell))
}

/// What `me` does this tick: where it looks, the keys it holds, the
/// spells it casts.
pub fn act(
    w: &World,
    k: usize,
    m: &mut Mind,
    plan: &Plan,
    mark: Option<&Mark>,
    tick: u32,
) -> Input {
    let me = &w.players[k];
    let mut bits = spells(w, me, mark, plan, tick, m.seed);
    let mut keys = 0;
    let (mut yaw, mut pitch);
    match mark.filter(|_| !plan.retreat) {
        Some(mk) => {
            // One spell aimed a tick (the rest wait), and the wand.
            let spell = AIMED
                .into_iter()
                .find(|&sp| slot_of(me, bits, sp).is_some());
            if let Some(sp) = spell {
                let keep = slot_of(me, bits, sp).map_or(0, |s| cast::SLOT[s]);
                bits &= !aimed_bits(me) | keep;
            }
            let (ay, ap) = aim(me, mk, spell, tick, m.seed);
            let (y, there) = turn(me.yaw, ay, BOT_AIM_TURN);
            let p = (me.pitch as i32
                + (ap as i32 - me.pitch as i32).clamp(-BOT_AIM_TURN, BOT_AIM_TURN))
                as i16;
            (yaw, pitch) = (y, p);
            // Only at a mark in sight, and once its aim is on it.
            let on = there && p == ap && mk.seen;
            if !on {
                bits &= !aimed_bits(me);
            }
            let want = range(me);
            if on && tick >= m.ready_at && (!plan.flee || mk.d < want * BOT_BAND.1) {
                keys |= keys::FIRE;
            }
            keys |= if plan.flee {
                walk::keys(plan, yaw) | keys::SPRINT
            } else {
                duel(w, me, m, mk, yaw, tick)
            };
            // Blinking away: back and to whichever side goes further.
            if !plan.flee && slot_of(me, bits, spell::BLINK).is_some() {
                keys = (keys & keys::FIRE) | escape(w, k, yaw, pitch);
            }
        }
        None => {
            // Walking: turning toward where it goes, a little at a time,
            // at a sprint; far, hopping, each hop timed to the landing (a
            // press, so let go between). Gliding down near its spot, it
            // lets the broom drop; a sparring dummy at its post waits.
            yaw = turn(me.yaw, plan.heading, BOT_TURN).0;
            pitch = 0;
            bits &= !aimed_bits(me);
            if !plan.stay {
                keys |= walk::keys(plan, yaw) | keys::SPRINT;
            }
            let b = &me.body;
            let lucky = unit(m.seed, tick, 9) < BOT_HOP_ODDS;
            if plan.far && b.ground && !b.held && (b.landed <= 1 || lucky) {
                keys |= keys::JUMP;
            }
        }
    }
    // A Tether thrown to get away (the storm, a fight it is losing) goes
    // the way it runs, up into the air; that tick it aims at nothing else.
    if (plan.flee || plan.retreat) && slot_of(me, bits, spell::TETHER).is_some() {
        (yaw, pitch) = (plan.heading, BOT_TETHER_UP);
        bits &= !aimed_bits(me);
        keys &= !keys::FIRE;
    }
    Input {
        seq: 0,
        yaw,
        pitch,
        keys,
        cast: bits,
        view: 0,
    }
}

/// Where to aim at `mk` for what leaves the wand this tick (`spell`, or
/// the wand's bolt): led by how long it takes to get there, a little off
/// (more far off, against a wizard moving across its sight or in the
/// air, and as shaky as this bot's hand).
pub(super) fn aim(me: &Player, mk: &Mark, spell: Option<u8>, tick: u32, seed: u64) -> (u16, i16) {
    let eye = me.eye();
    let (lead, low) = match spell {
        // Instant: only the tick it takes.
        Some(spell::LANCE) => (DT, 0.0),
        // Low, so a near miss still bursts by them.
        Some(spell::FIREBALL) => (mk.d / FIREBALL_SPEED, BOT_FIREBALL_LOW),
        Some(spell::FROST) => (mk.d / FROST_SPEED, 0.0),
        // At their feet, where they will be (some of the way: they may
        // turn).
        Some(spell::LIGHTNING) => (
            LIGHTNING_DELAY as f32 * DT * BOT_LIGHTNING_LEAD,
            mk.chest[1] - mk.feet[1],
        ),
        _ => (mk.d / BOLT_SPEED, 0.0),
    };
    let to = [
        mk.chest[0] + mk.v[0] * lead,
        mk.chest[1] - low + mk.v[1] * lead * BOT_LEAD_UP,
        mk.chest[2] + mk.v[2] * lead,
    ];
    let (dx, dy, dz) = (to[0] - eye[0], to[1] - eye[1], to[2] - eye[2]);
    let flat = dx.hypot(dz).max(1e-3);
    let (ux, uz) = (dx / flat, dz / flat);
    let along = mk.v[0] * ux + mk.v[2] * uz;
    let side = (mk.v[0] - along * ux).hypot(mk.v[2] - along * uz);
    let air = if mk.ground { 0.0 } else { BOT_AIR_ERROR };
    let per = BOT_AIM_ERROR + BOT_LEAD_ERROR * side / RUN + air;
    let miss = (BOT_MISS_NEAR + mk.d * per) * shake(seed);
    let off = miss / mk.d.max(1.0);
    let jy = (unit(seed, tick / 4, 4) - 0.5) * 2.0 * off;
    let jp = (unit(seed, tick / 4, 5) - 0.5) * 2.0 * off;
    (
        trig::heading(dz.atan2(dx) + jy),
        trig::pitch(dy.atan2(flat) + jp),
    )
}

/// A duel's footwork, facing `yaw` at `mk`: strafing (some legs hopped,
/// each hop timed to the landing), stepping in or back to the distance it
/// likes, and out from under Lightning about to strike.
fn duel(w: &World, me: &Player, m: &mut Mind, mk: &Mark, yaw: u16, tick: u32) -> u16 {
    if tick >= m.strafe_until {
        m.strafe = if unit(m.seed, tick, 6) < 0.5 { -1 } else { 1 };
        m.strafe_until = tick + BOT_STRAFE + (unit(m.seed, tick, 7) * BOT_STRAFE_MORE) as u32;
    }
    let mut keys = if m.strafe > 0 {
        keys::RIGHT
    } else {
        keys::LEFT
    };
    let want = range(me);
    if mk.d > want * BOT_BAND.1 {
        keys |= keys::FWD;
    } else if mk.d < want * BOT_BAND.0 {
        keys |= keys::BACK;
    }
    let b = &me.body;
    let hopping = unit(m.seed, m.strafe_until, 10) < BOT_DUEL_HOP;
    if b.ground && !b.held && ((hopping && b.landed <= 1) || unit(m.seed, tick, 8) < BOT_JUMP_ODDS)
    {
        keys |= keys::JUMP;
    }
    let p = b.p;
    let under = w.zones.iter().find(|z| {
        z.by != me.id
            && z.land.saturating_sub(tick) <= BOT_DODGE
            && (z.at[0] - p[0]).hypot(z.at[2] - p[2]) < LIGHTNING_RADIUS + BOT_DODGE_MARGIN
    });
    if let Some(z) = under {
        let away = trig::heading((p[2] - z.at[2]).atan2(p[0] - z.at[0]));
        keys = keys_toward(yaw, away) | if b.held { 0 } else { keys::JUMP };
    }
    keys
}

/// The keys that Blink `k` away from its foe, facing it: back and to
/// one side, whichever side has room to go further.
fn escape(w: &World, k: usize, yaw: u16, pitch: i16) -> u16 {
    let me = &w.players[k];
    let rank = me
        .slots
        .iter()
        .flatten()
        .find(|s| s.spell == spell::BLINK)
        .map_or(1, |s| s.rank);
    let reach = power(spell::BLINK, rank) as f32;
    let far = |keys| {
        let aim = Aim {
            eye: me.eye(),
            yaw,
            pitch,
            keys,
            behind: 0,
        };
        blink_to(w, k, aim, reach)
            .map_or(0.0, |at| (at[0] - me.body.p[0]).hypot(at[2] - me.body.p[2]))
    };
    [
        keys::BACK | keys::RIGHT,
        keys::BACK | keys::LEFT,
        keys::BACK,
    ]
    .into_iter()
    .max_by(|a, b| far(*a).total_cmp(&far(*b)))
    .unwrap_or(keys::BACK)
}

/// An enemy's Fireball or Frost coming at `me`, near enough to blow away.
fn incoming(w: &World, me: &Player) -> bool {
    let p = me.body.p;
    w.bolts.iter().any(|b| {
        let d = [p[0] - b.p[0], p[1] + 1.0 - b.p[1], p[2] - b.p[2]];
        let near = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() < GUST_RADIUS;
        let closing = d[0] * b.v[0] + d[1] * b.v[1] + d[2] * b.v[2] > 0.0;
        b.by != me.id && (b.kind == spell::FIREBALL || b.kind == spell::FROST) && near && closing
    })
}

/// Which spells a bot casts now: at its mark (each from the distance it
/// is good at; Lightning at one still, warded, mending or on the ground,
/// not in the air), to save itself when hurt, to blow away what comes at
/// it, to run when the storm comes or a fight is lost, or to chase.
fn spells(w: &World, me: &Player, mark: Option<&Mark>, plan: &Plan, tick: u32, seed: u64) -> u8 {
    let mut bits = 0;
    let hurt = me.hp * 100 / me.max_hp().max(1);
    let struck = tick.saturating_sub(me.hurt_at) < BOT_STRUCK;
    let seen = mark.filter(|mk| mk.seen);
    let d = seen.map(|mk| mk.d);
    for (k, slot) in me.slots.iter().enumerate() {
        let Some(s) = slot else {
            continue;
        };
        if me.cds[k] > 0 || unit(seed, tick, 20 + k as u64) > BOT_CAST_ODDS {
            continue;
        }
        let (lo, hi) = BOT_CAST_LIGHTNING;
        let go = match (s.spell, d) {
            (spell::LANCE, Some(d)) => d < BOT_CAST_LANCE,
            (spell::FIREBALL, Some(d)) => d < BOT_CAST_FIREBALL,
            (spell::FROST, Some(d)) => d < BOT_CAST_FROST,
            (spell::LIGHTNING, Some(d)) => {
                d > lo && d < hi && seen.is_some_and(|mk| mk.still || mk.ground)
            }
            (spell::GUST, _) => d.is_some_and(|d| d < GUST_RADIUS) || incoming(w, me),
            (spell::WARD, _) => struck && hurt < BOT_WARD_HP,
            (spell::MEND, _) => hurt < BOT_MEND_HP,
            (spell::BLINK, _) => {
                plan.flee || (d.is_some() && (me.body.chill > 0 || (struck && hurt < BOT_BLINK_HP)))
            }
            // Away; or after a mark beyond its reach (the air holds it
            // short of them), for those who fight close.
            (spell::TETHER, d) => {
                let reach = power(spell::TETHER, s.rank) as f32;
                plan.flee || plan.retreat || d.is_some_and(|d| d > reach && range(me) <= BOT_RANGE)
            }
            _ => false,
        };
        if go {
            bits |= cast::SLOT[k];
        }
    }
    bits
}
