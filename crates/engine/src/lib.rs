//! What every game here shares. A game is a `Room` the server hosts (its
//! world, ticked, and what each browser is told) plus a page that draws
//! it; both ends speak through `wire`. Who is on a socket is `who`; save
//! files are `snap`. What a page predicts must come out bit for bit as the
//! server's: Wandfall's is f32 kept to arithmetic and its own trig, and
//! `fixed` (Q16.16) is Luciphon's. The hub's live numbers are `hub`;
//! sounds are written in `synth`. std only.

pub mod crc32;
pub mod fixed;
pub mod hub;
pub mod rng;
pub mod room;
pub mod sha1;
pub mod snap;
pub mod synth;
pub mod who;
pub mod wire;
pub mod words;
