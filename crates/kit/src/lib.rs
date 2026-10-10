//! The browser end every page here shares, so a game's page is only its
//! own drawing and rules: a pixel `Screen` the game draws into and shows
//! once a frame (or a WebGL2 screen, `gl`, with a pixel layer over it), a
//! `Link` to its room, the `Session` that says who this is, every hand at
//! once (`input`), the page's `version`, an invisible `TextField` (so a
//! phone still offers its keyboard), storage, `audio`, a frame loop,
//! `report`s to the server (feedback, crashes), the menu every game
//! shares (`meta`), and the `shell` the front page opens games in. Rust
//! only: the page's one line of script just starts the wasm. A page can
//! also run inside another page, as a cartridge (`host`): then the size,
//! storage, server, pointer lock and text come from its host.

pub mod audio;
pub mod gl;
pub mod host;
pub mod input;
pub mod link;
pub mod meta;
pub mod report;
pub mod session;
pub mod shell;
pub mod version;

pub use link::{Link, Net};
pub use session::Session;
pub use version::Version;

use std::cell::RefCell;
use std::rc::Rc;

use pixels::Canvas;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{Clamped, JsCast, JsValue};
use web_sys::{
    CanvasRenderingContext2d, Document, HtmlCanvasElement, HtmlElement, HtmlInputElement,
    ImageData, Window,
};

pub fn window() -> Window {
    web_sys::window().expect("a window")
}

pub fn document() -> Document {
    window().document().expect("a document")
}

/// Milliseconds since the page opened.
pub fn now() -> f64 {
    window()
        .performance()
        .map_or_else(js_sys::Date::now, |p| p.now())
}

pub fn on(target: &web_sys::EventTarget, event: &str, f: impl FnMut(web_sys::Event) + 'static) {
    let c = Closure::<dyn FnMut(web_sys::Event)>::new(f);
    let _ = target.add_event_listener_with_callback(event, c.as_ref().unchecked_ref());
    c.forget();
}

/// Call `f` with the time on every animation frame, forever: the frame's
/// own time (when the browser began it, on `now`'s clock), the same for
/// everything drawn in it, so steps between frames come out even.
pub fn frames(mut f: impl FnMut(f64) + 'static) {
    type Loop = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;
    let raf: Loop = Rc::new(RefCell::new(None));
    let again = raf.clone();
    *raf.borrow_mut() = Some(Closure::new(move |t: f64| {
        f(t);
        if let Some(c) = again.borrow().as_ref() {
            let _ = window().request_animation_frame(c.as_ref().unchecked_ref());
        }
    }));
    if let Some(c) = raf.borrow().as_ref() {
        let _ = window().request_animation_frame(c.as_ref().unchecked_ref());
    }
    std::mem::forget(raf);
}

/// Call `f` with the time on every animation frame (as `frames`) until it
/// says false: the loop ends there and lets go of `f`.
pub fn frames_while(mut f: impl FnMut(f64) -> bool + 'static) {
    type Loop = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;
    let raf: Loop = Rc::new(RefCell::new(None));
    let again = raf.clone();
    *raf.borrow_mut() = Some(Closure::new(move |t: f64| {
        if f(t) {
            if let Some(c) = again.borrow().as_ref() {
                let _ = window().request_animation_frame(c.as_ref().unchecked_ref());
            }
        } else if let Some(c) = again.borrow_mut().take() {
            // Not dropped while it runs: once this frame is over.
            let free = Closure::once_into_js(move || drop(c));
            let _ = window().set_timeout_with_callback(free.unchecked_ref());
        }
    }));
    if let Some(c) = raf.borrow().as_ref() {
        let _ = window().request_animation_frame(c.as_ref().unchecked_ref());
    };
}

/// Call `f` whenever the screen changes under the page: the window's size,
/// the part of it a phone shows (its toolbars, which do not always resize
/// the window), or its pixels (dragged to another monitor, or zoomed).
pub fn on_resize(f: impl FnMut() + 'static) {
    let f: Rc<RefCell<dyn FnMut()>> = Rc::new(RefCell::new(f));
    let g = f.clone();
    on(&window(), "resize", move |_| (g.borrow_mut())());
    if let Some(v) = window().visual_viewport() {
        let g = f.clone();
        on(&v, "resize", move |_| (g.borrow_mut())());
    }
    on_new_pixels(f);
}

