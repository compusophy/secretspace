//! secretspace: a world made of browser tabs.
//!
//! Every tab holds one island. Islands link at their edges to strangers'
//! islands, and the motes that live on them, minds written in a small total
//! language whose fuel is their money, walk across from one tab to the
//! next. There is no server holding the world and no map of it: it exists
//! only in the tabs of the people looking at it.
//!
//! This crate is the world itself, with no dependencies: the mind language
//! (`mind`), what minds may do (`caps`), heredity (`genome`), one island and
//! its ledger (`island`), the wire format (`wire`), the portal protocol
//! (`net`) and the census nobody runs (`census`). Every law is in `laws`.

pub mod caps;
pub mod census;
pub mod founders;
pub mod genome;
pub mod hash;
pub mod island;
pub mod laws;
pub mod mind;
pub mod net;
pub mod rng;
pub mod wire;
