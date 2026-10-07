//! Luciphon's page: one pointer (five gestures and the Heart) to Inputs 30
//! times a second, its own Lumen predicted, everyone else 66 ms behind,
//! the island drawn by `lucilook` at most 60 times a second. `?perf=1`
//! shows frame times; `?trace=1` records the pointer for download.

pub mod first;
pub mod gesture;
pub mod input;
pub mod laws;
pub mod state;

use std::cell::RefCell;

use engine::who::{clean_name, Seen, Status};
use kit::Net;
use lucilook::{hud, palette, Look, View};
use luciphon::combat::Act;
use luciphon::proto::{Up, PROTO};
use pixels::{Rect, Rgba};
use wasm_bindgen::prelude::*;

use first::Panel;
use input::{Input, TICK_MS};
use laws::FEEL;
use state::State;

const TAUGHT: &str = "secretspace/luciphon/taught";
const VERSION_EVERY: f64 = 5.0 * 60_000.0;

struct Page {
    screen: kit::Screen,
    look: Look,
    st: State,
    input: Input,
    link: kit::Link,
    session: kit::Session,
    version: kit::Version,
    pointer: kit::Pointer,
    name: kit::TextField,
    words: kit::TextField,
    panel: Panel,
    spots: first::Spots,
    seen: Option<String>,
    taken: bool,
    renaming: bool,
    joining: bool,
    restore_bad: bool,
    touch: bool,
    drawn_at: f64,
    ticked_at: f64,
    ping_at: f64,
    perf: Option<f64>,
    /// ?light=1: a night light pass every frame, to measure its cost.
    light: bool,
    /// Where each body was drawn last frame (for skid marks and streaks).
    was: std::collections::HashMap<u16, (f32, f32)>,
    heart_down: bool,
}

thread_local! {
    static PAGE: RefCell<Option<Page>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Page) -> R) -> Option<R> {
    PAGE.with(|p| p.try_borrow_mut().ok()?.as_mut().map(f))
}

fn query(k: &str) -> bool {
    kit::window()
        .location()
        .search()
        .is_ok_and(|q| q.trim_start_matches('?').split('&').any(|kv| kv == k))
}

/// The Heart, in CSS pixels: centre and radius.
fn heart(p: &Page) -> (f64, f64, f64) {
    let (w, h) = p.screen.css;
    (
        w / 2.0,
        h - FEEL.heart_above - FEEL.heart_px / 2.0,
        FEEL.heart_hit,
    )
}

fn facing(p: &Page) -> u16 {
    p.st.pred.me.body.facing
}

fn send(p: &Page, up: &Up) {
    p.link.send(&up.encode());
}

fn join(p: &mut Page) {
    p.joining = true;
    if !p.renaming {
        send(p, &Up::Join { proto: PROTO });
    }
}

fn play(p: &mut Page) {
    let name = clean_name(&p.name.value());
    p.name.set_value(&name);
    p.name.blur();
    p.session.set_name(&name);
    if !name.is_empty() && p.seen.as_deref() != Some(name.as_str()) {
        p.renaming = true;
        let hello = p.session.hello(&name, true);
        p.link.send(&hello);
        p.link.set_hello(hello);
    }
    join(p);
}

fn net(p: &mut Page, now: f64) {
    for ev in p.link.poll(now) {
        match ev {
            Net::Up => {
                p.st.connected = true;
                send(
                    p,
                    &Up::Device {
                        w: p.screen.css.0 as u16,
                        h: p.screen.css.1 as u16,
                        touch: p.touch,
                    },
                );
                // Back from a Stillness: the Lumen is waiting for its soul.
                if p.st.joined || p.joining {
                    p.st.joined = false;
                    join(p);
                }
            }
            Net::Holding => p.st.connected = false,
            Net::Message(b) => {
                if let Some(seen) = Seen::decode(&b) {
                    if seen.status == Status::Taken {
                        p.taken = true;
                        p.renaming = false;
                        p.joining = false;
                        continue;
                    }
                    if !seen.name.is_empty() && p.name.value() == p.session.name() {
                        p.name.set_value(&seen.name);
                    }
                    p.session.set_name(&seen.name);
                    p.link.set_hello(p.session.hello(&seen.name, false));
                    p.seen = Some(seen.name);
                    if p.renaming {
                        p.renaming = false;
                        p.taken = false;
                        if p.joining {
                            send(p, &Up::Join { proto: PROTO });
                        }
                    }
                    continue;
                }
                let was = p.st.joined;
                p.st.receive(&b, now, &mut p.look);
                if p.st.joined && !was {
                    p.joining = false;
                    p.input.next_at = now;
                }
            }
        }
    }
    for ms in p.st.felt.drain(..) {
        kit::vibrate(ms);
    }
}

