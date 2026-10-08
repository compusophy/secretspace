//! The page itself: start, the frame loop, the network, the hands.

use std::cell::RefCell;

use engine::room::{Outbox, Room, Who};
use engine::who::{Seen as Named, Status};
use kit::input::{Hand, Hands};
use kit::link::Net;
use render::{Camera, Frame, Renderer};
use wandfall::laws::{BOLT_COOLDOWN, EYE, TICK_HZ};
use wandfall::map::Map;
use wandfall::motion::{cast, keys, Body, Input};
use wandfall::practice;
use wandfall::predict::Predict;
use wandfall::proto::{self, flag, Up, PROTO};
use wandfall::room::Wandfall;
use wandfall::trig;
use wasm_bindgen::prelude::*;

use crate::fx::{self, Draw};
use crate::hud;
use crate::look::{self, Look};
use crate::menu::{self, Act, Spots};
use crate::sound::Sounds;
use crate::state::State;
use crate::touch::Touch;

const MIN_SHORT: f64 = 352.0;
const MAX_DPR: f64 = 1.5;
const MS_A_TICK: f64 = 1000.0 / TICK_HZ as f64;
/// Radians a pixel of mouse.
const MOUSE: f32 = 0.0022;
const FOV: f32 = 1.2;
/// The field of view aiming down the wand.
const AIM_FOV: f32 = 0.72;
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
    /// The field of view now (it eases toward aiming's and back).
    fov: f32,
    /// Spells asked for since the last input (a bit a slot).
    asked: u8,
    pad: Touch,
    /// When you last cast (ms), and inputs until you may again; when you
    /// last cast a spell, and which.
    cast_at: f64,
    spell_at: (f64, u8),
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
    book_slot: usize,
    paused: bool,
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

fn net(p: &mut Page, now: f64, dt: f64) {
    let mut got = std::mem::take(&mut p.inbox);
    let mut up = false;
    match &mut p.mode {
        Mode::Online(link) => {
            for ev in link.poll(now) {
                match ev {
                    Net::Up => up = true,
                    Net::Holding => {}
                    Net::Message(b) => got.push(b),
                }
            }
        }
        Mode::Practice(room) => {
            p.local = (p.local + dt).min(MS_A_TICK * 6.0);
            while p.local >= MS_A_TICK {
                p.local -= MS_A_TICK;
                let mut out = Outbox::default();
                room.tick(&mut out);
                got.extend(out.0.into_iter().map(|m| m.1));
            }
        }
        Mode::Title => {}
    }
    if up {
        send(p, &Up::Join { proto: PROTO });
    }
    for b in got {
        receive(p, &b, now);
    }
}

