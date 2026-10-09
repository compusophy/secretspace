//! Wandfall's page. It asks the engine for WebGPU, joins the island,
//! sends the server an input thirty times a second while predicting its
//! own wizard with the server's own code, draws everyone else a tenth of
//! a second in the past, and draws it all with `render`; the HUD in
//! pixels over it. Hooks: `?perf=1` (numbers in the title and the HUD),
//! `?practice` (straight to the range), `?spells=0,1,4,5` (its four
//! slots), `?nocd` (no cooldowns), `?spar` (the dummies fight back),
//! `?look=yaw,pitch` (degrees, where you face), `?orbit=ID` (the camera
//! turns about a wizard; 0 is you), `?q=low|medium|high`, `?hold=ms`
//! (every effect held at that age, to look at it).

pub mod bar;
pub mod hud;
pub mod menu;
pub mod sound;
pub mod touch;

// How it looks, shared with the hub's card.
pub use wandfall_look::{aura, fx, icon, land, look, rig, scene, state};

#[cfg(target_arch = "wasm32")]
mod page;
