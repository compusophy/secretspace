//! The browser end every page here shares, so a game's page is only its
//! own drawing and rules: a pixel `Screen` the game draws into and shows
//! once a frame, a `Socket` to its room, an invisible `TextField` (so a
//! phone still offers its keyboard), storage, and a frame loop. Rust only:
//! the page's one line of script just starts the wasm.

use std::cell::RefCell;
use std::rc::Rc;

use pixels::Canvas;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{Clamped, JsCast};
use web_sys::{
    BinaryType, CanvasRenderingContext2d, Document, HtmlCanvasElement, HtmlElement,
    HtmlInputElement, ImageData, MessageEvent, WebSocket, Window,
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

/// Call `f` with the time on every animation frame, forever.
pub fn frames(mut f: impl FnMut(f64) + 'static) {
    type Loop = Rc<RefCell<Option<Closure<dyn FnMut()>>>>;
    let raf: Loop = Rc::new(RefCell::new(None));
    let again = raf.clone();
    *raf.borrow_mut() = Some(Closure::new(move || {
        f(now());
        if let Some(c) = again.borrow().as_ref() {
            let _ = window().request_animation_frame(c.as_ref().unchecked_ref());
        }
    }));
    if let Some(c) = raf.borrow().as_ref() {
        let _ = window().request_animation_frame(c.as_ref().unchecked_ref());
    }
    std::mem::forget(raf);
}

pub fn load(key: &str) -> Option<String> {
    window().local_storage().ok()??.get_item(key).ok()?
}

pub fn save(key: &str, v: &str) {
    if let Ok(Some(s)) = window().local_storage() {
        let _ = s.set_item(key, v);
    }
}

/// Go to another page.
pub fn go(url: &str) {
    let _ = window().location().assign(url);
}

/// Whether the main pointer is a finger.
pub fn touch() -> bool {
    window()
        .match_media("(pointer: coarse)")
        .ok()
        .flatten()
        .is_some_and(|m| m.matches())
}

/// The WebSocket address of a room (or "hub"): `?server=` if given, else
/// `<meta name="server">` (a `wss://host/ws` address), else this page's
/// own origin. The first connection a page makes counts as a visit.
pub fn room_url(room: &str, first: bool) -> String {
    let loc = window().location();
    let param = loc.search().ok().and_then(|q| {
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
    let base = base.trim_end_matches('/').trim_end_matches("/ws");
    format!("{base}/ws/{room}{}", if first { "?v=1" } else { "" })
}

/// The canvas, and the pixels drawn into it: one buffer pixel is `scale`
/// CSS pixels, shown sharp.
pub struct Screen {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    pub px: Canvas,
    /// CSS pixels per buffer pixel.
    pub scale: f64,
    /// The window, in CSS pixels.
    pub css: (f64, f64),
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
        };
        s.fit();
        s
    }

    /// Match the window: pick the pixel size, size the buffer.
    pub fn fit(&mut self) {
        let w = window()
            .inner_width()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(800.0);
        let h = window()
            .inner_height()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(600.0);
        // About 960 buffer pixels across, at most: big screens get bigger
        // pixels, a phone gets one per CSS pixel.
        self.scale = (w / 960.0).ceil().clamp(1.0, 4.0);
        self.css = (w, h);
        let (bw, bh) = (
            (w / self.scale).ceil() as i32,
            (h / self.scale).ceil() as i32,
        );
        self.px.resize(bw, bh);
        self.canvas.set_width(bw as u32);
        self.canvas.set_height(bh as u32);
    }

    /// The text scale that reads the same size on any screen.
    pub fn ui(&self) -> i32 {
        if self.scale >= 2.0 {
            1
        } else {
            2
        }
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

/// A WebSocket to a room. Messages arrive through the callback given to
/// `open`; whether it is up is `up()`.
pub struct Socket {
    ws: WebSocket,
}

impl Socket {
    pub fn open(
        url: &str,
        on_open: impl FnMut() + 'static,
        mut on_message: impl FnMut(Vec<u8>) + 'static,
        on_close: impl FnMut() + 'static,
    ) -> Option<Socket> {
        let ws = WebSocket::new(url).ok()?;
        ws.set_binary_type(BinaryType::Arraybuffer);
        let mut on_open = on_open;
        on(&ws, "open", move |_| on_open());
        on(&ws, "message", move |e| {
            if let Ok(e) = e.dyn_into::<MessageEvent>() {
                on_message(js_sys::Uint8Array::new(&e.data()).to_vec());
            }
        });
        let mut on_close = on_close;
        on(&ws, "close", move |_| on_close());
        Some(Socket { ws })
    }

    pub fn up(&self) -> bool {
        self.ws.ready_state() == WebSocket::OPEN
    }

    pub fn send(&self, bytes: &[u8]) {
        if self.up() {
            let _ = self.ws.send_with_u8_array(bytes);
        }
    }
}

/// A text box the page draws itself: an invisible input laid over where
/// it is drawn, so a click (or a tap, with its keyboard) types into it.
pub struct TextField {
    input: HtmlInputElement,
}

impl TextField {
    pub fn new(max: u32, placeholder: &str) -> TextField {
        let input: HtmlInputElement = document()
            .create_element("input")
            .ok()
            .and_then(|e| e.dyn_into().ok())
            .expect("an input");
        input.set_max_length(max as i32);
        input.set_placeholder(placeholder);
        let _ = input.set_attribute("autocomplete", "off");
        let _ = input.set_attribute("spellcheck", "false");
        let _ = input.set_attribute("enterkeyhint", "go");
        let _ = input.set_attribute(
            "style",
            "position:fixed;opacity:0;border:0;padding:0;margin:0;font-size:16px;z-index:2;display:none",
        );
        if let Some(body) = document().body() {
            let _ = body.append_child(&input);
        }
        TextField { input }
    }

    pub fn value(&self) -> String {
        self.input.value()
    }

    pub fn set_value(&self, v: &str) {
        self.input.set_value(v);
    }

    pub fn focused(&self) -> bool {
        document()
            .active_element()
            .is_some_and(|e| e == **self.input)
    }

    pub fn focus(&self) {
        let _ = self.input.focus();
    }

    pub fn blur(&self) {
        let _ = self.input.blur();
    }

    /// Lay it over this CSS box, or hide it.
    pub fn place(&self, at: Option<(f64, f64, f64, f64)>) {
        let style = HtmlElement::style(&self.input);
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
}
