//! Wandfall's page. It asks the engine for WebGPU, joins the island,
//! sends the server an input thirty times a second while predicting its
//! own wizard with the server's own code, draws everyone else a tenth of
//! a second in the past, and draws it all with `render`; the HUD in
//! pixels over it. Hooks: `?perf=1` (numbers in the title and the HUD),
//! `?practice` (straight to the range), `?spells=0,1,4,5` (its four
//! slots), `?nocd` (no cooldowns), `?spar` (the dummies fight back), `?q=low|medium|high`, `?hold=ms`
//! (every effect held at that age, to look at it).

pub mod bar;
pub mod fx;
pub mod hud;
pub mod look;
pub mod menu;
pub mod state;
pub mod touch;

#[cfg(target_arch = "wasm32")]
mod page;
