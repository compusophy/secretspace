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
/// Mass burned per boosting tick; part of it is dropped as food behind.
pub const BOOST_COST: f32 = 0.5;
pub const BOOST_DROP: f32 = 0.6;
/// Of a dead snake's mass, how much is left on the ground as food.
pub const DEATH_DROP: f32 = 0.75;

/// Natural food kept on the ground, and how fast it grows back.
pub const FOOD_TARGET: usize = 1100;
pub const FOOD_REGROW: usize = 12;
/// The biggest value one pellet holds.
pub const FOOD_MAX: u8 = 24;

/// Snakes in the arena when it is quiet: bots fill up to this, and leave
/// as people arrive.
pub const CROWD: usize = 18;
/// The fewest bots kept, however busy it gets.
pub const MIN_BOTS: usize = 4;
/// Ticks a dead bot waits before a new one takes its place.
pub const BOT_RESPAWN: u32 = 60;

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

/// Screen pixels per world unit for a screen `w` x `h` and a snake of this
/// radius: big snakes see further. Server and browser both use it, so the
/// server sends what the browser shows.
pub fn view_scale(radius: f32, w: f32, h: f32) -> f32 {
    let w = w.clamp(200.0, 3840.0);
    let h = h.clamp(200.0, 2160.0);
    (w * h).sqrt() / (1020.0 * (radius / 10.0).max(1.0).powf(0.45))
}

/// Radius of a pellet worth `value`.
pub fn food_radius(value: u8) -> f32 {
    3.0 + (value as f32).sqrt() * 1.6
}
