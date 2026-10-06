//! Every law of the world, in one place. Constants live nowhere else; the
//! physics card prints them, so the card and the law cannot drift.

/// Island width and height in cells. Every island has the same shape, so
/// a mote leaving one edge lands at the same offset on the next.
pub const W: usize = 48;
pub const H: usize = 30;
pub const CELLS: usize = W * H;

/// Ticks per second while the world runs in real time.
pub const TICKS_PER_SEC: u64 = 10;

// ---- the sun: attention ----------------------------------------------------

/// The sun's level runs 0..=SUN_MAX. It climbs toward SUN_MAX while someone
/// watches the island and falls to 0 when no one does.
pub const SUN_MAX: u32 = 100;
/// How far the level moves toward its target each tick: dusk and dawn take
/// SUN_MAX / SUN_RAMP ticks (10 seconds).
pub const SUN_RAMP: u32 = 1;
/// Light minted into every cell each tick at full sun.
pub const SUN_BASE: u32 = 1;
/// Extra light at the centre of the warm spot, at full sun.
pub const SPOT_PEAK: u32 = 40;
/// Radius of the warm spot that drifts across the island, in cells.
pub const SPOT_RADIUS: i32 = 11;
/// Ticks for the warm spot to trace its whole figure once.
pub const SPOT_PERIOD: u64 = 2400;
/// The most light a cell holds. Light past it is never minted.
pub const LIGHT_CAP: u32 = 300;

// ---- minds -----------------------------------------------------------------

/// The most fuel a mind may burn in one tick. Fuel is the mote's own
/// balance: thinking costs ergs, one per step of evaluation.
pub const TANK_MAX: u64 = 160;
/// A mind that runs out of fuel mid-thought forfeits its whole tank.
pub const MAX_LINES: usize = 40;
pub const MAX_SOURCE: usize = 1536;
pub const MAX_VARS: usize = 16;
/// Expression and block nesting the parser accepts.
pub const MAX_DEPTH: u32 = 24;
pub const MAX_REPEAT: i64 = 16;
pub const MEM_SLOTS: usize = 8;

// ---- capability costs (fuel, burned) ----------------------------------------

pub const COST_SENSE: u64 = 1;
pub const COST_SELF: u64 = 1;
pub const COST_STEP: u64 = 6;
pub const COST_HARVEST: u64 = 1;
pub const COST_BITE: u64 = 4;
pub const COST_GIVE: u64 = 2;
pub const COST_SPAWN: u64 = 30;
pub const COST_MEM: u64 = 1;
pub const COST_ROLL: u64 = 1;
/// Crossing a portal: the wire is not free.
pub const COST_CROSS: u64 = 25;

// ---- the economy ---------------------------------------------------------

/// How far a mote can sense, in cells (Chebyshev).
pub const SENSE_RADIUS: i64 = 3;
/// The first harvest in a tick takes up to this much; each further harvest
/// in the same tick takes half as much again (the gut fills).
pub const HARVEST_MAX: u32 = 64;
pub const BITE_MAX: u64 = 60;
/// Every transfer between motes burns 1.618% (the golden tithe), so wash
/// trading is thermodynamically lossy.
pub const TITHE_PER_100K: u64 = 1618;
/// What a newborn is endowed with, and what its parent must keep after.
pub const ENDOW: u64 = 240;
pub const SPAWN_RESERVE: u64 = 260;
/// A child's genome is mutated with probability MUTATE_NUM / MUTATE_DEN.
pub const MUTATE_NUM: u64 = 1;
pub const MUTATE_DEN: u64 = 2;
/// Senescence: wealth must be mortal or the world freezes into misers.
pub const MAX_AGE: u32 = 4000;
/// The most ergs a mote may carry across a portal; the rest stays behind
/// as light at the edge it left from.
pub const MAX_CARRY: u64 = 4000;
/// The most arrivals an island admits in one tick, and holds waiting. A
/// flood from a hostile tab queues, then is refused at the border.
pub const ARRIVALS_PER_TICK: usize = 48;
pub const ARRIVALS_WAITING: usize = 512;
/// A new mind is released as a clutch: one mote alone on a full island is
/// almost always lost to drift, however good its mind. Each is endowed
/// RELEASE_ENDOW, drawn from the island's brightest cells wherever they
/// are: the whole land pays, not the newborns' neighbourhood.
pub const CLUTCH: usize = 8;
pub const RELEASE_ENDOW: u64 = 600;
/// Releasing clears the ground first: motes within this radius return to
/// the light they were made of, and the clutch is made from that light.
pub const CLEARING: i64 = 7;

/// Opposite side of a portal: 0 north, 1 east, 2 south, 3 west.
pub const fn opposite(side: u8) -> u8 {
    (side + 2) % 4
}

/// The golden tithe on a transfer of `amount`.
pub const fn tithe(amount: u64) -> u64 {
    amount * TITHE_PER_100K / 100_000
}

/// The physics card: every law, printed. Paste it into anything that wants
/// to write a mind.
pub fn card() -> String {
    let caps = crate::caps::CAPS
        .iter()
        .map(|c| format!("  {}({}) cost {} - {}", c.name, c.params, c.cost, c.doc))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "secretspace physics\n\
         island {W}x{H} cells; {TICKS_PER_SEC} ticks a second while it runs\n\
         sun 0..{SUN_MAX}: rises while someone watches the island, falls when no one does\n\
         \x20 each cell gains {SUN_BASE} light a tick at full sun, up to {SPOT_PEAK} more under the warm spot (radius {SPOT_RADIUS})\n\
         \x20 a cell holds at most {LIGHT_CAP}\n\
         minds: fuel is money. each tick a mind may burn up to {TANK_MAX} of its own ergs, one per step\n\
         \x20 run dry mid-thought and the whole tank is forfeit\n\
         \x20 at most {MAX_LINES} lines, {MAX_VARS} variables; repeat at most {MAX_REPEAT}\n\
         spawn: endow {ENDOW}, keep {SPAWN_RESERVE}; half of children mutate one line\n\
         release: a clutch of {CLUTCH}, each endowed {RELEASE_ENDOW}, on ground cleared {CLEARING} cells around\n\
         transfers burn a {TITHE_PER_100K}/100000 tithe; motes die at age {MAX_AGE} or at zero ergs\n\
         portals: step off an edge with an open portal to cross to another person's island ({COST_CROSS} fuel; carry at most {MAX_CARRY})\n\
         capabilities:\n{caps}\n"
    )
}
