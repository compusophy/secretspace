//! A game in the shell. The front page opens a game over itself, in a
//! frame filling the screen (`open`): on the very press that chose it, it
//! takes the whole screen and the keyboard, so the game's start page is
//! already full screen (a new page would lose it). The address follows
//! (`/wandfall/`), so the browser's back works. The game leaves with
//! `exit` (from its menu): the front page takes the frame down and gives
//! the screen and the keyboard back. A game loaded on its own leaves to
//! the front page.
//!
//! Only the frame and the front page talk, and only within this origin.

use std::cell::RefCell;

use wasm_bindgen::JsCast;
use web_sys::{Document, HtmlIFrameElement, MessageEvent, Window};

use crate::{document, window};

/// What a game says to the front page to leave.
const LEAVE: &str = "secretspace:leave";

thread_local! {
    static FRAME: RefCell<Option<HtmlIFrameElement>> = const { RefCell::new(None) };
}

/// Whether this page is a game in the front page's frame (a frame of
/// this same origin; another site framing us does not count).
pub fn framed() -> bool {
    !crate::host::hosted() && window().frame_element().ok().flatten().is_some()
}

/// The window that holds the screen: the front page's, for a game in its
/// frame; else this page's own.
pub(crate) fn outer() -> Window {
    if framed() {
        if let Ok(Some(parent)) = window().parent() {
            return parent;
        }
    }
    window()
}

/// The document that goes full screen (see `outer`).
pub(crate) fn outer_document() -> Document {
    outer().document().unwrap_or_else(document)
}

/// From the front page: open the game at `path` over it, on this press
/// (the whole screen and the keyboard taken now, while the press allows).
pub fn open(path: &str) {
    let doc = document();
    if doc.fullscreen_element().is_none() {
        if let Some(root) = doc.document_element() {
            let _ = root.request_fullscreen();
        }
    }
    crate::input::hold_keys();
    let Some(frame) = doc
        .create_element("iframe")
        .ok()
        .and_then(|e| e.dyn_into::<HtmlIFrameElement>().ok())
    else {
        crate::go(path);
        return;
    };
    frame.set_src(path);
    let _ = frame.set_attribute("allow", "fullscreen; autoplay");
    let _ = frame.set_attribute("title", path.trim_matches('/'));
    let _ = frame.set_attribute(
        "style",
        "position:fixed;left:0;top:0;border:0;margin:0;padding:0;z-index:10;background:#000",
    );
    fill(&frame);
    let Some(body) = doc.body() else {
        crate::go(path);
        return;
    };
    if body.append_child(&frame).is_err() {
        crate::go(path);
        return;
    }
    let _ = frame.focus();
    if let Ok(h) = window().history() {
        let _ = h.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(path));
    }
    FRAME.with(|f| *f.borrow_mut() = Some(frame));
}

/// Whether a game is open over the front page (it can rest meanwhile).
pub fn playing() -> bool {
    FRAME.with(|f| f.borrow().is_some())
}

/// From the front page: take the game down and give everything back.
fn close() {
    if let Some(frame) = FRAME.with(|f| f.borrow_mut().take()) {
        frame.remove();
    }
    crate::input::release();
}

/// From the front page: listen for its games leaving (their menu's exit,
/// or the browser's back).
pub fn listen() {
    let origin = window().location().origin().unwrap_or_default();
    crate::on(&window(), "message", move |e| {
        let Ok(e) = e.dyn_into::<MessageEvent>() else {
            return;
        };
        if e.origin() != origin || e.data().as_string().as_deref() != Some(LEAVE) {
            return;
        }
        if !playing() {
            return;
        }
        // Back to the front page's own address: the back that `open`
        // pushed (its popstate closes the frame).
        match window().history() {
            Ok(h) if h.back().is_ok() => {}
            _ => close(),
        }
    });
    crate::on(&window(), "popstate", |_| {
        if playing() {
            close();
        }
    });
    crate::on_resize(|| {
        FRAME.with(|f| {
            if let Some(frame) = f.borrow().as_ref() {
                fill(frame);
            }
        })
    });
}

/// The frame over the whole window (as it shows: on a phone, between its
/// toolbars).
fn fill(frame: &HtmlIFrameElement) {
    let (w, h) = crate::window_css();
    crate::place(frame, w, h);
}

/// From a game: leave it. In the front page's frame, the front page takes
/// it down; on its own, the screen and the keyboard are given back and the
/// front page loads. Hosted, the host is asked to close it.
pub fn exit() {
    crate::input::release();
    if crate::host::hosted() {
        return crate::host::close();
    }
    if framed() {
        if let Ok(Some(parent)) = window().parent() {
            let origin = window().location().origin().unwrap_or_default();
            if parent.post_message(&LEAVE.into(), &origin).is_ok() {
                return;
            }
        }
    }
    crate::go("/");
}
