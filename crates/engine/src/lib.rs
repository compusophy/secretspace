//! What every game here shares. A game is a `Room` the server hosts (its
//! world, ticked, and what each browser is told) plus a page that draws
//! it; both ends speak through `wire`. The hub's live numbers are `hub`.
//! std only.

pub mod hub;
pub mod rng;
pub mod room;
pub mod wire;
