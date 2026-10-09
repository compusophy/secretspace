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
/// Trees, rocks, giant mushrooms and ruined pillars on the island.
pub const TREES: usize = 400;
pub const ROCKS: usize = 130;
pub const SHROOMS: usize = 40;
pub const RUINS: usize = 8;
/// The Spire at the centre: a plaza this high and this wide, falling to
/// the hills over this far, and its tower.
pub const PLATEAU_TOP: f32 = 7.0;
pub const PLATEAU: f32 = 20.0;
pub const PLATEAU_FALL: f32 = 16.0;
pub const TOWER_RADIUS: f32 = 4.6;
pub const TOWER_HEIGHT: f32 = 30.0;
/// Its stair: this wide, winding this many times about it up to the
/// balcony, this high above its foot, which runs this much of a turn.
pub const STAIR_WIDTH: f32 = 2.0;
pub const STAIR_TURNS: f32 = 2.0;
pub const BALCONY: f32 = 23.0;
pub const BALCONY_SPAN: f32 = 0.8;
/// Where the stair starts about the tower (radians from +x, toward +z).
pub const STAIR_FROM: f32 = 0.35;
/// The places about it (a stone circle, a demon rift, a crystal grove):
/// this far out, this wide; how deep the rift's bowl is.
pub const POI_RING: f32 = 76.0;
pub const POI_RADIUS: f32 = 16.0;
pub const RIFT_DEPTH: f32 = 3.5;

// Moving.
pub const RUN: f32 = 7.0;
/// Sprinting (forward only; not aiming, casting or crouched).
pub const SPRINT: f32 = 10.0;
/// Stamina, spent sprinting (ten a tick): a full store sprints 6 s; after
/// a breath (1 s) of not sprinting it comes back (in 4 s); spent to
/// nothing, a wizard is winded until a third is back.
pub const STAMINA: u16 = 6 * TICK_HZ as u16 * 10;
pub const STAMINA_SPEND: u16 = 10;
pub const STAMINA_BACK: u16 = 15;
pub const STAMINA_BREATH: u8 = TICK_HZ as u8;
pub const WINDED_UNTIL: u16 = STAMINA * 2 / 3;
pub const WADE: f32 = 0.55;
/// Speeding up toward where you steer, on the ground and in the air;
/// slowing with no keys held; and how fast speed above your pace bleeds
/// away on the ground (in the air it holds): momentum, so a slide or a
/// hill carries on into a run or a jump.
pub const ACCEL_GROUND: f32 = 48.0;
pub const ACCEL_AIR: f32 = 16.0;
pub const BRAKE: f32 = 36.0;
pub const OVERSPEED: f32 = 5.0;
/// Hills: walking up a slope of 1 (45°) this much slower, down it this
/// much faster (and less, gentler).
pub const HILL: f32 = 0.45;
/// Sliding: crouch while going fast (sprinting, or landing at speed). A
/// boost (unless the last came within the cooldown, in ticks), friction
/// on the flat, gravity down a slope, a little steering, a top speed; it
/// ends below a crouch's pace, or standing up.
pub const SLIDE_MIN: f32 = 7.5;
pub const SLIDE_BOOST: f32 = 3.0;
pub const SLIDE_COOLDOWN: u8 = 30;
pub const SLIDE_FRICTION: f32 = 4.5;
pub const SLIDE_STEER: f32 = 1.6;
pub const SLIDE_MAX: f32 = 18.0;
/// Falling is quicker than rising; holding jump rises on the lighter
/// pull, so a held jump goes higher than a tapped one.
pub const GRAVITY: f32 = 30.0;
pub const GRAVITY_UP: f32 = 20.0;
pub const JUMP: f32 = 8.0;
/// Ticks after running off an edge that a jump still works, and ticks a
/// jump pressed early waits for the ground.
pub const COYOTE: u8 = 4;
pub const JUMP_BUFFER: u8 = 4;
/// A jump is a press (holding does not hop again). Pressed within this
/// many ticks of landing (or just before), a hop keeps its speed and adds
/// a little, up to a most: timed hops carry you faster than a sprint;
/// late ones, the ground has slowed.
pub const HOP_WINDOW: u8 = 3;
pub const HOP_BOOST: f32 = 0.6;
pub const HOP_MAX: f32 = 12.5;
/// Once in the air, a second jump (magic): this fast up, the way you
/// steer; it costs stamina, so not when winded.
pub const AIR_JUMP: f32 = 7.0;
/// Climbing a ledge (a rock, a pillar, a stone, the altar) in the air,
/// pushing toward it: its top no more than this far over your feet (and
/// more than the low mark), its edge this close beyond your body, ahead
/// of you at least this much (a cosine). Up you go to clear it by this
/// much, and over it at least this fast (m/s).
pub const MANTLE_REACH: f32 = 1.3;
pub const MANTLE_LOW: f32 = 0.3;
pub const MANTLE_NEAR: f32 = 0.35;
pub const MANTLE_AHEAD: f32 = 0.5;
pub const MANTLE_OVER: f32 = 0.35;
pub const MANTLE_PUSH: f32 = 3.5;
/// A boulder's top over its trunk's (its look is rounder and taller).
pub const ROCK_TOP: f32 = 1.15;
pub const AIR_JUMP_STAMINA: u16 = STAMINA / 6;
/// Crouching: this much of the speed, eyes this high, a body this tall.
pub const CROUCH_SLOW: f32 = 0.5;
pub const CROUCH_EYE: f32 = 1.05;
pub const CROUCH_HEIGHT: f32 = 1.25;
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
pub const BOLT_SPEED: f32 = 80.0;
/// Ticks a bolt flies (a second: 80 m).
pub const BOLT_LIFE: u32 = 30;
/// The wand is the heartbeat, spells the moments: a bolt is a quarter of
/// a spell.
pub const BOLT_DAMAGE: i32 = 8;
/// Ticks between bolts.
pub const BOLT_COOLDOWN: u32 = 13;
pub const BOLT_RADIUS: f32 = 0.2;