fn receive(p: &mut Page, b: &[u8], now: f64) {
    if let Some(seen) = Named::decode(b) {
        if seen.status != Status::Taken && !seen.name.is_empty() {
            p.session.set_name(&seen.name);
            if let Mode::Online(link) = &p.mode {
                link.set_hello(p.session.hello(&seen.name, false));
            }
        }
        return;
    }
    if let Some((v, you, seed, _)) = proto::read_welcome(b) {
        if v != PROTO {
            kit::version::reload();
            return;
        }
        p.st.you = you;
        p.st.joined = you != 0;
        island(p, seed);
        return;
    }
    if let Some(list) = proto::read_roster(b) {
        p.st.names = list
            .into_iter()
            .map(|(id, bot, n)| (id, (n, bot)))
            .collect();
        return;
    }
    if let Some(list) = proto::read_events(b) {
        p.sounds.events(&list, p.st.you, p.alive, p.ear);
        p.st.events(list, now);
        return;
    }
    if let Some(l) = proto::Loot::decode(b) {
        p.sounds.loot(&p.st.loot, &l, p.ear);
        p.st.loot = l;
        return;
    }
    if let Some(f) = proto::Frame::decode(b) {
        p.sounds.frame(p.st.frame.as_ref(), &f, p.st.you, p.ear);
        match (&f.you, &p.island) {
            (Some(own), Some(i)) => {
                if !p.alive {
                    p.pred.reset(own.body);
                    p.prev = own.body;
                    p.alive = true;
                } else {
                    p.pred.confirm(own.body, own.seq, &i.map);
                }
            }
            _ => p.alive = false,
        }
        p.st.take(f, now);
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

/// Start over in a new place: nothing known, nobody you.
fn fresh(p: &mut Page) {
    let seed = p.st.seed;
    p.st = State::default();
    p.st.seed = seed;
    p.alive = false;
    p.book = false;
    p.paused = false;
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
        kit::input::lock(p.g.canvas());
    }
}

fn practise(p: &mut Page) {
    leave(p);
    let mut room = Box::new(Wandfall::practice(PRACTICE_SEED));
    let name = Some(p.session.name())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "you".to_string());
    let who = Who {
        name,
        ..Who::default()
    };
    let mut out = Outbox::default();
    room.open(ME, &who, &mut out);
    p.inbox.extend(out.0.into_iter().map(|m| m.1));
    p.mode = Mode::Practice(room);
    send(p, &Up::Join { proto: PROTO });
    // `?spells=0,1,4,5` sets out the four slots; `?nocd` drops cooldowns;
    // `?spar` has the dummies fight back.
    if let Mode::Practice(room) = &mut p.mode {
        let w = room.world();
        let me = w.players.iter().find(|p| !p.bot).map_or(0, |p| p.id);
        if let Some(list) = query_value("spells") {
            if let Some(q) = w.find_mut(me) {
                q.slots = [None; 4];
            }
            for (slot, sp) in list.split(',').filter_map(|v| v.parse().ok()).enumerate() {
                practice::equip(w, me, slot, sp, 1);
            }
        }
        if let Some(r) = w.practice.as_mut() {
            r.no_cooldowns |= query("nocd");
            r.sparring |= query("spar");
        }
    }
    // `?look=yaw,pitch` (degrees) faces you a way to start.
    if let Some(v) = query_value("look") {
        let mut it = v.split(',').filter_map(|x| x.parse::<f32>().ok());
        p.yaw = it.next().unwrap_or(0.0).to_radians();
        p.pitch = it.next().unwrap_or(0.0).to_radians();
    }
    if !p.touch {
        kit::input::lock(p.g.canvas());
    }
}

/// A menu button pressed.
fn act(p: &mut Page, a: Act) {
    let you = p.st.you;
    let own = p.st.frame.as_ref().and_then(|f| f.you);
    match a {
        Act::Online => online(p),
        Act::Practice => practise(p),
        Act::Leave => leave(p),
        Act::Resume => {
            p.paused = false;
            if !p.touch {
                kit::input::lock(p.g.canvas());
            }
        }
        Act::Book => {
            p.book = true;
            p.paused = false;
            kit::input::unlock();
        }
        Act::CloseBook => {
            p.book = false;
            if !p.touch {
                kit::input::lock(p.g.canvas());
            }
        }
        Act::Slot(k) => p.book_slot = k,
        _ => {
            let Mode::Practice(room) = &mut p.mode else {
                return;
            };
            let w = room.world();
            let slot = p.book_slot;
            let held = own.and_then(|o| o.slots[slot]);
            match a {
                Act::Spell(sp) => {
                    practice::equip(w, you, slot, sp, held.map_or(1, |h| h.1));
                }
                Act::Rank(r) => {
                    if let Some((sp, _)) = held {
                        practice::equip(w, you, slot, sp, r);
                    }
                }
                Act::Level(d) => {
                    let level = own.map_or(1, |o| o.level) as i8;
                    let to = if d == 20 {
                        20
                    } else {
                        (level + d).clamp(1, 20)
                    };
                    practice::set_level(w, you, to as u8);
                }
                Act::NoCooldowns => {
                    if let Some(r) = w.practice.as_mut() {
                        r.no_cooldowns = !r.no_cooldowns;
                    }
                }
                Act::Sparring => {
                    if let Some(r) = w.practice.as_mut() {
                        r.sparring = !r.sparring;
                    }
                }
                _ => {}
            }
        }
    }
}

/// Whether a menu is up (the title, the spellbook, or paused).
fn in_menu(p: &Page) -> bool {
    matches!(p.mode, Mode::Title) || p.book || p.paused || (!p.touch && !kit::input::locked())
}

