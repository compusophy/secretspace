//! A page run as a cartridge: inside another page (compusophyOS, a test
//! page) instead of being one. A cartridge's constructor `enter`s with what
//! its host said (the surface's size, the storage prefix, the options);
//! from then on the rest of kit answers from here instead of from the
//! window around it: the size and the pixel ratio, storage keys (each
//! under `ns`), the game server (never the host page's own address or
//! meta), whether the pointer is locked (the host's to say), a text field
//! with no `<input>` (the host sends the text). Until then nothing here is
//! consulted and every page behaves exactly as it always has.
//!
//! What a hosted page asks of its host it only notes, for the host to
//! poll: the pointer locked (`wants_lock`), to be closed (`closed`), why
//! it stopped (`error`). It draws only while shown and polled (`drawing`):
//! a host that stops asking for frames pauses it for free.
//!
//! Once entered, a module instance stays hosted for good: `end` (its host
//! let the cartridge go) only marks it ended, and a new `enter` may take
//! it again. Work still under way when the host let go (a device still
//! coming) so goes on as a cartridge's, never as the host page's: it
//! keeps to `ns`, makes no `<input>`, never draws and dials no server.
//! `era` moves on at every enter and every end, so such work can tell.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use wasm_bindgen::JsValue;

/// A hosted page not polled for this long (ms) stops drawing and goes
/// silent; its network and its world go on.
pub const POLL_MS: f64 = 1000.0;
/// The longest `opts.query` and `opts.server` taken (bytes).
const MOST_QUERY: usize = 1024;
const MOST_SERVER: usize = 512;
/// The biggest surface side taken (CSS pixels).
const MOST_CSS: f64 = 8192.0;
/// What a second constructor in this module instance is told.
const BUSY: &str = "busy: this module already runs a cartridge; load it again under another URL";
/// The game server baked in at build time (CI's `vars.RELAY`).
const RELAY: Option<&str> = option_env!("SECRETSPACE_RELAY");

/// The host's options (the constructor's sixth argument).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Opts {
    /// The protocol the host speaks (0: it did not say).
    pub v: u32,
    /// A game server to dial instead (honoured only in a dev build or on a
    /// page on this machine: the soul's key goes in every Hello).
    pub server: Option<String>,
    /// In place of `location.search`, without its `?`.
    pub query: String,
}

/// The options as a host gives them: a JSON string, a plain object, or
/// nothing (an older host's five arguments). Malformed: why.
pub fn opts(o: &JsValue) -> Result<Opts, String> {
    if o.is_undefined() || o.is_null() {
        return Ok(Opts::default());
    }
    let obj = match o.as_string() {
        Some(text) => js_sys::JSON::parse(&text).map_err(|_| "opts is not JSON".to_string())?,
        None => o.clone(),
    };
    if !obj.is_object() || js_sys::Array::is_array(&obj) {
        return Err("opts is not an object".into());
    }
    let get = |k: &str| js_sys::Reflect::get(&obj, &k.into()).unwrap_or(JsValue::UNDEFINED);
    let given = |v: &JsValue| !(v.is_undefined() || v.is_null());
    let v = get("v");
    let v = match given(&v) {
        true => Some(v.as_f64().ok_or("opts.v is not a number")?),
        false => None,
    };
    let text = |k: &str| -> Result<Option<String>, String> {
        let t = get(k);
        match given(&t) {
            true => t
                .as_string()
                .map(Some)
                .ok_or_else(|| format!("opts.{k} is not text")),
            false => Ok(None),
        }
    };
    check(v, text("server")?, text("query")?)
}

/// The options checked: a version that is a whole number, a server that is
/// a WebSocket address, a query not too long.
pub fn check(
    v: Option<f64>,
    server: Option<String>,
    query: Option<String>,
) -> Result<Opts, String> {
    let v = match v {
        None => 0,
        Some(v) if v.is_finite() && v >= 0.0 && v.fract() == 0.0 && v <= u32::MAX as f64 => {
            v as u32
        }
        Some(_) => return Err("opts.v is not a version".into()),
    };
    let server = match server.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => None,
        Some(s)
            if (s.starts_with("ws://") || s.starts_with("wss://"))
                && s.len() <= MOST_SERVER
                && !s.contains(char::is_whitespace) =>
        {
            Some(s.to_string())
        }
        Some(_) => return Err("opts.server is not a ws:// or wss:// address".into()),
    };
    let query = query.unwrap_or_default();
    let query = query.strip_prefix('?').unwrap_or(&query).to_string();
    if query.len() > MOST_QUERY {
        return Err("opts.query is too long".into());
    }
    Ok(Opts { v, server, query })
}

