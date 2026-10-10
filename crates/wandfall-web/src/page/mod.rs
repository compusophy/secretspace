//! The page itself: start, the frame loop, the network, the hands.

use std::cell::RefCell;
use std::collections::HashMap;

use engine::room::{Outbox, Room};
use engine::who::{Seen as Named, Status};
use kit::input::{Hand, Hands};
use kit::link::Net;
use render::{Camera, Frame, Renderer};
use wandfall::laws::{
    spell, BOLT_COOLDOWN, LANCE_RANGE, LIGHTNING_RANGE, RANGE_HOUR, SEA, TICK_HZ,
};
use wandfall::map::Map;
use wandfall::motion::{cast, keys, Body, Input};
use wandfall::predict::Predict;
use wandfall::proto::{self, flag, Up, PROTO};
use wandfall::room::Wandfall;
use wandfall::trig;
use wasm_bindgen::prelude::*;

use crate::camera::{self, Chase};
use crate::fx::{self, Draw};
use crate::hud;
use crate::lessons::{self, Lessons};
use crate::look::Look;
use crate::menu::{self, Act, Spots};
use crate::rig;
use crate::scene;
use crate::settings::Settings;
use crate::sky::{self, Hour, Sky, Weather};
use crate::sound::Sounds;
use crate::state::{Line, State};
use crate::touch::{self, Touch};

mod input;
mod net;
mod range;
mod view;
use input::{hands, inputs, pace};
use net::net;
use range::{act, practise};
use view::frame;

const MIN_SHORT: f64 = 352.0;
const MAX_DPR: f64 = 1.5;
const MS_A_TICK: f64 = 1000.0 / TICK_HZ as f64;
/// Radians a pixel of mouse.
const MOUSE: f32 = 0.0022;
const VERSION_EVERY: f64 = 5.0 * 60_000.0;
/// The practice range's island (and the title's).
const PRACTICE_SEED: u64 = 0x5eed_0007;
/// The page's own connection to its practice world.
const ME: u32 = 1;
/// Kept across a reload: the mark it left (`reload::mark`: so the same
/// page does not reload for the same thing in a loop), and where it was
/// (back online after it).
const RELOADED: &str = "wandfall.reloaded";
const RESUME: &str = "wandfall.resume";

/// Where the page is: the title, a match online, or its own practice range.
enum Mode {
    Title,
    Online(kit::Link),
    Practice(Box<Wandfall>),
}

struct Island {
    map: Map,
    look: Look,
    mini: hud::Mini,
}

