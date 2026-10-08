//! Wandfall: a wand battle royale. Everyone drops onto one island, finds
//! their footing, and duels with wands while a storm closes in; the last
//! one standing wins, and the next match begins. The server runs the
//! world (`world`, `room`); the page predicts its own wizard with the same
//! code (`motion`, `predict`), so movement is exact on both ends.

pub mod bots;
pub mod laws;
pub mod loot;
pub mod map;
pub mod motion;
pub mod predict;
pub mod proto;
pub mod room;
pub mod spells;
pub mod storm;
pub mod trig;
pub mod view;
pub mod world;