/// What a host said of the page it runs.
#[derive(Clone, Debug, PartialEq)]
pub struct Hosted {
    /// The surface, CSS pixels.
    pub css: (f64, f64),
    /// Device pixels per CSS pixel.
    pub dpr: f64,
    /// Where the cartridge's own files are (its `pkg/`), ending in `/`.
    pub base: String,
    /// What every key it keeps begins with.
    pub ns: String,
    pub opts: Opts,
}

/// A size a host gave, made sane: at least 1, at most `MOST_CSS`.
fn sane(css: (f64, f64), dpr: f64) -> ((f64, f64), f64) {
    let side = |v: f64| {
        if v.is_finite() {
            v.clamp(1.0, MOST_CSS)
        } else {
            1.0
        }
    };
    let dpr = if dpr.is_finite() && dpr > 0.0 {
        dpr.min(8.0)
    } else {
        1.0
    };
    ((side(css.0), side(css.1)), dpr)
}

#[derive(Default)]
struct State {
    page: Option<Hosted>,
    /// Let go (`end`): still hosted, never drawn again.
    ended: bool,
    /// One more at every enter and every end.
    era: u32,
    shown: bool,
    polled: Option<f64>,
    wish: bool,
    locked: bool,
    closed: bool,
    failed: String,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    /// Every text field made while hosted, with the era it was made in
    /// (the focused one of this era takes the text).
    static FIELDS: RefCell<Vec<(u32, Weak<RefCell<Virtual>>)>> = const { RefCell::new(Vec::new()) };
}

/// Read the state, or `or` if it cannot be now (never a panic: these are
/// reached from a cartridge's exports).
fn read<R>(or: R, f: impl FnOnce(&State) -> R) -> R {
    STATE
        .try_with(|s| s.try_borrow().ok().map(|s| f(&s)))
        .ok()
        .flatten()
        .unwrap_or(or)
}

fn write(f: impl FnOnce(&mut State)) {
    let _ = STATE.try_with(|s| {
        if let Ok(mut s) = s.try_borrow_mut() {
            f(&mut s);
        }
    });
}

/// Become hosted (again, after an `end`), or "busy: …" if this module
/// instance runs a cartridge now.
pub fn enter(h: Hosted) -> Result<(), String> {
    let (css, dpr) = sane(h.css, h.dpr);
    STATE
        .try_with(|s| {
            let mut s = s.try_borrow_mut().map_err(|_| BUSY.to_string())?;
            if s.page.is_some() && !s.ended {
                return Err(BUSY.to_string());
            }
            *s = State {
                page: Some(Hosted { css, dpr, ..h }),
                era: s.era.wrapping_add(1),
                shown: true,
                ..State::default()
            };
            Ok(())
        })
        .unwrap_or_else(|_| Err(BUSY.to_string()))
}

/// The host let the cartridge go: the next constructor may enter. Still
/// hosted (under the same `ns`), so what is left running never turns to
/// the host page; it never draws again, and dials no server.
pub fn end() {
    write(|s| {
        if s.page.is_some() && !s.ended {
            *s = State {
                page: s.page.take(),
                ended: true,
                era: s.era.wrapping_add(1),
                ..State::default()
            };
        }
    });
    let _ = FIELDS.try_with(|f| f.try_borrow_mut().map(|mut f| f.clear()));
}

/// Whether this module instance runs as a cartridge: from its first
/// `enter` on, for good (ended too).
pub fn hosted() -> bool {
    read(false, |s| s.page.is_some())
}

/// Whether the host let the cartridge go (and none has entered since).
pub fn ended() -> bool {
    read(false, |s| s.ended)
}

/// Which hosting this is: one more at every enter and every end. Work
/// begun in one era and done in another is for a cartridge that is gone.
pub fn era() -> u32 {
    read(0, |s| s.era)
}

/// The surface, CSS pixels.
pub fn css() -> (f64, f64) {
    read((1.0, 1.0), |s| {
        s.page.as_ref().map_or((1.0, 1.0), |h| h.css)
    })
}

