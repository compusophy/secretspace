//! Battlestation run by a host (`kit::cartridge!`): the desk drawn off
//! screen for it, the host's keys and wheel heard as a page hears the
//! window's, its pointer as the mouse (no lock: where it points on the
//! surface the arrow is on the monitor, as on a page), and the monitor's
//! compusophyOS the module the host gave it (`os::mount`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::input::{heard_key, hold_back};
use super::{Heard, KeyEv, Page, MAX_DPR, MIN_SHORT, PAGE};

thread_local! {
    /// Shift held, as the host's last key said (Shift+wheel leans in).
    static SHIFT: Cell<bool> = const { Cell::new(false) };
}

fn with<R>(f: impl FnOnce(&mut Page) -> R) -> Option<R> {
    PAGE.with(|p| p.try_borrow_mut().ok()?.as_mut().map(f))
}

/// What the page has heard (none before the desk is up).
fn heard() -> Option<Rc<RefCell<Heard>>> {
    PAGE.with(|p| p.try_borrow().ok()?.as_ref().map(|p| p.heard.clone()))
}

pub fn start() {
    wasm_bindgen_futures::spawn_local(super::boot());
}

pub fn pointer(kind: u8, x: f64, y: f64, button: u8) -> bool {
    kit::cart::to_hands(kind, x, y, button)
}

pub fn wheel(_x: f64, _y: f64, dy: f64) -> bool {
    let (Some(h), true) = (heard(), dy.is_finite()) else {
        return false;
    };
    let Ok(mut h) = h.try_borrow_mut() else {
        return false;
    };
    h.wheel += dy as f32;
    h.wheel_shift |= SHIFT.with(Cell::get);
    true
}

pub fn key(down: bool, code: &str, key: &str, mods: u8, repeat: bool) -> bool {
    SHIFT.with(|s| s.set(mods & kit::cart::SHIFT != 0));
    let Some(h) = heard() else {
        return false;
    };
    heard_key(
        &h,
        KeyEv {
            code: code.to_string(),
            key: key.to_string(),
            down,
            mods,
            repeat,
        },
    );
    hold_back(code, key, mods)
}

pub fn motion(_dx: f64, _dy: f64) {}

pub fn resized() {
    with(|p| p.out.fit(MIN_SHORT, MAX_DPR));
}

/// Let go: the OS told, the sound and the device let go, the page dropped
/// (its frame loop ends with the hosting).
pub fn end() {
    let page = PAGE.with(|p| p.try_borrow_mut().ok().and_then(|mut p| p.take()));
    if let Some(mut p) = page {
        if let Some(os) = &p.os {
            os.end();
        }
        p.sounds.audio.close();
        p.out.destroy();
    }
}
