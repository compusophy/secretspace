//! Every number that tunes Wandfall (rule 3).

/// Ticks a second, and seconds a tick.
pub const TICK_HZ: u32 = 30;
pub const DT: f32 = 1.0 / TICK_HZ as f32;

// The island.
/// Metres from the centre to the map's edge (the island is smaller).
pub const MAP_HALF: f32 = 160.0;
/// The sea's surface: below it you wade, slowed.
pub const SEA: f32 = -1.0;
/// Where the island's shore falls into the sea (metres from the centre).
pub const SHORE: f32 = 140.0;
/// Hills: how tall, and how wide one is.
pub const HILLS: f32 = 9.0;
pub const HILL_SIZE: f32 = 48.0;
/// Trees, rocks and ruined pillars on the island.
pub const TREES: usize = 420;
pub const ROCKS: usize = 140;
pub const RUINS: usize = 14;

// Moving.
pub const RUN: f32 = 7.0;
pub const WADE: f32 = 0.55;
pub const ACCEL_GROUND: f32 = 70.0;
pub const ACCEL_AIR: f32 = 10.0;
pub const GRAVITY: f32 = 24.0;
pub const JUMP: f32 = 8.0;
/// A wizard: how wide, how tall, where the eyes are.
pub const RADIUS: f32 = 0.45;
pub const HEIGHT: f32 = 1.85;
pub const EYE: f32 = 1.6;
/// Steps up this high without jumping; sticks to the ground going down.
pub const STEP: f32 = 0.6;

// The match.
pub const MATCH_SIZE: usize = 16;
/// Seconds of lobby once someone is waiting.
pub const LOBBY_SECS: u32 = 12;
/// The drop: from this high, falling no faster than this, steering
/// faster than running.
pub const DROP_HEIGHT: f32 = 70.0;
pub const GLIDE_FALL: f32 = 5.0;
pub const GLIDE_SPEED: f32 = 14.0;
/// Seconds the winner is shown before the next lobby.
pub const OVER_SECS: u32 = 9;

// The storm: a circle closing in phases. Each phase: seconds it waits,
// seconds it shrinks, the radius it shrinks to, damage a second outside.
pub const STORM_START: f32 = 235.0;
pub const STORM: [(u32, u32, f32, i32); 5] = [
    (40, 25, 110.0, 2),
    (30, 22, 60.0, 4),
    (25, 18, 30.0, 7),
    (20, 15, 12.0, 10),
    (15, 15, 0.0, 15),
];
/// How far a phase's circle may drift from the last one's centre, as a
/// share of the room it has.
pub const STORM_DRIFT: f32 = 0.7;

// Health and the wand.
pub const HEALTH: i32 = 100;
/// Seconds without being hurt before health returns, and how fast.
pub const REGEN_AFTER: u32 = 6;
pub const REGEN: i32 = 3;
pub const BOLT_SPEED: f32 = 75.0;
/// Ticks a bolt flies (a second: 75 m).
pub const BOLT_LIFE: u32 = 30;
pub const BOLT_DAMAGE: i32 = 12;
/// Ticks between bolts.
pub const BOLT_COOLDOWN: u32 = 11;
pub const BOLT_RADIUS: f32 = 0.2;

// Bots.
pub const BOT_SIGHT: f32 = 48.0;
/// Radians their aim wanders, and ticks before they notice someone.
pub const BOT_AIM_ERROR: f32 = 0.05;
pub const BOT_NOTICE: u32 = 14;
/// The distance they like to duel at.
pub const BOT_RANGE: f32 = 16.0;
pub const BOT_NAMES: &[&str] = &[
    "Ashwick", "Brindle", "Corvane", "Dusk", "Elowen", "Fenwick", "Gilly", "Harrow", "Ivo",
    "Juniper", "Kestrel", "Lark", "Marrow", "Nettle", "Orrin", "Pell", "Quill", "Rook", "Sable",
    "Thistle", "Umber", "Vesper", "Wren", "Yarrow",
];
