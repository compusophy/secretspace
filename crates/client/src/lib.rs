//! The page: one canvas showing the arena, a menu to join it, and the
//! wiring between the browser (pointer, keys, a WebSocket) and the game.

mod render;
mod state;

use std::cell::RefCell;
use std::rc::Rc;

use game::proto::{angle_to_u16, Down, Up};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    BinaryType, CanvasRenderingContext2d, Document, HtmlCanvasElement, HtmlElement,
    HtmlInputElement, KeyboardEvent, MessageEvent, PointerEvent, WebSocket, Window,
};

use state::State;

const NAME: &str = "secretspace/name";
const BEST: &str = "secretspace/best";
/// Steering goes out at most this often (ms), and only when it changed.
const STEER_EVERY: f64 = 45.0;
/// A heartbeat (the screen size) keeps a watching page connected.
const HEARTBEAT: f64 = 10_000.0;
/// After dying, the burst plays this long before the menu comes back.
const MENU_AFTER: f64 = 1200.0;

struct Page {
    ctx: CanvasRenderingContext2d,
    canvas: HtmlCanvasElement,
    w: f64,
    h: f64,
    st: State,
    ws: Option<WebSocket>,
    url: String,
    /// Where the pointer says to head, in radians.
    steer: Option<f32>,
    boost_pointer: bool,
    boost_key: bool,
    boost_button: bool,
    /// What was last sent: angle, boost, when.
    sent: (u16, bool, f64),
    /// Play was pressed; join as soon as the server is there.
    joining: bool,
    retry_at: f64,
    retries: u32,
    beat_at: f64,
    menu: bool,
}

thread_local! {
    static PAGE: RefCell<Option<Page>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Page) -> R) -> Option<R> {
    PAGE.with(|p| p.try_borrow_mut().ok()?.as_mut().map(f))
}

fn window() -> Window {
    web_sys::window().expect("a window")
}

fn document() -> Document {
    window().document().expect("a document")
}

fn now() -> f64 {
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

fn store_get(key: &str) -> Option<String> {
    window().local_storage().ok()??.get_item(key).ok()?
}

fn store_set(key: &str, v: &str) {
    if let Ok(Some(s)) = window().local_storage() {
        let _ = s.set_item(key, v);
    }
}

fn show(id: &str, yes: bool) {
    el::<HtmlElement>(id).set_hidden(!yes);
}

fn set_text(id: &str, s: &str) {
    el::<HtmlElement>(id).set_text_content(Some(s));
}

/// The server: `?server=`, else `<meta name="server">`, else this page's
/// own origin.
fn server_url() -> String {
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
    param.or(meta).unwrap_or_else(|| {
        let host = loc.host().unwrap_or_default();
        let secure = loc.protocol().is_ok_and(|p| p == "https:");
        format!("{}://{host}/ws", if secure { "wss" } else { "ws" })
    })
}

fn send(p: &Page, up: &Up) {
    if let Some(ws) = &p.ws {
        if ws.ready_state() == WebSocket::OPEN {
            let _ = ws.send_with_u8_array(&up.encode());
        }
    }
}

fn screen(p: &Page) -> Up {
    Up::Screen {
        w: p.w as u16,
        h: p.h as u16,
    }
}

fn connect(p: &mut Page) {
    let Ok(ws) = WebSocket::new(&p.url) else {
        p.retry_at = now() + 2000.0;
        return;
    };
    ws.set_binary_type(BinaryType::Arraybuffer);
    on(&ws, "open", |_| {
        with(|p| {
            p.st.connected = true;
            p.retries = 0;
            send(p, &screen(p));
            if p.joining {
                send(p, &join_msg());
            }
        });
    });
    on(&ws, "message", |e| {
        let Ok(e) = e.dyn_into::<MessageEvent>() else {
            return;
        };
        let bytes = js_sys::Uint8Array::new(&e.data()).to_vec();
        if let Some(msg) = Down::decode(&bytes) {
            with(|p| {
                let died = matches!(msg, Down::Died { .. });
                p.st.receive(now(), msg);
                if died {
                    p.joining = false;
                    store_set(BEST, &p.st.best.to_string());
                }
                if p.st.playing() {
                    p.joining = false;
                }
            });
        }
    });
    on(&ws, "close", |_| {
        with(|p| {
            p.st.connected = false;
            p.ws = None;
            p.retries += 1;
            p.retry_at = now() + (500.0 * 2f64.powi(p.retries.min(4) as i32));
            // Whoever was playing is gone with the connection.
            p.st.mirror = Default::default();
        });
    });
    p.ws = Some(ws);
}

fn join_msg() -> Up {
    let input: HtmlInputElement = el("name");
    let name = input.value().trim().to_string();
    store_set(NAME, &name);
    Up::Join { name }
}

fn play() {
    with(|p| {
        p.joining = true;
        p.st.death = None;
        p.menu = false;
        show("menu", false);
        send(p, &join_msg());
    });
}

fn resize(p: &mut Page) {
    let dpr = window().device_pixel_ratio().clamp(1.0, 2.0);
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
    p.canvas.set_width((w * dpr) as u32);
    p.canvas.set_height((h * dpr) as u32);
    let _ = p.ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);
    p.w = w;
    p.h = h;
    send(p, &screen(p));
}

