//! Every number that makes Luciphon feel the way it does, in one place.
//!
//! Distances are in tiles and speeds in tiles a tick, as `Fx` (Q16.16);
//! times are in ticks (30 a second); headings are `u16`, 65536 to a turn;
//! multipliers are thousandths. The laws are written in readable units
//! through the `const fn`s below (`tps(6_000)` is 6 tiles a second). The
//! server sends its laws in the Welcome, so a page always predicts with the
//! server's numbers.

use engine::crc32::crc32;
use engine::fixed::Fx;
use engine::wire::{Reader, Writer};

pub const HZ: u32 = 30;

/// Milli-tiles a second, as tiles a tick.
pub const fn tps(milli: i32) -> Fx {
    Fx(((milli as i64 * 65536) / (1000 * HZ as i64)) as i32)
}

/// Milli-tiles a second squared, as tiles a tick squared.
pub const fn tps2(milli: i32) -> Fx {
    Fx(((milli as i64 * 65536) / (1000 * (HZ * HZ) as i64)) as i32)
}

/// Milli-radians a second, as heading units a tick.
pub const fn rps(milli: i32) -> u16 {
    // 65536 / 2π = 10430.378 heading units a radian.
    ((milli as i64 * 10_430_378) / (1_000_000 * HZ as i64)) as u16
}

/// Degrees as heading units.
pub const fn deg(d: i32) -> u16 {
    ((d as i64 * 65536) / 360) as u16
}

/// Milliseconds as ticks, rounded.
pub const fn ms(m: u32) -> u32 {
    (m * HZ + 500) / 1000
}

/// Milli-tiles as tiles.
pub const fn tiles(milli: i32) -> Fx {
    Fx::milli(milli)
}

/// A number the laws hold, and how it goes on the wire.
pub trait Law: Sized {
    fn put(&self, w: &mut Writer);
    fn get(r: &mut Reader) -> Option<Self>;
}

impl Law for Fx {
    fn put(&self, w: &mut Writer) {
        w.i32(self.0);
    }
    fn get(r: &mut Reader) -> Option<Fx> {
        r.i32().map(Fx)
    }
}

impl Law for u32 {
    fn put(&self, w: &mut Writer) {
        w.u32(*self);
    }
    fn get(r: &mut Reader) -> Option<u32> {
        r.u32()
    }
}

impl Law for i32 {
    fn put(&self, w: &mut Writer) {
        w.i32(*self);
    }
    fn get(r: &mut Reader) -> Option<i32> {
        r.i32()
    }
}

impl Law for u16 {
    fn put(&self, w: &mut Writer) {
        w.u16(*self);
    }
    fn get(r: &mut Reader) -> Option<u16> {
        r.u16()
    }
}

impl Law for bool {
    fn put(&self, w: &mut Writer) {
        w.u8(*self as u8);
    }
    fn get(r: &mut Reader) -> Option<bool> {
        Some(r.u8()? != 0)
    }
}

