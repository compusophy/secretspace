//! The page: one island in this tab. The world crate does the living; this
//! crate wires it to the browser: a canvas to draw on, a timer to tick,
//! visibility for the sun, a channel to the other tabs, and two panels
//! (write a mote, inspect one).

mod app;
mod bus;
mod draw;
mod mesh;
mod place;
mod store;

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    CanvasRenderingContext2d, Document, HtmlCanvasElement, HtmlElement, HtmlInputElement,
    HtmlTextAreaElement, PointerEvent, VisibilityState, Window,
};

use app::App;
use bus::Bus;
use mesh::Mesh;

/// Every tab of every browser on this channel is one world.
const CHANNEL: &str = "secretspace/1";

struct Page {
    app: App,
    bus: Option<Bus>,
    mesh: Option<Mesh>,
    ctx: CanvasRenderingContext2d,
    canvas: HtmlCanvasElement,
    field: HtmlCanvasElement,
    frame: draw::Frame,
    w: f64,
    h: f64,
}

/// A frame callback that schedules itself again.
type FrameLoop = Rc<RefCell<Option<Closure<dyn FnMut()>>>>;

thread_local! {
    static PAGE: RefCell<Option<Page>> = const { RefCell::new(None) };
}

/// Run `f` on the page. Every callback enters through here; the event
/// loop never re-enters while one runs, but if it ever did, the call is
/// skipped rather than panicking.
fn with<R>(f: impl FnOnce(&mut Page) -> R) -> Option<R> {
    PAGE.with(|p| p.try_borrow_mut().ok()?.as_mut().map(f))
}

fn window() -> Window {
    web_sys::window().expect("a window")
}

fn document() -> Document {
    window().document().expect("a document")
}

pub(crate) fn now() -> f64 {
    window()
        .performance()
        .map_or_else(js_sys::Date::now, |p| p.now())
}

fn el<T: JsCast>(id: &str) -> T {
    document()
        .get_element_by_id(id)
        .unwrap_or_else(|| panic!("#{id} is in index.html"))
        .dyn_into::<T>()
        .unwrap_or_else(|_| panic!("#{id} has the expected type"))
}

fn on(target: &web_sys::EventTarget, event: &str, f: impl FnMut(web_sys::Event) + 'static) {
    let c = Closure::<dyn FnMut(web_sys::Event)>::new(f);
    let _ = target.add_event_listener_with_callback(event, c.as_ref().unchecked_ref());
    c.forget();
}

fn visible() -> bool {
    document().visibility_state() == VisibilityState::Visible
}

fn random_id() -> u64 {
    let mut b = [0u8; 8];
    if let Ok(c) = window().crypto() {
        let _ = c.get_random_values_with_u8_array(&mut b);
    }
    let id = u64::from_le_bytes(b);
    if id == 0 {
        (js_sys::Math::random() * 9.0e15) as u64 + 1
    } else {
        id
    }
}

/// Hand queued envelopes to every transport: the tabs of this browser
/// hear everything; a device hears what is addressed to it or to all.
fn flush(p: &mut Page) {
    for (to, route, bytes) in p.app.outbox.drain(..) {
        match route {
            app::Route::Islands => {
                if let Some(bus) = &p.bus {
                    bus.post(&bytes);
                }
                if let Some(mesh) = &p.mesh {
                    mesh.send(to, &bytes);
                }
            }
            app::Route::Relay => {
                if let Some(mesh) = &p.mesh {
                    mesh.census(&bytes);
                }
            }
        }
    }
}

/// A binary message from the relay: the world's census.
fn relay_bytes(p: &mut Page, bytes: &[u8]) {
    p.app.relay(now(), bytes);
}

/// An envelope from another island over a direct channel.
fn envelope_bytes(p: &mut Page, bytes: &[u8]) {
    p.app.receive(now(), bytes);
    flush(p);
}

fn meta(name: &str) -> Option<String> {
    document()
        .query_selector(&format!("meta[name={name}]"))
        .ok()
        .flatten()
        .and_then(|m| m.get_attribute("content"))
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
}

/// Who introduces this island to islands on other devices: this project's
/// relay (`<meta name="relay">`, else `?relay=`, else this page's own
/// origin; "off" for none) and public trackers (`<meta name="trackers">`,
/// comma separated; `?trackers=off` for none). Each is (url, is ours).
fn trackers() -> Vec<(String, bool)> {
    let mut out = Vec::new();
    match place::param("relay").or_else(|| meta("relay")) {
        Some(u) if u == "off" => {}
        Some(u) => out.push((u, true)),
        None => {
            let loc = window().location();
            if let (Ok(host), Ok(proto)) = (loc.host(), loc.protocol()) {
                let scheme = if proto == "https:" { "wss" } else { "ws" };
                out.push((format!("{scheme}://{host}/ws"), true));
            }
        }
    }
    let public = place::param("trackers")
        .or_else(|| meta("trackers"))
        .unwrap_or_default();
    if public != "off" {
        for url in public
            .split(',')
            .map(str::trim)
            .filter(|u| u.starts_with("ws"))
        {
            out.push((url.to_string(), false));
        }
    }
    out
}

