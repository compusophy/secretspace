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
/// Trees grow in woods about this wide (m), where the woods' noise (0..1)
/// is over the edge, thick this much over it; out in the meadows, one
/// takes root alone at these odds. A tree's crown (a pine's boughs)
/// reaches this far out from its trunk, for each of its scale's units
/// (m, as drawn, its leaves and all), never over a ruin's pillar or a
/// mushroom's cap. Mushrooms grow by the woods under open sky: their
/// caps clear of every crown, within this far (m) of one.
pub const TREES: usize = 400;
pub const WOODS: f32 = 40.0;
pub const WOOD_EDGE: f32 = 0.4;
pub const WOOD_SOFT: f32 = 0.2;
pub const WOOD_LONE: f32 = 0.06;
pub const CROWN: f32 = 3.4;
pub const SHROOM_WOOD: f32 = 2.5;
pub const ROCKS: usize = 130;
pub const SHROOMS: usize = 40;
pub const RUINS: usize = 8;
/// A giant mushroom's cap, for every metre the mushroom stands, as it is
/// drawn: its top, out from its middle (how far, how high: a dome, down
/// to its rim); its underside; and how far out its top holds you up.
pub const SHROOM_DOME: [(f32, f32); 4] = [(0.0, 1.19), (0.22, 1.16), (0.4, 1.06), (0.47, 0.94)];
pub const SHROOM_UNDER: f32 = 0.88;
pub const SHROOM_CAP: f32 = 0.44;
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
/// The places about it (a stone circle, a demon rift, a crystal grove,
/// a basalt causeway): this far out, this wide; how deep the rift's bowl
/// is.
pub const POI_RING: f32 = 76.0;
pub const POI_RADIUS: f32 = 16.0;
pub const RIFT_DEPTH: f32 = 3.5;
/// The causeway: six-sided columns this far apart, packed this far out
/// from its middle, rising across it by this much (m) to a crown this
/// tall; one in so many sunk this far (a pit to hop or wall-jump out
/// of); and sea stacks standing about it, so many.
pub const COLUMN_APART: f32 = 2.2;
pub const COLUMN_FIELD: f32 = 9.0;
pub const COLUMN_RISE: f32 = 7.5;
pub const COLUMN_CROWN: f32 = 12.0;
pub const COLUMN_PITS: f32 = 0.09;
pub const COLUMN_SINK: f32 = 2.4;
pub const COLUMN_STACKS: usize = 8;
/// A place's spikes and crystals stand this far clear of its caches (m,
/// past their own width).
pub const CACHE_CLEAR: f32 = 2.0;

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
/// Wading in the sea: this share of the pace; every step in it keeps
/// this share of the speed over that pace; a jump out of it this share
/// of a jump.
pub const WADE: f32 = 0.55;
pub const WADE_KEEP: f32 = 0.5;
pub const WADE_JUMP: f32 = 0.75;
/// Speeding up toward where you steer, on the ground and in the air;
/// slowing with no keys held; and how fast speed above your pace bleeds
/// away on the ground (in the air it holds): momentum, so a slide or a
/// hill carries on into a run or a jump.
pub const ACCEL_GROUND: f32 = 48.0;
pub const ACCEL_AIR: f32 = 16.0;
pub const BRAKE: f32 = 36.0;
pub const OVERSPEED: f32 = 5.0;
/// Momentum holds steering more or less its way (a cosine): in the air
/// more than this; on the ground, bleeding at OVERSPEED steering more
/// than the first, and as fast as it speeds up by the second.
pub const AIR_ALONG: f32 = -0.3;
pub const MOMENTUM_ALONG: (f32, f32) = (0.2, -0.2);
/// Hills: walking up a slope of 1 (45°) this much slower, down it this
/// much faster (and less, gentler).
pub const HILL: f32 = 0.45;
/// Sliding: crouch while going fast (sprinting, or landing at speed). A
/// boost, from slower than a timed hop's most and up to no faster than
/// this (once a cooldown, in ticks, has passed since the last); friction
/// on the flat, gravity down a slope, a little steering, a top speed (and
/// over it, slowing this fast: m/s a second, more than any hill speeds
/// it); it ends below a crouch's pace, or standing up.
pub const SLIDE_MIN: f32 = 7.5;
pub const SLIDE_BOOST: f32 = 3.0;
pub const SLIDE_BOOST_TO: f32 = 14.0;
pub const SLIDE_COOLDOWN: u8 = 30;
pub const SLIDE_FRICTION: f32 = 4.5;
pub const SLIDE_STEER: f32 = 1.6;
pub const SLIDE_MAX: f32 = 18.0;
pub const SLIDE_OVER: f32 = 18.0;
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
/// steer; it costs stamina, so not when winded. Turned straight back, it
/// keeps this share of the speed it had over a run (more, turned less).
pub const AIR_JUMP: f32 = 7.0;
pub const AIR_JUMP_KEEP: f32 = 0.5;
/// Wall jumps: off a wall touched in the last few ticks, out from it and
/// up (m/s), turned toward where you steer (by this much: 1 would turn a
/// kick steered along the wall half way to it; never nearer the wall
/// than this, a cosine), keeping a share of the speed along it; so many
/// before you land. Pushing into a wall you can still kick off, you
/// slide down it no faster than this (m/s).
pub const WALL_GRACE: u8 = 6;
pub const WALL_KICK: f32 = 7.5;
pub const WALL_JUMP: f32 = 8.5;
pub const WALL_AIM: f32 = 0.8;
pub const WALL_OUT: f32 = 0.25;
pub const WALL_KEEP: f32 = 0.95;
pub const WALL_JUMPS: u8 = 3;
pub const WALL_SLIDE_FALL: f32 = 2.5;
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
/// Come at a ledge faster than a run, you go on over it at this share of
/// that speed (a hop timed to the landing keeps it).
pub const MANTLE_KEEP: f32 = 0.75;
/// Launch runes: how many out in the wild (and one by each place), how
/// wide one is (m), how far apart they lie (m), and how hard one throws
/// you up (m/s; onto your broom, to glide where you will).
pub const PADS_WILD: usize = 8;
pub const PAD_R: f32 = 1.3;
pub const PAD_APART: f32 = 45.0;
pub const PAD_UP: f32 = 33.0;
/// A boulder blocks this wide and this tall, times its size; its top
/// stands this much over its trunk's (its look is rounder and taller).
pub const ROCK_GIRTH: f32 = 1.1;
pub const ROCK_TALL: f32 = 1.3;
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
/// Chilled by Frost, speed over a chilled run drains this fast (m/s a
/// second), in the air as on the ground.
pub const CHILL_DRAG: f32 = 16.0;
/// On the broom (the drop's, a launch rune's): steering this quick (m/s
/// a second); let go, speed drifting away this slowly, as speed over its
/// pace does. Looking straight down it dives, falling this fast and
/// going this share faster (less, looking less steeply); out of a dive
/// it pulls up this quickly (m/s a second).
pub const GLIDE_ACCEL: f32 = 14.0;
pub const GLIDE_DRAG: f32 = 2.0;
pub const GLIDE_DIVE: f32 = 20.0;
pub const GLIDE_DIVE_FAST: f32 = 0.3;
pub const GLIDE_PULL: f32 = 60.0;
/// Your own wizard, drawn: a correction to the page's prediction eases
/// away over about this long (ms), unless it is this far (m: a Blink, a
/// respawn), drawn at once.
pub const SMOOTH_MS: f64 = 90.0;
pub const SMOOTH_SNAP: f32 = 4.0;
/// The Tether: steering square to its rope swings you across the ground
/// (m/s a second), and a swing dies away this much a second (a sag up or
/// down is taken up as fast as the pull takes hold, `TETHER_GRIP`).
pub const TETHER_STEER: f32 = 16.0;
pub const TETHER_SWAY: f32 = 2.0;

