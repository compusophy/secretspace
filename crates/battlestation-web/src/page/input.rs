//! Your hands: keys to the desk's keyboard and the computer, your mouse
//! to the desk's mouse and the monitor's arrow (where you point on the
//! page is where it points on the monitor: no click to start, nothing
//! to let go of), the wheel to scroll or lean in; a phone's taps, drags
//! and keyboard; the shared menu (Esc).

use std::cell::RefCell;
use std::rc::Rc;

use battlestation::hands::mouse_for;
use battlestation::keys;
use battlestation::laws::{CURSOR_SPEED, LEAN_NOTCH, SCREEN_PX};
use battlestation::term::{Act, Facts};
use kit::input::{Hand, Kind};
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, KeyboardEvent, WheelEvent};

use super::os;
use super::{Heard, KeyEv, Page, CAPTURE};

/// How much of the page, from each edge, lies past the monitor's edge
/// (so its edge is easy to reach): across, and up and down.
const MARGIN: (f64, f64) = (0.04, 0.06);
/// How long a menu just opened ignores Esc (ms).
const MENU_SETTLE: f64 = 400.0;

/// Whether a key's own use in the browser is held back at the desk: all
/// but the function keys and the browser's own shortcuts (reload, a tab,
/// a window).
fn hold_back(k: &KeyboardEvent) -> bool {
    let code = k.code();
    if k.meta_key() {
        return false;
    }
    let function = code.len() <= 3 && code.starts_with('F') && code[1..].parse::<u8>().is_ok();
    if function {
        return false;
    }
    if k.ctrl_key() {
        return !matches!(
            k.key().to_ascii_lowercase().as_str(),
            "r" | "w" | "t" | "n" | "tab" | "pageup" | "pagedown" | "+" | "-" | "=" | "0"
        );
    }
    true
}

/// Listen for keys anywhere, the wheel on the canvas, the page losing
/// the keyboard.
pub(super) fn listen(heard: &Rc<RefCell<Heard>>, canvas: &HtmlCanvasElement) {
    for (name, down) in [("keydown", true), ("keyup", false)] {
        let heard = heard.clone();
        kit::on(&kit::window(), name, move |e| {
            let Some(k) = e.dyn_ref::<KeyboardEvent>() else {
                return;
            };
            if CAPTURE.with(|c| c.get()) && hold_back(k) {
                e.prevent_default();
            }
            let mut h = heard.borrow_mut();
            if h.keys.len() < 256 {
                h.keys.push(KeyEv {
                    code: k.code(),
                    key: k.key(),
                    down,
                    mods: os::mods(
                        k.shift_key(),
                        k.ctrl_key(),
                        k.alt_key(),
                        k.meta_key(),
                        k.get_modifier_state("AltGraph"),
                    ),
                    repeat: k.repeat(),
                });
            }
        });
    }
    let h = heard.clone();
    kit::on(canvas, "wheel", move |e| {
        if let Some(w) = e.dyn_ref::<WheelEvent>() {
            e.prevent_default();
            let mut h = h.borrow_mut();
            h.wheel += w.delta_y() as f32;
            h.wheel_shift |= w.shift_key();
        }
    });
    let h = heard.clone();
    kit::on(&kit::window(), "blur", move |_| h.borrow_mut().blur = true);
}

/// The date as the terminal says it, and the clock on the screen's bar.
pub(super) fn date() -> String {
    let d = js_sys::Date::new_0();
    String::from(d.to_string()).chars().take(24).collect()
}

pub(super) fn clock() -> String {
    let d = js_sys::Date::new_0();
    format!("{:02}:{:02}", d.get_hours(), d.get_minutes())
}

