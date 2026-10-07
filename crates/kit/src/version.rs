//! Which build of the pages this is, and whether a newer one is out: the
//! page polls `/version.txt` (written by `build-web.sh`, never cached) and
//! reloads itself at a moment that costs nothing, like the menu after a
//! death.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::XmlHttpRequest;

/// This page's build: the hash `build-web.sh` gives every page it builds.
pub const PAGE: &str = match option_env!("SECRETSPACE_PAGE") {
    Some(b) => b,
    None => "dev",
};

/// The build as a number, for the platform Hello (0 when built by hand).
pub fn build() -> u32 {
    PAGE.get(..8)
        .and_then(|h| u32::from_str_radix(h, 16).ok())
        .unwrap_or(0)
}

/// The newest build, as `/version.txt` last said.
pub struct Version {
    latest: Rc<RefCell<Option<String>>>,
    every: f64,
    next: f64,
}

impl Version {
    /// Ask now, and every `every` ms after.
    pub fn watch(every: f64) -> Version {
        Version {
            latest: Rc::new(RefCell::new(None)),
            every,
            next: 0.0,
        }
    }

    /// Ask again when it is time (call once a frame), or now with `soon`.
    pub fn poll(&mut self, now: f64, soon: bool) {
        if PAGE == "dev" || (now < self.next && !soon) {
            return;
        }
        self.next = now + self.every;
        let Ok(req) = XmlHttpRequest::new() else {
            return;
        };
        let url = format!("/version.txt?t={}", now as u64);
        if req.open_with_async("GET", &url, true).is_err() {
            return;
        }
        let latest = self.latest.clone();
        let r = req.clone();
        let done = Closure::<dyn FnMut()>::new(move || {
            if r.status().unwrap_or(0) == 200 {
                if let Ok(Some(text)) = r.response_text() {
                    let v = text.trim().to_string();
                    if !v.is_empty() && v.len() <= 64 {
                        *latest.borrow_mut() = Some(v);
                    }
                }
            }
        });
        req.set_onload(Some(done.as_ref().unchecked_ref()));
        done.forget();
        let _ = req.send();
    }

    /// Whether a newer build is out than the one running.
    pub fn newer(&self) -> bool {
        self.latest
            .borrow()
            .as_deref()
            .is_some_and(|v| PAGE != "dev" && v != PAGE)
    }
}

/// Load the page again (the newer build).
pub fn reload() {
    let _ = crate::window().location().reload();
}
