//! The capability table: the COMPLETE effect surface of a mind. A mind
//! provably cannot touch anything not named here, and every call burns its
//! cost from the mind's tank, which is the mote's own balance.
//!
//! House rule: the physics never faults. An impossible act returns a
//! sentinel (0 or -1); there is no error path out of a capability.

use crate::island::Island;
use crate::laws::*;
use crate::mind::{Cap, Host};

pub static CAPS: &[Cap] = &[
    Cap {
        name: "light",
        arity: 2,
        params: "dx,dy",
        cost: COST_SENSE,
        doc: "light in the cell at offset (dx,dy), within 3; -1 past the island's edge",
    },
    Cap {
        name: "occupied",
        arity: 2,
        params: "dx,dy",
        cost: COST_SENSE,
        doc: "1 if a mote stands at (dx,dy)",
    },
    Cap {
        name: "kin",
        arity: 2,
        params: "dx,dy",
        cost: COST_SENSE,
        doc: "1 if the mote at (dx,dy) is of your lineage",
    },
    Cap {
        name: "energy",
        arity: 0,
        params: "",
        cost: COST_SELF,
        doc: "your ergs, not counting this tick's tank",
    },
    Cap {
        name: "age",
        arity: 0,
        params: "",
        cost: COST_SELF,
        doc: "ticks since you were born",
    },
    Cap {
        name: "hops",
        arity: 0,
        params: "",
        cost: COST_SELF,
        doc: "how many portals you have crossed",
    },
    Cap {
        name: "sun",
        arity: 0,
        params: "",
        cost: COST_SENSE,
        doc: "this island's sun, 0..100: how watched it is",
    },
    Cap {
        name: "portal",
        arity: 1,
        params: "side",
        cost: COST_SENSE,
        doc: "0 if the portal on side (0 N, 1 E, 2 S, 3 W) is shut, else 1 + the sun beyond it",
    },
    Cap {
        name: "edge",
        arity: 1,
        params: "side",
        cost: COST_SELF,
        doc: "cells between you and that side's edge",
    },
    Cap {
        name: "step",
        arity: 2,
        params: "dx,dy",
        cost: COST_STEP,
        doc: "move one cell; 1 moved, 0 blocked, 2 crossed a portal (25 more ergs)",
    },
    Cap {
        name: "go",
        arity: 1,
        params: "side",
        cost: COST_STEP,
        doc: "step toward a side; same results as step",
    },
    Cap {
        name: "harvest",
        arity: 0,
        params: "",
        cost: COST_HARVEST,
        doc: "take up to 64 light from your cell; each harvest more this tick takes half as much",
    },
    Cap {
        name: "bite",
        arity: 2,
        params: "dx,dy",
        cost: COST_BITE,
        doc: "take up to 60 ergs from the adjacent mote (tithed); -1 if no one there",
    },
    Cap {
        name: "give",
        arity: 3,
        params: "dx,dy,amt",
        cost: COST_GIVE,
        doc: "give ergs to the adjacent mote (tithed); what it received, -1 if no one there",
    },
    Cap {
        name: "spawn",
        arity: 0,
        params: "",
        cost: COST_SPAWN,
        doc: "a child in a free adjacent cell, endowed 240; needs 500 ergs; 1 born, 0 not",
    },
    Cap {
        name: "load",
        arity: 1,
        params: "slot",
        cost: COST_MEM,
        doc: "your memory slot 0..7, kept across ticks",
    },
    Cap {
        name: "store",
        arity: 2,
        params: "slot,v",
        cost: COST_MEM,
        doc: "write v to memory slot 0..7; returns v",
    },
    Cap {
        name: "roll",
        arity: 1,
        params: "n",
        cost: COST_ROLL,
        doc: "a number in [0, n) from the island's dice",
    },
];

const LIGHT: usize = 0;
const OCCUPIED: usize = 1;
const KIN: usize = 2;
const ENERGY: usize = 3;
const AGE: usize = 4;
const HOPS: usize = 5;
const SUN: usize = 6;
const PORTAL: usize = 7;
const EDGE: usize = 8;
const STEP: usize = 9;
const GO: usize = 10;
const HARVEST: usize = 11;
const BITE: usize = 12;
const GIVE: usize = 13;
const SPAWN: usize = 14;
const LOAD: usize = 15;
const STORE: usize = 16;
const ROLL: usize = 17;

/// Unit step toward a side.
pub fn side_delta(side: i64) -> (i64, i64) {
    match side.rem_euclid(4) {
        0 => (0, -1),
        1 => (1, 0),
        2 => (0, 1),
        _ => (-1, 0),
    }
}

/// A mind's view of the world for one run.
pub(crate) struct TickHost<'a> {
    pub island: &'a mut Island,
    pub me: usize,
    pub harvests: u32,
}

fn clamp_unit(v: i64) -> i64 {
    v.clamp(-1, 1)
}

impl Host for TickHost<'_> {
    fn call(&mut self, cap: usize, a: &[i64; 3]) -> i64 {
        let isl = &mut *self.island;
        let me = self.me;
        // A mote in the wire has left: the island answers it nothing.
        if isl.motes[me].leaving.is_some() {
            return -1;
        }
        let (x, y) = (isl.motes[me].x as i64, isl.motes[me].y as i64);
        let sensed = |dx: i64, dy: i64| -> Option<usize> {
            let (dx, dy) = (
                dx.clamp(-SENSE_RADIUS, SENSE_RADIUS),
                dy.clamp(-SENSE_RADIUS, SENSE_RADIUS),
            );
            Island::cell(x + dx, y + dy)
        };
        match cap {
            LIGHT => sensed(a[0], a[1]).map_or(-1, |c| isl.light[c] as i64),
            OCCUPIED => sensed(a[0], a[1]).map_or(0, |c| (isl.occ[c] != Island::EMPTY) as i64),
            KIN => sensed(a[0], a[1]).map_or(0, |c| match isl.mote_at(c) {
                Some(o) if o != me => (isl.motes[o].lineage == isl.motes[me].lineage) as i64,
                _ => 0,
            }),
            ENERGY => isl.motes[me].balance as i64,
            AGE => isl.motes[me].age as i64,
            HOPS => isl.motes[me].hops as i64,
            SUN => isl.sun as i64,
            PORTAL => isl.portals[a[0].rem_euclid(4) as usize] as i64,
            EDGE => match a[0].rem_euclid(4) {
                0 => y,
                1 => W as i64 - 1 - x,
                2 => H as i64 - 1 - y,
                _ => x,
            },
            STEP => isl.step_mote(me, clamp_unit(a[0]), clamp_unit(a[1])),
            GO => {
                let (dx, dy) = side_delta(a[0]);
                isl.step_mote(me, dx, dy)
            }
            HARVEST => {
                let got = isl.harvest(me, self.harvests);
                self.harvests += 1;
                got
            }
            BITE => isl.bite(me, a[0], a[1]),
            GIVE => isl.give(me, a[0], a[1], a[2]),
            SPAWN => isl.spawn(me),
            LOAD => isl.motes[me].mem[a[0].rem_euclid(MEM_SLOTS as i64) as usize],
            STORE => {
                isl.motes[me].mem[a[0].rem_euclid(MEM_SLOTS as i64) as usize] = a[1];
                a[1]
            }
            ROLL => {
                if a[0] <= 0 {
                    0
                } else {
                    isl.rng.below(a[0] as u64) as i64
                }
            }
            _ => -1,
        }
    }
}
