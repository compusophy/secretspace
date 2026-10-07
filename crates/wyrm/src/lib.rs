//! wyrm: snakes steering through a shared arena, eating, growing, and
//! bursting into food when they run into someone. One server runs the
//! `World`; every browser gets its own `view` of it over the `proto` wire.
//! Built only on the engine (std only), so the server and the wasm page
//! share every line of it. `room` is how the server hosts it.

pub mod bots;
pub mod grid;
pub mod laws;
pub mod mirror;
pub mod proto;
pub mod room;
pub mod view;
pub mod world;
