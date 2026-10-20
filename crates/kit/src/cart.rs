//! A cartridge's half of the protocol (v1), the part every game shares:
//! the surface its host copies (an `OffscreenCanvas` that keeps the last
//! picture until the next), how many pictures it has had, the cursor
//! asked for, and the host's pointer and keys as a game's `input::Hands`
//! hear them. `cartridge!` writes a game's exported `Cartridge` class over
//! this: the game gives only its start, its input and its end.
//!
//! The class, as a host uses it: `new Cartridge(w, h, dpr, base, ns,
//! opts)` (CSS pixels, device pixels per CSS pixel, where its files are,
//! what its keys begin with, `{ v: 1, query, ... }` as an object or JSON);
//! then, every frame and never throwing, `frames()` (asking is the host's
//! poll: unasked, it stops drawing) and `surface()` to copy when the count
//! moved, `width()` and `height()` (the surface's pixels), `resize`,
//! `visible`, `pointer(kind, x, y, button)` (kind `DOWN`, `MOVE`, `UP`,
//! `LEFT`; CSS pixels in the surface; the DOM's buttons), `wheel(x, y,
//! dy)`, `key(down, code, key, mods, repeat)` (mods `SHIFT` ... `ALTGR`),
//! `text` and `typing`, `cursor`, `capture` and `locked` and `motion` (the
//! pointer lock: asked for, given, its steps), `closed`, `error` and
//! `end`. A constructor that cannot run its game (options it cannot read;
//! this module instance runs one already) still makes an object, which
//! draws nothing and says why in `error()`; so does nothing from an
//! object whose cartridge was let go.

use std::cell::RefCell;

use pixels::Canvas;
use wasm_bindgen::{Clamped, JsCast, JsValue};
use web_sys::{ImageData, OffscreenCanvas, OffscreenCanvasRenderingContext2d};

use crate::host;
use crate::input::{Hand, Hands};

/// The protocol this end speaks.
pub const PROTOCOL: u32 = 1;

/// What the host's pointer did (`pointer`'s kind).
pub const DOWN: u8 = 0;
pub const MOVE: u8 = 1;
pub const UP: u8 = 2;
pub const LEFT: u8 = 3;

/// The modifier keys held (`key`'s mods).
pub const SHIFT: u8 = 1;
pub const CTRL: u8 = 2;
pub const ALT: u8 = 4;
pub const META: u8 = 8;
pub const ALTGR: u8 = 16;

/// An offscreen canvas and its 2D context.
struct Plane {
    canvas: OffscreenCanvas,
    ctx: OffscreenCanvasRenderingContext2d,
}

impl Plane {
    /// `have`, sized to `size` if it is not, or a new one (none if the
    /// browser will not make one).
    fn sized(have: Option<Plane>, size: (u32, u32)) -> Option<Plane> {
        if let Some(p) = have {
            if (p.canvas.width(), p.canvas.height()) != size {
                p.canvas.set_width(size.0);
                p.canvas.set_height(size.1);
            }
            return Some(p);
        }
        let canvas = OffscreenCanvas::new(size.0, size.1).ok()?;
        let ctx = canvas.get_context("2d").ok()??.dyn_into().ok()?;
        Some(Plane { canvas, ctx })
    }

    /// `px` at its top left, pixel for pixel.
    fn put(&self, px: &Canvas) -> bool {
        ImageData::new_with_u8_clamped_array_and_sh(Clamped(&px.data), px.w as u32, px.h as u32)
            .and_then(|img| self.ctx.put_image_data(&img, 0.0, 0.0))
            .is_ok()
    }
}

#[derive(Default)]
struct Cart {
    /// The surface the host copies.
    out: Option<Plane>,
    /// A picture shown bigger is put here first, then drawn onto `out`.
    small: Option<Plane>,
    frames: u32,
    /// The surface's size, device pixels.
    size: (u32, u32),
    cursor: String,
    hands: Option<Hands>,
    /// Where the host's pointer was last (CSS pixels).
    at: Option<(f64, f64)>,
    /// The options as an object (a module namespace in one is no JSON).
    opts: Option<JsValue>,
}

thread_local! {
    static CART: RefCell<Cart> = RefCell::new(Cart::default());
}