// Bots.
pub const BOT_SIGHT: f32 = 48.0;
/// Radians their aim wanders, and ticks before they notice someone.
pub const BOT_AIM_ERROR: f32 = 0.05;
pub const BOT_NOTICE: u32 = 14;
/// The distance they like to duel at.
pub const BOT_RANGE: f32 = 16.0;
/// How far a bot goes out of its way for a cube.
pub const LOOT_SIGHT: f32 = 70.0;
pub const BOT_NAMES: &[&str] = &[
    "Ashwick", "Brindle", "Corvane", "Dusk", "Elowen", "Fenwick", "Gilly", "Harrow", "Ivo",
    "Juniper", "Kestrel", "Lark", "Marrow", "Nettle", "Orrin", "Pell", "Quill", "Rook", "Sable",
    "Thistle", "Umber", "Vesper", "Wren", "Yarrow",
];

// Levels: 1 to 20 within a match, from cubes, damage and knockouts.
pub const MAX_LEVEL: u8 = 20;
pub const XP_PER_LEVEL: u32 = 100;
/// Max health and damage (percent) a level above the first.
pub const HEALTH_PER_LEVEL: i32 = 8;
pub const POWER_PER_LEVEL: i32 = 5;
/// XP for a spell cube run over.
pub const XP_CUBE: u32 = 20;
pub const XP_KNOCKOUT: u32 = 110;
/// More for knocking out someone of a higher level, a level.
pub const XP_KNOCKOUT_LEVEL: u32 = 12;
/// And the fallen's own XP joins the victor's (percent of it).
pub const XP_SHARE: u32 = 100;
/// One XP for this much damage dealt.
pub const XP_DAMAGE: i32 = 3;

// Loot: spell cubes, lying loose across the island and in pairs at the
// caches (by each place, at the ruins), and every spell the fallen held.
// Running over a cube learns its spell or ranks it up (and gives XP); the
// spellbook puts what you know in your four slots.
pub const CACHES: usize = 28;
pub const CUBES_A_CACHE: usize = 2;
pub const LOOSE_CUBES: usize = 36;
/// Ticks a spell put in a slot waits before it can be cast.
pub const EQUIP_COOLDOWN: u32 = 2 * TICK_HZ;
/// How close picks up a cube.
pub const SCROLL_REACH: f32 = 1.4;
pub const MAX_RANK: u8 = 3;
/// One cube in this many set out is rank 2.
pub const RARE_SCROLL: u64 = 5;
/// Each rank above the first: this much more power (percent), and this
/// much less cooldown.
pub const RANK_POWER: i32 = 25;
pub const RANK_COOLDOWN: u32 = 10;

// Aiming down the wand (right click): slower, and the page zooms.
pub const AIM_SLOW: f32 = 0.55;

/// Spells: two to hurt (Q, E), two to live (R, F).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Offense,
    Utility,
}