// The match.
pub const MATCH_SIZE: usize = 16;
/// Seconds of lobby once someone is waiting.
pub const LOBBY_SECS: u32 = 12;
/// People on the island at most (watching, waiting or playing); whoever
/// comes past them watches. A page gone mid-fight (a phone between
/// networks, a reload) has this many seconds to come back to its wizard,
/// which stands where it was meanwhile.
pub const MAX_PEOPLE: usize = 3 * MATCH_SIZE;
pub const RECONNECT_SECS: u32 = 15;
/// The island's day: each lobby turns it on an hour (dawn, day, dusk,
/// night, and dawn again), so match after match the light changes; the
/// range stays at dusk.
pub const HOURS: u8 = 4;
pub const RANGE_HOUR: u8 = 2;
/// And its weather, a match at a time (0 clear, 1 mist, 2 rain): the
/// odds of rain and of mist, in a hundred.
pub const RAIN_ODDS: u64 = 22;
pub const MIST_ODDS: u64 = 18;
/// The drop: from this high, falling no faster than this, steering
/// faster than running.
pub const DROP_HEIGHT: f32 = 70.0;
pub const GLIDE_FALL: f32 = 5.0;
pub const GLIDE_SPEED: f32 = 14.0;
/// Seconds the winner is shown before the next lobby; and, once every
/// person in a match is out, seconds before a new one gathers for them
/// (the bots' fight does not go on without them).
pub const OVER_SECS: u32 = 6;
pub const OUT_LINGER_SECS: u32 = 5;
/// A page's inputs, one a tick: at most this many wait their turn (a
/// flood is cut short); this many are kept back against the network's
/// jitter (more, and it catches up); steps are banked a tick at a time
/// up to this many, a second's worth (a page that stalled for up to a
/// second catches up at once, one that floods runs no faster than the
/// clock, and one that holds its inputs back to let them go in a burst
/// gets no further, though it covers the bank's worth in one tick); and
/// after this many ticks with none, a stalled page's wizard stands and
/// falls on its own (those steps cost nothing from the bank).
pub const INPUT_QUEUE: usize = 30;
pub const INPUT_JITTER: usize = 3;
pub const INPUT_BANK: u32 = TICK_HZ;
pub const INPUT_IDLE: u32 = TICK_HZ / 3;

