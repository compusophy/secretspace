//! How Wandfall looks, drawn by the engine (`render`): the island
//! (`land`, its basalt in `basalt`, and its places' magic in `aura`), every wizard jointed and
//! moving (`rig`), spells, loot and the storm (`fx`, `look`), each spell's
//! icon (`icon`), the sky at the island's hour (`sky`), what a page has been told of the match (`state`), and the
//! match drawn from it (`scene`). Shared by Wandfall's page and the hub's
//! card, which spectates the match live (`spectate`).

pub mod aura;
mod basalt;
pub mod camera;
pub mod flora;
pub mod fx;
pub mod icon;
pub mod land;
pub mod look;
pub mod rig;
pub mod scene;
pub mod sky;
pub mod spectate;
pub mod state;
