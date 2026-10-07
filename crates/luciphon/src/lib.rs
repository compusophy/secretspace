//! Luciphon: carry your light out, knock them off the edge. A persistent
//! one-thumb brawler-builder on a floating island of light; the spec is
//! `docs/luciphon.md`. The core is std and `engine` only, shared by the
//! server and the page, and deterministic (Q16.16, `engine::fixed`) so the
//! page can predict exactly what the server will do.

pub mod beasts;
pub mod bots;
pub mod build;
pub mod combat;
pub mod gather;
pub mod gear;
pub mod hits;
pub mod island;
pub mod land;
pub mod laws;
pub mod life;
pub mod mirror;
pub mod motion;
pub mod persist;
pub mod predict;
pub mod proto;
pub mod room;
pub mod thumb;
pub mod tiles;
pub mod view;
pub mod world;