/// `f` once the device's pixels per CSS pixel change, and each time after.
fn on_new_pixels(f: Rc<RefCell<dyn FnMut()>>) {
    let q = format!("(resolution: {}dppx)", window().device_pixel_ratio());
    let Ok(Some(m)) = window().match_media(&q) else {
        return;
    };
    let c = Closure::once(move |_: web_sys::Event| {
        (f.borrow_mut())();
        on_new_pixels(f);
    });
    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_once(true);
    let _ = m.add_event_listener_with_callback_and_add_event_listener_options(
        "change",
        c.as_ref().unchecked_ref(),
        &opts,
    );
    c.forget();
}

/// The window's size in CSS pixels: the part of it the page is shown in
/// (on a phone, between its toolbars); hosted, the host's surface.
pub fn window_css() -> (f64, f64) {
    if host::hosted() {
        return host::css();
    }
    let w = window();
    let px = |v: Result<JsValue, JsValue>, or: f64| v.ok().and_then(|v| v.as_f64()).unwrap_or(or);
    (px(w.inner_width(), 800.0), px(w.inner_height(), 600.0))
}

/// Device pixels per CSS pixel (hosted: as the host said).
pub fn dpr() -> f64 {
    if host::hosted() {
        return host::dpr();
    }
    window().device_pixel_ratio()
}

/// This page's query (`location.search`, with its `?`); hosted, the one
/// its host gave, so the host's own address never leaks in.
pub fn search() -> String {
    if host::hosted() {
        let q = host::query();
        return if q.is_empty() { q } else { format!("?{q}") };
    }
    window().location().search().unwrap_or_default()
}

/// Size `el` (a canvas, a frame) to `w` by `h` CSS pixels: set here,
/// never by the page's style, whose `100vh` on a phone is the screen with
/// its toolbars hidden, taller than what shows.
pub fn place(el: &HtmlElement, w: f64, h: f64) {
    let s = el.style();
    let _ = s.set_property("width", &format!("{w}px"));
    let _ = s.set_property("height", &format!("{h}px"));
}

/// CSS pixels per buffer pixel near `scale` that make each buffer pixel a
/// whole number of the device's pixels (`dpr` of them per CSS pixel), so
/// every stroke of the font is as thick as every other.
pub fn snap(scale: f64, dpr: f64) -> f64 {
    let dpr = if dpr.is_finite() { dpr.max(0.25) } else { 1.0 };
    (scale * dpr).round().max(1.0) / dpr
}

/// A key as stored: hosted, under the host's prefix (`ns`), so a
/// cartridge never reads or writes its host's own keys.
fn kept(key: &str) -> String {
    format!("{}{key}", host::ns())
}

pub fn load(key: &str) -> Option<String> {
    window().local_storage().ok()??.get_item(&kept(key)).ok()?
}

pub fn save(key: &str, v: &str) {
    if let Ok(Some(s)) = window().local_storage() {
        let _ = s.set_item(&kept(key), v);
    }
}

/// A value kept under `key`, or under `old` from before it moved (moved
/// over the first time it is read).
pub fn load_moved(key: &str, old: &str) -> Option<String> {
    load(key).or_else(|| {
        let v = load(old)?;
        save(key, &v);
        if let Ok(Some(s)) = window().local_storage() {
            let _ = s.remove_item(&kept(old));
        }
        Some(v)
    })
}

/// A buzz, where the device has one (Android; iOS has none).
pub fn vibrate(ms: u32) {
    let _ = window().navigator().vibrate_with_duration(ms);
}

/// Go to another page (hosted: ask the host to close this one).
pub fn go(url: &str) {
    if host::hosted() {
        return host::close();
    }
    let _ = window().location().assign(url);
}

/// Whether the main pointer is a finger (hosted, never: the host sends a
/// mouse's events).
pub fn touch() -> bool {
    if host::hosted() {
        return false;
    }
    window()
        .match_media("(pointer: coarse)")
        .ok()
        .flatten()
        .is_some_and(|m| m.matches())
}