pub(super) fn input(p: &mut Page, now: f64) {
    let heard = std::mem::take(&mut *p.heard.borrow_mut());
    if heard.blur {
        for k in keys::layout() {
            p.hands.key(k.code, false);
        }
    }
    for e in heard.keys {
        key(p, e, now);
    }
    for h in p.pointer.drain() {
        pointer(p, h, now);
    }
    // The wheel scrolls the computer where it takes it; elsewhere (or
    // with Shift) it leans you in and out.
    if heard.wheel != 0.0 && !p.meta.is_open() {
        let taken = match &p.os {
            Some(o) if p.on_os() && !heard.wheel_shift => o.wheel(p.screen.cursor, heard.wheel),
            _ => false,
        };
        if !taken {
            p.lean_to = (p.lean_to - heard.wheel.signum() * LEAN_NOTCH).clamp(0.0, 1.0);
        }
    }
    typed_on_phone(p, now);
    let due: Vec<&'static str> = p
        .ups
        .iter()
        .filter(|(t, _)| *t <= now)
        .map(|(_, c)| *c)
        .collect();
    p.ups.retain(|(t, _)| *t > now);
    for code in due {
        if code == "#mouse" {
            p.hands.button(0, false);
            if let Some(o) = p.os.as_ref().filter(|_| p.on_os()) {
                o.pointer(os::UP, p.screen.cursor, 0);
            }
        } else {
            press(p, code, "", false, 0, false, now);
        }
    }
}

fn key(p: &mut Page, e: KeyEv, now: f64) {
    if e.down {
        p.wake();
    }
    // Esc: the menu; Esc again, back (a menu only just opened stays: a
    // held Esc repeats).
    if e.code == "Escape" {
        if e.down && !e.repeat {
            if !p.meta.is_open() {
                p.open_menu();
            } else if now - p.menu_at > MENU_SETTLE
                && p.meta.escape() == Some(kit::meta::Pick::Resume)
            {
                p.resume();
            }
        }
        return;
    }
    if p.meta.typing() {
        if e.down && e.code == "Enter" {
            p.meta.enter(now);
        }
        return;
    }
    if p.meta.is_open() {
        return;
    }
    let Some(code) = keys::find(&e.code).map(|k| k.code) else {
        // Not on this keyboard: the computer may still want it.
        if let Some(o) = p.os.as_ref().filter(|_| p.on_os()) {
            o.key(e.down, (&e.code, &e.key), e.mods, e.repeat);
        } else if e.down {
            let facts_gpu = p.out.caps().adapter.clone();
            run(p, &e.key, e.mods & 2 != 0, &facts_gpu, now);
        }
        return;
    };
    press(p, code, &e.key, e.down, e.mods, e.repeat, now);
}

/// A key of the desk's keyboard down or up: the hands, the light, the
/// sound, and the computer (or, going down, the desk's own terminal).
/// The modifiers are `mods` (as `os::mods` counts them).
fn press(
    p: &mut Page,
    code: &'static str,
    key: &str,
    down: bool,
    mods: u8,
    repeat: bool,
    now: f64,
) {
    if !repeat {
        p.hands.key(code, down);
        if let Some(i) = keys::layout().iter().position(|k| k.code == code) {
            if down {
                p.heat[i] = 1.0;
            }
            let x = keys::layout()[i].top()[0];
            p.sounds.key(code, x, down);
        }
    }
    if let Some(o) = p.os.as_ref().filter(|_| p.on_os()) {
        o.key(down, (code, key), mods, repeat);
    } else if down && !key.is_empty() {
        let gpu = p.out.caps().adapter.clone();
        run(p, key, mods & 2 != 0, &gpu, now);
    }
}

fn run(p: &mut Page, key: &str, ctrl: bool, gpu: &str, now: f64) {
    let when = date();
    let facts = Facts {
        gpu,
        when: &when,
        up: (now - p.since) / 1000.0,
        screen: SCREEN_PX,
    };
    match p.term.key(key, ctrl, &facts) {
        Some(Act::StandUp) => p.open_menu(),
        Some(Act::Open(url)) => {
            let _ = kit::window().open_with_url_and_target(url, "_blank");
        }
        None => {}
    }
}

/// The mouse moved (pixels): the arrow on the screen, the mouse on the
/// desk under it, and the right hand to it.
fn move_mouse(p: &mut Page, dx: f32, dy: f32) {
    p.screen.nudge(dx * CURSOR_SPEED, dy * CURSOR_SPEED);
    moved(p, dx.hypot(dy));
}

/// Your mouse is at (x, y) on the page (CSS pixels): the arrow is there on
/// the monitor.
fn point_mouse(p: &mut Page, x: f64, y: f64) {
    let (w, h) = p.out.css();
    let at = |v: f64, size: f64, m: f64, px: i32| {
        (((v / size.max(1.0) - m) / (1.0 - 2.0 * m)).clamp(0.0, 1.0) * (px as f64 - 1.0)) as f32
    };
    let to = (
        at(x, w, MARGIN.0, SCREEN_PX.0),
        at(y, h, MARGIN.1, SCREEN_PX.1),
    );
    let (dx, dy) = (to.0 - p.screen.cursor.0, to.1 - p.screen.cursor.1);
    p.screen.cursor = to;
    moved(p, dx.hypot(dy) / CURSOR_SPEED);
}

