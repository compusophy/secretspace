//! Input for a game played with both hands: the keys held down (and
//! each new press), every finger at once with its own id, the mouse's
//! buttons each on its own, and its movement, locked or not (a first-person
//! look). Events keep the browser's own timestamps.

use std::cell::{Cell, RefCell};
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;

use wasm_bindgen::prelude::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{EventTarget, KeyboardEvent, MouseEvent, PointerEvent, WheelEvent};

thread_local! {
    /// The browser agreed to hold the keyboard (while full screen).
    static HELD: Cell<bool> = const { Cell::new(false) };
}

pub use crate::pointer::Kind;

/// Something a hand did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hand {
    /// A finger (or a pen): its id, what it did, where (CSS pixels).
    Finger {
        id: i32,
        kind: Kind,
        x: f64,
        y: f64,
        t: f64,
    },
    /// A mouse button pressed or let go: 0 left, 1 middle, 2 right.
    Button {
        button: i16,
        down: bool,
        x: f64,
        y: f64,
        t: f64,
    },
    /// The mouse moved, by (dx, dy), to (x, y).
    Mouse { x: f64, y: f64, dx: f64, dy: f64 },
}

#[derive(Default)]
struct Inner {
    queue: VecDeque<Hand>,
    held: HashSet<String>,
    pressed: Vec<String>,
}

/// The keyboard, the mouse and every finger, since the last `drain`.
pub struct Hands(Rc<RefCell<Inner>>);

/// An event's time on `kit::now`'s clock: its own stamp, unless that is
/// on some other clock (synthetic events can carry anything).
fn stamp(t: f64) -> f64 {
    let now = crate::now();
    if (now - t).abs() < 1000.0 {
        t
    } else {
        now
    }
}

fn push(i: &Rc<RefCell<Inner>>, h: Hand) {
    let mut i = i.borrow_mut();
    i.queue.push_back(h);
    if i.queue.len() > 1024 {
        i.queue.pop_front();
    }
}

impl Hands {
    /// Listen on `target` (the canvas) for presses; moves, lifts and keys
    /// are heard anywhere.
    pub fn attach(target: &EventTarget) -> Hands {
        let inner = Rc::new(RefCell::new(Inner::default()));
        let window: &EventTarget = &crate::window();
        for (on, name, kind) in [
            (target, "pointerdown", Kind::Down),
            (window, "pointermove", Kind::Move),
            (window, "pointerup", Kind::Up),
            (window, "pointercancel", Kind::Cancel),
        ] {
            let me = inner.clone();
            crate::on(on, name, move |e| {
                let Ok(e) = e.dyn_into::<PointerEvent>() else {
                    return;
                };
                // The mouse comes through its own events below.
                if e.pointer_type() == "mouse" {
                    return;
                }
                if kind == Kind::Down {
                    e.prevent_default();
                }
                push(
                    &me,
                    Hand::Finger {
                        id: e.pointer_id(),
                        kind,
                        x: e.client_x() as f64,
                        y: e.client_y() as f64,
                        t: stamp(e.time_stamp()),
                    },
                );
            });
        }
        for (on, name, down) in [(target, "mousedown", true), (window, "mouseup", false)] {
            let me = inner.clone();
            crate::on(on, name, move |e| {
                let Ok(e) = e.dyn_into::<MouseEvent>() else {
                    return;
                };
                // Playing, the buttons are the game's: the side ones do
                // not go back a page, the middle one does not scroll.
                if locked() {
                    e.prevent_default();
                }
                push(
                    &me,
                    Hand::Button {
                        button: e.button(),
                        down,
                        x: e.client_x() as f64,
                        y: e.client_y() as f64,
                        t: stamp(e.time_stamp()),
                    },
                );
            });
        }
        let me = inner.clone();
        crate::on(window, "mousemove", move |e| {
            let Ok(e) = e.dyn_into::<MouseEvent>() else {
                return;
            };
            push(
                &me,
                Hand::Mouse {
                    x: e.client_x() as f64,
                    y: e.client_y() as f64,
                    dx: e.movement_x() as f64,
                    dy: e.movement_y() as f64,
                },
            );
        });
        let me = inner.clone();
        crate::on(window, "keydown", move |e| {
            let Ok(e) = e.dyn_into::<KeyboardEvent>() else {
                return;
            };
            let code = e.code();
            // While the game has the pointer the keys are the game's:
            // nothing scrolls, bookmarks (Ctrl+D), saves, finds or prints.
            // F5, F11 and F12 still work; the browser keeps Ctrl+W, Ctrl+T
            // and Ctrl+N whatever a page does, so no game key needs Ctrl.
            if locked() && !matches!(code.as_str(), "F5" | "F11" | "F12") {
                e.prevent_default();
            }
            let mut i = me.borrow_mut();
            if !e.repeat() {
                i.pressed.push(code.clone());
            }
            i.held.insert(code);
        });
        let me = inner.clone();
        crate::on(window, "keyup", move |e| {
            if let Ok(e) = e.dyn_into::<KeyboardEvent>() {
                me.borrow_mut().held.remove(&e.code());
            }
        });
        // Leaving the window lets every key go.
        let me = inner.clone();
        crate::on(window, "blur", move |_| me.borrow_mut().held.clear());
        crate::on(target, "contextmenu", |e| e.prevent_default());
        // Nor does the wheel scroll or (with Ctrl) zoom the page. A wheel
        // listener on the window is passive unless it says otherwise.
        let opts = web_sys::AddEventListenerOptions::new();
        opts.set_passive(false);
        let wheel = Closure::<dyn FnMut(web_sys::Event)>::new(|e: web_sys::Event| {
            if let Ok(e) = e.dyn_into::<WheelEvent>() {
                if locked() || full() || e.ctrl_key() {
                    e.prevent_default();
                }
            }
        });
        let _ = window.add_event_listener_with_callback_and_add_event_listener_options(
            "wheel",
            wheel.as_ref().unchecked_ref(),
            &opts,
        );
        wheel.forget();
        Hands(inner)
    }

