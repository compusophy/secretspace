//! One pointer at a time, as the game sees it: presses, moves and lifts in
//! CSS pixels with the browser's own timestamps (so a gesture is read from
//! when things happened, not when a frame came round). A second finger or
//! button while one is down is ignored until the first lifts; a mouse's
//! hover comes through too, for the latched stick.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use web_sys::{EventTarget, PointerEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Down,
    Move,
    Up,
    /// The browser took the pointer away (a system gesture, say).
    Cancel,
    /// A mouse or pen moving with nothing pressed.
    Hover,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Press {
    pub kind: Kind,
    pub x: f64,
    pub y: f64,
    /// Milliseconds, on the same clock as `kit::now`.
    pub t: f64,
    /// A finger (else a mouse or a pen).
    pub touch: bool,
    /// The secondary button (a right-click).
    pub second: bool,
}

#[derive(Default)]
struct Inner {
    queue: VecDeque<Press>,
    /// The pointer that is down, and whether it is the secondary button.
    down: Option<(i32, bool)>,
}

pub struct Pointer(Rc<RefCell<Inner>>);

impl Pointer {
    /// Listen on `target` (the canvas); lifts are heard anywhere.
    pub fn attach(target: &EventTarget) -> Pointer {
        let inner = Rc::new(RefCell::new(Inner::default()));
        let window: &EventTarget = &crate::window();
        let listen = |on: &EventTarget, name: &str, kind: Kind| {
            let me = inner.clone();
            crate::on(on, name, move |e| {
                let Ok(e) = e.dyn_into::<PointerEvent>() else {
                    return;
                };
                let mut i = me.borrow_mut();
                let id = e.pointer_id();
                let touch = e.pointer_type() == "touch";
                let held = i.down;
                let second = held.map_or(e.button() == 2, |d| d.1);
                let kind = match kind {
                    Kind::Down if held.is_none() => {
                        i.down = Some((id, second));
                        Kind::Down
                    }
                    Kind::Down => return,
                    Kind::Move => match held {
                        Some((d, _)) if d == id => Kind::Move,
                        None if !touch => Kind::Hover,
                        _ => return,
                    },
                    Kind::Up | Kind::Cancel => match held {
                        Some((d, _)) if d == id => {
                            i.down = None;
                            kind
                        }
                        _ => return,
                    },
                    Kind::Hover => return,
                };
                i.queue.push_back(Press {
                    kind,
                    x: e.client_x() as f64,
                    y: e.client_y() as f64,
                    t: e.time_stamp(),
                    touch,
                    second,
                });
                if i.queue.len() > 512 {
                    i.queue.pop_front();
                }
            });
        };
        listen(target, "pointerdown", Kind::Down);
        listen(target, "pointermove", Kind::Move);
        listen(window, "pointerup", Kind::Up);
        listen(window, "pointercancel", Kind::Cancel);
        // A right-click is the Heart, not a menu.
        crate::on(target, "contextmenu", |e| e.prevent_default());
        Pointer(inner)
    }

    /// Everything since the last call, oldest first.
    pub fn drain(&self) -> Vec<Press> {
        self.0.borrow_mut().queue.drain(..).collect()
    }

    pub fn is_down(&self) -> bool {
        self.0.borrow().down.is_some()
    }
}

/// The latched stick (mouse and pen only): a slow lift from a run leaves
/// the stick where it was, and hover keeps steering it from the same
/// origin; bringing the pointer back near the origin, or pressing again,
/// lets it go.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Latch {
    pub origin: Option<(f64, f64)>,
}

impl Latch {
    /// Back within this many CSS px of the origin lets go.
    pub const RELEASE: f64 = 10.0;

    pub fn latch(&mut self, origin: (f64, f64)) {
        self.origin = Some(origin);
    }

    /// Hover at (x, y): the stick, from its origin, while it is latched.
    pub fn hover(&mut self, x: f64, y: f64) -> Option<(f64, f64)> {
        let (ox, oy) = self.origin?;
        let (dx, dy) = (x - ox, y - oy);
        if dx * dx + dy * dy < Self::RELEASE * Self::RELEASE {
            self.origin = None;
            return None;
        }
        Some((dx, dy))
    }

    /// A press: whether it was latched (the press is then read as a lift
    /// as well as a new gesture).
    pub fn press(&mut self) -> bool {
        self.origin.take().is_some()
    }
}