pub fn dpr() -> f64 {
    read(1.0, |s| s.page.as_ref().map_or(1.0, |h| h.dpr))
}

/// The host resized the surface.
pub fn resize(css: (f64, f64), dpr: f64) {
    let (css, dpr) = sane(css, dpr);
    write(|s| {
        if let Some(h) = s.page.as_mut() {
            (h.css, h.dpr) = (css, dpr);
        }
    });
}

/// The storage prefix ("" when not hosted).
pub fn ns() -> String {
    read(String::new(), |s| {
        s.page.as_ref().map(|h| h.ns.clone()).unwrap_or_default()
    })
}

/// Where the cartridge's files are ("" when not hosted).
pub fn base() -> String {
    read(String::new(), |s| {
        s.page.as_ref().map(|h| h.base.clone()).unwrap_or_default()
    })
}

/// The query the host gave, in place of the page's address ("" when none).
pub fn query() -> String {
    read(String::new(), |s| {
        s.page
            .as_ref()
            .map(|h| h.opts.query.clone())
            .unwrap_or_default()
    })
}

/// The game server a hosted page dials (`ws(s)://…/ws`): the host's,
/// where that is allowed; else the one baked in at build time; else where
/// its files come from; else none. Not hosted, or ended: none.
pub fn server() -> Option<String> {
    let (asked, base) = read(None, |s| {
        s.page
            .as_ref()
            .filter(|_| !s.ended)
            .map(|h| (h.opts.server.clone(), h.base.clone()))
    })?;
    server_in(asked.as_deref(), local(), RELAY, &base)
}

/// The order `server` picks in (pure).
pub fn server_in(
    asked: Option<&str>,
    local: bool,
    relay: Option<&str>,
    base: &str,
) -> Option<String> {
    asked
        .filter(|_| local)
        .map(str::to_string)
        .or_else(|| {
            relay
                .map(str::trim)
                .filter(|r| !r.is_empty())
                .map(str::to_string)
        })
        .or_else(|| server_of(base))
}

/// The server beside a cartridge's files: "http://127.0.0.1:8787/wandfall/pkg/"
/// is "ws://127.0.0.1:8787/ws" (a local `--static dist`); not http(s), none.
pub fn server_of(base: &str) -> Option<String> {
    let (scheme, rest) = match base.strip_prefix("https://") {
        Some(r) => ("wss", r),
        None => ("ws", base.strip_prefix("http://")?),
    };
    let host = rest.split(['/', '?', '#']).next()?;
    (!host.is_empty() && !host.contains(char::is_whitespace))
        .then(|| format!("{scheme}://{host}/ws"))
}

/// Whether a host may name the server: a build made by hand, or a page on
/// this machine (else a link could send the soul's key anywhere).
fn local() -> bool {
    if crate::version::PAGE == "dev" {
        return true;
    }
    #[cfg(target_arch = "wasm32")]
    {
        matches!(
            crate::window().location().hostname().as_deref(),
            Ok("localhost" | "127.0.0.1" | "[::1]")
        )
    }
    #[cfg(not(target_arch = "wasm32"))]
    false
}

/// Whether the host shows it.
pub fn shown(on: bool) {
    write(|s| s.shown = on);
}

/// The host asked for a picture now.
pub fn polled(now: f64) {
    if now.is_finite() {
        write(|s| s.polled = Some(now));
    }
}

/// Whether to draw (and sound): hosted and not ended, shown, and polled
/// within `POLL_MS`.
pub fn drawing(now: f64) -> bool {
    read(false, |s| {
        s.page.is_some() && !s.ended && s.shown && s.polled.is_some_and(|t| now - t < POLL_MS)
    })
}

/// The page would like the pointer locked (or no longer). Ended, it
/// asks nothing.
pub fn want_lock(on: bool) {
    write(|s| s.wish = on && !s.ended);
}

/// Whether it wants the pointer locked, locked already or not: what a
/// cartridge's `capture()` answers (false asks the host to let it go, so
/// it is not `wants_lock() && !locked()`).
pub fn wants_lock() -> bool {
    read(false, |s| s.wish)
}

/// The host locked the pointer, or it was let go (Esc, a refusal): then
/// the wish is over too, until the page asks again.
pub fn set_locked(on: bool) {
    write(|s| {
        s.locked = on;
        if !on {
            s.wish = false;
        }
    });
}