struct Page {
    g: gpu::Gpu,
    r: Renderer,
    island: Option<Island>,
    st: State,
    pred: Predict,
    /// The body one input ago (to draw between inputs).
    prev: Body,
    alive: bool,
    acc: f64,
    seq: u16,
    outbox: Vec<Input>,
    yaw: f32,
    pitch: f32,
    firing: bool,
    aiming: bool,
    /// The camera over your shoulder; where you aim (yaw, pitch, radians:
    /// from your eyes to what the crosshair is on), sent each tick.
    chase: Chase,
    aim: (f32, f32),
    /// Spells asked for since the last input (a bit a slot), and when
    /// each slot's cast was last heard.
    asked: u8,
    cast_heard: [f64; 4],
    pad: Touch,
    /// Inputs until your wand may fire again.
    cool: u32,
    hands: Hands,
    mode: Mode,
    /// Messages from the practice world, to read like the server's.
    inbox: Vec<Vec<u8>>,
    /// Time not yet ticked in the practice world (ms).
    local: f64,
    spots: Spots,
    /// The spellbook is open (at `book_slot`).
    book: bool,
    /// The mouse was locked last frame; mouse moves to pass over (the
    /// first after a lock can carry the whole way the cursor jumped).
    was_locked: bool,
    skip: u8,
    book_slot: usize,
    /// The menu every game shares (Esc): back to the game, this game's
    /// own entries (`items`, as last drawn: a pick is one of those, even
    /// if the list has changed since), feedback, leaving.
    meta: kit::meta::Meta,
    shown: Vec<(&'static str, Act)>,
    session: kit::Session,
    version: kit::Version,
    last: f64,
    fps: f64,
    /// Watching the frame time to step the quality down: (frames counted,
    /// ms they took), and whether a tier was asked for (then it stays).
    pace: (u32, f64),
    fixed: bool,
    perf: bool,
    touch: bool,
    sounds: Sounds,
    /// `?hold=ms`: every effect held at that age.
    hold: Option<f64>,
    /// The range's first lessons, and where the chip that skips one is
    /// (to tap).
    lessons: Lessons,
    lesson_skip: Option<pixels::Rect>,
    /// `?fx=name` (`fx::NAMES`): that effect shown by you, again and
    /// again, or (`&age=ms`) held at that age.
    gallery: Option<(String, Option<f64>)>,
    /// The sky, turning with the island's day; `?hour=` holds it at one.
    sky: Sky,
    hour: Option<Hour>,
    weather: Option<Weather>,
    /// The last lightning far off (s), and its thunder still to come
    /// (when, from which side).
    thunder: (f64, Option<(f64, f32)>),
    /// How each wizard has been moving (for its stride).
    anims: HashMap<u16, rig::Anim>,
    /// When each wizard was last in sight (its name shows till a moment
    /// after).
    sighted: HashMap<u16, f64>,
    /// `?orbit=ID`: the camera turns about that wizard (0: you).
    orbit: Option<u16>,
    orbit_at: Option<[f32; 3]>,
    /// What the player set (kept between visits).
    set: Settings,
    /// Out: who you watch, and a step to the next (or the last) asked.
    watch: Option<u16>,
    cycle: i32,
    /// Where sound is heard from (the camera) and which way it faces.
    ear: ([f32; 3], f32),
    /// Your name as you write it on the title (a field laid over its
    /// box), and whether the room said the one asked for is taken.
    name: kit::TextField,
    taken: bool,
    /// Since when the link to the room has been lost, if it is.
    lost: Option<f64>,
    /// A word for the title in place of how to play (why you are back).
    notice: Option<&'static str>,
    /// A newer page is out, but this one came of reloading for it a
    /// moment ago (it was not here yet): no try again before this (ms).
    next_try: f64,
}

thread_local! {
    static PAGE: RefCell<Option<Page>> = const { RefCell::new(None) };
}

fn query(k: &str) -> bool {
    kit::window()
        .location()
        .search()
        .is_ok_and(|q| q.trim_start_matches('?').split('&').any(|kv| kv == k))
}

/// The value of `k=...` in the page's address, if there.
fn query_value(k: &str) -> Option<String> {
    let q = kit::window().location().search().ok()?;
    q.trim_start_matches('?')
        .split('&')
        .find_map(|kv| kv.strip_prefix(k)?.strip_prefix('=').map(str::to_string))
}

fn say(text: &str) {
    if let Some(b) = kit::document().body() {
        b.set_inner_html(&format!(
            "<p style=\"color:#f4eede;font:16px sans-serif;padding:24px;max-width:560px\">{text}</p>"
        ));
    }
}

fn send(p: &mut Page, up: &Up) {
    let bytes = up.encode();
    match &mut p.mode {
        Mode::Online(link) => link.send(&bytes),
        Mode::Practice(room) => {
            let mut out = Outbox::default();
            room.message(ME, &bytes, &mut out);
            p.inbox.extend(out.0.into_iter().map(|m| m.1));
        }
        Mode::Title => {}
    }
}

/// The island of this seed, built if it is not the one already here.
fn island(p: &mut Page, seed: u64) {
    if p.st.seed == Some(seed) && p.island.is_some() {
        return;
    }
    p.st.seed = Some(seed);
    if let Some(old) = p.island.take() {
        old.look.free(&mut p.r);
    }
    let map = Map::new(seed);
    let look = Look::new(&mut p.r, &map);
    p.island = Some(Island {
        map,
        look,
        mini: hud::Mini::default(),
    });
}

/// The picture redrawn at `q` (the island kept: nothing built again).
fn repaint(p: &mut Page, q: render::Quality) {
    p.r.set_quality(q);
}

/// Start over in a new place: nothing known, nobody you.
fn fresh(p: &mut Page) {
    forget(p);
    p.meta.hide();
    p.inbox.clear();
    p.lost = None;
}

/// Nothing of you or the match kept, only the island: somewhere new, or
/// the room found again after the link was lost (it let your wizard go,
/// and a new one joins).
fn forget(p: &mut Page) {
    let seed = p.st.seed;
    p.st = State::default();
    p.st.seed = seed;
    p.alive = false;
    p.book = false;
    p.pred.reset(Body::default());
    p.prev = p.pred.body;
    p.outbox.clear();
    p.pad.reset();
    p.sighted.clear();
}

/// Off the title for somewhere (the title puts your name's field back).
fn leave(p: &mut Page) {
    if let Mode::Online(link) = &p.mode {
        link.close();
    }
    p.name.blur();
    p.name.place(None);
    p.mode = Mode::Title;
    fresh(p);
    island(p, PRACTICE_SEED);
    kit::input::unlock();
}

/// The name written on the title, cleaned as the room would (empty if
/// none): else the one last used here.
fn wanted(p: &Page) -> String {
    let typed = engine::who::clean_name(&p.name.value());
    if typed.is_empty() {
        p.session.name()
    } else {
        typed
    }
}

fn online(p: &mut Page) {
    leave(p);
    // A name written that is not this soul's yet: asked for (the room
    // says whether it is someone else's).
    let name = wanted(p);
    let rename = !name.is_empty() && name != p.session.name();
    p.taken = false;
    p.notice = None;
    let link = kit::Link::open("wandfall", p.session.hello(&name, rename), false);
    p.mode = Mode::Online(link);
    grab(p);
}

/// Load the page again for `what` (`reload::BUILD`, `reload::proto`: a
/// newer build or protocol is out), unless this very page just did for
/// the same: then the newer one is not here yet, and another reload now
/// would only loop. Whether it reloads.
fn reload(what: &str) -> bool {
    let (page, now) = (
        kit::version::PAGE,
        wasm_bindgen_futures::js_sys::Date::now(),
    );
    if !crate::reload::may(kit::load(RELOADED).as_deref(), page, what, now) {
        return false;
    }
    kit::save(RELOADED, &crate::reload::mark(page, what, now));
    kit::version::reload();
    true
}

/// Take the screen, keys and mouse to play (`?windowed`: the mouse only);
/// on a phone, the whole screen, held sideways (`sideways`).
fn grab(p: &Page) {
    if p.touch {
        if !query("windowed") {
            sideways();
        }
    } else {
        kit::input::play(p.g.canvas(), !query("windowed"));
    }
}

/// A phone: the whole screen (the hub's frame has it already) and the
/// phone turned sideways, as the buttons are laid out for. It must come
/// from a press; a browser that will not (iOS) is left as it is.
fn sideways() {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::js_sys::{Function, Promise, Reflect};
    use wasm_bindgen_futures::JsFuture;
    // `obj.name(arg)`, if it is there, as the promise it gives.
    let call = |obj: &JsValue, name: &str, arg: Option<&JsValue>| -> Option<Promise> {
        let f: Function = Reflect::get(obj, &name.into()).ok()?.dyn_into().ok()?;
        let r = match arg {
            Some(a) => f.call1(obj, a),
            None => f.call0(obj),
        };
        r.ok()?.dyn_into::<Promise>().ok()
    };
    let full = if kit::shell::framed() || kit::input::full() {
        None
    } else {
        let root = kit::document().document_element().map(JsValue::from);
        root.and_then(|r| call(&r, "requestFullscreen", None))
    };
    // The window holding the screen: the hub's, in its frame.
    let win = kit::window();
    let outer = match win.parent() {
        Ok(Some(parent)) if kit::shell::framed() => parent,
        _ => win,
    };
    let screen = Reflect::get(&outer.into(), &"screen".into());
    let turn = screen.and_then(|s| Reflect::get(&s, &"orientation".into()));
    wasm_bindgen_futures::spawn_local(async move {
        if let Some(p) = full {
            let _ = JsFuture::from(p).await;
        }
        if let Some(p) = turn
            .ok()
            .and_then(|o| call(&o, "lock", Some(&"landscape".into())))
        {
            let _ = JsFuture::from(p).await;
        }
    });
}

/// Whether a phone is held upright (the game wants it sideways).
fn upright(p: &Page) -> bool {
    p.touch && p.g.css.1 > p.g.css.0
}

/// Whether a menu is up (the title, the spellbook, the shared menu, or
/// the mouse let go).
fn in_menu(p: &Page) -> bool {
    matches!(p.mode, Mode::Title)
        || p.book
        || p.meta.is_open()
        || (!p.touch && !kit::input::locked())
}

/// This game's own entries in the shared menu, as it stands.
fn items(p: &Page) -> Vec<(&'static str, Act)> {
    // The spellbook (a phone has no B to press); out, there is no book of
    // yours to open.
    let book = p.alive.then_some((
        if p.touch {
            "spellbook"
        } else {
            "spellbook (B)"
        },
        Act::Book,
    ));
    let rest: &[(&'static str, Act)] = match p.mode {
        Mode::Title => return vec![("settings", Act::Settings)],
        Mode::Practice(_) => &[
            ("lessons", Act::Lessons),
            ("settings", Act::Settings),
            ("leave the range", Act::Leave),
        ],
        Mode::Online(_) => &[("settings", Act::Settings), ("leave the match", Act::Leave)],
    };
    book.into_iter().chain(rest.iter().copied()).collect()
}

/// The shared menu up (the mouse let go), knowing what a report should.
fn open_menu(p: &mut Page) {
    kit::input::unlock();
    p.meta.context = context(p);
    p.meta.show();
}

/// Back to the game from a menu: the mouse taken again (not on the title,
/// not on a phone).
fn resume(p: &mut Page) {
    p.meta.hide();
    if !p.touch && !matches!(p.mode, Mode::Title) {
        grab(p);
    }
}

/// What the shared menu's pick does.
fn picked(p: &mut Page, pick: kit::meta::Pick) {
    match pick {
        kit::meta::Pick::Resume => resume(p),
        kit::meta::Pick::Game(i) => {
            if let Some(&(_, a)) = p.shown.get(i) {
                act(p, a);
            }
        }
        kit::meta::Pick::Exit => {
            if let Mode::Online(link) = &p.mode {
                link.close();
            }
            kit::shell::exit();
        }
    }
}

/// What a report says of the game: where you are and how it draws.
fn context(p: &Page) -> String {
    let mode = match p.mode {
        Mode::Title => "title",
        Mode::Online(_) => "online",
        Mode::Practice(_) => "range",
    };
    let f = p.st.frame.as_ref();
    format!(
        "game: wandfall\nmode: {mode}\nphase: {}\nalive: {}\nplayers: {}\nfps: {:.0}\nquality: {:?}\npicture: {}\nadapter: {}\nsoftware: {}\n",
        f.map_or(-1, |f| f.phase as i32),
        p.alive,
        f.map_or(0, |f| f.players.len()),
        p.fps,
        p.r.quality(),
        p.set.picture,
        p.g.caps.adapter,
        p.g.caps.software,
    )
}

// The page's own start, called by its index.html after init(): never on
// init, so a host importing this module as a cartridge is left alone
// (the start-guard rule, which scripts/caps.sh holds).
#[wasm_bindgen]
pub fn page() {
    wasm_bindgen_futures::spawn_local(async {
        kit::report::on_panic();
        // Who cannot play, and why, is worth knowing too.
        if !gpu::offered() {
            kit::report::send("no webgpu", "game: wandfall\n", "");
            say("Wandfall is drawn with WebGPU, which this browser does not offer yet. It runs in Chrome or Edge (desktop and Android), and Safari 26.");
            return;
        }
        let g = match gpu::Gpu::new("screen", MIN_SHORT, MAX_DPR).await {
            Ok(g) => g,
            Err(e) => {
                kit::report::send("no webgpu", "game: wandfall\n", &e.to_string());
                say(&format!("Wandfall could not start WebGPU here ({e}). Try Chrome or Edge with hardware acceleration on."));
                return;
            }
        };
        let search = kit::window().location().search().unwrap_or_default();
        let set = Settings::load();
        let q = set.quality(&search, g.caps.software, kit::touch());
        let r = Renderer::new(&g.device, &g.queue, g.format(), q);
        let session = kit::Session::load();
        let hands = Hands::attach(g.canvas());
        PAGE.with(|p| {
            *p.borrow_mut() = Some(Page {
                g,
                r,
                island: None,
                st: State::default(),
                pred: Predict::default(),
                prev: Body::default(),
                alive: false,
                acc: 0.0,
                seq: 0,
                outbox: Vec::new(),
                yaw: 0.0,
                pitch: 0.0,
                firing: false,
                aiming: false,
                chase: Chase::default(),
                aim: (0.0, 0.0),
                asked: 0,
                cast_heard: [f64::NEG_INFINITY; 4],
                pad: Touch::default(),
                cool: 0,
                hands,
                mode: Mode::Title,
                inbox: Vec::new(),
                local: 0.0,
                spots: Spots::default(),
                book: false,
                was_locked: false,
                skip: 0,
                book_slot: 0,
                meta: kit::meta::Meta::new("wandfall"),
                shown: Vec::new(),
                session,
                version: kit::Version::watch(VERSION_EVERY),
                last: kit::now(),
                fps: 60.0,
                pace: (0, 0.0),
                fixed: query("q=low") || query("q=medium") || query("q=high"),
                perf: query("perf=1"),
                touch: kit::touch(),
                sounds: Sounds::new(),
                hold: query_value("hold").and_then(|v| v.parse().ok()),
                lessons: Lessons::new(),
                lesson_skip: None,
                gallery: query_value("fx")
                    .map(|n| (n, query_value("age").and_then(|v| v.parse().ok()))),
                sky: Sky::default(),
                hour: query_value("hour").and_then(|h| Hour::named(&h)),
                weather: query_value("weather").and_then(|w| Weather::named(&w)),
                thunder: (0.0, None),
                anims: HashMap::new(),
                sighted: HashMap::new(),
                orbit: query_value("orbit").and_then(|v| v.parse().ok()),
                orbit_at: None,
                set,
                watch: None,
                cycle: 0,
                ear: ([0.0; 3], 0.0),
                name: kit::TextField::new(engine::who::MAX_NAME as u32, "your name"),
                taken: false,
                lost: None,
                notice: None,
                next_try: 0.0,
            })
        });
        PAGE.with(|p| {
            if let Some(p) = p.borrow_mut().as_mut() {
                p.sounds.audio.set_volume(p.set.volume);
                p.name.set_value(&p.session.name());
                island(p, PRACTICE_SEED);
                // `?practice` goes straight to the range; a page reloaded
                // for a newer build back to the match it was in.
                let resume = kit::load(RESUME).is_some_and(|v| v == "online");
                if resume {
                    kit::save(RESUME, "");
                }
                if query("practice") {
                    practise(p);
                } else if resume {
                    online(p);
                }
            }
        });
        kit::frames(|now| {
            PAGE.with(|p| {
                if let Some(p) = p.borrow_mut().as_mut() {
                    frame(p, now);
                }
            })
        });
        kit::on(&kit::window(), "resize", |_| {
            PAGE.with(|p| {
                if let Some(p) = p.borrow_mut().as_mut() {
                    p.g.fit(MIN_SHORT, MAX_DPR);
                }
            })
        });
    });
}