fn resize(p: &mut Page) {
    let dpr = window().device_pixel_ratio().max(1.0);
    let (w, h) = (
        window()
            .inner_width()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(800.0),
        window()
            .inner_height()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(600.0),
    );
    p.canvas.set_width((w * dpr) as u32);
    p.canvas.set_height((h * dpr) as u32);
    let _ = p.ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);
    p.w = w;
    p.h = h;
}

fn draw(p: &mut Page) {
    p.frame = draw::frame(&p.ctx, &p.field, &p.app, p.w, p.h, now());
}

// ---- panels --------------------------------------------------------------

fn show(id: &str, yes: bool) {
    let e: HtmlElement = el(id);
    e.set_hidden(!yes);
}

fn set_text(id: &str, s: &str) {
    let e: HtmlElement = el(id);
    e.set_text_content(Some(s));
}

fn inspect(p: &Page) {
    let Some(id) = p.app.selected else {
        show("inspect", false);
        return;
    };
    show("inspect", true);
    match p.app.island.motes().iter().find(|m| m.id == id) {
        Some(m) => {
            set_text("i-name", &m.name);
            let by = if &*m.author == "genesis" {
                "a founder".to_string()
            } else {
                format!("by {}", m.author)
            };
            set_text(
                "i-meta",
                &format!(
                    "{by} · generation {} · crossed {} portal{}\n{} ergs · thought for {} last tick · age {}\nat ({}, {}) · lineage {:016x}",
                    m.gen,
                    m.hops,
                    if m.hops == 1 { "" } else { "s" },
                    m.balance,
                    m.last_used,
                    m.age,
                    m.x,
                    m.y,
                    m.lineage
                ),
            );
            let src: HtmlElement = el("i-src");
            if src.text_content().as_deref() != Some(m.genome.src.as_str()) {
                src.set_text_content(Some(&m.genome.src));
            }
        }
        None => set_text("i-meta", "gone: it left this island, or died."),
    }
}

fn pick(p: &mut Page, x: f64, y: f64) {
    let (cx, cy) = p.frame.to_cell(x, y);
    if p.app.placing.is_some() {
        let inside =
            cx >= 0.0 && cy >= 0.0 && cx < space::laws::W as f64 && cy < space::laws::H as f64;
        if inside {
            if let Err(e) = p.app.place(cx as i64, cy as i64, now()) {
                p.app
                    .say(now(), format!("could not release here: {e}"), None);
            }
        }
        return;
    }
    let best = p
        .app
        .island
        .motes()
        .iter()
        .map(|m| {
            let (dx, dy) = (m.x as f64 + 0.5 - cx, m.y as f64 + 0.5 - cy);
            (dx * dx + dy * dy, m.id)
        })
        .filter(|(d, _)| *d < 2.5)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    p.app.selected = best.map(|(_, id)| id);
    inspect(p);
}

fn open_editor() {
    let src: HtmlTextAreaElement = el("m-src");
    if src.value().trim().is_empty() {
        src.set_value(space::founders::TEMPLATE);
    }
    let author: HtmlInputElement = el("m-author");
    author.set_value(&store::author());
    show("intro", false);
    show("editor", true);
}

fn close_intro() {
    show("intro", false);
    store::set(store::SEEN, "1");
}

