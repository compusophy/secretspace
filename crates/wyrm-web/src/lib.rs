//! wyrm's page: the game drawn pixel by pixel into one buffer, a menu
//! drawn the same way, and the wiring between the browser (pointer, keys,
//! the link to its room) and the game. The link says Hello first, so the
//! server knows this soul and the name it goes by; when the server holds
//! still (a deploy) the last picture stays, dimmed, and the snake carries
//! on where it was once the link is back. Esc (or, between lives on a
//! phone, the menu button) opens the menu every game shares
//! (`kit::meta`): feedback, and the only way out of the game.

mod menu;
mod render;
mod state;

use std::cell::RefCell;

use engine::who::{clean_name, Seen, Status};
use kit::Net;
use pixels::Rect;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{KeyboardEvent, PointerEvent};
use wyrm::proto::{angle_to_u16, Down, Up};

use state::State;

const BEST: &str = "secretspace/wyrm/best";
/// Where the best length was kept before every game had its own keys.
const OLD_BEST: &str = "secretspace/best";
/// A newer page is looked for this often (ms), and after every death.
const VERSION_EVERY: f64 = 5.0 * 60_000.0;
/// Steering goes out at most this often (ms), and only when it changed.
const STEER_EVERY: f64 = 45.0;
/// A heartbeat (the screen size) keeps a watching page connected.
const HEARTBEAT: f64 = 10_000.0;
/// After dying, the burst plays this long before the menu comes back.
const MENU_AFTER: f64 = 1200.0;

struct Page {
    screen: kit::Screen,
    st: State,
    link: kit::Link,
    session: kit::Session,
    version: kit::Version,
    /// The name the server says this soul goes by.
    seen: Option<String>,
    /// The name asked for belongs to someone else.
    taken: bool,
    /// A new name was asked for; join once the server says it is ours.
    renaming: bool,
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
    /// The menu every game shares.
    meta: kit::meta::Meta,
}

thread_local! {
    static PAGE: RefCell<Option<Page>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Page) -> R) -> Option<R> {
    PAGE.with(|p| p.try_borrow_mut().ok()?.as_mut().map(f))
}

fn send(p: &Page, up: &Up) {
    p.link.send(&up.encode());
}

fn screen_msg(p: &Page) -> Up {
    Up::Screen {
        w: p.screen.css.0 as u16,
        h: p.screen.css.1 as u16,
    }
}

/// What the link brought since the last frame.
fn net(p: &mut Page, now: f64) {
    for ev in p.link.poll(now) {
        match ev {
            Net::Up => {
                p.st.connected = true;
                send(p, &screen_msg(p));
                // Back after a hold: the snake is waiting for its soul.
                if p.st.playing() {
                    p.joining = true;
                }
                if p.joining && !p.renaming {
                    send(p, &join_msg(p));
                }
            }
            // Keep the picture; it is coming back.
            Net::Holding => p.st.connected = false,
            Net::Message(bytes) => receive(p, now, &bytes),
        }
    }
}

fn receive(p: &mut Page, now: f64, bytes: &[u8]) {
    if let Some(seen) = Seen::decode(bytes) {
        if seen.status == Status::Taken {
            p.taken = true;
            p.joining = false;
            p.renaming = false;
            return;
        }
        // The stored name wins over what this browser last typed.
        if !seen.name.is_empty() && p.field.value() == p.session.name() {
            p.field.set_value(&seen.name);
        }
        p.session.set_name(&seen.name);
        p.link.set_hello(p.session.hello(&seen.name, false));
        p.seen = Some(seen.name);
        if p.renaming {
            p.renaming = false;
            p.taken = false;
            if p.joining {
                send(p, &join_msg(p));
            }
        }
        return;
    }
    let Some(msg) = Down::decode(bytes) else {
        return;
    };
    let died = matches!(msg, Down::Died { .. });
    p.st.receive(now, msg);
    if died {
        p.joining = false;
        kit::save(BEST, &p.st.best.to_string());
        p.version.poll(now, true);
    }
    if p.st.playing() {
        p.joining = false;
    }
}