/// The arrow moved, `px` of your mouse: the desk's mouse under it, the
/// right hand to it, the computer told.
fn moved(p: &mut Page, px: f32) {
    p.hands.stir(px);
    let (x, z) = mouse_for(p.screen.cursor, SCREEN_PX);
    p.hands.put_mouse(x, z);
    if let Some(o) = p.os.as_ref().filter(|_| p.on_os()) {
        o.pointer(os::MOVE, p.screen.cursor, 0);
    }
}

fn pointer(p: &mut Page, h: Hand, now: f64) {
    match h {
        Hand::Mouse { x, y, .. } => {
            if !p.meta.is_open() && !p.touch {
                point_mouse(p, x, y);
            }
        }
        Hand::Button {
            button, down, x, y, ..
        } => {
            p.wake();
            if p.meta.is_open() {
                if down {
                    let (px, py) = p.out.to_px(x, y);
                    if let Some(Some(pick)) = p.meta.click(px, py, now) {
                        p.picked(pick);
                    }
                }
                return;
            }
            if !p.touch {
                point_mouse(p, x, y);
            }
            if button == 0 || button == 2 {
                p.hands.button(button, down);
                if down {
                    p.sounds.click();
                }
            }
            if let Some(o) = p.os.as_ref().filter(|_| p.on_os()) {
                o.pointer(
                    if down { os::DOWN } else { os::UP },
                    p.screen.cursor,
                    button,
                );
            }
        }
        Hand::Finger { kind, x, y, t, .. } => finger(p, kind, x, y, t, now),
    }
}

/// A phone: a finger dragged moves the mouse; a tap clicks it, and opens
/// the phone's keyboard to type with.
fn finger(p: &mut Page, kind: Kind, x: f64, y: f64, t: f64, now: f64) {
    p.wake();
    match kind {
        Kind::Down => {
            if p.meta.is_open() {
                let (px, py) = p.out.to_px(x, y);
                if let Some(Some(pick)) = p.meta.click(px, py, now) {
                    p.picked(pick);
                }
                return;
            }
            p.finger = Some((x, y, x, t));
        }
        Kind::Move => {
            if let Some((lx, ly, x0, t0)) = p.finger {
                if !p.meta.is_open() {
                    move_mouse(p, ((x - lx) * 1.4) as f32, ((y - ly) * 1.4) as f32);
                }
                p.finger = Some((x, y, x0, t0));
            }
        }
        _ => {
            if let Some((_, _, x0, t0)) = p.finger.take() {
                if t - t0 < 260.0 && (x - x0).abs() < 12.0 && !p.meta.is_open() {
                    p.hands.button(0, true);
                    if let Some(o) = p.os.as_ref().filter(|_| p.on_os()) {
                        o.pointer(os::DOWN, p.screen.cursor, 0);
                    }
                    p.sounds.click();
                    p.ups.push((now + 90.0, "#mouse"));
                    if let Some(f) = &p.field {
                        let (w, h) = p.out.css();
                        f.place(Some((w * 0.1, h - 60.0, w * 0.8, 40.0)));
                        f.focus();
                    }
                }
            }
        }
    }
}

/// What a phone's keyboard typed into the hidden field since last frame,
/// pressed on the desk key by key.
fn typed_on_phone(p: &mut Page, now: f64) {
    let Some(f) = &p.field else {
        return;
    };
    let v = f.value();
    if v.is_empty() {
        return;
    }
    f.set_value("");
    for (n, c) in v.chars().take(32).enumerate() {
        let s = c.to_string();
        match keys::for_char(c) {
            Some((code, shift)) => {
                press(
                    p,
                    code,
                    &s,
                    true,
                    os::mods(shift, false, false, false, false),
                    false,
                    now,
                );
                p.ups.push((now + 70.0 + n as f64 * 10.0, code));
            }
            None => match p.os.as_ref().filter(|_| p.on_os()) {
                Some(o) => o.key(true, ("", &s), 0, false),
                None => {
                    let gpu = p.out.caps().adapter.clone();
                    run(p, &s, false, &gpu, now);
                }
            },
        }
    }
}
