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
use crate::state::State;
use crate::touch::Touch;

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

/// Where the page is: the title, a match online, or its own practice range.
enum Mode {
    Title,
    Online(kit::Link),
    Practice(Box<Wandfall>),
}

struct Island {
    map: Map,
    look: Look,
    mini: pixels::Canvas,
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
    /// Spells asked for since the last input (a bit a slot).
    asked: u8,
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
    /// The spellbook is open, at this slot; the pause menu is (touch).
    book: bool,
    /// The mouse was locked last frame; mouse moves to pass over (the
    /// first after a lock can carry the whole way the cursor jumped).
    was_locked: bool,
    skip: u8,
    book_slot: usize,
    /// The menu every game shares (Esc): back to the game, this game's
    /// own entries (`items`), feedback, leaving.
    meta: kit::meta::Meta,
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
    /// The range's first lessons, and where their panel is (to tap).
    lessons: Lessons,
    lesson_panel: Option<pixels::Rect>,
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
    let mini = hud::island(&map);
    p.island = Some(Island { map, look, mini });
}

/// The picture redrawn at `q`: a new renderer, the island built again.
fn repaint(p: &mut Page, q: render::Quality) {
    p.r = Renderer::new(&p.g.device, &p.g.queue, p.g.format(), q);
    p.island = None;
    if let Some(seed) = p.st.seed.take() {
        island(p, seed);
    }
}

/// Start over in a new place: nothing known, nobody you.
fn fresh(p: &mut Page) {
    let seed = p.st.seed;
    p.st = State::default();
    p.st.seed = seed;
    p.alive = false;
    p.book = false;
    p.meta.hide();
    p.inbox.clear();
    p.pred.reset(Body::default());
}

fn leave(p: &mut Page) {
    if let Mode::Online(link) = &p.mode {
        link.close();
    }
    p.mode = Mode::Title;
    fresh(p);
    island(p, PRACTICE_SEED);
    kit::input::unlock();
}

fn online(p: &mut Page) {
    leave(p);
    let link = kit::Link::open("wandfall", p.session.hello(&p.session.name(), false), false);
    p.mode = Mode::Online(link);
    if !p.touch {
        grab(p);
    }
}

/// Take the screen, keys and mouse to play (`?windowed`: the mouse only).
fn grab(p: &Page) {
    kit::input::play(p.g.canvas(), !query("windowed"));
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
    match p.mode {
        Mode::Title => vec![("settings", Act::Settings)],
        Mode::Practice(_) => vec![
            ("spellbook (B)", Act::Book),
            ("lessons", Act::Lessons),
            ("settings", Act::Settings),
            ("leave the range", Act::Leave),
        ],
        Mode::Online(_) => vec![("settings", Act::Settings), ("leave the match", Act::Leave)],
    }
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
            if let Some(&(_, a)) = items(p).get(i) {
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

#[wasm_bindgen(start)]
pub fn start() {
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
                lesson_panel: None,
                gallery: query_value("fx")
                    .map(|n| (n, query_value("age").and_then(|v| v.parse().ok()))),
                sky: Sky::default(),
                hour: query_value("hour").and_then(|h| Hour::named(&h)),
                weather: query_value("weather").and_then(|w| Weather::named(&w)),
                thunder: (0.0, None),
                anims: HashMap::new(),
                orbit: query_value("orbit").and_then(|v| v.parse().ok()),
                orbit_at: None,
                set,
                watch: None,
                cycle: 0,
                ear: ([0.0; 3], 0.0),
            })
        });
        PAGE.with(|p| {
            if let Some(p) = p.borrow_mut().as_mut() {
                p.sounds.audio.set_volume(p.set.volume);
                island(p, PRACTICE_SEED);
                // `?practice` goes straight to the range.
                if query("practice") {
                    practise(p);
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