macro_rules! laws {
    ($( $(#[$m:meta])* $name:ident : $ty:ty = $val:expr, )*) => {
        #[derive(Clone, Debug, PartialEq)]
        pub struct Laws { $( $(#[$m])* pub $name: $ty, )* }

        pub const LAWS: Laws = Laws { $( $name: $val, )* };

        impl Laws {
            pub fn encode(&self) -> Vec<u8> {
                let mut w = Writer::default();
                $( Law::put(&self.$name, &mut w); )*
                w.0
            }

            pub fn decode(b: &[u8]) -> Option<Laws> {
                let mut r = Reader::new(b);
                let laws = Laws { $( $name: Law::get(&mut r)?, )* };
                r.done().then_some(laws)
            }
        }
    };
}

impl Laws {
    /// What the Welcome says the laws are, to tell two sets apart.
    pub fn hash(&self) -> u32 {
        crc32(&self.encode())
    }
}

laws! {
    // The island (§6).
    /// Seed of the island when the room has none saved.
    world_seed: u32 = 0x1u32 << 20 | 0x5eed,
    /// The grid is `size` tiles square, in chunks of `chunk`.
    size: i32 = 128,
    chunk: i32 = 32,
    /// The island's radius, and how ragged the Rim is.
    island: i32 = 60,
    rim_noise: i32 = 3,
    /// Where each ring ends.
    sanctum: i32 = 10,
    glow: i32 = 34,
    dim: i32 = 52,
    /// How far you see by day, in tiles.
    sight: i32 = 24,

    // Movement (§7): first person, quick to start and quick to stop.
    walk: Fx = tps(3_000),
    run: Fx = tps(5_500),
    accel: Fx = tps2(45_000),
    stop: Fx = tps2(45_000),
    /// Control in the air, thousandths.
    air_control: i32 = 300,
    /// A jump's speed up, gravity, and how far below the island the Dark
    /// takes you.
    jump: Fx = tps(6_500),
    gravity: Fx = tps2(20_000),
    fall_depth: Fx = tiles(8_000),
    /// Top speed while striking and while charging, thousandths.
    strike_slow: i32 = 700,
    charge_slow: i32 = 500,
    body: Fx = tiles(350),
    /// Top speed falls by `weight_cut` for every `weight_step` materials.
    weight_step: i32 = 50,
    weight_cut: i32 = 50,
    weight_floor: i32 = 700,
    own_land: i32 = 1_150,
    ice_grip: i32 = 150,
    mud_speed: i32 = 500,
    water_speed: i32 = 600,

    // Dash.
    dash_ticks: u32 = 5,
    dash_speed: Fx = tps(18_000),
    iframes: u32 = 3,
    dash_breath: i32 = 30_000,
    dash_cooldown: u32 = ms(350),
    /// A flick in a dash's last ticks is held for a wall-kick.
    kick_hold: u32 = 3,
    kick_window: u32 = 10,

    // Breath, in thousandths: 100 of it.
    breath: i32 = 100_000,
    breath_regen: i32 = 35_000 / 30,
    breath_rest: u32 = ms(400),

    // Flow.
    flow_window: u32 = ms(1_500),
    flow_max: u32 = 3,
    flow_kb: i32 = 1_150,

    // Flame and knockback, in thousandths: 100 Flame.
    flame: i32 = 100_000,
    flame_regen: i32 = 3_000 / 30,
    flame_rest: u32 = ms(6_000),
    flight_decay: Fx = tps2(12_000),
    /// The strike (a tap).
    reach: Fx = tiles(1_200),
    cone: u16 = deg(100),
    inner_cone: u16 = deg(40),
    face_turn: u16 = deg(50),
    windup: u32 = 4,
    active: u32 = 2,
    recovery: u32 = 6,
    strike_base: i32 = 8_000,
    /// Damage a tile/s of closing speed adds, in thousandths, and its cap.
    strike_closing: i32 = 750,
    strike_cap: i32 = 16_000,
    strike_kb: Fx = tps(3_500),
    strike_carry: i32 = 500,
    engaged: u32 = ms(10_000),
    launcher_within: u32 = 36,
    launcher: i32 = 2_000,
    /// The lance: a tap in a dash, or this long after.
    lance_late: u32 = 2,
    lance_reach: Fx = tiles(1_800),
    lance_damage: i32 = 14_000,
    lance_kb: i32 = 1_500,
    lance_whiff: u32 = 12,
    /// The heavy and the throw: a hold, released at a charge in ticks.
    charge_min: u32 = 12,
    charge_full: u32 = 18,
    perfect_to: u32 = 22,
    charge_max: u32 = 36,
    lunge: Fx = tiles(1_500),
    lunge_ticks: u32 = 4,
    heavy_recovery: u32 = 10,
    heavy_low: i32 = 12_000,
    heavy_full: i32 = 28_000,
    perfect: i32 = 34_000,
    heavy_kb_low: Fx = tps(6_000),
    heavy_kb_full: Fx = tps(10_000),
    perfect_kb: i32 = 1_500,
    throw_glim: u32 = 3,
    mote_speed: Fx = tps(14_000),
    throw_near: Fx = tiles(4_000),
    throw_far: Fx = tiles(9_000),
    throw_low: i32 = 6_000,
    throw_full: i32 = 14_000,
    throw_perfect: i32 = 20_000,
    throw_kb: Fx = tps(3_000),
    /// A hit this hard ends a charge.
    interrupt: i32 = 8_000,

    // Hazards.
    thorns: i32 = 12_000,
    thorn_bounce: Fx = tps(8_000),
    thorn_rest: u32 = ms(500),
    slam_speed: Fx = tps(7_000),
    /// Damage a tile/s over `slam_speed` deals, in thousandths.
    slam: i32 = 4_000,

    // Falling and coming back.
    descent: u32 = ms(3_000),
    ghost: u32 = ms(3_000),
    rekindled: u32 = ms(60_000),
    rekindled_breath: i32 = 150,
    down: u32 = ms(2_000),
    stand_flame: i32 = 30_000,
    stand_ghost: u32 = ms(5_000),
    spared: u32 = ms(60_000),
    /// Of what you carry, what a fall in the Dim or on the Rim drops.
    dim_drop: i32 = 500,
    rim_drop: i32 = 1_000,
    scatter: Fx = tiles(1_500),

    // Newcomers: Sparks, for the first half hour of play.
    sparks: bool = true,
    sparks_ticks: u32 = 30 * 60 * HZ,
    join_glim: u32 = 0,

    // Gathering (§8): per strike, strikes a node holds, its regrowth.
    birch_yield: u32 = 3,
    birch_strikes: u32 = 8,
    birch_regrow: u32 = ms(240_000),
    oak_yield: u32 = 4,
    oak_strikes: u32 = 10,
    oak_regrow: u32 = ms(360_000),
    rock_yield: u32 = 2,
    rock_strikes: u32 = 8,
    rock_regrow: u32 = ms(300_000),
    moss_yield: u32 = 1,
    moss_strikes: u32 = 3,
    moss_regrow: u32 = ms(180_000),
    crystal_yield: u32 = 2,
    crystal_strikes: u32 = 4,
    crystal_regrow: u32 = ms(480_000),
    /// Yields in the Dim and on the Rim, thousandths.
    dim_mult: i32 = 1_600,
    rim_mult: i32 = 2_500,
    /// Nodes ring every `ring_every` ticks; a strike within `ring_window`
    /// of a ring is resonant; three in a row ring the node out.
    ring_every: u32 = 30,
    ring_window: u32 = 2,
    ring_out_material: u32 = 3,
    ring_out_glim: u32 = 1,
    ring_out_regrow: i32 = 750,
    dwell_every: u32 = 30,
    carry_max: u32 = 300,
    glim_max: u32 = 500,
    beacon: u32 = 150,

    // Building.
    hearth_wood: u32 = 30,
    hearth_stone: u32 = 20,
    hearth_glim: u32 = 10,
    wall_wood: u32 = 4,
    door_wood: u32 = 6,
    thorns_wood: u32 = 3,
    thorns_stone: u32 = 1,
    lantern_glim: u32 = 2,
    planter_wood: u32 = 4,
    lantern_level: u32 = 5,
    build_channel: u32 = 12,
    build_idle: u32 = ms(8_000),
    hearth_near: i32 = 16,
    hearth_far: i32 = 48,
    hearth_apart: i32 = 10,
    no_build: i32 = 13,
    claim_gap: i32 = 2,
    refund: i32 = 500,
    lamp: u32 = 30,

    // Farming: Sunwheat.
    wheat_ripe: u32 = ms(900_000),
    wheat_full: u32 = ms(1_800_000),
    wheat_harvest: u32 = 4,

    // Kindling.
    kindle_reach: i32 = 16,
    wick_max: u32 = 48,
    wick_life: u32 = ms(60_000),
    fill_box: i32 = 64,
    fill_max: u32 = 300,
    cap_base: u32 = 64,
    cap_per_hour: u32 = 32,
    cap_max: u32 = 600,
    snuff_damage: i32 = 20_000,
    snuff_stun: u32 = ms(500),
    /// Land: yield a tile an hour and the upkeep curve (0.03 n^1.4),
    /// both in thousandths of glim; settled every `land_every`.
    land_yield: i32 = 250,
    upkeep: i32 = 30,
    upkeep_power: i32 = 1_400,
    land_every: u32 = ms(60_000),
    fade_every: u32 = ms(120_000),
    outline_days: u32 = 30,
    cold_days: u32 = 30,

    // Rekindle and Recall.
    rekindle_glim: u32 = 10,
    rekindle_flame: i32 = 25_000,
    rekindle_ticks: u32 = ms(2_000),
    recall_ticks: u32 = ms(8_000),

    // Leaving and coming back.
    idle_dream: u32 = ms(20_000),
    linger: u32 = 300,
    wake_ghost: u32 = ms(3_000),

    // Skill XP for each thing done.
    xp_material: u32 = 10,
    xp_glim: u32 = 12,
    xp_tile: u32 = 12,
    xp_piece: u32 = 30,
    xp_hearth: u32 = 120,
    xp_crop: u32 = 40,
    xp_valor: u32 = 2,
    xp_way: u32 = 15,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_read_as_written() {
        // 6 tiles a second is a fifth of a tile a tick.
        assert_eq!(tps(6_000), Fx::ratio(1, 5));
        assert_eq!(ms(300), 9);
        assert_eq!(deg(90), 16384);
        // 14 rad/s is about 4867 heading units a tick.
        assert!((4860..4875).contains(&rps(14_000)));
    }

    #[test]
    fn laws_round_trip_the_wire() {
        let b = LAWS.encode();
        assert_eq!(Laws::decode(&b), Some(LAWS));
        assert_eq!(Laws::decode(&b[..b.len() - 1]), None);
        assert_ne!(LAWS.hash(), 0);
    }
}
