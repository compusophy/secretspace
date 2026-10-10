//! Your hands: keys to the desk's keyboard and the terminal, the mouse
//! to the desk's mouse and the screen's arrow, the wheel to lean in; a
//! phone's taps, drags and keyboard; the demo's hands before you sit;
//! the shared menu (Esc, or the mouse let go).

use std::cell::RefCell;
use std::rc::Rc;

use battlestation::demo::Ev;
use battlestation::hands::mouse_for;
use battlestation::keys;
use battlestation::laws::{CURSOR_SPEED, LEAN_NOTCH, SCREEN_PX};
use battlestation::term::{Act, Facts};
use kit::input::{Hand, Kind};
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, KeyboardEvent, WheelEvent};

use super::{Heard, KeyEv, Page, CAPTURE};

/// The most one mouse move may move the arrow (pixels).
const MOUSE_MOST: f64 = 250.0;

/// Whether a key's own use in the browser is held back while seated: all
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
                    ctrl: k.ctrl_key(),
                    repeat: k.repeat(),
                });
            }
        });
    }
    let h = heard.clone();
    kit::on(canvas, "wheel", move |e| {
        if let Some(w) = e.dyn_ref::<WheelEvent>() {
            e.prevent_default();
            h.borrow_mut().wheel += w.delta_y() as f32;
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

pub(super) fn input(p: &mut Page, now: f64, dt: f32) {
    // The mouse let go while seated (the browser's own Esc, a switch
    // away): the menu, as if Esc had come to the page.
    let locked = kit::input::locked();
    if locked != p.was_locked {
        p.skip = 2;
    }
    if p.was_locked && !locked && p.seated && !p.touch && !p.meta.is_open() {
        p.open_menu();
    }
    p.was_locked = locked;
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
    if heard.wheel != 0.0 && p.seated && !p.meta.is_open() {
        p.lean_to = (p.lean_to - heard.wheel.signum() * LEAN_NOTCH).clamp(0.0, 1.0);
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
        } else {
            press(p, code, "", false, false, false, now);
        }
    }
    if let Some(mut demo) = p.demo.take() {
        for ev in demo.step(dt) {
            match ev {
                Ev::Key { code, key, down } => press(p, code, &key, down, false, false, now),
                Ev::Move { dx, dy } => move_mouse(p, dx, dy),
                Ev::Button { b, down } => p.hands.button(b, down),
            }
        }
        p.demo = Some(demo);
    }
}

fn key(p: &mut Page, e: KeyEv, now: f64) {
    if e.down {
        p.sounds.audio.wake();
    }
    if e.code == "Escape" {
        if e.down && !e.repeat {
            if p.meta.is_open() {
                if p.meta.escape() == Some(kit::meta::Pick::Resume) {
                    p.resume();
                }
            } else if p.seated {
                p.open_menu();
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
    if !p.seated {
        if !e.down {
            return;
        }
        // A key sits you down (the mouse waits for a click).
        p.sit(false);
    }
    let Some(code) = keys::find(&e.code).map(|k| k.code) else {
        // Not on this keyboard: the terminal may still want it.
        if e.down {
            let facts_gpu = p.out.caps().adapter.clone();
            run(p, &e.key, e.ctrl, &facts_gpu, now);
        }
        return;
    };
    press(p, code, &e.key, e.down, e.ctrl, e.repeat, now);
}

/// A key of the desk's keyboard down or up: the hands, the light, the
/// sound, and (going down) the terminal.
fn press(
    p: &mut Page,
    code: &'static str,
    key: &str,
    down: bool,
    ctrl: bool,
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
    if down && !key.is_empty() {
        let gpu = p.out.caps().adapter.clone();
        run(p, key, ctrl, &gpu, now);
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
        Some(Act::StandUp) => p.stand(),
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
    p.hands.stir(dx.hypot(dy));
    let (x, z) = mouse_for(p.screen.cursor, SCREEN_PX);
    p.hands.put_mouse(x, z);
}

fn pointer(p: &mut Page, h: Hand, now: f64) {
    match h {
        Hand::Mouse { x, y, dx, dy } => {
            // Some browsers (and test drivers) give no movement unless the
            // mouse is held: then it is where it went from where it was.
            let (dx, dy) = match p.last_mouse {
                Some((lx, ly)) if dx == 0.0 && dy == 0.0 && !kit::input::locked() => {
                    (x - lx, y - ly)
                }
                _ => (dx, dy),
            };
            let first = p.last_mouse.is_none();
            p.last_mouse = Some((x, y));
            // The first move, and those just after the mouse is taken or
            // let go, can carry the whole way the cursor jumped.
            if first || p.skip > 0 {
                p.skip = p.skip.saturating_sub(1);
                return;
            }
            if p.seated && !p.meta.is_open() && !p.touch {
                let most = MOUSE_MOST;
                move_mouse(
                    p,
                    dx.clamp(-most, most) as f32,
                    dy.clamp(-most, most) as f32,
                );
            }
        }
        Hand::Button {
            button, down, x, y, ..
        } => {
            p.sounds.audio.wake();
            if p.meta.is_open() {
                if down {
                    let (px, py) = p.out.to_px(x, y);
                    if let Some(Some(pick)) = p.meta.click(px, py, now) {
                        p.picked(pick);
                    }
                }
                return;
            }
            if !p.seated {
                if down {
                    p.sit(true);
                }
                return;
            }
            if down && !p.touch && !kit::input::locked() {
                kit::input::play(p.out.canvas(), !p.hooks.windowed);
            }
            if button == 0 || button == 2 {
                p.hands.button(button, down);
                if down {
                    p.sounds.click();
                }
            }
        }
        Hand::Finger { kind, x, y, t, .. } => finger(p, kind, x, y, t, now),
    }
}

/// A phone: a finger dragged moves the mouse; a tap clicks it, and opens
/// the phone's keyboard to type with.
fn finger(p: &mut Page, kind: Kind, x: f64, y: f64, t: f64, now: f64) {
    p.sounds.audio.wake();
    match kind {
        Kind::Down => {
            if p.meta.is_open() {
                let (px, py) = p.out.to_px(x, y);
                if let Some(Some(pick)) = p.meta.click(px, py, now) {
                    p.picked(pick);
                }
                return;
            }
            if !p.seated {
                p.sit(false);
            }
            p.finger = Some((x, y, x, t));
        }
        Kind::Move => {
            if let Some((lx, ly, x0, t0)) = p.finger {
                if p.seated && !p.meta.is_open() {
                    move_mouse(p, ((x - lx) * 1.4) as f32, ((y - ly) * 1.4) as f32);
                }
                p.finger = Some((x, y, x0, t0));
            }
        }
        _ => {
            if let Some((_, _, x0, t0)) = p.finger.take() {
                if t - t0 < 260.0 && (x - x0).abs() < 12.0 && p.seated {
                    p.hands.button(0, true);
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
            Some((code, _)) => {
                press(p, code, &s, true, false, false, now);
                p.ups.push((now + 70.0 + n as f64 * 10.0, code));
            }
            None => {
                let gpu = p.out.caps().adapter.clone();
                run(p, &s, false, &gpu, now);
            }
        }
    }
}