/// Read the state, or `or` if it cannot be now (these are reached from a
/// cartridge's exports: never a panic).
fn read<R>(or: R, f: impl FnOnce(&Cart) -> R) -> R {
    CART.try_with(|c| c.try_borrow().ok().map(|c| f(&c)))
        .ok()
        .flatten()
        .unwrap_or(or)
}

fn write(f: impl FnOnce(&mut Cart)) {
    let _ = CART.try_with(|c| {
        if let Ok(mut c) = c.try_borrow_mut() {
            f(&mut c);
        }
    });
}

/// A constructor's arguments taken: hosted from now on (`host::enter`),
/// nothing shown yet. The hosting's era (calls from an object of another
/// era reach nothing: `current`), or why the game cannot run here.
pub fn begin(
    css: (f64, f64),
    dpr: f64,
    base: String,
    ns: String,
    opts: &JsValue,
) -> Result<u32, String> {
    let given = host::opts(opts)?;
    host::enter(host::Hosted {
        css,
        dpr,
        base,
        ns,
        opts: given,
    })?;
    let obj = match opts.as_string() {
        Some(text) => js_sys::JSON::parse(&text).ok(),
        None => Some(opts.clone()),
    };
    write(|c| {
        *c = Cart {
            opts: obj,
            ..Cart::default()
        }
    });
    Ok(host::era())
}

/// Whether an object made in `era` is the cartridge running now (not one
/// refused, which has era 0, nor one let go).
pub fn current(era: u32) -> bool {
    era != 0 && era == host::era() && !host::ended()
}

/// The host let the cartridge go: ended (`host::end`), its surface, hands
/// and options let go.
pub fn finish() {
    host::end();
    write(|c| *c = Cart::default());
}

/// An option as the host gave it (`osModule`: a module, no JSON), or
/// undefined.
pub fn option(name: &str) -> JsValue {
    read(None, |c| c.opts.clone())
        .and_then(|o| js_sys::Reflect::get(&o, &name.into()).ok())
        .unwrap_or(JsValue::UNDEFINED)
}

/// Pictures shown so far. Asking is the host's poll (`host::polled`): a
/// host that stops asking stops the drawing.
pub fn frames() -> u32 {
    host::polled(crate::now());
    read(0, |c| c.frames)
}

/// The surface to copy (an `OffscreenCanvas`), or null before the first
/// picture.
pub fn surface() -> JsValue {
    read(JsValue::NULL, |c| {
        c.out
            .as_ref()
            .map_or(JsValue::NULL, |p| p.canvas.clone().into())
    })
}

/// The surface's size, device pixels (0 before the first picture).
pub fn size() -> (u32, u32) {
    read((0, 0), |c| c.size)
}

/// The CSS cursor asked for over the surface ("" for the host's own).
pub fn cursor() -> String {
    read(String::new(), |c| c.cursor.clone())
}

pub fn set_cursor(cursor: &str) {
    write(|c| {
        if c.cursor != cursor {
            c.cursor = cursor.to_string();
        }
    });
}

/// A picture onto the surface, one more frame for the host: each of its
/// pixels `k` device pixels square (1: as it is), on a surface `size`
/// device pixels (what spills past is cut). Never once the host let go.
pub fn show(px: &Canvas, k: u32, size: (u32, u32)) {
    if px.w <= 0 || px.h <= 0 || !host::hosted() || host::ended() {
        return;
    }
    let own = (px.w as u32, px.h as u32);
    write(|c| {
        if k <= 1 && own == size {
            c.out = Plane::sized(c.out.take(), own);
            if !c.out.as_ref().is_some_and(|p| p.put(px)) {
                return;
            }
        } else {
            c.small = Plane::sized(c.small.take(), own);
            c.out = Plane::sized(c.out.take(), size);
            let (Some(small), Some(out)) = (&c.small, &c.out) else {
                return;
            };
            if !small.put(px) {
                return;
            }
            // Sharp: a pixel is a square, never a blur.
            out.ctx.set_image_smoothing_enabled(false);
            let k = f64::from(k.max(1));
            let drawn = out.ctx.draw_image_with_offscreen_canvas_and_dw_and_dh(
                &small.canvas,
                0.0,
                0.0,
                f64::from(own.0) * k,
                f64::from(own.1) * k,
            );
            if drawn.is_err() {
                return;
            }
        }
        c.size = (
            c.out.as_ref().map_or(0, |p| p.canvas.width()),
            c.out.as_ref().map_or(0, |p| p.canvas.height()),
        );
        c.frames = c.frames.wrapping_add(1);
    });
}

