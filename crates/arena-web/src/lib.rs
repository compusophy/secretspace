//! The arena's page: the game drawn pixel by pixel into one buffer, a menu
//! drawn the same way, and the wiring between the browser (pointer, keys,
//! the socket to the arena's room) and the game.

mod menu;
mod render;
mod state;

use std::cell::RefCell;

use arena::proto::{angle_to_u16, Down, Up};
use pixels::Rect;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{KeyboardEvent, PointerEvent};

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
    screen: kit::Screen,
    st: State,
    socket: Option<kit::Socket>,
    /// The first connection counts this visit.
    counted: bool,
    retry_at: f64,
    retries: u32,
    field: kit::TextField,
    touch: bool,
    pointer: Option<(f32, f32)>,
    /// Where the pointer says to head, in radians.
    steer: Option<f32>,
    boost_mouse: bool,
    boost_key: bool,
    /// The finger holding the boost button.
    boost_finger: Option<i32>,
    /// What was last sent: angle, boost, when.
    sent: (u16, bool, f64),
    /// Play was pressed; join as soon as the server is there.
    joining: bool,
    beat_at: f64,
    menu: bool,
    spots: menu::Spots,
}

thread_local! {
    static PAGE: RefCell<Option<Page>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Page) -> R) -> Option<R> {
    PAGE.with(|p| p.try_borrow_mut().ok()?.as_mut().map(f))
}

fn send(p: &Page, up: &Up) {
    if let Some(s) = &p.socket {
        s.send(&up.encode());
    }
}

fn screen_msg(p: &Page) -> Up {
    Up::Screen {
        w: p.screen.css.0 as u16,
        h: p.screen.css.1 as u16,
    }
}

fn connect(p: &mut Page) {
    let url = kit::room_url("arena", !p.counted);
    p.counted = true;
    p.socket = kit::Socket::open(
        &url,
        || {
            with(|p| {
                p.st.connected = true;
                p.retries = 0;
                send(p, &screen_msg(p));
                if p.joining {
                    send(p, &join_msg(p));
                }
            });
        },
        |bytes| {
            let Some(msg) = Down::decode(&bytes) else {
                return;
            };
            with(|p| {
                let died = matches!(msg, Down::Died { .. });
                p.st.receive(kit::now(), msg);
                if died {
                    p.joining = false;
                    kit::save(BEST, &p.st.best.to_string());
                }
                if p.st.playing() {
                    p.joining = false;
                }
            });
        },
        || {
            with(|p| {
                p.st.connected = false;
                p.socket = None;
                p.retries += 1;
                p.retry_at = kit::now() + 500.0 * 2f64.powi(p.retries.min(4) as i32);
                p.st.mirror = Default::default();
            });
        },
    );
}

fn join_msg(p: &Page) -> Up {
    let name = p.field.value().trim().to_string();
    kit::save(NAME, &name);
    Up::Join { name }
}

fn play(p: &mut Page) {
    p.joining = true;
    p.st.death = None;
    p.field.blur();
    send(p, &join_msg(p));
}

/// The phone's boost button.
fn boost_button(p: &Page) -> Option<Rect> {
    if !p.touch {
        return None;
    }
    let u = p.screen.ui() as f32;
    let r = 30.0 * u;
    Some(Rect::new(
        10.0 * u,
        p.screen.px.h as f32 - 10.0 * u - 2.0 * r,
        2.0 * r,
        2.0 * r,
    ))
}

/// Every animation frame: draw, steer, keep the connection and the menu.
fn tick(p: &mut Page, now: f64) {
    if p.socket.is_none() && now >= p.retry_at {
        connect(p);
    }
    p.st.age(now);
    let hud = render::Hud {
        u: p.screen.ui(),
        steer: p.steer,
        boost: boost_button(p).map(|b| (b, p.boost_finger.is_some())),
    };
    let (css, scale) = (p.screen.css, p.screen.scale);
    render::frame(&mut p.screen.px, &mut p.st, css, scale, now, &hud);

    // The menu: at the start, and a moment after dying.
    let menu = !p.st.playing()
        && !p.joining
        && p.st.death.as_ref().is_none_or(|d| now - d.at > MENU_AFTER);
    if menu {
        let name = p.field.value();
        let look = menu::Look {
            u: p.screen.ui(),
            name: &name,
            editing: p.field.focused(),
            pointer: p.pointer,
            touch: p.touch,
        };
        p.spots = menu::draw(&mut p.screen.px, &p.st, &look, now);
        let n = p.spots.name;
        p.field.place(Some(p.screen.to_css(n.x, n.y, n.w, n.h)));
        if !p.menu && !p.touch {
            p.field.focus();
        }
        let over = p
            .pointer
            .is_some_and(|(x, y)| p.spots.play.contains(x, y) || p.spots.back.contains(x, y));
        p.screen.cursor(if over { "pointer" } else { "default" });
    } else if p.menu {
        p.field.place(None);
        p.field.blur();
        p.screen.cursor("crosshair");
    }
    p.menu = menu;
    p.screen.present();

    if p.st.playing() {
        let you = p.st.mirror.snakes.get(&p.st.mirror.you);
        let angle = p.steer.or(you.map(|s| s.angle)).unwrap_or(0.0);
        let a = angle_to_u16(angle);
        let boost = p.boost_mouse || p.boost_key || p.boost_finger.is_some();
        let turned = (a.wrapping_sub(p.sent.0) as i16).unsigned_abs() > 60;
        if (turned || boost != p.sent.1) && now - p.sent.2 >= STEER_EVERY {
            send(p, &Up::Steer { angle: a, boost });
            p.sent = (a, boost, now);
        }
    }
    if now - p.beat_at > HEARTBEAT {
        p.beat_at = now;
        send(p, &screen_msg(p));
    }
}

