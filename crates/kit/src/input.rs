//! Input for a game played with both hands: the keys held down (and
//! each new press), every finger at once with its own id, the mouse's
//! buttons each on its own, and its movement, locked or not (a first-person
//! look). Events keep the browser's own timestamps.

use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;

use wasm_bindgen::JsCast;
use web_sys::{EventTarget, KeyboardEvent, MouseEvent, PointerEvent};

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
            // Keys that would scroll or leave the page do neither while
            // the game has the pointer.
            if locked() && matches!(code.as_str(), "Space" | "Tab" | "Backquote") {
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

pub fn unlock() {
    crate::document().exit_pointer_lock();
}

/// Whether the page has the mouse.
pub fn locked() -> bool {
    crate::document().pointer_lock_element().is_some()
}