/// Every animation frame: draw, steer, keep the connection and the menu.
fn tick(p: &mut Page) {
    let t = now();
    if p.ws.is_none() && t >= p.retry_at {
        connect(p);
    }
    p.st.age(t);
    render::frame(&p.ctx, &mut p.st, p.w, p.h, t, p.steer);

    if p.st.playing() {
        let you = p.st.mirror.snakes.get(&p.st.mirror.you);
        let angle = p.steer.or(you.map(|s| s.angle)).unwrap_or(0.0);
        let a = angle_to_u16(angle);
        let boost = p.boost_pointer || p.boost_key || p.boost_button;
        let turned = (a.wrapping_sub(p.sent.0) as i16).unsigned_abs() > 60;
        if (turned || boost != p.sent.1) && t - p.sent.2 >= STEER_EVERY {
            send(p, &Up::Steer { angle: a, boost });
            p.sent = (a, boost, t);
        }
    }
    if t - p.beat_at > HEARTBEAT {
        p.beat_at = t;
        send(p, &screen(p));
    }

    // The menu: at the start, and a moment after dying.
    let want_menu =
        !p.st.playing() && !p.joining && p.st.death.as_ref().is_none_or(|d| t - d.at > MENU_AFTER);
    if want_menu != p.menu {
        p.menu = want_menu;
        if want_menu {
            match &p.st.death {
                Some(d) => {
                    let by = if d.by.is_empty() {
                        "the edge of the world".to_string()
                    } else {
                        d.by.clone()
                    };
                    set_text("verdict", &format!("you ran into {by}"));
                    set_text("score", &format!("length {} · best {}", d.score, p.st.best));
                    set_text("play", "play again");
                    show("result", true);
                }
                None => show("result", false),
            }
            show("menu", true);
            let input: HtmlInputElement = el("name");
            let _ = input.focus();
        } else {
            show("menu", false);
        }
    }
    let online = if p.st.connected {
        let n = p.st.board.people;
        format!(
            "{n} {} playing now",
            if n == 1 { "person" } else { "people" }
        )
    } else {
        "connecting to the arena…".to_string()
    };
    let el_online: HtmlElement = el("online");
    if el_online.text_content().as_deref() != Some(online.as_str()) {
        el_online.set_text_content(Some(&online));
    }
}

fn pointer_angle(p: &Page, x: f64, y: f64) -> f32 {
    ((y - p.h / 2.0).atan2(x - p.w / 2.0)) as f32
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let canvas: HtmlCanvasElement = el("arena");
    let ctx = canvas
        .get_context("2d")?
        .ok_or("no 2d context")?
        .dyn_into::<CanvasRenderingContext2d>()?;
    let best = store_get(BEST).and_then(|b| b.parse().ok()).unwrap_or(0);
    if let Some(n) = store_get(NAME) {
        el::<HtmlInputElement>("name").set_value(&n);
    }
    let mut page = Page {
        ctx,
        canvas: canvas.clone(),
        w: 800.0,
        h: 600.0,
        st: State::new(best),
        ws: None,
        url: server_url(),
        steer: None,
        boost_pointer: false,
        boost_key: false,
        boost_button: false,
        sent: (0, false, 0.0),
        joining: false,
        retry_at: 0.0,
        retries: 0,
        beat_at: 0.0,
        menu: true,
    };
    resize(&mut page);
    PAGE.with(|p| *p.borrow_mut() = Some(page));

    // Draw every frame.
    type Loop = Rc<RefCell<Option<Closure<dyn FnMut()>>>>;
    let raf: Loop = Rc::new(RefCell::new(None));
    let again = raf.clone();
    *raf.borrow_mut() = Some(Closure::new(move || {
        with(tick);
        if let Some(c) = again.borrow().as_ref() {
            let _ = window().request_animation_frame(c.as_ref().unchecked_ref());
        }
    }));
    if let Some(c) = raf.borrow().as_ref() {
        window().request_animation_frame(c.as_ref().unchecked_ref())?;
    }
    std::mem::forget(raf);

    on(&window(), "resize", |_| {
        with(resize);
    });

    // Steering: the head follows the pointer; a mouse button boosts.
    on(&canvas, "pointermove", |e| {
        if let Ok(e) = e.dyn_into::<PointerEvent>() {
            with(|p| p.steer = Some(pointer_angle(p, e.client_x() as f64, e.client_y() as f64)));
        }
    });
    on(&canvas, "pointerdown", |e| {
        if let Ok(e) = e.dyn_into::<PointerEvent>() {
            with(|p| {
                p.steer = Some(pointer_angle(p, e.client_x() as f64, e.client_y() as f64));
                if e.pointer_type() == "mouse" {
                    p.boost_pointer = true;
                }
            });
        }
    });
    let release = |_| {
        with(|p| p.boost_pointer = false);
    };
    on(&window(), "pointerup", release);
    on(&window(), "blur", |_| {
        with(|p| {
            p.boost_pointer = false;
            p.boost_key = false;
            p.boost_button = false;
        });
    });
    on(&window(), "keydown", |e| {
        if let Ok(e) = e.dyn_into::<KeyboardEvent>() {
            let menu = with(|p| p.menu).unwrap_or(true);
            if menu {
                if e.key() == "Enter" {
                    play();
                }
                return;
            }
            if matches!(e.key().as_str(), " " | "ArrowUp" | "w" | "W" | "Shift") {
                e.prevent_default();
                with(|p| p.boost_key = true);
            }
        }
    });
    on(&window(), "keyup", |e| {
        if let Ok(e) = e.dyn_into::<KeyboardEvent>() {
            if matches!(e.key().as_str(), " " | "ArrowUp" | "w" | "W" | "Shift") {
                with(|p| p.boost_key = false);
            }
        }
    });

    // Phones: a button to hold for speed.
    let boost: HtmlElement = el("boost");
    on(&boost, "pointerdown", |e| {
        e.prevent_default();
        with(|p| p.boost_button = true);
    });
    for ev in ["pointerup", "pointercancel", "pointerleave"] {
        on(&boost, ev, |_| {
            with(|p| p.boost_button = false);
        });
    }

    on(&el::<HtmlElement>("play"), "click", |_| play());
    Ok(())
}