fn wire_panels() {
    on(&el::<HtmlElement>("write"), "click", |_| open_editor());
    on(&el::<HtmlElement>("about"), "click", |_| {
        show("editor", false);
        show("intro", true);
    });
    on(&el::<HtmlElement>("intro-close"), "click", |_| {
        close_intro()
    });
    on(&el::<HtmlElement>("intro-watch"), "click", |_| {
        close_intro()
    });
    on(&el::<HtmlElement>("intro-write"), "click", |_| {
        close_intro();
        open_editor();
    });
    if store::get(store::SEEN).is_none() {
        show("intro", true);
    }
    on(&el::<HtmlElement>("editor-close"), "click", |_| {
        show("editor", false)
    });
    on(&el::<HtmlElement>("i-close"), "click", |_| {
        with(|p| {
            p.app.selected = None;
            inspect(p);
        });
    });
    on(&el::<HtmlElement>("m-card"), "click", |_| {
        let card: HtmlElement = el("m-cardtext");
        card.set_text_content(Some(&space::laws::card()));
        card.set_hidden(!card.hidden());
    });
    on(&el::<HtmlElement>("m-author"), "change", |_| {
        let a: HtmlInputElement = el("m-author");
        store::set_author(&a.value());
        with(|p| p.app.author = store::author());
    });
    on(&el::<HtmlElement>("i-fork"), "click", |_| {
        let src = with(|p| {
            let id = p.app.selected?;
            p.app
                .island
                .motes()
                .iter()
                .find(|m| m.id == id)
                .map(|m| m.genome.src.clone())
        })
        .flatten();
        if let Some(src) = src {
            let t: HtmlTextAreaElement = el("m-src");
            t.set_value(&src);
            open_editor();
        }
    });
    on(&el::<HtmlElement>("m-release"), "click", |_| {
        let src: HtmlTextAreaElement = el("m-src");
        let name: HtmlInputElement = el("m-name");
        let author: HtmlInputElement = el("m-author");
        store::set_author(&author.value());
        let n = name.value();
        let n = if n.trim().is_empty() {
            "unnamed".to_string()
        } else {
            n.trim().to_string()
        };
        let result = with(|p| {
            p.app.author = store::author();
            p.app.prepare(&src.value(), &n)
        });
        match result {
            Some(Ok(())) => {
                set_text("m-err", "");
                show("editor", false);
            }
            Some(Err(e)) => set_text("m-err", &e),
            None => {}
        }
    });
}

// ---- start ---------------------------------------------------------------

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let canvas: HtmlCanvasElement = el("world");
    let ctx = canvas
        .get_context("2d")?
        .ok_or("no 2d context")?
        .dyn_into::<CanvasRenderingContext2d>()?;
    let field = document()
        .create_element("canvas")?
        .dyn_into::<HtmlCanvasElement>()?;
    field.set_width(space::laws::W as u32);
    field.set_height(space::laws::H as u32);

    let id = random_id();
    let mut app = App::new(id, place::here(), store::author(), now());
    app.set_watched(visible(), now());
    let mut page = Page {
        app,
        bus: None,
        mesh: Some(Mesh::new(id, trackers())),
        ctx,
        canvas: canvas.clone(),
        field,
        frame: draw::layout(800.0, 600.0),
        w: 800.0,
        h: 600.0,
    };
    resize(&mut page);
    PAGE.with(|p| *p.borrow_mut() = Some(page));

    // Other tabs.
    let bus = Bus::new(CHANNEL, |bytes| {
        with(|p| {
            p.app.receive(now(), &bytes);
            flush(p);
        });
    })?;
    with(|p| p.bus = Some(bus));

    // The clock: ten ticks a second; a hidden tab's throttled timer catches up.
    let tick = Closure::<dyn FnMut()>::new(|| {
        with(|p| {
            let t = now();
            if let Some(m) = p.mesh.as_mut() {
                m.maintain(t);
                p.app.links_open = m.open_links();
                p.app.relay_up = m.trackers_up() > 0;
                p.app.swarm = m.swarm();
            }
            p.app.pump(t);
            flush(p);
            if p.app.selected.is_some() {
                inspect(p);
            }
        });
    });
    window().set_interval_with_callback_and_timeout_and_arguments_0(
        tick.as_ref().unchecked_ref(),
        50,
    )?;
    tick.forget();

    // Frames, only while someone can see them.
    let raf: FrameLoop = Rc::new(RefCell::new(None));
    let again = raf.clone();
    *raf.borrow_mut() = Some(Closure::new(move || {
        with(draw);
        if let Some(c) = again.borrow().as_ref() {
            let _ = window().request_animation_frame(c.as_ref().unchecked_ref());
        }
    }));
    if let Some(c) = raf.borrow().as_ref() {
        window().request_animation_frame(c.as_ref().unchecked_ref())?;
    }
    std::mem::forget(raf);

    // The sun is attention.
    on(&document(), "visibilitychange", |_| {
        with(|p| p.app.set_watched(visible(), now()));
    });
    on(&window(), "pagehide", |_| {
        with(|p| {
            p.app.close();
            flush(p);
        });
    });
    on(&window(), "resize", |_| {
        with(resize);
    });
    on(&window(), "keydown", |e| {
        if let Ok(e) = e.dyn_into::<web_sys::KeyboardEvent>() {
            if e.key() == "Escape" {
                show("editor", false);
                if !el::<HtmlElement>("intro").hidden() {
                    close_intro();
                }
                with(|p| {
                    p.app.placing = None;
                    p.app.selected = None;
                    inspect(p);
                });
            }
        }
    });
    on(&canvas, "pointerdown", |e| {
        if let Ok(e) = e.dyn_into::<PointerEvent>() {
            let (x, y) = (e.client_x() as f64, e.client_y() as f64);
            with(|p| pick(p, x, y));
        }
    });
    wire_panels();
    Ok(())
}
