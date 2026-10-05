//! The founding genomes every new island starts with. Hand-written minds,
//! one statement per line. Each founder is one lineage across the whole
//! world, so its descendants can be counted everywhere at once.

/// (name, genome, how many each new island starts with).
pub const FOUNDERS: &[(&str, &str, usize)] = &[
    (
        "sprout",
        "harvest()\n\
         if energy() > 700 { spawn() }",
        10,
    ),
    (
        "grazer",
        "if light(0,0) < 20 { let b = light(1,0); let d = 1; if light(0,1) > b { b = light(0,1); d = 2 }; if light(-1,0) > b { b = light(-1,0); d = 3 }; if light(0,-1) > b { b = light(0,-1); d = 0 }; go(d) }\n\
         harvest()\n\
         if energy() > 650 { spawn() }",
        10,
    ),
    (
        // When its sun goes down, tries sides until it finds a portal
        // brighter than home, then walks for it.
        "nomad",
        "if sun() < 60 { let s = load(0); if portal(s) <= sun() + 1 { store(0, roll(4)) } else { go(s); go(s) } }\n\
         harvest()\n\
         if light(0,0) < 12 { go(roll(4)) }\n\
         if energy() > 700 { spawn() }",
        8,
    ),
    (
        "drifter",
        "go(roll(4))\n\
         harvest()\n\
         if energy() > 600 { spawn() }",
        6,
    ),
    (
        "wolf",
        "let dx = roll(3) - 1; let dy = roll(3) - 1\n\
         if occupied(dx,dy) && !kin(dx,dy) { bite(dx,dy) } else { step(dx,dy) }\n\
         harvest()\n\
         if energy() > 900 { spawn() }",
        3,
    ),
];

/// What each founder is granted at genesis.
pub const FOUNDER_ENDOW: u64 = 400;

/// A starting point for a person writing their own mind.
pub const TEMPLATE: &str =
    "# your mote thinks this every tick. fuel is money: every step costs an erg.\n\
harvest()\n\
if light(0,0) < 15 { go(roll(4)) }\n\
if sun() < 50 && portal(1) > sun() { go(1) }\n\
if energy() > 650 { spawn() }";

/// Seed a fresh island with every founder.
pub fn genesis(island: &mut crate::island::Island) {
    for (name, src, count) in FOUNDERS {
        island.seed(name, src, *count, FOUNDER_ENDOW);
    }
}
