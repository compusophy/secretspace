//! Wandfall run by a host (`kit::cartridge!`): drawn off screen for it
//! (`gpu::Gpu::hosted`), its hands fed by the host (`kit::cart::hands`),
//! the mouse locked when the host gives it (asked for by `grab`, as on a
//! page; the host's to grant).

use kit::cart::DOWN;

use super::{Mode, Page, MAX_DPR, MIN_SHORT, PAGE};

fn with<R>(f: impl FnOnce(&mut Page) -> R) -> Option<R> {
    PAGE.with(|p| p.try_borrow_mut().ok()?.as_mut().map(f))
}

pub fn start() {
    wasm_bindgen_futures::spawn_local(super::boot());
}

pub fn pointer(kind: u8, x: f64, y: f64, button: u8) -> bool {
    // A press on the name's box gives it the keys (as a click on a page's
    // input would), and is not the game's; one off it takes them away.
    if kind == DOWN && with(|p| p.name.press(x, y)).unwrap_or(false) {
        return true;
    }
    kit::cart::to_hands(kind, x, y, button)
}

pub fn wheel(_x: f64, _y: f64, _dy: f64) -> bool {
    false
}

pub fn key(down: bool, code: &str, _key: &str, mods: u8, repeat: bool) -> bool {
    kit::cart::game_key(code, mods) && kit::cart::key_to_hands(down, code, repeat)
}

pub fn motion(dx: f64, dy: f64) {
    kit::cart::motion_to_hands(dx, dy);
}

pub fn resized() {
    with(|p| p.g.fit(MIN_SHORT, MAX_DPR));
}

/// Let go: out of the match, the sound and the device let go, the page
/// dropped (its frame loop ends with the hosting).
pub fn end() {
    let page = PAGE.with(|p| p.try_borrow_mut().ok().and_then(|mut p| p.take()));
    if let Some(mut p) = page {
        if let Mode::Online(link) = &p.mode {
            link.close();
        }
        p.sounds.audio.close();
        p.g.destroy();
    }
}