/// Hands for a game run by a host: they hear nothing themselves; the
/// host's pointer, keys and steps come to them (`to_hands`,
/// `key_to_hands`, `motion_to_hands`).
pub fn hands() -> Hands {
    let h = Hands::detached();
    write(|c| c.hands = Some(h.clone()));
    h
}

/// The hands the host's input goes to, and where its pointer last was.
fn heard() -> Option<(Hands, Option<(f64, f64)>)> {
    read(None, |c| c.hands.clone().map(|h| (h, c.at)))
}

/// The host's pointer, as a mouse's hands hear it (`hand`). Whether there
/// were hands to hear it.
pub fn to_hands(kind: u8, x: f64, y: f64, button: u8) -> bool {
    let Some((hands, last)) = heard() else {
        return false;
    };
    let h = hand(kind, x, y, button, last, host::locked(), crate::now());
    write(|c| {
        c.at = match kind {
            LEFT => None,
            _ if h.is_some() => Some((x, y)),
            _ => c.at,
        }
    });
    if let Some(h) = h {
        hands.push(h);
    }
    true
}

/// The pointer's event as a mouse's (pure): a press or a lift is a button
/// (0 main, 1 middle, 2 secondary), a move is the mouse moved by its step
/// from `last` (none for the first; none while `locked`, when the steps
/// come by `motion`), leaving is nothing; nor is a place not a number.
pub fn hand(
    kind: u8,
    x: f64,
    y: f64,
    button: u8,
    last: Option<(f64, f64)>,
    locked: bool,
    t: f64,
) -> Option<Hand> {
    if !(x.is_finite() && y.is_finite()) {
        return None;
    }
    match kind {
        DOWN | UP => Some(Hand::Button {
            button: i16::from(button),
            down: kind == DOWN,
            x,
            y,
            t,
        }),
        MOVE => {
            let (dx, dy) = match last {
                Some((lx, ly)) if !locked => (x - lx, y - ly),
                _ => (0.0, 0.0),
            };
            Some(Hand::Mouse { x, y, dx, dy })
        }
        _ => None,
    }
}

/// The locked pointer moved by (dx, dy) CSS pixels: the mouse's step, from
/// where it last was.
pub fn motion_to_hands(dx: f64, dy: f64) {
    if !(dx.is_finite() && dy.is_finite()) {
        return;
    }
    if let Some((hands, at)) = heard() {
        let (x, y) = at.unwrap_or((0.0, 0.0));
        hands.push(Hand::Mouse { x, y, dx, dy });
    }
}

/// A key from the host, down or up, as the hands hear one. Whether there
/// were hands to hear it.
pub fn key_to_hands(down: bool, code: &str, repeat: bool) -> bool {
    match heard() {
        Some((hands, _)) => {
            hands.key(code, down, repeat);
            true
        }
        None => false,
    }
}

/// Whether a key is a game's to keep from its host (pure): not a function
/// key, nor with Ctrl or Meta held (the host's and the browser's own).
pub fn game_key(code: &str, mods: u8) -> bool {
    let function = code
        .strip_prefix('F')
        .is_some_and(|n| (1..=2).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_digit()));
    !function && mods & (CTRL | META) == 0
}

