//! wyrm run by a host (`kit::cartridge!`): the same page, drawn onto the
//! host's surface, its pointer and keys handed in by the host instead of
//! heard from the window.

use kit::cart::{DOWN, LEFT, MOVE, UP};

use crate::{with, PAGE};

/// The host's pointer is one mouse (whose id a page's events give as 1).
const MOUSE: i32 = 1;

pub fn start() {
    crate::boot();
}

pub fn pointer(kind: u8, x: f64, y: f64, _button: u8) -> bool {
    if !(x.is_finite() && y.is_finite()) {
        return false;
    }
    with(|p| match kind {
        DOWN => crate::down(p, x, y, MOUSE, true),
        MOVE => crate::moved(p, x, y, MOUSE),
        // Gone off the surface: as if let go, or the boost sticks.
        UP | LEFT => crate::up(p, MOUSE, true),
        _ => {}
    })
    .is_some()
}

pub fn wheel(_x: f64, _y: f64, _dy: f64) -> bool {
    false
}

pub fn key(down: bool, code: &str, key: &str, mods: u8, _repeat: bool) -> bool {
    if !kit::cart::game_key(code, mods) {
        return false;
    }
    with(|p| match down {
        true => crate::key_down(p, key),
        false => crate::key_up(p, key),
    })
    .unwrap_or(false)
}

pub fn motion(_dx: f64, _dy: f64) {}

pub fn resized() {
    with(crate::resized);
}

/// Let go: the link closed, the page dropped (its frame loop ends with
/// the hosting).
pub fn end() {
    let page = PAGE.with(|p| p.try_borrow_mut().ok().and_then(|mut p| p.take()));
    if let Some(p) = page {
        p.link.close();
    }
}