// The storm: a circle closing in phases. Each phase: seconds it waits,
// seconds it shrinks, the radius it shrinks to, and damage a second
// outside (of every HEALTH of a wizard's whole health: a share of it).
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
/// Damage a second outside once it has closed to nothing.
pub const STORM_END_DPS: i32 = 30;

// Health and the wand.
pub const HEALTH: i32 = 100;
/// Seconds without being hurt before health returns, and how fast (of
/// every HEALTH of a wizard's whole health).
pub const REGEN_AFTER: u32 = 6;
pub const REGEN: i32 = 3;
/// Knocked out by the storm, or gone from the match, within this many
/// seconds of a wizard's hurt: the knockout is theirs.
pub const KILL_CREDIT_SECS: u32 = 6;
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
/// How far they see (m); how wide ahead of them (a cosine: about 75° to
/// either side); and how near they hear anyone, whichever way they face.
/// Whoever hurt them they know of for `BOT_MEMORY` ticks, as far off as a
/// Lance reaches.
pub const BOT_SIGHT: f32 = 48.0;
pub const BOT_FOV_COS: f32 = 0.26;
pub const BOT_HEAR: f32 = 12.0;
/// Ticks before they notice someone new (and up to this many more); ticks
/// they remember one gone from sight (aiming where it was, not shooting);
/// and how much nearer the one they fight seems (they keep to it).
pub const BOT_NOTICE: u32 = 14;
pub const BOT_NOTICE_MORE: f32 = 10.0;
pub const BOT_MEMORY: u32 = TICK_HZ;
pub const BOT_STICKY: f32 = 0.7;
/// How fast they turn to aim, and to walk (of 65536 a turn, a tick).
pub const BOT_AIM_TURN: i32 = 1800;
pub const BOT_TURN: i32 = 1800;
/// How far off they shoot (m): this much at any range, and this share of
/// the distance, more against a wizard crossing their sight (at a run)
/// and one in the air; each bot's hand is steadier or shakier by up to
/// this share.
pub const BOT_MISS_NEAR: f32 = 0.3;
pub const BOT_AIM_ERROR: f32 = 0.05;
pub const BOT_LEAD_ERROR: f32 = 0.04;
pub const BOT_AIR_ERROR: f32 = 0.03;
pub const BOT_SKILL: f32 = 0.3;
/// Where they aim: at the chest, this high over a wizard's feet (m); a
/// Fireball this far below it (a near miss still bursts by them). They
/// lead a wizard by the time a spell takes to get there, its climb or
/// fall by this share of it (it curves), and Lightning by this share of
/// its delay (it may turn).
pub const CHEST: f32 = 1.1;
pub const BOT_FIREBALL_LOW: f32 = 0.8;
pub const BOT_LEAD_UP: f32 = 0.5;
pub const BOT_LIGHTNING_LEAD: f32 = 0.5;
/// For the first seconds of a fight they loot, fighting only whoever is
/// this near (m) or hurts them.
pub const BOT_CALM_SECS: u32 = 30;
pub const BOT_CALM_RANGE: f32 = 14.0;
/// The distance they like to duel at; closer by this share with Frost,
/// further with the Lance; past the band about it (shares) they step in
/// or back.
pub const BOT_RANGE: f32 = 16.0;
pub const BOT_RANGE_CLOSE: f32 = 0.75;
pub const BOT_RANGE_FAR: f32 = 1.15;
pub const BOT_BAND: (f32, f32) = (0.6, 1.4);
/// Strafing: legs this long (ticks) and up to this many more, this share
/// of them hopped; a jump now and then (odds a tick); and out from under
/// a Lightning mark about to strike (within so many ticks), from as far
/// as this beyond its reach (m).
pub const BOT_STRAFE: u32 = 15;
pub const BOT_STRAFE_MORE: f32 = 40.0;
pub const BOT_DUEL_HOP: f32 = 0.5;
pub const BOT_JUMP_ODDS: f32 = 0.02;
pub const BOT_DODGE: u32 = 12;
pub const BOT_DODGE_MARGIN: f32 = 1.0;
/// Spells: the odds a tick of casting one that is ready; how far off each
/// attack spell is cast (m; Lightning not nearer than the first, and not
/// at one in the air unless it is warded, mending or slower than this,
/// m/s).
pub const BOT_CAST_ODDS: f32 = 0.08;
pub const BOT_CAST_LANCE: f32 = 50.0;
pub const BOT_CAST_FIREBALL: f32 = 32.0;
pub const BOT_CAST_FROST: f32 = 13.0;
pub const BOT_CAST_LIGHTNING: (f32, f32) = (6.0, 40.0);
pub const BOT_STILL: f32 = 3.0;
/// Hurt (health, percent): struck (within so many ticks) they Ward, they
/// Mend, struck they Blink away; with neither Mend nor Ward ready they
/// run off to heal for a few seconds, from a foe no nearer than this (m:
/// one that near would only shoot them in the back), a Tether thrown
/// this high (of 65536 a turn) to get away.
pub const BOT_STRUCK: u32 = 20;
pub const BOT_WARD_HP: i32 = 85;
pub const BOT_MEND_HP: i32 = 55;
pub const BOT_BLINK_HP: i32 = 30;
pub const BOT_RETREAT_HP: i32 = 35;
pub const BOT_RETREAT_SECS: u32 = 3;
pub const BOT_RETREAT_NEAR: f32 = 8.0;
pub const BOT_TETHER_UP: i16 = 4000;
/// The drop: each lands within this far (m) of the island's middle, and
/// within this share of how far the broom carries it, as far as it can
/// from where the others are bound; it lets the broom drop this near its
/// spot (m).
pub const BOT_DROP_SPREAD: f32 = 105.0;
pub const BOT_DROP_REACH: f32 = 0.8;
pub const BOT_DROP_NEAR: f32 = 6.0;
/// Where they wander with no one about: within this share of the storm's
/// next circle (of the whole island, before it first closes), for at
/// most this long (s), or till they are this near (m).
pub const BOT_GOAL_SHARE: f32 = 0.7;
pub const BOT_GOAL_SECS: u32 = 20;
pub const BOT_GOAL_NEAR: f32 = 3.0;
/// The storm: they head in once they would not otherwise make it with
/// this many seconds to spare; within this far (m) of the last circle's
/// middle they hold their ground.
pub const BOT_FLEE_SPARE: f32 = 5.0;
pub const BOT_SAFE_R: f32 = 8.0;
/// Bots going this far (m) hop their way there (now and then off a late
/// landing too: odds a tick); running from the storm, they take a launch
/// rune this near (m) on the way.
pub const BOT_HOP_FAR: f32 = 25.0;
pub const BOT_HOP_ODDS: f32 = 0.05;
pub const BOT_PAD: f32 = 30.0;
/// They step round what stands this near ahead (m), turning this much
/// (radians), and give it this much room past their own width (m).
pub const BOT_AVOID: f32 = 4.0;
pub const BOT_AVOID_TURN: f32 = 1.1;
pub const BOT_AVOID_ROOM: f32 = 0.4;
/// Stuck (moved less than this in a second, m), a bot backs off this far
/// (m) for this long (s) before going elsewhere (kicking off a wall it
/// slides down for the first of them, s), and gives up for a while (s)
/// on a cube this near (m) it could not reach.
pub const BOT_STUCK_MOVE: f32 = 0.7;
pub const BOT_BACK_OFF: f32 = 8.0;
pub const BOT_BACK_SECS: u32 = 3;
pub const BOT_KICK_SECS: u32 = 2;
pub const BOT_SKIP_NEAR: f32 = 4.0;
pub const BOT_SKIP_SECS: u32 = 30;
/// How far a bot goes out of its way for a cube, and how high over the
/// ground under it one may lie and still be reached (m: a held jump and
/// a climb).
pub const LOOT_SIGHT: f32 = 70.0;
pub const BOT_REACH_UP: f32 = 2.6;
pub const BOT_NAMES: &[&str] = &[
    "Ashwick", "Brindle", "Corvane", "Dusk", "Elowen", "Fenwick", "Gilly", "Harrow", "Ivo",
    "Juniper", "Kestrel", "Lark", "Marrow", "Nettle", "Orrin", "Pell", "Quill", "Rook", "Sable",
    "Thistle", "Umber", "Vesper", "Wren", "Yarrow",
];