fn join_msg(p: &Page) -> Up {
    Up::Join {
        name: clean_name(&p.field.value()),
    }
}

fn play(p: &mut Page) {
    p.joining = true;
    p.st.death = None;
    p.field.blur();
    let name = clean_name(&p.field.value());
    p.field.set_value(&name);
    if !name.is_empty() && p.seen.as_deref() != Some(name.as_str()) {
        // A new name: the server says whether it is free first.
        p.renaming = true;
        let hello = p.session.hello(&name, true);
        p.link.send(&hello);
        p.link.set_hello(hello);
    } else {
        send(p, &join_msg(p));
    }
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
    net(p, now);
    p.version.poll(now, false);
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
    // A newer page is out: take it now, while it costs nothing.
    if menu && p.st.death.is_some() && p.version.newer() {
        kit::version::reload();
    }
    if p.meta.is_open() {
        // The shared menu over everything: the name waits.
        p.field.place(None);
        p.field.blur();
        p.screen.cursor("default");
    } else if menu {
        let name = p.field.value();
        let look = menu::Look {
            u: p.screen.ui(),
            name: &name,
            editing: p.field.focused(),
            pointer: p.pointer,
            touch: p.touch,
            taken: p.taken,
        };
        p.spots = menu::draw(&mut p.screen.px, &p.st, &look, now);
        let n = p.spots.name;
        p.field.place(Some(p.screen.to_css(n.x, n.y, n.w, n.h)));
        if !p.menu && !p.touch {
            p.field.focus();
        }
        let over = p
            .pointer
            .is_some_and(|(x, y)| p.spots.play.contains(x, y) || p.spots.menu.contains(x, y));
        p.screen.cursor(if over { "pointer" } else { "default" });
    } else if p.menu {
        p.field.place(None);
        p.field.blur();
        p.screen.cursor("crosshair");
    }
    p.menu = menu;
    if p.meta.is_open() {
        p.meta.context = format!(
            "game: wyrm\nplaying: {}\nconnected: {}\nbest: {}\n",
            p.st.playing(),
            p.st.connected,
            p.st.best
        );
        let s = p.screen.scale;
        let u = p.screen.ui();
        p.meta.draw(&mut p.screen.px, u, &[], &[], now, |r| {
            (
                r.x as f64 * s,
                r.y as f64 * s,
                r.w as f64 * s,
                r.h as f64 * s,
            )
        });
    }
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
    if p.meta.is_open() {
        match p.meta.click(x, y, kit::now()) {
            Some(Some(kit::meta::Pick::Exit)) => kit::shell::exit(),
            Some(_) => {}
            // Off the menu: back to the game.
            None => p.meta.hide(),
        }
        return;
    }
    if p.menu {
        if p.spots.play.contains(x, y) {
            play(p);
        } else if p.spots.menu.contains(x, y) {
            p.meta.show();
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
    let field = kit::TextField::new(wyrm::laws::MAX_NAME as u32, "your name");
    let session = kit::Session::load();
    let name = session.name();
    field.set_value(&name);
    let best = kit::load_moved(BEST, OLD_BEST)
        .and_then(|b| b.parse().ok())
        .unwrap_or(0);
    let link = kit::Link::open("wyrm", session.hello(&name, false), false);
    PAGE.with(|p| {
        *p.borrow_mut() = Some(Page {
            screen,
            st: State::new(best),
            link,
            session,
            version: kit::Version::watch(VERSION_EVERY),
            seen: None,
            taken: false,
            renaming: false,
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
            meta: kit::meta::Meta::new("wyrm"),
        })
    });
    kit::report::on_panic();
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
                // Under the menu the snake keeps its heading.
                if !p.menu && !p.meta.is_open() && p.boost_finger != Some(e.pointer_id()) {
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
                // Esc: the shared menu (open it, step back, close it).
                if e.key() == "Escape" {
                    e.prevent_default();
                    p.meta.escape();
                    return;
                }
                if p.meta.is_open() {
                    if e.key() == "Enter" {
                        p.meta.enter(kit::now());
                    }
                    return;
                }
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
