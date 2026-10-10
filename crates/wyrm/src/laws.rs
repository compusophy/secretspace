//! Every number that makes the arena feel the way it does, in one place.

/// Server ticks a second. Every tick each snake moves one or two steps.
pub const TICK_HZ: u32 = 20;
/// The arena is a disc of this radius around (0, 0), in world units.
pub const ARENA: f32 = 2400.0;
/// Distance a snake travels in one step; body points are this far apart.
pub const STEP: f32 = 9.0;
/// Steps a tick at normal speed and while boosting.
pub const STEPS: u8 = 1;
pub const BOOST_STEPS: u8 = 2;

/// Mass a new snake starts with.
pub const START_MASS: f32 = 12.0;
/// Below this a snake cannot boost.
pub const MIN_BOOST_MASS: f32 = 20.0;
/// Of the mass burned boosting, how much is dropped as food behind.
pub const BOOST_DROP: f32 = 0.6;
/// Of a dead snake's mass, how much is left on the ground as food.
pub const DEATH_DROP: f32 = 0.75;

/// Mass burned per boosting tick by a snake of this mass. It grows with
/// size, so a big snake pays for every lap it races round someone (about
/// 3% of itself a second), and a small one can still dart (about 8%).
pub fn boost_cost(mass: f32) -> f32 {
    0.25 + mass.max(0.0) * 0.0015
}

/// How far past its body a head eats, in world units.
pub const MOUTH_REACH: f32 = 14.0;
/// A head hits a body when their centres are closer than this share of
/// their radii added: below 1, brushing past is forgiven.
pub const HIT_FORGIVE: f32 = 0.72;

/// Natural food kept on the ground, and how fast it grows back.
pub const FOOD_TARGET: usize = 1100;
pub const FOOD_REGROW: usize = 12;
/// Food that grows on its own is worth 1 up to this.
pub const NATURAL_FOOD_MAX: u8 = 3;
/// The biggest value one pellet holds.
pub const FOOD_MAX: u8 = 24;
/// Dropped food (from boosting or a burst) rots after this many ticks.
pub const ROT_TICKS: u32 = 60 * TICK_HZ;
/// A burst's pellets take the snake's hue, give or take this much (of 256).
pub const BURST_HUES: u8 = 24;

/// A new snake is placed this far (a share of the arena's radius) from
/// the middle at most, at the clearest of this many tries, and stops
/// trying once a spot is this far from every body.
pub const SPAWN_RING: f32 = 0.7;
pub const SPAWN_TRIES: u32 = 24;
pub const SPAWN_CLEAR: f32 = 320.0;

/// Snakes in the arena when it is quiet: bots fill up to this, and leave
/// as people arrive.
pub const CROWD: usize = 18;
/// The fewest bots kept, however busy it gets.
pub const MIN_BOTS: usize = 4;
/// Ticks a dead bot waits before a new one takes its place.
pub const BOT_RESPAWN: u32 = 60;
/// A bot leaving to make room goes unseen if it can: one this far from
/// every person's head is out of their sight.
pub const BOT_UNSEEN: f32 = 1500.0;

// How bots behave.

/// Bots wander like this when nothing calls: how fast the wish drifts (a
/// tick) and how far it swings either way (radians).
pub const BOT_MEANDER: f32 = 0.035;
pub const BOT_SWING: f32 = 0.45;
/// How far a bot looks for food.
pub const BOT_FOOD_SIGHT: f32 = 360.0;
/// A bot of at least `BOT_BOOST_MASS` may race for a pellet worth at least
/// `BOT_BOOST_FOOD` within `BOT_BOOST_REACH`: one time in `BOT_BOOST_ODDS`,
/// decided afresh every `BOT_BOOST_HOLD` ticks, so it boosts in runs.
pub const BOT_BOOST_FOOD: u8 = 6;
pub const BOT_BOOST_REACH: f32 = 240.0;
pub const BOT_BOOST_MASS: f32 = 60.0;
pub const BOT_BOOST_ODDS: u64 = 3;
pub const BOT_BOOST_HOLD: u32 = 10;
/// Bots this bold (nerve, 0..=255) and this heavy hunt: they cut across
/// the path of a snake under `BOT_PREY_SHARE` of their mass whose head is
/// within `BOT_PREY_SIGHT`, aiming `BOT_PREY_LEAD` plus three of its radii
/// ahead of it, and boost once within `BOT_PREY_BOOST`.
pub const BOT_HUNT_NERVE: u8 = 200;
pub const BOT_HUNT_MASS: f32 = 80.0;
pub const BOT_PREY_SHARE: f32 = 0.8;
pub const BOT_PREY_SIGHT: f32 = 320.0;
pub const BOT_PREY_LEAD: f32 = 60.0;
pub const BOT_PREY_BOOST: f32 = 220.0;
/// Past this share of the arena's radius a bot turns for home, fully by
/// `BOT_EDGE + BOT_EDGE_TURN`.
pub const BOT_EDGE: f32 = 0.82;
pub const BOT_EDGE_TURN: f32 = 0.12;
/// How far ahead a bot checks its way is clear: this, five of its radii,
/// and as much again while boosting.
pub const BOT_LOOK: f32 = 90.0;
/// When it is not, it tries this many headings either side, this far
/// apart (radians).
pub const BOT_FAN: u32 = 9;
pub const BOT_FAN_STEP: f32 = 0.33;