// Levels: 1 to 20 within a match, from cubes, damage and knockouts.
pub const MAX_LEVEL: u8 = 20;
pub const XP_PER_LEVEL: u32 = 100;
/// Max health and damage (percent) a level above the first; a level
/// gained heals this share (percent) of the health it adds.
pub const HEALTH_PER_LEVEL: i32 = 5;
pub const POWER_PER_LEVEL: i32 = 3;
pub const LEVEL_HEAL: i32 = 50;
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
/// Caches lie at least this far apart (m), and this far from the ruin
/// they are by.
pub const CACHE_APART: f32 = 10.0;
pub const CACHE_RUIN: f32 = 2.5;
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

/// Nine spells, one verb each. Every style has its answer: range beats
/// the mender, close beats the sniper, Gust and Blink beat close, the sky
/// beats the still, moving beats the sky; the Tether climbs and chases.
pub mod spell {
    pub const FIREBALL: u8 = 0;
    pub const LANCE: u8 = 1;
    pub const FROST: u8 = 2;
    pub const LIGHTNING: u8 = 3;
    pub const BLINK: u8 = 4;
    pub const WARD: u8 = 5;
    pub const MEND: u8 = 6;
    pub const GUST: u8 = 7;
    pub const TETHER: u8 = 8;
    pub const OFFENSE: [u8; 4] = [FIREBALL, LANCE, FROST, LIGHTNING];
    pub const UTILITY: [u8; 5] = [BLINK, WARD, MEND, GUST, TETHER];
}