fn menu_press(p: &mut Page, x: f32, y: f32) {
    let s = p.spots;
    match p.panel {
        Panel::Title => {
            if s.play.contains(x, y) {
                play(p);
            } else if s.words.contains(x, y) {
                p.panel = Panel::Words;
            } else if s.restore.contains(x, y) {
                p.panel = Panel::Restore;
                p.words.set_value("");
                p.words.focus();
            } else if s.home.contains(x, y) {
                kit::go("/");
            } else if !s.name.contains(x, y) {
                p.name.blur();
            }
        }
        Panel::Words => {
            if s.back.contains(x, y) {
                p.panel = Panel::Title;
            }
        }
        Panel::Restore => {
            if s.go.contains(x, y) {
                if p.session.restore(&p.words.value()) {
                    // A new soul: start again as it.
                    kit::version::reload();
                } else {
                    p.restore_bad = true;
                }
            } else if s.back.contains(x, y) {
                p.panel = Panel::Title;
                p.words.blur();
            }
        }
    }
}

fn frame(p: &mut Page, now: f64) {
    net(p, now);
    let playing = p.st.joined && p.st.connected;
    let (hx, hy, hr) = heart(p);
    let on_heart = move |x: f64, y: f64| ((x - hx).powi(2) + (y - hy).powi(2)).sqrt() < hr;
    for press in p.pointer.drain() {
        if !playing {
            if press.kind == kit::pointer::Kind::Down {
                let (x, y) = p.screen.to_px(press.x, press.y);
                menu_press(p, x, y);
            }
            continue;
        }
        if press.kind == kit::pointer::Kind::Down {
            p.heart_down = on_heart(press.x, press.y);
        }
        if press.kind == kit::pointer::Kind::Up {
            p.heart_down = false;
        }
        let f = facing(p);
        p.input.feed(&press, &FEEL, &on_heart, f);
    }
    let f = facing(p);
    p.input.tick(now, &FEEL, f);
    if p.input.done != 0 {
        kit::save(TAUGHT, &p.input.done.to_string());
    }
    // Inputs, 30 a second.
    if playing {
        if p.input.next_at < now - 250.0 {
            p.input.next_at = now;
        }
        while now >= p.input.next_at {
            let (seq, it) = p.input.sample(&FEEL, facing(p));
            send(p, &Up::Input { seq, it });
            p.st.pred.push(seq, it, &p.st.mirror.tiles, &p.st.laws);
            p.input.next_at += TICK_MS;
        }
        while p.input.hearts > 0 {
            p.input.hearts -= 1;
            send(p, &Up::Heart { act: 1, arg: 0 });
            // ?trace=1: the Heart saves the pointer trace (a phone has no keys).
            if let Some(t) = &p.input.trace {
                kit::download("luciphon-trace.txt", t.join("\n").as_bytes());
            }
        }
    }
    if now - p.ping_at > 1000.0 {
        p.ping_at = now;
        let rtt = (p.st.rtt / 4.0).clamp(0.0, 255.0) as u8;
        send(p, &Up::Ping { t: now as u32, rtt });
    }
    p.version.poll(now, false);
    p.st.age(now, now - p.ticked_at);
    p.ticked_at = now;

    // At most 60 frames a second; none in a heavy hit's hit-stop.
    if now - p.drawn_at < FEEL.frame_ms || now < p.look.fx.stop_until {
        return;
    }
    p.drawn_at = now;
    draw(p, now);
    if let Some(avg) = &mut p.perf {
        let ms = kit::now() - now;
        *avg = *avg * 0.95 + ms * 0.05;
        let line = format!("{:.2} ms", *avg);
        p.screen
            .px
            .text_shadowed(4, p.screen.px.h - 12, &line, 1, palette::INK);
        kit::document().set_title(&line);
    }
    p.screen.present();
}