pub const MAX_NAME: usize = engine::who::MAX_NAME;

/// A person's new snake is a ghost this long: nothing can kill it and it
/// kills nothing, while they find their bearings.
pub const GHOST_TICKS: u32 = 3 * TICK_HZ;
/// After a restart, a person's snake waits this long for them, frozen and
/// harmless, then bursts; coming back, it is a ghost this long.
pub const HOLD_TICKS: u32 = 30 * TICK_HZ;
pub const RESUME_GHOST: u32 = 2 * TICK_HZ;

/// Body length, in points, of a snake of this mass.
pub fn body_len(mass: f32) -> usize {
    (8.0 + mass * 0.35).min(1400.0) as usize
}

/// Body radius of a snake of this mass.
pub fn radius(mass: f32) -> f32 {
    (8.0 + mass.max(0.0).sqrt() * 0.6).min(42.0)
}

/// The most a snake of this radius can turn in one step, in radians.
pub fn turn(radius: f32) -> f32 {
    (0.26 * (12.0 / radius).sqrt()).clamp(0.08, 0.3)
}

/// An angle brought into -PI..PI.
pub fn wrap(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    (a + PI).rem_euclid(TAU) - PI
}

/// The part of the arena a screen `w` x `h` (CSS pixels) shows around a
/// snake of this radius: screen pixels per world unit, and the half width
/// and half height it covers, in world units. Big snakes see further. A
/// screen bigger than 3840 x 2160, or more stretched than 3.6 to 1, sees
/// no more than one that is not: it is shown closer instead. Server and
/// browser both use it, so the server sends what the browser shows.
pub fn view(radius: f32, w: f32, h: f32) -> (f32, f32, f32) {
    let (w, h) = (w.max(1.0), h.max(1.0));
    let (mut bw, mut bh) = (w.clamp(200.0, 3840.0), h.clamp(200.0, 2160.0));
    bw = bw.min(bh * 3.6);
    bh = bh.min(bw * 3.6);
    let fit = (bw * bh).sqrt() / (1020.0 * (radius / 10.0).max(1.0).powf(0.45));
    let scale = fit * (w / bw).max(h / bh);
    (scale, bw / 2.0 / fit, bh / 2.0 / fit)
}

/// Screen pixels per world unit, as `view` has it.
pub fn view_scale(radius: f32, w: f32, h: f32) -> f32 {
    view(radius, w, h).0
}

/// Radius of a pellet worth `value`.
pub fn food_radius(value: u8) -> f32 {
    3.0 + (value as f32).sqrt() * 1.6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_screen_sees_no_more_than_the_box_it_is_sent() {
        for (w, h) in [
            (1280.0, 800.0),
            (390.0, 844.0),
            (844.0, 390.0),
            (5120.0, 1440.0),
            (3840.0, 200.0),
            (200.0, 2160.0),
            (100.0, 800.0),
            (7680.0, 4320.0),
        ] {
            for r in [10.0, 18.0, 42.0] {
                let (scale, hw, hh) = view(r, w, h);
                assert!(w / 2.0 / scale <= hw * 1.0001, "{w}x{h} r{r} wide");
                assert!(h / 2.0 / scale <= hh * 1.0001, "{w}x{h} r{r} tall");
                // Every shape covers the same area, and none is stretched
                // past 3.6 to 1 to see further one way.
                let (_, hw0, hh0) = view(r, 1280.0, 800.0);
                assert!((hw * hh / (hw0 * hh0) - 1.0).abs() < 0.001, "{w}x{h} r{r}");
                assert!(hw / hh <= 3.6001 && hh / hw <= 3.6001, "{w}x{h} r{r}");
            }
        }
        // An ordinary screen is shown just as it was.
        let (s, hw, _) = view(18.0, 1280.0, 800.0);
        assert!((640.0 / s - hw).abs() < 0.01);
    }

    #[test]
    fn angles_wrap_into_a_half_turn_either_way() {
        use std::f32::consts::PI;
        for a in [0.0, 1.0, -1.0, 3.0, -3.0, 7.0, -7.0, 200.0, -2e4] {
            let b = wrap(a);
            assert!((-PI..PI).contains(&b), "{a} -> {b}");
            assert!((b.sin() - a.sin()).abs() < 1e-2 && (b.cos() - a.cos()).abs() < 1e-2);
        }
    }
}