pub fn locked() -> bool {
    read(false, |s| s.locked)
}

/// The page asks to be closed (its Exit).
pub fn close() {
    write(|s| s.closed |= !s.ended);
}

pub fn closed() -> bool {
    read(false, |s| s.closed)
}

/// Why the page stopped (the first reason stays; ended, none is kept).
pub fn fail(why: &str) {
    write(|s| {
        if s.failed.is_empty() && !s.ended {
            s.failed = why.to_string();
        }
    });
}

/// "" while fine; "crashed" after a panic; else what `fail` said.
pub fn error() -> String {
    if crate::report::crashed() {
        return "crashed".into();
    }
    read(String::new(), |s| s.failed.clone())
}

/// A text field with no `<input>`: what it holds, how much it takes (in
/// UTF-16 units, as an input's `maxlength`), where it is drawn (CSS
/// pixels), whether it has the keys.
#[derive(Debug, Default)]
pub struct Virtual {
    pub value: String,
    pub max: usize,
    pub at: Option<(f64, f64, f64, f64)>,
    pub focused: bool,
}

impl Virtual {
    /// Typed text, what fits of it (no control characters, as an input).
    pub fn type_in(&mut self, text: &str) {
        let mut used = self.value.encode_utf16().count();
        for c in text.chars().filter(|c| !c.is_control()) {
            if used + c.len_utf16() > self.max {
                break;
            }
            used += c.len_utf16();
            self.value.push(c);
        }
    }

    pub fn erase(&mut self) {
        self.value.pop();
    }
}

/// A new field, known to the host's text from now on.
pub fn field(max: usize) -> Rc<RefCell<Virtual>> {
    let f = Rc::new(RefCell::new(Virtual {
        max,
        ..Virtual::default()
    }));
    let era = era();
    let _ = FIELDS.try_with(|all| {
        if let Ok(mut all) = all.try_borrow_mut() {
            all.retain(|(_, w)| w.strong_count() > 0);
            all.push((era, Rc::downgrade(&f)));
        }
    });
    f
}

/// Give `f` the keys (and take them from any other), if it is shown and
/// of this era, and the cartridge is not ended.
pub fn focus(f: &Rc<RefCell<Virtual>>) {
    if f.try_borrow().map_or(true, |v| v.at.is_none()) || ended() || !current(f) {
        return;
    }
    each(|v| v.focused = false);
    if let Ok(mut v) = f.try_borrow_mut() {
        v.focused = true;
    }
}

/// Whether `f` was made in this era (not by work left over from a
/// cartridge let go).
fn current(f: &Rc<RefCell<Virtual>>) -> bool {
    let era = era();
    FIELDS
        .try_with(|all| {
            all.try_borrow().is_ok_and(|all| {
                all.iter()
                    .any(|(e, w)| *e == era && std::ptr::eq(w.as_ptr(), Rc::as_ptr(f)))
            })
        })
        .unwrap_or(false)
}

/// `f` on every live field of this era.
fn each(mut f: impl FnMut(&mut Virtual)) {
    let era = era();
    let _ = FIELDS.try_with(|all| {
        if let Ok(all) = all.try_borrow() {
            let live = all.iter().filter(|(e, _)| *e == era);
            for v in live.filter_map(|(_, w)| w.upgrade()) {
                if let Ok(mut v) = v.try_borrow_mut() {
                    f(&mut v);
                }
            }
        }
    });
}

/// `f` on the focused field, if one is.
fn focused<R>(f: impl FnOnce(&mut Virtual) -> R) -> Option<R> {
    let mut f = Some(f);
    let mut out = None;
    each(|v| {
        if v.focused {
            if let Some(f) = f.take() {
                out = Some(f(v));
            }
        }
    });
    out
}

/// Whether a field has the keys (the host opens its text sink).
pub fn typing() -> bool {
    focused(|_| ()).is_some()
}

/// Committed text from the host, into the focused field.
pub fn text(t: &str) {
    focused(|v| v.type_in(t));
}