/// The WebSocket address of a room (or "hub"): `<meta name="server">` (a
/// `wss://host/ws` address), else this page's own origin. `query` goes on
/// the end: "v=1" on a page's first connection counts a visit; "watch=1"
/// only looks. A page on this machine (localhost, or built by hand) may
/// name another server with `?server=`; a deployed page never does, or a
/// link could send the soul's key, said in every Hello, to anyone.
/// Hosted, the host's page is never asked: `host::server`, or no server
/// (the cartridge fails, and the address is one no socket opens).
pub fn room_url(room: &str, query: &str) -> String {
    if host::hosted() {
        let Some(base) = host::server() else {
            host::fail("no game server");
            return "about:blank#no-game-server".into();
        };
        return at(&base, room, query);
    }
    let loc = window().location();
    let local = version::PAGE == "dev"
        || matches!(
            loc.hostname().as_deref(),
            Ok("localhost" | "127.0.0.1" | "[::1]")
        );
    let param = loc.search().ok().filter(|_| local).and_then(|q| {
        q.trim_start_matches('?')
            .split('&')
            .find_map(|kv| kv.strip_prefix("server=").map(str::to_string))
    });
    let meta = document()
        .query_selector("meta[name=server]")
        .ok()
        .flatten()
        .and_then(|m| m.get_attribute("content"))
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty());
    let base = param.or(meta).unwrap_or_else(|| {
        let host = loc.host().unwrap_or_default();
        let secure = loc.protocol().is_ok_and(|p| p == "https:");
        format!("{}://{host}/ws", if secure { "wss" } else { "ws" })
    });
    at(&base, room, query)
}

/// A room's address on the server at `base` (`wss://host/ws`).
fn at(base: &str, room: &str, query: &str) -> String {
    let base = base.trim_end_matches('/').trim_end_matches("/ws");
    let q = if query.is_empty() {
        String::new()
    } else {
        format!("?{query}")
    };
    format!("{base}/ws/{room}{q}")
}

/// The canvas, and the pixels drawn into it: one buffer pixel is `scale`
/// CSS pixels (a whole number of the device's), shown sharp.
pub struct Screen {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    pub px: Canvas,
    /// CSS pixels per buffer pixel.
    pub scale: f64,
    /// The window, in CSS pixels.
    pub css: (f64, f64),
    ui: i32,
}

impl Screen {
    /// The canvas with this id.
    pub fn new(id: &str) -> Screen {
        let canvas: HtmlCanvasElement = document()
            .get_element_by_id(id)
            .and_then(|e| e.dyn_into().ok())
            .unwrap_or_else(|| panic!("#{id} is a canvas"));
        let ctx = canvas
            .get_context("2d")
            .ok()
            .flatten()
            .and_then(|c| c.dyn_into::<CanvasRenderingContext2d>().ok())
            .expect("a 2d context");
        let mut s = Screen {
            canvas,
            ctx,
            px: Canvas::new(1, 1),
            scale: 1.0,
            css: (1.0, 1.0),
            ui: 2,
        };
        s.fit();
        s
    }

    /// Match the window: pick the pixel size, size the buffer.
    pub fn fit(&mut self) {
        let (w, h) = window_css();
        // About 960 buffer pixels across, at most: big screens get bigger
        // pixels, a phone gets one per CSS pixel.
        let pick = (w / 960.0).ceil().clamp(1.0, 4.0);
        // Text keeps its size in buffer pixels however that is snapped,
        // so on the screen it moves with the snap, by up to half a device
        // pixel a buffer pixel: a third bigger in a window under 960 CSS
        // pixels at 150%, a fifth smaller at 125%, a fifth bigger on a
        // laptop at 125%. The buffer is as much narrower or wider, which
        // can change a layout (the hub's cards: two to a row, or one).
        self.ui = if pick >= 2.0 { 1 } else { 2 };
        self.size(snap(pick, window().device_pixel_ratio()), w, h);
    }

    fn size(&mut self, scale: f64, w: f64, h: f64) {
        self.scale = scale;
        self.css = (w, h);
        let (bw, bh) = (
            (w / self.scale).ceil() as i32,
            (h / self.scale).ceil() as i32,
        );
        self.px.resize(bw, bh);
        self.canvas.set_width(bw as u32);
        self.canvas.set_height(bh as u32);
        // Exactly `scale` CSS pixels a buffer pixel: what spills past the
        // window's edge (less than one) is cut off.
        place(&self.canvas, bw as f64 * scale, bh as f64 * scale);
    }

    /// The text scale, in buffer pixels: 2 where a buffer pixel is about
    /// a CSS pixel (a phone, a small window), 1 where it is two or more,
    /// so text is about the same size on any screen (how near: `fit`).
    pub fn ui(&self) -> i32 {
        self.ui
    }

    /// A CSS point as a buffer point.
    pub fn to_px(&self, x: f64, y: f64) -> (f32, f32) {
        ((x / self.scale) as f32, (y / self.scale) as f32)
    }

    /// A buffer box as a CSS box.
    pub fn to_css(&self, x: f32, y: f32, w: f32, h: f32) -> (f64, f64, f64, f64) {
        let s = self.scale;
        (x as f64 * s, y as f64 * s, w as f64 * s, h as f64 * s)
    }