fn aim(p: &mut Page, x: f32, y: f32) {
    let (cx, cy) = (p.screen.px.w as f32 / 2.0, p.screen.px.h as f32 / 2.0);
    p.steer = Some((y - cy).atan2(x - cx));
}

fn down(p: &mut Page, e: &PointerEvent) {
    let (x, y) = p.screen.to_px(e.client_x() as f64, e.client_y() as f64);
    p.pointer = Some((x, y));
    if p.menu {
        if p.spots.play.contains(x, y) {
            play(p);
        } else if p.spots.back.contains(x, y) {
            kit::go("/");
        } else if !p.spots.name.contains(x, y) {
            p.field.blur();
        }
        return;
    }
    if boost_button(p).is_some_and(|b| b.grow(6.0).contains(x, y)) {
        p.boost_finger = Some(e.pointer_id());
        return;
    }
    aim(p, x, y);
    if e.pointer_type() == "mouse" {
        p.boost_mouse = true;
    }
}

fn boost_key(key: &str) -> bool {
    matches!(key, " " | "ArrowUp" | "w" | "W" | "Shift")
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let screen = kit::Screen::new("screen");
    let canvas = kit::document()
        .get_element_by_id("screen")
        .ok_or("no #screen")?;
    let field = kit::TextField::new(arena::laws::MAX_NAME as u32, "your name");
    if let Some(n) = kit::load(NAME) {
        field.set_value(&n);
    }
    let best = kit::load(BEST).and_then(|b| b.parse().ok()).unwrap_or(0);
    PAGE.with(|p| {
        *p.borrow_mut() = Some(Page {
            screen,
            st: State::new(best),
            socket: None,
            counted: false,
            retry_at: 0.0,
            retries: 0,
            field,
            touch: kit::touch(),
            pointer: None,
            steer: None,
            boost_mouse: false,
            boost_key: false,
            boost_finger: None,
            sent: (0, false, 0.0),
            joining: false,
            beat_at: 0.0,
            menu: false,
            spots: menu::Spots::default(),
        })
    });
    kit::frames(|now| {
        with(|p| tick(p, now));
    });

    kit::on(&kit::window(), "resize", |_| {
        with(|p| {
            p.screen.fit();
            send(p, &screen_msg(p));
        });
    });
    kit::on(&canvas, "pointerdown", |e| {
        if let Ok(e) = e.dyn_into::<PointerEvent>() {
            with(|p| down(p, &e));
        }
    });
    kit::on(&canvas, "pointermove", |e| {
        if let Ok(e) = e.dyn_into::<PointerEvent>() {
            with(|p| {
                let (x, y) = p.screen.to_px(e.client_x() as f64, e.client_y() as f64);
                p.pointer = Some((x, y));
                if !p.menu && p.boost_finger != Some(e.pointer_id()) {
                    aim(p, x, y);
                }
            });
        }
    });
    for ev in ["pointerup", "pointercancel"] {
        kit::on(&kit::window(), ev, |e| {
            if let Ok(e) = e.dyn_into::<PointerEvent>() {
                with(|p| {
                    if p.boost_finger == Some(e.pointer_id()) {
                        p.boost_finger = None;
                    }
                    if e.pointer_type() == "mouse" {
                        p.boost_mouse = false;
                    }
                });
            }
        });
    }
    kit::on(&kit::window(), "blur", |_| {
        with(|p| {
            p.boost_mouse = false;
            p.boost_key = false;
            p.boost_finger = None;
        });
    });
    kit::on(&kit::window(), "keydown", |e| {
        if let Ok(e) = e.dyn_into::<KeyboardEvent>() {
            with(|p| {
                if p.menu {
                    if e.key() == "Enter" {
                        play(p);
                    }
                    return;
                }
                if boost_key(&e.key()) {
                    e.prevent_default();
                    p.boost_key = true;
                }
            });
        }
    });
    kit::on(&kit::window(), "keyup", |e| {
        if let Ok(e) = e.dyn_into::<KeyboardEvent>() {
            if boost_key(&e.key()) {
                with(|p| p.boost_key = false);
            }
        }
    });
    Ok(())
}