    /// Every hand event since the last call, oldest first.
    pub fn drain(&self) -> Vec<Hand> {
        self.0.borrow_mut().queue.drain(..).collect()
    }

    /// Keys pressed since the last call (`KeyboardEvent.code`: "KeyW").
    pub fn pressed(&self) -> Vec<String> {
        std::mem::take(&mut self.0.borrow_mut().pressed)
    }

    pub fn held(&self, code: &str) -> bool {
        self.0.borrow().held.contains(code)
    }
}

/// Take the mouse for looking around (it must come from a press).
pub fn lock(el: &web_sys::Element) {
    el.request_pointer_lock();
}

/// Take everything for playing (it must come from a press): the whole
/// screen, the keyboard where the browser allows it (Chrome and Edge, full
/// screen: then even Ctrl+W comes to the game, and Esc must be held to
/// leave), and the mouse.
pub fn play(el: &web_sys::Element) {
    let doc = crate::document();
    if !full() {
        if let Some(root) = doc.document_element() {
            let _ = root.request_fullscreen();
        }
    }
    hold_keys();
    lock(el);
}

/// Whether the page has the whole screen.
pub fn full() -> bool {
    crate::document().fullscreen_element().is_some()
}

/// Whether every key comes to the page (the keyboard held, full screen):
/// only then may a game use Ctrl, which otherwise closes tabs.
pub fn keys_held() -> bool {
    HELD.with(Cell::get) && full()
}

/// Ask for the keyboard (`navigator.keyboard.lock()`, where there is one).
fn hold_keys() {
    let nav: JsValue = crate::window().navigator().into();
    let Ok(kb) = js_sys::Reflect::get(&nav, &"keyboard".into()) else {
        return;
    };
    let Ok(lock) = js_sys::Reflect::get(&kb, &"lock".into()) else {
        return;
    };
    let Some(lock) = lock.dyn_ref::<js_sys::Function>() else {
        return;
    };
    let Ok(promise) = lock.call0(&kb) else {
        return;
    };
    if let Ok(promise) = promise.dyn_into::<js_sys::Promise>() {
        let ok = Closure::once(|_: JsValue| HELD.with(|h| h.set(true)));
        let no = Closure::once(|_: JsValue| HELD.with(|h| h.set(false)));
        let _ = promise.then2(&ok, &no);
        ok.forget();
        no.forget();
    }
}

pub fn unlock() {
    crate::document().exit_pointer_lock();
}

/// Whether the page has the mouse.
pub fn locked() -> bool {
    crate::document().pointer_lock_element().is_some()
}