    /// The mouse cursor over the canvas ("pointer", "default", ...).
    pub fn cursor(&self, c: &str) {
        let _ = HtmlElement::style(&self.canvas).set_property("cursor", c);
    }

    /// Show what was drawn.
    pub fn present(&self) {
        if let Ok(img) = ImageData::new_with_u8_clamped_array_and_sh(
            Clamped(&self.px.data),
            self.px.w as u32,
            self.px.h as u32,
        ) {
            let _ = self.ctx.put_image_data(&img, 0.0, 0.0);
        }
    }
}

/// A text box the page draws itself: an invisible input laid over where
/// it is drawn, so a click (or a tap, with its keyboard) types into it.
/// Hosted, there is no input: the field is kept here, a press inside it
/// (`press`) gives it the keys, and the host sends the text (`host::text`).
pub struct TextField {
    field: Field,
}

enum Field {
    Dom(HtmlInputElement),
    Virtual(Rc<RefCell<host::Virtual>>),
}

impl TextField {
    /// A field for a name or words to keep as typed: a phone's keyboard
    /// neither capitalises nor corrects them.
    pub fn new(max: u32, placeholder: &str) -> TextField {
        let f = TextField::prose(max, placeholder);
        if let Field::Dom(input) = &f.field {
            let _ = input.set_attribute("autocapitalize", "none");
            let _ = input.set_attribute("autocorrect", "off");
            let _ = input.set_attribute("spellcheck", "false");
        }
        f
    }

    /// A field for sentences (feedback): the keyboard helps as it would.
    pub fn prose(max: u32, placeholder: &str) -> TextField {
        if host::hosted() {
            return TextField {
                field: Field::Virtual(host::field(max as usize)),
            };
        }
        let input: HtmlInputElement = document()
            .create_element("input")
            .ok()
            .and_then(|e| e.dyn_into().ok())
            .expect("an input");
        input.set_max_length(max as i32);
        input.set_placeholder(placeholder);
        let _ = input.set_attribute("aria-label", placeholder);
        let _ = input.set_attribute("autocomplete", "off");
        let _ = input.set_attribute("enterkeyhint", "go");
        let _ = input.set_attribute(
            "style",
            "position:fixed;opacity:0;border:0;padding:0;margin:0;font-size:16px;z-index:2;display:none",
        );
        if let Some(body) = document().body() {
            let _ = body.append_child(&input);
        }
        TextField {
            field: Field::Dom(input),
        }
    }

    pub fn value(&self) -> String {
        match &self.field {
            Field::Dom(input) => input.value(),
            Field::Virtual(v) => v.try_borrow().map(|v| v.value.clone()).unwrap_or_default(),
        }
    }

    pub fn set_value(&self, value: &str) {
        match &self.field {
            Field::Dom(input) => input.set_value(value),
            Field::Virtual(v) => {
                if let Ok(mut v) = v.try_borrow_mut() {
                    v.value = value.to_string();
                }
            }
        }
    }

    pub fn focused(&self) -> bool {
        match &self.field {
            Field::Dom(input) => document().active_element().is_some_and(|e| e == ***input),
            Field::Virtual(v) => v.try_borrow().is_ok_and(|v| v.focused),
        }
    }

    pub fn focus(&self) {
        match &self.field {
            Field::Dom(input) => {
                let _ = input.focus();
            }
            Field::Virtual(v) => host::focus(v),
        }
    }

    pub fn blur(&self) {
        match &self.field {
            Field::Dom(input) => {
                let _ = input.blur();
            }
            Field::Virtual(v) => {
                if let Ok(mut v) = v.try_borrow_mut() {
                    v.focused = false;
                }
            }
        }
    }

    /// Lay it over this CSS box, or hide it (which lets the keys go).
    pub fn place(&self, at: Option<(f64, f64, f64, f64)>) {
        let input = match &self.field {
            Field::Dom(input) => input,
            Field::Virtual(v) => {
                if let Ok(mut v) = v.try_borrow_mut() {
                    v.at = at;
                    v.focused &= at.is_some();
                }
                return;
            }
        };
        let style = HtmlElement::style(input);
        match at {
            Some((x, y, w, h)) => {
                let _ = style.set_property("display", "block");
                let _ = style.set_property("left", &format!("{x}px"));
                let _ = style.set_property("top", &format!("{y}px"));
                let _ = style.set_property("width", &format!("{w}px"));
                let _ = style.set_property("height", &format!("{h}px"));
            }
            None => {
                let _ = style.set_property("display", "none");
            }
        }
    }