/// A game's exported `Cartridge` class (the protocol above), over the
/// module `$game`, which has `start()` (run the game hosted; called once
/// the constructor has entered), `pointer(kind, x, y, button) -> bool`,
/// `wheel(x, y, dy) -> bool`, `key(down, code, key, mods, repeat) ->
/// bool` (each: whether the game took it), `motion(dx, dy)`, `resized()`
/// and `end()` (its link, its device and its sound let go). The rest is
/// the same for every game, here: a typed field's keys, the text, the
/// lock, the surface, closing and failing.
#[macro_export]
macro_rules! cartridge {
    ($($game:ident)::+) => {
        /// This game run inside another page, as a cartridge (protocol
        /// v1): see `kit::cart`.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub struct Cartridge {
            era: u32,
            why: ::std::string::String,
        }

        #[::wasm_bindgen::prelude::wasm_bindgen]
        impl Cartridge {
            /// The game on a surface `w` x `h` CSS pixels at `dpr`, its
            /// files under `base`, its keys under `ns`; `opts` the host's.
            #[::wasm_bindgen::prelude::wasm_bindgen(constructor)]
            pub fn new(
                w: f64,
                h: f64,
                dpr: f64,
                base: ::std::string::String,
                ns: ::std::string::String,
                opts: ::wasm_bindgen::JsValue,
            ) -> Cartridge {
                match $crate::cart::begin((w, h), dpr, base, ns, &opts) {
                    Ok(era) => {
                        $($game)::+::start();
                        Cartridge {
                            era,
                            why: ::std::string::String::new(),
                        }
                    }
                    Err(why) => Cartridge { era: 0, why },
                }
            }

            /// The `OffscreenCanvas` drawn into (null before the first
            /// picture): copy it when `frames()` moved.
            pub fn surface(&self) -> ::wasm_bindgen::JsValue {
                if $crate::cart::current(self.era) {
                    $crate::cart::surface()
                } else {
                    ::wasm_bindgen::JsValue::NULL
                }
            }

            /// Pictures drawn so far; asking is the host's poll.
            pub fn frames(&self) -> u32 {
                if $crate::cart::current(self.era) {
                    $crate::cart::frames()
                } else {
                    0
                }
            }

            /// The surface's width, its pixels.
            pub fn width(&self) -> u32 {
                if $crate::cart::current(self.era) {
                    $crate::cart::size().0
                } else {
                    0
                }
            }

            /// The surface's height, its pixels.
            pub fn height(&self) -> u32 {
                if $crate::cart::current(self.era) {
                    $crate::cart::size().1
                } else {
                    0
                }
            }

            /// The surface is now `w` x `h` CSS pixels at `dpr`.
            pub fn resize(&self, w: f64, h: f64, dpr: f64) {
                if $crate::cart::current(self.era) {
                    $crate::host::resize((w, h), dpr);
                    $($game)::+::resized();
                }
            }

            /// Whether the host shows it (hidden, it draws nothing).
            pub fn visible(&self, on: bool) {
                if $crate::cart::current(self.era) {
                    $crate::host::shown(on);
                }
            }

            /// The pointer: kind 0 down, 1 moved, 2 up, 3 left; CSS pixels
            /// in the surface; the DOM's button. Whether it was taken.
            pub fn pointer(&self, kind: u8, x: f64, y: f64, button: u8) -> bool {
                $crate::cart::current(self.era) && $($game)::+::pointer(kind, x, y, button)
            }

            /// The wheel, `dy` CSS pixels down. Whether it was taken.
            pub fn wheel(&self, x: f64, y: f64, dy: f64) -> bool {
                $crate::cart::current(self.era) && $($game)::+::wheel(x, y, dy)
            }

            /// A key down or up (`KeyboardEvent`'s code and key; mods
            /// Shift 1, Ctrl 2, Alt 4, Meta 8, AltGr 16). Whether it was
            /// taken.
            pub fn key(
                &self,
                down: bool,
                code: ::std::string::String,
                key: ::std::string::String,
                mods: u8,
                repeat: bool,
            ) -> bool {
                if !$crate::cart::current(self.era) {
                    return false;
                }
                let typing = $crate::host::typing();
                if typing && down && $crate::host::edit(&code) {
                    return true;
                }
                $($game)::+::key(down, &code, &key, mods, repeat) || typing
            }

            /// Text typed while `typing()`.
            pub fn text(&self, text: ::std::string::String) {
                if $crate::cart::current(self.era) {
                    $crate::host::text(&text);
                }
            }

            /// Whether a text field of it has the keys.
            pub fn typing(&self) -> bool {
                $crate::cart::current(self.era) && $crate::host::typing()
            }

            /// The CSS cursor over the surface ("" the host's own).
            pub fn cursor(&self) -> ::std::string::String {
                if $crate::cart::current(self.era) {
                    $crate::cart::cursor()
                } else {
                    ::std::string::String::new()
                }
            }

            /// Whether it wants the pointer locked (held, too).
            pub fn capture(&self) -> bool {
                $crate::cart::current(self.era) && $crate::host::wants_lock()
            }

            /// The host locked the pointer to it, or let it go.
            pub fn locked(&self, on: bool) {
                if $crate::cart::current(self.era) {
                    $crate::host::set_locked(on);
                }
            }

            /// The locked pointer moved by (dx, dy) CSS pixels.
            pub fn motion(&self, dx: f64, dy: f64) {
                if $crate::cart::current(self.era) {
                    $($game)::+::motion(dx, dy);
                }
            }

            /// Whether it asked to be closed (its Exit).
            pub fn closed(&self) -> bool {
                $crate::cart::current(self.era) && $crate::host::closed()
            }

            /// Why it stopped ("" while fine).
            pub fn error(&self) -> ::std::string::String {
                if self.era == 0 {
                    self.why.clone()
                } else if $crate::cart::current(self.era) {
                    $crate::host::error()
                } else {
                    ::std::string::String::new()
                }
            }

            /// The host let it go: it stops drawing, its link and device
            /// let go.
            pub fn end(&self) {
                if $crate::cart::current(self.era) {
                    $($game)::+::end();
                    $crate::cart::finish();
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hosts_pointer_is_a_mouse() {
        let press = hand(DOWN, 10.0, 20.0, 2, None, false, 5.0);
        assert_eq!(
            press,
            Some(Hand::Button {
                button: 2,
                down: true,
                x: 10.0,
                y: 20.0,
                t: 5.0
            })
        );
        assert!(matches!(
            hand(UP, 1.0, 1.0, 0, None, false, 0.0),
            Some(Hand::Button {
                button: 0,
                down: false,
                ..
            })
        ));
        // A move is a step from where it was; the first is no step.
        assert_eq!(
            hand(MOVE, 15.0, 18.0, 0, Some((10.0, 20.0)), false, 0.0),
            Some(Hand::Mouse {
                x: 15.0,
                y: 18.0,
                dx: 5.0,
                dy: -2.0
            })
        );
        assert_eq!(
            hand(MOVE, 15.0, 18.0, 0, None, false, 0.0),
            Some(Hand::Mouse {
                x: 15.0,
                y: 18.0,
                dx: 0.0,
                dy: 0.0
            })
        );
        // Locked, the steps come by `motion`: a move only says where.
        assert_eq!(
            hand(MOVE, 15.0, 18.0, 0, Some((10.0, 20.0)), true, 0.0),
            Some(Hand::Mouse {
                x: 15.0,
                y: 18.0,
                dx: 0.0,
                dy: 0.0
            })
        );
        assert_eq!(hand(LEFT, 1.0, 1.0, 0, None, false, 0.0), None);
        assert_eq!(hand(9, 1.0, 1.0, 0, None, false, 0.0), None);
        assert_eq!(hand(DOWN, f64::NAN, 1.0, 0, None, false, 0.0), None);
    }

    #[test]
    fn a_games_keys_leave_the_hosts_alone() {
        assert!(game_key("KeyW", 0) && game_key("KeyW", SHIFT | ALT));
        assert!(game_key("Escape", 0) && game_key("Space", ALTGR));
        assert!(!game_key("KeyW", CTRL) && !game_key("KeyR", META));
        assert!(!game_key("F5", 0) && !game_key("F12", SHIFT));
        // Not a function key, though it begins with F.
        assert!(game_key("Fn", 0) && game_key("F", 0) && game_key("F123", 0));
    }

    #[test]
    fn a_cartridge_refused_or_let_go_is_not_current() {
        host::end();
        assert!(!current(0));
        host::enter(host::Hosted {
            css: (960.0, 600.0),
            dpr: 1.0,
            base: "http://127.0.0.1:8787/wyrm/pkg/".into(),
            ns: "t.".into(),
            opts: host::Opts::default(),
        })
        .unwrap();
        let era = host::era();
        assert!(current(era) && !current(0) && !current(era + 1));
        host::end();
        assert!(!current(era), "let go");
        assert_eq!(size(), (0, 0));
        assert_eq!(cursor(), "");
        set_cursor("crosshair");
        assert_eq!(cursor(), "crosshair");
    }
}