/// A key while typing: whether the focused field took it (Backspace).
/// Enter, Esc and the rest go on to the page, as they do from an input.
pub fn edit(code: &str) -> bool {
    focused(|v| match code {
        "Backspace" => {
            v.erase();
            true
        }
        _ => false,
    })
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(ns: &str, server: Option<&str>) -> Hosted {
        Hosted {
            css: (960.0, 540.0),
            dpr: 1.0,
            base: "http://127.0.0.1:8787/wandfall/pkg/".into(),
            ns: ns.into(),
            opts: Opts {
                v: 1,
                server: server.map(str::to_string),
                query: "practice".into(),
            },
        }
    }

    #[test]
    fn opts_read_and_refuse() {
        assert_eq!(check(None, None, None), Ok(Opts::default()));
        assert_eq!(
            check(
                Some(1.0),
                Some(" ws://127.0.0.1:8787/ws ".into()),
                Some("?q=low".into())
            ),
            Ok(Opts {
                v: 1,
                server: Some("ws://127.0.0.1:8787/ws".into()),
                query: "q=low".into()
            })
        );
        assert_eq!(
            check(Some(1.0), Some(String::new()), None).map(|o| o.server),
            Ok(None)
        );
        for v in [-1.0, 1.5, f64::NAN, f64::INFINITY, 1e12] {
            assert!(check(Some(v), None, None).is_err(), "{v}");
        }
        for s in ["https://x/ws", "javascript:alert(1)", "ws://a b/ws", "/ws"] {
            assert!(check(None, Some(s.into()), None).is_err(), "{s}");
        }
        assert!(check(None, Some(format!("wss://{}/ws", "a".repeat(600))), None).is_err());
        assert!(check(None, None, Some("x".repeat(2000))).is_err());
        assert!(check(None, None, Some("x".repeat(1000))).is_ok());
    }

    #[test]
    fn hosting_is_one_at_a_time() {
        // Each test runs on a thread of its own: a page, never entered.
        end();
        assert!(!hosted() && !ended() && era() == 0);
        assert_eq!(enter(page("a.", None)), Ok(()));
        assert!(hosted() && era() == 1);
        assert!(enter(page("b.", None)).unwrap_err().starts_with("busy"));
        assert_eq!(ns(), "a.");
        assert_eq!(query(), "practice");
        resize((f64::NAN, 1e9), -2.0);
        assert_eq!((css(), dpr()), ((1.0, MOST_CSS), 1.0));
        close();
        fail("stale");
        fail("later");
        assert!(closed());
        assert_eq!(error(), "stale");
        end();
        assert!(ended() && !closed() && error().is_empty() && era() == 2);
        assert_eq!(enter(page("b.", None)), Ok(()));
        assert!(!ended() && era() == 3);
        assert_eq!(ns(), "b.");
        end();
        end();
        assert_eq!(era(), 4, "a second end is no new era");
    }

    #[test]
    fn a_cartridge_let_go_stays_one() {
        // Work still under way when the host let go (a device still
        // coming) must not find a page: it keeps to its own keys and
        // size, asks nothing, draws nothing and dials nowhere.
        enter(page("a.", Some("ws://127.0.0.1:9/ws"))).unwrap();
        polled(0.0);
        assert!(drawing(1.0) && server().is_some());
        end();
        assert!(hosted() && ended());
        assert_eq!(
            (ns(), query(), css()),
            ("a.".into(), "practice".into(), (960.0, 540.0))
        );
        assert!(!drawing(1.0));
        shown(true);
        polled(1.0);
        assert!(!drawing(2.0), "never drawn again");
        assert_eq!(server(), None);
        want_lock(true);
        close();
        fail("no game server");
        assert!(!wants_lock() && !closed() && error().is_empty());
        // The next cartridge starts clean.
        enter(page("b.", None)).unwrap();
        assert!(!ended() && ns() == "b." && server().is_some());
    }

    #[test]
    fn server_order_when_hosted() {
        let base = "http://127.0.0.1:8787/wandfall/pkg/";
        let relay = Some("wss://relay.example/ws");
        let asked = Some("ws://dev.example:9000/ws");
        // The host's own server only where it may name one.
        assert_eq!(server_in(asked, true, relay, base).as_deref(), asked);
        assert_eq!(server_in(asked, false, relay, base).as_deref(), relay);
        // Then the relay baked in, then beside the files, then none.
        assert_eq!(server_in(None, true, relay, base).as_deref(), relay);
        assert_eq!(
            server_in(None, true, Some(" "), base).as_deref(),
            Some("ws://127.0.0.1:8787/ws")
        );
        assert_eq!(
            server_in(None, true, None, base).as_deref(),
            Some("ws://127.0.0.1:8787/ws")
        );
        assert_eq!(server_in(None, false, None, "file:///x/pkg/"), None);
        // Through the state: a test build is a dev build, so the host's
        // server is honoured.
        end();
        enter(page("t.", asked)).unwrap();
        assert_eq!(server().as_deref(), asked);
        end();
        enter(page("t.", None)).unwrap();
        assert_eq!(server(), server_in(None, true, RELAY, base));
        end();
    }

    #[test]
    fn server_of_base() {
        assert_eq!(
            server_of("http://127.0.0.1:8787/wandfall/pkg/").as_deref(),
            Some("ws://127.0.0.1:8787/ws")
        );
        assert_eq!(
            server_of("https://secretspace-seven.vercel.app/wandfall/pkg/").as_deref(),
            Some("wss://secretspace-seven.vercel.app/ws")
        );
        assert_eq!(
            server_of("https://x.example").as_deref(),
            Some("wss://x.example/ws")
        );
        assert_eq!(server_of("http://h:1?q#f").as_deref(), Some("ws://h:1/ws"));
        for b in [
            "",
            "./pkg/",
            "/wandfall/pkg/",
            "file:///pkg/",
            "http:///pkg/",
            "ws://h/ws",
        ] {
            assert_eq!(server_of(b), None, "{b}");
        }
    }

    #[test]
    fn drawing_stops_unpolled() {
        assert!(!drawing(0.0));
        enter(page("t.", None)).unwrap();
        // Not polled yet: nothing drawn.
        assert!(!drawing(100.0));
        polled(1000.0);
        assert!(drawing(1000.0) && drawing(1999.0));
        assert!(!drawing(2000.0));
        polled(f64::NAN);
        assert!(!drawing(2000.0));
        polled(2500.0);
        assert!(drawing(2600.0));
        shown(false);
        assert!(!drawing(2600.0));
        shown(true);
        assert!(drawing(2600.0));
        end();
        assert!(!drawing(2600.0));
    }

    #[test]
    fn the_lock_is_asked_for_and_given() {
        enter(page("t.", None)).unwrap();
        want_lock(true);
        assert!(wants_lock() && !locked());
        set_locked(true);
        // Held, it is still wanted: `capture()` stays true, or the host
        // would let it go the frame after it locked.
        assert!(wants_lock() && locked());
        // Let go (Esc): the wish ends with it, so the host does not lock
        // again until the page asks.
        set_locked(false);
        assert!(!wants_lock() && !locked());
        end();
    }

    #[test]
    fn virtual_fields_take_the_keys_one_at_a_time() {
        let a = field(8);
        let b = field(8);
        // Not shown: no focus.
        focus(&a);
        assert!(!typing());
        a.borrow_mut().at = Some((0.0, 0.0, 10.0, 10.0));
        b.borrow_mut().at = Some((0.0, 20.0, 10.0, 10.0));
        focus(&a);
        text("hi");
        focus(&b);
        text("yo");
        assert_eq!(
            (a.borrow().value.as_str(), b.borrow().value.as_str()),
            ("hi", "yo")
        );
        assert!(!a.borrow().focused && b.borrow().focused);
        assert!(edit("Backspace") && !edit("Enter") && !edit("KeyA"));
        assert_eq!(b.borrow().value, "y");
        drop(b);
        assert!(!typing());
        assert!(!edit("Backspace"));
    }

    #[test]
    fn a_field_left_over_never_takes_the_keys() {
        enter(page("a.", None)).unwrap();
        let old = field(8);
        old.borrow_mut().at = Some((0.0, 0.0, 10.0, 10.0));
        focus(&old);
        assert!(typing());
        end();
        assert!(!typing());
        // Made after the host let go, or kept into the next cartridge:
        // never focused, never typed into.
        let late = field(8);
        late.borrow_mut().at = Some((0.0, 0.0, 10.0, 10.0));
        focus(&late);
        assert!(!late.borrow().focused && !typing());
        enter(page("b.", None)).unwrap();
        focus(&old);
        focus(&late);
        text("x");
        assert!(!typing());
        assert_eq!(
            (old.borrow().value.as_str(), late.borrow().value.as_str()),
            ("", "")
        );
        let now = field(8);
        now.borrow_mut().at = Some((0.0, 0.0, 10.0, 10.0));
        focus(&now);
        text("x");
        assert_eq!(now.borrow().value, "x");
    }
}