pub struct Spell {
    pub name: &'static str,
    pub kind: Kind,
    /// Ticks between casts at rank 1.
    pub cooldown: u32,
    /// Damage, metres, shield, healing or push at rank 1.
    pub power: i32,
    /// What it does, what it beats, and what beats it.
    pub what: &'static str,
    pub beats: &'static str,
    pub beaten: &'static str,
}

/// Eight spells, one verb each. Every style has its answer: range beats
/// the mender, close beats the sniper, Gust and Blink beat close, the sky
/// beats the still, moving beats the sky.
pub mod spell {
    pub const FIREBALL: u8 = 0;
    pub const LANCE: u8 = 1;
    pub const FROST: u8 = 2;
    pub const LIGHTNING: u8 = 3;
    pub const BLINK: u8 = 4;
    pub const WARD: u8 = 5;
    pub const MEND: u8 = 6;
    pub const GUST: u8 = 7;
    pub const OFFENSE: [u8; 4] = [FIREBALL, LANCE, FROST, LIGHTNING];
    pub const UTILITY: [u8; 4] = [BLINK, WARD, MEND, GUST];
}

pub const SPELLS: [Spell; 8] = [
    Spell {
        name: "Fireball",
        kind: Kind::Offense,
        cooldown: 150,
        power: 30,
        what: "a ball of fire that bursts where it lands",
        beats: "the splash finds them behind cover",
        beaten: "a Ward; a sidestep, far off",
    },
    Spell {
        name: "Lance",
        kind: Kind::Offense,
        cooldown: 180,
        power: 34,
        what: "an instant beam of light, far",
        beats: "the still, the far, the mending",
        beaten: "cover, a Ward, a foe in your face",
    },
    Spell {
        name: "Frost",
        kind: Kind::Offense,
        cooldown: 150,
        power: 6,
        what: "a fan of ice: deadly close, and it chills",
        beats: "the runner, the sniper caught close",
        beaten: "Gust, Blink, range",
    },
    Spell {
        name: "Lightning",
        kind: Kind::Offense,
        cooldown: 240,
        power: 42,
        what: "strikes where you look, a breath later",
        beats: "the still, the shielded, the hidden",
        beaten: "anyone who moves",
    },
    Spell {
        name: "Blink",
        kind: Kind::Utility,
        cooldown: 240,
        power: 12,
        what: "step through the air; shakes off chill",
        beats: "Lightning, Frost, a corner",
        beaten: "the Lance (no dodging light)",
    },
    Spell {
        name: "Ward",
        kind: Kind::Utility,
        cooldown: 360,
        power: 40,
        what: "a brief shield that eats the next big hit",
        beats: "a Lance, a Fireball",
        beaten: "patience: it is brief",
    },
    Spell {
        name: "Mend",
        kind: Kind::Utility,
        cooldown: 420,
        power: 40,
        what: "heal, quickly",
        beats: "the long fight, the storm",
        beaten: "a burst; Lightning on the still",
    },
    Spell {
        name: "Gust",
        kind: Kind::Utility,
        cooldown: 270,
        power: 16,
        what: "throw back all near you, and their bolts",
        beats: "Frost, a rush, a ledge, the storm",
        beaten: "anything from afar",
    },
];

// The spells' shapes.
pub const FIREBALL_SPEED: f32 = 48.0;
pub const FIREBALL_LIFE: u32 = 45;
pub const FIREBALL_RADIUS: f32 = 3.5;
pub const LANCE_RANGE: f32 = 80.0;
/// Frost: shards in a fan, this far apart (of 65536 a turn), how fast and
/// how long they fly, and how long and how much they chill.
pub const FROST_SHARDS: usize = 7;
pub const FROST_SPREAD: i32 = 620;
pub const FROST_SPEED: f32 = 55.0;
pub const FROST_LIFE: u32 = 12;
pub const CHILL_TICKS: u16 = 60;
pub const CHILL_SLOW: f32 = 0.6;
pub const LIGHTNING_RANGE: f32 = 50.0;
pub const LIGHTNING_DELAY: u32 = 24;
pub const LIGHTNING_RADIUS: f32 = 4.0;
pub const WARD_TICKS: u32 = 120;
pub const MEND_TICKS: u32 = 60;
pub const GUST_RADIUS: f32 = 7.0;
pub const GUST_LIFT: f32 = 7.0;
pub const GUST_DAMAGE: i32 = 5;

// The practice range: dummies about where you start (metres away,
// degrees round, and how they behave: 1 stands, 2 strafes, 3 spars when
// sparring is on), how soon they stand again, and how soon a hurt one is
// whole; cubes are set out again this often.
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
pub const PRACTICE_LOOT_EVERY: u32 = 120 * TICK_HZ;
