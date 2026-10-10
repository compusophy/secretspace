//! Reports to the game's server: what a player tells us (feedback) and
//! what went wrong (a crash, a browser that cannot run the game), each
//! with what the page knows of where it happened (its build, the browser,
//! the window), sent as plain text to `/feedback` beside the rooms.

use std::cell::Cell;

use web_sys::XmlHttpRequest;

/// The most a report carries (bytes); the server holds to the same.
pub const MOST: usize = 4000;

thread_local! {
    static CRASHED: Cell<bool> = const { Cell::new(false) };
}

/// Whether the page has panicked (it still answers this after).
pub(crate) fn crashed() -> bool {
    CRASHED.try_with(Cell::get).unwrap_or(true)
}

/// The server's own address for `path` ("/feedback"): where the rooms
/// are (`room_url`), over http(s).
pub fn server_url(path: &str) -> String {
    let ws = crate::room_url("_", "");
    let base = ws.trim_end_matches("/ws/_");
    let base = if let Some(rest) = base.strip_prefix("wss://") {
        format!("https://{rest}")
    } else if let Some(rest) = base.strip_prefix("ws://") {
        format!("http://{rest}")
    } else {
        base.to_string()
    };
    format!("{base}{path}")
}

/// What the page knows of where it is: the page and its build, the
/// browser, the window, fingers or a mouse.
pub fn context() -> String {
    if crate::host::hosted() {
        let (cw, ch) = crate::host::css();
        let w = crate::window();
        return format!(
            "page: {}\nbuild: {}\nbrowser: {}\nwindow: {cw}x{ch} at {:.2}\ntouch: false\nhosted: 1\n",
            crate::host::base(),
            crate::version::PAGE,
            w.navigator().user_agent().unwrap_or_default(),
            crate::host::dpr(),
        );
    }
    let w = crate::window();
    let size = |v: Result<wasm_bindgen::JsValue, _>| v.ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
    format!(
        "page: {}\nbuild: {}\nbrowser: {}\nwindow: {}x{} at {:.2}\ntouch: {}\n",
        w.location().pathname().unwrap_or_default(),
        crate::version::PAGE,
        w.navigator().user_agent().unwrap_or_default(),
        size(w.inner_width()),
        size(w.inner_height()),
        w.device_pixel_ratio(),
        crate::touch(),
    )
}

/// The report as sent: its kind, the page's context and the game's own
/// (`key: value` lines), a blank line, then what it says; cut to `MOST`.
pub fn body(kind: &str, game: &str, text: &str) -> String {
    let mut b = format!("kind: {kind}\n{}{game}\n{}", context(), text.trim());
    if b.len() > MOST {
        let mut end = MOST;
        while !b.is_char_boundary(end) {
            end -= 1;
        }
        b.truncate(end);
    }
    b
}

/// Send a report (`kind`: "feedback", "crash", ...). Whether it went out
/// (not whether it arrived).
pub fn send(kind: &str, game: &str, text: &str) -> bool {
    // Hosted with no server of its own: nowhere to send it.
    if crate::host::hosted() && crate::host::server().is_none() {
        return false;
    }
    let Ok(req) = XmlHttpRequest::new() else {
        return false;
    };
    if req
        .open_with_async("POST", &server_url("/feedback"), true)
        .is_err()
    {
        return false;
    }
    // Plain text: no preflight, the server answers anyone.
    let _ = req.set_request_header("Content-Type", "text/plain");
    req.send_with_opt_str(Some(&body(kind, game, text))).is_ok()
}

/// Report the page's first panic (a crash) to the server, once, and let
/// the browser's console show it too.
pub fn on_panic() {
    std::panic::set_hook(Box::new(|info| {
        let text = info.to_string();
        web_sys::console::error_1(&text.clone().into());
        if !CRASHED.with(|c| c.replace(true)) {
            send("crash", "", &text);
        }
    }));
}