pub const SPELLS: [Spell; 9] = [
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
        power: 30,
        what: "an instant beam of light, far",
        beats: "the still, the far, the mending",
        beaten: "cover, a Ward, a foe in your face",
    },
    Spell {
        name: "Frost",
        kind: Kind::Offense,
        cooldown: 120,
        power: 7,
        what: "a fan of ice: deadly close, and it chills",
        beats: "the runner, the sniper caught close",
        beaten: "Gust, Blink, range",
    },
    Spell {
        name: "Lightning",
        kind: Kind::Offense,
        cooldown: 240,
        power: 48,
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
        cooldown: 480,
        power: 32,
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
    Spell {
        name: "Tether",
        kind: Kind::Utility,
        cooldown: 210,
        power: 40,
        what: "a rope of light that pulls you where it catches",
        beats: "a cliff, a tower, a runner, the storm",
        beaten: "Frost (it slips), the Lance (a line in the air)",
    },
];

// The spells' shapes.
pub const FIREBALL_SPEED: f32 = 48.0;
pub const FIREBALL_LIFE: u32 = 45;
pub const FIREBALL_RADIUS: f32 = 3.5;
/// A burst's damage at its edge (a share of the whole); a Fireball's
/// burst this share wider a rank above the first.
pub const BURST_EDGE: f32 = 0.5;
pub const FIREBALL_RANK_RADIUS: f32 = 0.1;
pub const LANCE_RANGE: f32 = 80.0;
/// The Lance strikes others where its caster's page drew them, but never
/// from further back than this (ticks; a third of a second).
pub const REWIND: u32 = 10;
/// Frost: shards in a fan, this far apart (of 65536 a turn), how fast and
/// how long they fly, and how long and how much they chill.
pub const FROST_SHARDS: usize = 7;
pub const FROST_SPREAD: i32 = 520;
pub const FROST_SPEED: f32 = 55.0;
pub const FROST_LIFE: u32 = 12;
pub const CHILL_TICKS: u16 = 60;
pub const CHILL_SLOW: f32 = 0.6;
pub const LIGHTNING_RANGE: f32 = 50.0;
pub const LIGHTNING_DELAY: u32 = 24;
pub const LIGHTNING_RADIUS: f32 = 5.0;
pub const WARD_TICKS: u32 = 120;
pub const MEND_TICKS: u32 = 60;
/// Blink: it looks ahead in steps this long (m), stops short of a rise
/// higher than this (m) and this near anyone (m), and goes nowhere (and
/// is not spent) if it would not go this far.
pub const BLINK_STEP: f32 = 0.5;
pub const BLINK_CLIMB: f32 = 3.0;
pub const BLINK_CROWD: f32 = RADIUS * 2.2;
pub const BLINK_MIN: f32 = 1.0;
/// Gust: how far it reaches about you, how far above or below (m).
pub const GUST_RADIUS: f32 = 8.5;
pub const GUST_BAND: f32 = 3.0;
pub const GUST_LIFT: f32 = 7.0;
pub const GUST_DAMAGE: i32 = 5;
/// The Tether: how long it can pull at the first rank's reach (ticks;
/// longer as it reaches further), how fast (m/s) and how quickly the
/// pull takes hold (a share a second); how near counts as there (m), the
/// hop up off it (m/s), and how much gravity still pulls while it holds
/// you.
pub const TETHER_TICKS: u8 = 60;
pub const TETHER_SPEED: f32 = 24.0;
pub const TETHER_GRIP: f32 = 7.0;
pub const TETHER_ARRIVE: f32 = 1.6;
pub const TETHER_POP: f32 = 7.0;
pub const TETHER_GRAVITY: f32 = 0.2;
/// Caught on nothing, the air holds it this share of its reach.
pub const TETHER_AIR: f32 = 0.6;

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
/// A strafing dummy: legs this long (ticks) and up to this many more, a
/// jump's odds a tick, and how far from its post it strays (m); a
/// sparring one, between fights, waits once back this near its post (m).
pub const DUMMY_STRAFE: u32 = 35;
pub const DUMMY_STRAFE_MORE: f32 = 40.0;
pub const DUMMY_JUMP_ODDS: f32 = 0.01;
pub const DUMMY_LEASH: f32 = 4.0;
pub const DUMMY_HOME_NEAR: f32 = 2.0;
/// A dummy's post is turned this many degrees at a time, up to this far
/// either way, till you can see it from where you start; it and each
/// lesson cube have nothing standing within this far (m).
pub const DUMMY_TURN_STEP: f32 = 10.0;
pub const DUMMY_TURN_MAX: f32 = 40.0;
pub const RANGE_ROOM: f32 = 1.2;
/// The lesson cubes lie in a fan ahead of where you start: this far apart
/// (radians), the first this far off (m), each one this much further.
pub const LESSON_APART: f32 = 0.35;
pub const LESSON_FROM: f32 = 4.5;
pub const LESSON_STEP: f32 = 1.2;
/// Where you start on the range: nothing standing within this far (m),
/// no tree's or mushroom's cap within this far (m), and the way east
/// clear this far (m); looked for ring after ring about the ruin (this
/// many rings, the first this far out, each this much further, a spot
/// every this many degrees round).
pub const RANGE_CLEAR: f32 = 4.0;
pub const RANGE_CAPS: f32 = 8.0;
pub const RANGE_VIEW: f32 = 15.0;
pub const RANGE_RINGS: u32 = 12;
pub const RANGE_RING_FROM: f32 = 5.0;
pub const RANGE_RING_STEP: f32 = 1.5;
pub const RANGE_RING_DEG: f32 = 30.0;