fn hands(p: &mut Page) {
    let locked = kit::input::locked();
    for h in p.hands.drain() {
        // A browser lets sound play once a person acts.
        if matches!(
            h,
            Hand::Button { down: true, .. }
                | Hand::Finger {
                    kind: kit::input::Kind::Down,
                    ..
                }
        ) {
            p.sounds.audio.wake();
        }
        match h {
            Hand::Mouse { dx, dy, .. } if locked => {
                // Slower when zoomed, so the aim holds.
                let k = MOUSE * p.fov / FOV;
                p.yaw += dx as f32 * k;
                p.pitch = (p.pitch - dy as f32 * k).clamp(-1.5, 1.5);
            }
            Hand::Button {
                button: 2, down, ..
            } => p.aiming = down && locked,
            Hand::Finger {
                kind: kit::input::Kind::Down,
                x,
                y,
                ..
            } if p.touch && in_menu(p) => {
                let (lx, ly) = p.g.to_px(x, y);
                if let Some(a) = p.spots.hit(lx, ly) {
                    act(p, a);
                }
            }
            Hand::Finger { id, kind, x, y, .. } if p.touch => {
                let (dy, dp) = p.pad.finger(id, kind, x, y, p.g.css);
                let k = p.fov / FOV;
                p.yaw += dy * k;
                p.pitch = (p.pitch + dp * k).clamp(-1.5, 1.5);
            }
            Hand::Button {
                button: 0,
                down,
                x,
                y,
                ..
            } => {
                if down && !locked && !p.touch {
                    let (lx, ly) = p.g.to_px(x, y);
                    match p.spots.hit(lx, ly) {
                        Some(a) => act(p, a),
                        // Off the buttons, in a game, with no book open: play.
                        None if !matches!(p.mode, Mode::Title) && !p.book => {
                            kit::input::lock(p.g.canvas());
                        }
                        None => {}
                    }
                } else {
                    p.firing = down && locked;
                }
            }
            _ => {}
        }
    }
    if p.touch {
        p.aiming = p.pad.aim;
    } else if !locked {
        p.firing = false;
        p.aiming = false;
    }
    if std::mem::take(&mut p.pad.menu) {
        p.paused = !p.paused;
    }
    for code in p.hands.pressed() {
        p.sounds.audio.wake();
        if code == "KeyM" {
            let on = !p.sounds.audio.muted;
            p.sounds.mute(on);
            continue;
        }
        if code == "KeyB" && matches!(p.mode, Mode::Practice(_)) {
            act(p, if p.book { Act::CloseBook } else { Act::Book });
            continue;
        }
        let slot = match code.as_str() {
            "KeyQ" | "Digit1" => 0,
            "KeyE" | "Digit2" => 1,
            "KeyR" | "Digit3" => 2,
            "KeyF" | "Digit4" => 3,
            _ => continue,
        };
        if locked || p.touch {
            p.asked |= cast::SLOT[slot];
        }
    }
}

/// The inputs due since the last frame, each applied at once (predicted)
/// and sent.
fn inputs(p: &mut Page, dt: f64) {
    let Some(island) = &p.island else {
        return;
    };
    p.acc = (p.acc + dt).min(MS_A_TICK * 6.0);
    if matches!(p.mode, Mode::Title) {
        p.acc = 0.0;
        return;
    }
    let busy = p.book || p.paused;
    let held = |k: &str| p.hands.held(k) && !busy;
    let mut k = 0;
    if held("KeyW") || held("ArrowUp") {
        k |= keys::FWD;
    }
    if held("KeyS") || held("ArrowDown") {
        k |= keys::BACK;
    }
    if held("KeyA") || held("ArrowLeft") {
        k |= keys::LEFT;
    }
    if held("KeyD") || held("ArrowRight") {
        k |= keys::RIGHT;
    }
    if held("Space") {
        k |= keys::JUMP;
    }
    if p.firing {
        k |= keys::FIRE;
    }
    if p.aiming {
        k |= keys::AIM;
    }
    let mut take = if held("KeyG") { cast::TAKE } else { 0 };
    if p.touch && !busy {
        k |= p.pad.keys();
        take |= p.pad.casts();
    }
    while p.acc >= MS_A_TICK {
        p.acc -= MS_A_TICK;
        if !p.alive {
            continue;
        }
        p.seq = p.seq.wrapping_add(1);
        let asked = std::mem::take(&mut p.asked);
        let own = p.st.frame.as_ref().and_then(|f| f.you.as_ref());
        if let Some(sp) = own.and_then(|o| {
            (0..4).find_map(|k| {
                (asked & cast::SLOT[k] != 0 && o.cds[k] == 0)
                    .then_some(o.slots[k])
                    .flatten()
                    .map(|s| s.0)
            })
        }) {
            p.spell_at = (kit::now(), sp);
            p.sounds.cast(sp, None, p.ear);
        }
        let i = Input {
            seq: p.seq,
            yaw: trig::heading(p.yaw),
            pitch: trig::pitch(p.pitch),
            keys: k,
            cast: asked | take,
        };
        p.prev = p.pred.body;
        p.pred.push(i, &island.map);
        p.cool = p.cool.saturating_sub(1);
        if k & keys::FIRE != 0 && p.cool == 0 && !p.pred.body.glide {
            p.cool = BOLT_COOLDOWN;
            p.cast_at = kit::now();
            p.sounds.wand(None, p.ear, (p.seq % 5) as f32 / 5.0);
        }
        p.outbox.push(i);
    }
    let out = std::mem::take(&mut p.outbox);
    for chunk in out.chunks(proto::MAX_INPUTS) {
        send(p, &Up::Inputs(chunk.to_vec()));
    }
}

