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
/// How far a bot goes out of its way for a chest.
pub const LOOT_SIGHT: f32 = 70.0;
pub const BOT_NAMES: &[&str] = &[
    "Ashwick", "Brindle", "Corvane", "Dusk", "Elowen", "Fenwick", "Gilly", "Harrow", "Ivo",
    "Juniper", "Kestrel", "Lark", "Marrow", "Nettle", "Orrin", "Pell", "Quill", "Rook", "Sable",
    "Thistle", "Umber", "Vesper", "Wren", "Yarrow",
];

// Levels: 1 to 20 within a match, from chests, damage and knockouts.
pub const MAX_LEVEL: u8 = 20;
pub const XP_PER_LEVEL: u32 = 100;
/// Max health and damage (percent) a level above the first.
pub const HEALTH_PER_LEVEL: i32 = 8;
pub const POWER_PER_LEVEL: i32 = 5;
pub const XP_CHEST: u32 = 45;
pub const XP_KNOCKOUT: u32 = 110;
/// More for knocking out someone of a higher level, a level.
pub const XP_KNOCKOUT_LEVEL: u32 = 12;
/// One XP for this much damage dealt.
pub const XP_DAMAGE: i32 = 3;

// Loot: chests across the island; scrolls of spells in them.
pub const CHESTS: usize = 40;
/// How close opens a chest, and picks up a scroll.
pub const CHEST_REACH: f32 = 1.7;
pub const SCROLL_REACH: f32 = 1.4;
pub const SCROLLS_A_CHEST: usize = 2;
pub const MAX_RANK: u8 = 5;
/// One scroll in this many from a chest is rank 2.
pub const RARE_SCROLL: u64 = 5;

// Aiming down the wand (right click): slower, and the page zooms.
pub const AIM_SLOW: f32 = 0.55;
pub const HASTE: f32 = 1.5;

/// Spells: two offensive slots (Q, E), two utility (R, F).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Offense,
    Utility,
}

pub struct Spell {
    pub name: &'static str,
    pub kind: Kind,
    /// Ticks between casts at rank 1 (each rank 6% less).
    pub cooldown: u32,
    /// Damage, healing, shield or ticks at rank 1, and percent more a rank.
    pub power: i32,
    pub per_rank: i32,
    pub what: &'static str,
}

pub mod spell {
    pub const LANCE: u8 = 0;
    pub const COMET: u8 = 1;
    pub const CHAIN: u8 = 2;
    pub const STARFALL: u8 = 3;
    pub const ROOT: u8 = 4;
    pub const BLINK: u8 = 5;
    pub const WARD: u8 = 6;
    pub const MEND: u8 = 7;
    pub const GUST: u8 = 8;
    pub const HASTE: u8 = 9;
}

pub const SPELLS: [Spell; 10] = [
    Spell {
        name: "Lance",
        kind: Kind::Offense,
        cooldown: 180,
        power: 30,
        per_rank: 15,
        what: "a piercing beam, fast and far",
    },
    Spell {
        name: "Comet",
        kind: Kind::Offense,
        cooldown: 240,
        power: 30,
        per_rank: 15,
        what: "a slow orb that bursts where it lands",
    },
    Spell {
        name: "Chain Spark",
        kind: Kind::Offense,
        cooldown: 210,
        power: 18,
        per_rank: 15,
        what: "leaps to the wizards near the one it strikes",
    },
    Spell {
        name: "Starfall",
        kind: Kind::Offense,
        cooldown: 360,
        power: 40,
        per_rank: 15,
        what: "light falls where you look, after a breath",
    },
    Spell {
        name: "Root",
        kind: Kind::Utility,
        cooldown: 360,
        power: 36,
        per_rank: 15,
        what: "a bolt that holds its mark in place",
    },
    Spell {
        name: "Blink",
        kind: Kind::Utility,
        cooldown: 300,
        power: 11,
        per_rank: 12,
        what: "step through the air to where you look",
    },
    Spell {
        name: "Ward",
        kind: Kind::Utility,
        cooldown: 480,
        power: 35,
        per_rank: 15,
        what: "a shield that takes the hurt for you",
    },
    Spell {
        name: "Mend",
        kind: Kind::Utility,
        cooldown: 540,
        power: 45,
        per_rank: 15,
        what: "heal, a little at a time",
    },
    Spell {
        name: "Gust",
        kind: Kind::Utility,
        cooldown: 300,
        power: 10,
        per_rank: 15,
        what: "throw back everyone near you",
    },
    Spell {
        name: "Haste",
        kind: Kind::Utility,
        cooldown: 450,
        power: 120,
        per_rank: 12,
        what: "run half again as fast, a while",
    },
];

// The spells' shapes.
pub const LANCE_SPEED: f32 = 160.0;
pub const LANCE_LIFE: u32 = 18;
pub const COMET_SPEED: f32 = 38.0;
pub const COMET_LIFE: u32 = 60;
pub const COMET_RADIUS: f32 = 4.5;
pub const CHAIN_SPEED: f32 = 95.0;
pub const CHAIN_LIFE: u32 = 25;
pub const CHAIN_REACH: f32 = 10.0;
/// Leaps: this many, one more every two ranks; each this share as strong.
pub const CHAIN_LEAPS: u8 = 2;
pub const CHAIN_FADE: i32 = 80;
pub const STARFALL_RANGE: f32 = 45.0;
pub const STARFALL_RADIUS: f32 = 5.0;
pub const STARFALL_DELAY: u32 = 24;
pub const ROOT_SPEED: f32 = 85.0;
pub const ROOT_LIFE: u32 = 25;
pub const ROOT_DAMAGE: i32 = 8;
pub const WARD_TICKS: u32 = 150;
pub const MEND_TICKS: u32 = 120;
pub const GUST_RADIUS: f32 = 6.5;
pub const GUST_PUSH: f32 = 15.0;
pub const GUST_LIFT: f32 = 7.0;

// The practice range: dummies about where you start (metres away,
// degrees round, and how they behave: 1 stands, 2 strafes, 3 spars when
// sparring is on), how soon they stand again, and how soon a hurt one is
// whole; chests are set out again this often.
pub const DUMMIES: [(f32, f32, u8); 9] = [
    (12.0, 0.0, 1),
    (16.0, 40.0, 1),
    (22.0, 80.0, 1),
    (30.0, 120.0, 1),
    (14.0, 200.0, 2),
    (20.0, 250.0, 2),
    (26.0, 300.0, 2),
    (34.0, 160.0, 3),
    (34.0, 340.0, 3),
];
pub const DUMMY_RESPAWN: u32 = 60;
pub const DUMMY_WHOLE: u32 = 60;
pub const PRACTICE_CHESTS_EVERY: u32 = 120 * TICK_HZ;