    /// Hosted: a press at (x, y), CSS pixels. Inside the field, it takes
    /// the keys (true); elsewhere it lets them go, as a click off an input
    /// does. A page's own field is the browser's to focus: false.
    pub fn press(&self, x: f64, y: f64) -> bool {
        let Field::Virtual(v) = &self.field else {
            return false;
        };
        let inside = v
            .try_borrow()
            .ok()
            .and_then(|v| v.at)
            .is_some_and(|(bx, by, bw, bh)| x >= bx && x < bx + bw && y >= by && y < by + bh);
        if inside {
            self.focus();
        } else {
            self.blur();
        }
        inside
    }
}

#[cfg(test)]
mod tests {
    use super::{host, kept, snap, TextField};

    fn hosted(ns: &str) {
        host::end();
        host::enter(host::Hosted {
            css: (960.0, 540.0),
            dpr: 1.5,
            base: "http://127.0.0.1:8787/wandfall/pkg/".into(),
            ns: ns.into(),
            opts: host::Opts::default(),
        })
        .unwrap();
    }

    #[test]
    fn keys_take_the_namespace() {
        host::end();
        // A page keeps its keys as ever.
        assert_eq!(kept("secretspace/key"), "secretspace/key");
        hosted("compusophy.cart.secretspace.");
        assert_eq!(
            kept("secretspace/key"),
            "compusophy.cart.secretspace.secretspace/key"
        );
        assert_eq!(
            kept("wandfall.muted"),
            "compusophy.cart.secretspace.wandfall.muted"
        );
        host::end();
        assert_eq!(kept("secretspace/key"), "secretspace/key");
    }

    #[test]
    fn hosted_the_window_is_the_hosts() {
        hosted("t.");
        assert_eq!(super::window_css(), (960.0, 540.0));
        assert_eq!(super::dpr(), 1.5);
        assert!(!super::touch());
        host::resize((640.0, 360.0), 1.0);
        assert_eq!(super::window_css(), (640.0, 360.0));
        assert_eq!(super::search(), "");
        host::end();
    }

    #[test]
    fn virtual_field_types_erases_and_caps() {
        hosted("t.");
        let name = TextField::new(5, "your name");
        // Shown nowhere: a press cannot give it the keys.
        assert!(!name.press(5.0, 5.0) && !host::typing());
        name.place(Some((10.0, 10.0, 100.0, 20.0)));
        assert!(!name.press(5.0, 5.0));
        assert!(name.press(50.0, 20.0));
        assert!(name.focused() && host::typing());
        // Typed text: what fits (5 UTF-16 units), no control characters.
        host::text("h\u{7}é\nllo world");
        assert_eq!(name.value(), "héllo");
        assert!(host::edit("Backspace"));
        assert_eq!(name.value(), "héll");
        host::text("😀");
        assert_eq!(name.value(), "héll", "a pair of units does not fit in one");
        // Enter is the page's (it puts the field down), not the field's.
        assert!(!host::edit("Enter"));
        name.set_value("ab");
        host::text("c");
        assert_eq!(name.value(), "abc");
        // A press off it, or hiding it, lets the keys go.
        assert!(!name.press(500.0, 500.0));
        assert!(!name.focused() && !host::typing());
        name.focus();
        assert!(name.focused());
        name.place(None);
        assert!(!name.focused());
        host::text("x");
        assert_eq!(name.value(), "abc");
        host::end();
    }

    #[test]
    fn every_buffer_pixel_is_whole_device_pixels() {
        // A laptop at 125%, a 4K screen at 150%, Androids at 2.625 and
        // 2.75, a phone at 3, a desktop at 1, a page zoomed out to 90%.
        for (pick, dpr) in [
            (2.0, 1.25),
            (3.0, 1.5),
            (1.0, 2.625),
            (1.0, 2.75),
            (1.0, 3.0),
            (2.0, 1.0),
            (1.0, 0.9),
        ] {
            let s = snap(pick, dpr);
            let dev = s * dpr;
            assert!((dev - dev.round()).abs() < 1e-9, "{pick} at {dpr}: {dev}");
            // Near what was picked: never more than half a device pixel off.
            assert!((dev - pick * dpr).abs() <= 0.5 + 1e-9, "{pick} at {dpr}");
        }
        assert_eq!(snap(2.0, 1.0), 2.0);
        assert_eq!(snap(1.0, 3.0), 1.0);
        assert_eq!(snap(1.0, f64::NAN), 1.0);
    }
}