/// Too slow for this tier (over 2.5 s of play, frames over 30 ms on
/// average): one tier down, the island built again on it.
fn pace(p: &mut Page, dt: f64) {
    if p.fixed || !p.alive || dt <= 0.0 {
        return;
    }
    p.pace.0 += 1;
    p.pace.1 += dt;
    if p.pace.1 < 2500.0 {
        return;
    }
    let slow = p.pace.1 / p.pace.0 as f64 > 30.0;
    p.pace = (0, 0.0);
    let Some(q) = p.r.quality().lower().filter(|_| slow) else {
        return;
    };
    p.r = Renderer::new(&p.g.device, &p.g.queue, p.g.format(), q);
    p.island = None;
    if let Some(seed) = p.st.seed.take() {
        island(p, seed);
    }
}

fn frame(p: &mut Page, now: f64) {
    let dt = (now - p.last).clamp(0.0, 250.0);
    p.last = now;
    if dt > 0.0 {
        p.fps += (1000.0 / dt - p.fps) * 0.05;
    }
    pace(p, dt);
    net(p, now, dt);
    hands(p);
    inputs(p, dt);
    p.version.poll(now, false);
    if p.version.newer() && (!p.alive || p.st.frame.as_ref().is_some_and(|f| f.phase != 1)) {
        kit::version::reload();
    }
    let others = p.st.others(now);
    let want = if p.aiming && p.alive { AIM_FOV } else { FOV };
    p.fov += (want - p.fov) * (1.0 - (-(dt as f32) / 70.0).exp());
    let (w, h) = p.g.css;
    let aspect = (w / h.max(1.0)) as f32;
    // Where you see from: your own eyes, or over someone's shoulder.
    let k = (p.acc / MS_A_TICK) as f32;
    let body = |a: [f32; 3], b: [f32; 3]| {
        [
            a[0] + (b[0] - a[0]) * k,
            a[1] + (b[1] - a[1]) * k,
            a[2] + (b[2] - a[2]) * k,
        ]
    };
    let mut watching = None;
    let cam = if p.alive {
        let at = body(p.prev.p, p.pred.body.p);
        Camera {
            eye: [at[0], at[1] + EYE, at[2]],
            yaw: p.yaw,
            pitch: p.pitch,
            fov: p.fov,
            aspect,
        }
    } else {
        let f = p.st.frame.as_ref();
        let pick = f
            .and_then(|f| others.iter().find(|s| s.id == f.winner && f.winner != 0))
            .or_else(|| others.iter().find(|s| s.flags & flag::ENTRANT != 0))
            .or(others.first());
        match pick {
            Some(s) => {
                watching = Some(p.st.name(s.id));
                let a = trig::radians(s.yaw) + (now / 9000.0) as f32;
                let back = [s.p[0] - a.cos() * 7.0, s.p[1] + 4.0, s.p[2] - a.sin() * 7.0];
                Camera {
                    eye: back,
                    yaw: a,
                    pitch: -0.35,
                    fov: FOV,
                    aspect,
                }
            }
            None => {
                let a = (now / 20000.0) as f32;
                Camera {
                    eye: [a.cos() * 120.0, 45.0, a.sin() * 120.0],
                    yaw: a + std::f32::consts::PI,
                    pitch: -0.25,
                    fov: FOV,
                    aspect,
                }
            }
        }
    };
    let t = (now / 1000.0) as f32;
    let mut d = Draw::default();
    let mut in_storm = false;
    if let Some(i) = &p.island {
        fx::loot(&i.look, &mut d, &p.st.loot, t, cam.eye);
        for s in &others {
            if s.flags & flag::ALIVE == 0 {
                continue;
            }
            if !(p.alive && s.id == p.st.you) {
                // Walking bobs it; casting (or firing) raises its arm.
                let tip = fx::tip(&p.st.shows, s.id, now);
                let fired = p.st.fired.get(&s.id).map_or(1e9, |&f| now - f);
                let speed = (p.st.speed(s.id) / 7.0).min(1.0);
                let ground = s.flags & flag::GROUND != 0;
                let step = ((now / 1000.0) as f32 * 9.0 + s.id as f32).sin().abs();
                let pose = look::Pose {
                    bob: if ground { step * 0.06 * speed } else { 0.0 },
                    arm: (tip.1 * 2.0)
                        .max((1.0 - fired as f32 / 450.0) * 1.5)
                        .min(1.0),
                    aim: s.pitch as f32 / 65536.0 * std::f32::consts::TAU,
                    tip,
                    glide: s.flags & flag::GLIDE != 0,
                };
                i.look
                    .wizard(&mut d, s.id, s.p, trig::radians(s.yaw), &pose);
            }
            fx::on_wizard(&i.look, &mut d, s, t, p.alive && s.id == p.st.you);
        }
        for b in p.st.bolts(now) {
            fx::bolt(&i.look, &mut d, &b, b.by == p.st.you, t);
        }
        for &(at, who, what) in &p.st.bursts {
            if let Some(s) = others.iter().find(|s| s.id == who) {
                let pos = [s.p[0], s.p[1] + 1.2, s.p[2]];
                i.look.burst(
                    &mut d.lights,
                    &mut d.sparks,
                    pos,
                    ((now - at) as f32, at as u32),
                    fx::colour(what),
                );
            }
        }
        // Your wand: kicked by a bolt, flaring in a spell's colour.
        let tip = p.alive.then(|| {
            let kick = (1.0 - (now - p.cast_at) / 180.0).clamp(0.0, 1.0) as f32;
            let flare = (1.0 - (now - p.spell_at.0) / 320.0).clamp(0.0, 1.0) as f32;
            let c = render::geo::mix(look::GOLD, fx::colour(p.spell_at.1), flare.min(1.0).sqrt());
            i.look
                .wand(&mut d.items, &mut d.lights, &cam, kick.max(flare * 1.4), c)
        });
        let me = (p.st.you, p.pred.body.p, p.alive);
        let at_of = |id: u16| {
            if me.2 && id == me.0 {
                return Some(me.1);
            }
            others.iter().find(|s| s.id == id).map(|s| s.p)
        };
        // `?hold=ms` holds every effect at that age (to look at them).
        let held: Vec<_>;
        let shows = match query_value("hold").and_then(|v| v.parse::<f64>().ok()) {
            Some(ms) => {
                held =
                    p.st.shows
                        .iter()
                        .map(|&(w, e)| (w.max(now - ms), e))
                        .collect();
                &held
            }
            None => &p.st.shows,
        };
        fx::shows(&i.look, &mut d, shows, now, (p.st.you, tip), at_of);
        if let Some(f) = &p.st.frame {
            if f.phase == 1 && !matches!(p.mode, Mode::Practice(_)) {
                i.look.storm(&mut d.items, f.storm.0, f.storm.1);
                let e = cam.eye;
                in_storm = p.alive
                    && (e[0] - f.storm.0[0]).powi(2) + (e[2] - f.storm.0[1]).powi(2)
                        > f.storm.1 * f.storm.1;
            }
        }
    }
    p.ear = (cam.eye, cam.yaw);
    let scene = Frame {
        cam,
        look: look::sky(in_storm),
        time: t,
        items: &d.items,
        lights: &d.lights,
        sparks: &d.sparks,
        view_fov: 0.9,
    };
    let perf = p.perf.then(|| {
        let s = p.r.stats;
        format!(
            "{:.0} fps  draws {} inst {} tris {}k lights {}  @{:.0},{:.0}{}",
            p.fps,
            s.draws,
            s.instances,
            s.triangles / 1000,
            s.lights,
            cam.eye[0],
            cam.eye[2],
            if p.touch {
                format!(" {}", p.pad.debug())
            } else {
                String::new()
            }
        )
    });
    let Some(mut fr) = p.g.frame() else {
        return;
    };
    p.r.draw(&mut fr.encoder, &fr.view, p.g.size, &scene);
    let (vp, _, _) = cam.matrices(cam.fov);
    let ui = p.g.ui();
    let me = p.alive.then_some((p.pred.body.p, p.yaw));
    p.g.hud.wipe();
    p.spots = Spots::default();
    let practice = matches!(p.mode, Mode::Practice(_));
    match (&p.island, &p.mode) {
        (Some(_), Mode::Title) => {
            let note = "the island is drawn by the secretspace engine, on WebGPU";
            menu::title(&mut p.g.hud, &mut p.spots, ui, note);
        }
        (Some(i), _) => {
            let view = hud::View {
                st: &p.st,
                frame: p.st.frame.as_ref(),
                others: &others,
                vp,
                me,
                own: p.st.frame.as_ref().and_then(|f| f.you.as_ref()),
                watching,
                locked: kit::input::locked(),
                in_storm,
                now,
                ui,
                perf: perf.clone(),
                touch: p.touch,
                practice,
            };
            hud::draw(&mut p.g.hud, &i.mini, &view);
            let own = p.st.frame.as_ref().and_then(|f| f.you.as_ref());
            if p.touch && p.alive && !p.book && !p.paused {
                let me = p.pred.body.p;
                p.pad.can_take =
                    p.st.loot
                        .scrolls
                        .iter()
                        .any(|s| (s.3[0] - me[0]).powi(2) + (s.3[2] - me[2]).powi(2) < 2.5);
                p.pad.draw(&mut p.g.hud, own, p.g.css, p.g.scale);
            }
            // The online lobby: who is waiting.
            if !practice && p.st.frame.as_ref().is_some_and(|f| f.phase == 0) {
                let names: Vec<String> =
                    p.st.names
                        .values()
                        .filter(|n| !n.1)
                        .map(|n| n.0.clone())
                        .collect();
                menu::lobby(&mut p.g.hud, ui, &names);
            }
            if p.book {
                if let (Some(o), Mode::Practice(room)) = (own, &mut p.mode) {
                    let rules = room
                        .world()
                        .practice
                        .as_ref()
                        .map_or((false, false), |r| (r.no_cooldowns, r.sparring));
                    menu::book(&mut p.g.hud, &mut p.spots, ui, o, p.book_slot, rules);
                }
            } else if p.paused || (!p.touch && !kit::input::locked()) {
                menu::pause(&mut p.g.hud, &mut p.spots, ui, practice, p.touch);
            }
        }
        (None, _) => {
            let c = &mut p.g.hud;
            c.text_centred(
                c.w / 2,
                c.h / 2,
                "finding the island...",
                2 * ui,
                pixels::Rgba::rgb(250, 246, 236),
            );
        }
    }
    p.g.present(fr);
    if let Some(line) = perf {
        if (now as u64 / 500).is_multiple_of(2) {
            kit::document().set_title(&line);
        }
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    wasm_bindgen_futures::spawn_local(async {
        if !gpu::offered() {
            say("Wandfall is drawn with WebGPU, which this browser does not offer yet. It runs in Chrome or Edge (desktop and Android), and Safari 26.");
            return;
        }
        let g = match gpu::Gpu::new("screen", MIN_SHORT, MAX_DPR).await {
            Ok(g) => g,
            Err(e) => {
                say(&format!("Wandfall could not start WebGPU here ({e}). Try Chrome or Edge with hardware acceleration on."));
                return;
            }
        };
        let search = kit::window().location().search().unwrap_or_default();
        let q = render::Quality::pick(&search, g.caps.software, kit::touch());
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
                fov: FOV,
                asked: 0,
                pad: Touch::default(),
                cast_at: -1e9,
                spell_at: (-1e9, 0),
                cool: 0,
                hands,
                mode: Mode::Title,
                inbox: Vec::new(),
                local: 0.0,
                spots: Spots::default(),
                book: false,
                book_slot: 0,
                paused: false,
                session,
                version: kit::Version::watch(VERSION_EVERY),
                last: kit::now(),
                fps: 60.0,
                pace: (0, 0.0),
                fixed: query("q=low") || query("q=medium") || query("q=high"),
                perf: query("perf=1"),
                touch: kit::touch(),
                sounds: Sounds::new(),
                ear: ([0.0; 3], 0.0),
            })
        });
        PAGE.with(|p| {
            if let Some(p) = p.borrow_mut().as_mut() {
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