fn draw(p: &mut Page, now: f64) {
    let u = p.screen.ui();
    let alive = p.st.joined && p.st.descent == 0 && p.st.pred.ready;
    // Not yet playing: the Sanctum, as the hub's preview shows it.
    let (cx, cy) = if p.st.joined {
        p.st.drawn_self()
    } else {
        (0.0, 3.0)
    };
    let view = View {
        cx,
        cy,
        sight: p.st.laws.sight as f32,
        u,
    };
    let bodies = p.st.bodies(now);
    // Flights leave streaks; skids leave marks.
    for b in &bodies {
        if let Some(&(x0, y0)) = p.was.get(&b.id) {
            let skidding = b.you && !p.input.rec.pressing() && b.moving && b.mv() == 0;
            if b.mv() == 3 || skidding {
                p.look.fx.skid(x0, y0, b.x, b.y, now);
            }
        }
    }
    p.was = bodies.iter().map(|b| (b.id, (b.x, b.y))).collect();
    let c = &mut p.screen.px;
    lucilook::world(c, &mut p.look, &view, &p.st.mirror.tiles, &bodies, now);
    if p.light {
        // A quarter-resolution light map, multiplied over the picture.
        let (lw, lh) = ((c.w + 3) / 4, (c.h + 3) / 4);
        let map: Vec<[u8; 3]> = (0..lw * lh)
            .map(|i| {
                let (x, y) = ((i % lw - lw / 2) as f32, (i / lw - lh / 2) as f32);
                let k = (1.0 - (x * x + y * y).sqrt() / 40.0).clamp(0.12, 1.0);
                [(k * 255.0) as u8, (k * 235.0) as u8, (k * 200.0) as u8]
            })
            .collect();
        c.light(&map, lw, lh, 4);
    }
    let scale = p.screen.scale;

    if alive {
        let me = &p.st.pred.me;
        let (sx, sy) = view.to_screen(c, cx, cy);
        let own = p.st.mirror.own.unwrap_or_default();
        let l = &p.st.laws;
        hud::vitals(
            c,
            sx,
            sy + 2.0,
            own.flame as f32 / l.flame as f32,
            me.body.breath as f32 / l.breath as f32,
        );
        if me.act.act == Act::Charge {
            hud::charge(c, sx, sy, me.act.c, l);
            if let Some((h, len)) = p.input.rec.aim() {
                if len >= FEEL.throw_px {
                    let shown = (len.min(FEEL.throw_far_px) / scale) as f32;
                    hud::arrow(c, sx, sy, h, shown, own.glim >= l.throw_glim);
                }
            }
        }
        // The floating stick.
        if let Some((ox, oy)) = p.input.rec.origin() {
            let (bx, by) = p.screen.to_px(ox, oy);
            let c = &mut p.screen.px;
            c.ring(
                bx,
                by,
                (FEEL.run_px / scale) as f32,
                1.0,
                Rgba(255, 255, 255, 50),
            );
            if let Some((h, d)) = p.input.rec.stick() {
                let (ux, uy) = engine::fixed::unit(h);
                let k = (d.min(FEEL.trail_px) / scale) as f32;
                c.circle(
                    bx + ux.to_f32() * k,
                    by + uy.to_f32() * k,
                    4.0,
                    Rgba(255, 255, 255, 110),
                );
            }
        }
    }
    let c = &mut p.screen.px;
    // The Luciphon, off screen: a marker toward it.
    let (lx, ly) = view.to_screen(c, 0.5, 0.5);
    hud::marker(c, lx, ly, palette::GOLD);
    // The Underlight while you are gone.
    if p.st.descent > 0 {
        if !p.look.under.active {
            p.look.under.begin(c, now);
        }
        let path: Vec<(f32, f32)> =
            p.st.path
                .iter()
                .map(|&(x, y)| view.to_screen(c, x, y))
                .collect();
        let killer =
            p.st.mirror
                .ents
                .get(&p.st.killer)
                .map(|e| view.to_screen(c, e.x as f32 / 256.0, e.y as f32 / 256.0));
        p.look.under.draw(c, &path, killer, now, u);
    } else if p.look.under.active {
        p.look.under.end();
        // Rebirth is the update: a newer page loads now, at the return.
        if p.version.newer() {
            kit::version::reload();
        }
    }
    if p.st.joined {
        let (hx, hy, _) = heart(p);
        let (bx, by) = p.screen.to_px(hx, hy);
        let r = (FEEL.heart_px / 2.0 / p.screen.scale) as f32;
        let c = &mut p.screen.px;
        hud::heart(
            c,
            Rect::new(bx - r, by - r, 2.0 * r, 2.0 * r),
            p.heart_down,
            now,
        );
        let own = p.st.mirror.own.unwrap_or_default();
        let glim = format!("{} glim", own.glim);
        c.text_shadowed(
            (bx + r) as i32 + 8,
            by as i32 - 4 * u,
            &glim,
            u,
            palette::GOLD,
        );
        let top = format!("{} here, {} awake", p.st.people, p.st.awake);
        c.text_shadowed(6 * u, 6 * u, &top, u, palette::DIM);
        let taught: u8 = kit::load(TAUGHT).and_then(|t| t.parse().ok()).unwrap_or(0);
        first::hint(c, taught | p.input.done, u, now);
    }
    // The Stillness: the world holds; the picture stays, under gold.
    if p.st.joined && !p.st.connected {
        let c = &mut p.screen.px;
        let k = (((now / 1200.0).sin() + 1.0) * 30.0) as u8;
        c.fill_rect(0, 0, c.w, c.h, Rgba(40, 30, 10, 90 + k));
        let line = "the world holds still";
        let s = pixels::fit_scale(line, c.w - 20, 2 * u);
        c.text_centred(c.w / 2, c.h / 3, line, s, palette::GOLD);
    }
    if !p.st.joined {
        let name = p.name.value();
        let restore = p.words.value();
        let words = p.session.words();
        let look = first::Look {
            u,
            panel: p.panel,
            name: &name,
            editing: p.name.focused(),
            taken: p.taken,
            words: &words,
            restore: &restore,
            restore_bad: p.restore_bad,
            connected: p.st.connected,
            people: p.st.people,
            awake: p.st.awake,
            touch: p.touch,
        };
        p.spots = first::draw(&mut p.screen.px, &look, now);
        let place = |r: Rect| Some(p.screen.to_css(r.x, r.y, r.w, r.h));
        p.name.place(if p.panel == Panel::Title {
            place(p.spots.name)
        } else {
            None
        });
        p.words.place(if p.panel == Panel::Restore {
            place(p.spots.field)
        } else {
            None
        });
    } else {
        p.name.place(None);
        p.words.place(None);
    }
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let mut screen = kit::Screen::new("screen");
    screen.fit_view(352.0);
    let canvas = kit::document()
        .get_element_by_id("screen")
        .ok_or("no #screen")?;
    let session = kit::Session::load();
    let name = kit::TextField::new(engine::who::MAX_NAME as u32, "your name");
    name.set_value(&session.name());
    let words = kit::TextField::new(120, "eleven words");
    let link = kit::Link::open("luciphon", session.hello(&session.name(), false), false);
    let mut input = Input::default();
    if query("trace=1") {
        input.trace = Some(Vec::new());
    }
    input.done = kit::load(TAUGHT).and_then(|t| t.parse().ok()).unwrap_or(0);
    PAGE.with(|p| {
        *p.borrow_mut() = Some(Page {
            screen,
            look: Look::default(),
            st: State::default(),
            input,
            link,
            session,
            version: kit::Version::watch(VERSION_EVERY),
            pointer: kit::Pointer::attach(&canvas),
            name,
            words,
            panel: Panel::Title,
            spots: first::Spots::default(),
            seen: None,
            taken: false,
            renaming: false,
            joining: false,
            restore_bad: false,
            touch: kit::touch(),
            drawn_at: 0.0,
            ticked_at: 0.0,
            ping_at: 0.0,
            perf: query("perf=1").then_some(0.0),
            light: query("light=1"),
            was: std::collections::HashMap::new(),
            heart_down: false,
        })
    });
    kit::frames(|now| {
        with(|p| frame(p, now));
    });
    kit::on(&kit::window(), "resize", |_| {
        with(|p| p.screen.fit_view(352.0));
    });
    kit::on(&kit::window(), "keydown", |e| {
        if let Ok(e) = e.dyn_into::<web_sys::KeyboardEvent>() {
            with(|p| {
                if e.key() == "Enter" && !p.st.joined && p.panel == Panel::Title {
                    play(p);
                }
                // ?trace=1: T saves the pointer trace.
                if e.key() == "t" && p.st.joined {
                    if let Some(t) = &p.input.trace {
                        kit::download("luciphon-trace.txt", t.join("\n").as_bytes());
                    }
                }
            });
        }
    });
    Ok(())
}
