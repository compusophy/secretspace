//! The page itself: start, the frame loop, the network, the hands.

use std::cell::RefCell;

use engine::who::{Seen as Named, Status};
use kit::input::{Hand, Hands};
use kit::link::Net;
use render::{Camera, Frame, Renderer};
use wandfall::laws::{BOLT_COOLDOWN, EYE, TICK_HZ};
use wandfall::map::Map;
use wandfall::motion::{cast, keys, Body, Input};
use wandfall::predict::Predict;
use wandfall::proto::{self, flag, Up, PROTO};
use wandfall::trig;
use wasm_bindgen::prelude::*;

use crate::fx::{self, Draw};
use crate::hud;
use crate::look::{self, Look};
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
    /// When you last cast (ms), and inputs until you may again.
    cast_at: f64,
    cool: u32,
    hands: Hands,
    link: kit::Link,
    session: kit::Session,
    version: kit::Version,
    last: f64,
    fps: f64,
    perf: bool,
    touch: bool,
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

fn say(text: &str) {
    if let Some(b) = kit::document().body() {
        b.set_inner_html(&format!(
            "<p style=\"color:#f4eede;font:16px sans-serif;padding:24px;max-width:560px\">{text}</p>"
        ));
    }
}

fn send(p: &Page, up: &Up) {
    p.link.send(&up.encode());
}

fn net(p: &mut Page, now: f64) {
    for ev in p.link.poll(now) {
        match ev {
            Net::Up => send(p, &Up::Join { proto: PROTO }),
            Net::Holding => {}
            Net::Message(b) => {
                if let Some(seen) = Named::decode(&b) {
                    if seen.status != Status::Taken && !seen.name.is_empty() {
                        p.session.set_name(&seen.name);
                        p.link.set_hello(p.session.hello(&seen.name, false));
                    }
                    continue;
                }
                if let Some((v, you, seed, _)) = proto::read_welcome(&b) {
                    if v != PROTO {
                        kit::version::reload();
                        return;
                    }
                    p.st.you = you;
                    p.st.joined = you != 0;
                    if p.st.seed != Some(seed) {
                        p.st.seed = Some(seed);
                        let map = Map::new(seed);
                        let look = Look::new(&mut p.r, &map);
                        let mini = hud::island(&map);
                        p.island = Some(Island { map, look, mini });
                    }
                    continue;
                }
                if let Some(list) = proto::read_roster(&b) {
                    p.st.names = list
                        .into_iter()
                        .map(|(id, bot, n)| (id, (n, bot)))
                        .collect();
                    continue;
                }
                if let Some(list) = proto::read_events(&b) {
                    p.st.events(list, now);
                    continue;
                }
                if let Some(l) = proto::Loot::decode(&b) {
                    p.st.loot = l;
                    continue;
                }
                if let Some(f) = proto::Frame::decode(&b) {
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
        }
    }
}

fn hands(p: &mut Page) {
    let locked = kit::input::locked();
    for h in p.hands.drain() {
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
            Hand::Finger { id, kind, x, y, .. } if p.touch => {
                let (dy, dp) = p.pad.finger(id, kind, x, y, p.g.css);
                let k = p.fov / FOV;
                p.yaw += dy * k;
                p.pitch = (p.pitch + dp * k).clamp(-1.5, 1.5);
            }
            Hand::Button {
                button: 0, down, ..
            } => {
                if down && !locked && !p.touch {
                    kit::input::lock(p.g.canvas());
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
    for code in p.hands.pressed() {
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
    let held = |k: &str| p.hands.held(k);
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
    if p.touch {
        k |= p.pad.keys();
        take |= p.pad.casts();
    }
    while p.acc >= MS_A_TICK {
        p.acc -= MS_A_TICK;
        if !p.alive {
            continue;
        }
        p.seq = p.seq.wrapping_add(1);
        let i = Input {
            seq: p.seq,
            yaw: trig::heading(p.yaw),
            pitch: trig::pitch(p.pitch),
            keys: k,
            cast: std::mem::take(&mut p.asked) | take,
        };
        p.prev = p.pred.body;
        p.pred.push(i, &island.map);
        p.cool = p.cool.saturating_sub(1);
        if k & keys::FIRE != 0 && p.cool == 0 && !p.pred.body.glide {
            p.cool = BOLT_COOLDOWN;
            p.cast_at = kit::now();
        }
        p.outbox.push(i);
    }
    for chunk in std::mem::take(&mut p.outbox).chunks(proto::MAX_INPUTS) {
        send(p, &Up::Inputs(chunk.to_vec()));
    }
}

fn frame(p: &mut Page, now: f64) {
    let dt = (now - p.last).clamp(0.0, 250.0);
    p.last = now;
    if dt > 0.0 {
        p.fps += (1000.0 / dt - p.fps) * 0.05;
    }
    net(p, now);
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
        i.look.sea(&mut d.items, t);
        fx::loot(&i.look, &mut d, &p.st.loot, t, cam.eye);
        for s in &others {
            if s.flags & flag::ALIVE == 0 {
                continue;
            }
            if !(p.alive && s.id == p.st.you) {
                i.look.wizard(
                    &mut d.items,
                    s.id,
                    s.p,
                    trig::radians(s.yaw),
                    s.flags & flag::GLIDE != 0,
                );
            }
            fx::on_wizard(&i.look, &mut d, s, t);
        }
        for b in p.st.bolts(now) {
            fx::bolt(&i.look, &mut d, &b, b.by == p.st.you);
        }
        for &(at, who) in &p.st.bursts {
            if let Some(s) = others.iter().find(|s| s.id == who) {
                let pos = [s.p[0], s.p[1] + 1.2, s.p[2]];
                i.look.burst(
                    &mut d.lights,
                    &mut d.sparks,
                    pos,
                    (now - at) as f32,
                    at as u32,
                );
            }
        }
        let me = (p.st.you, p.pred.body.p, p.alive);
        let at_of = |id: u16| {
            if me.2 && id == me.0 {
                return Some(me.1);
            }
            others.iter().find(|s| s.id == id).map(|s| s.p)
        };
        fx::shows(&i.look, &mut d, &p.st.shows, now, at_of);
        if let Some(f) = &p.st.frame {
            if f.phase == 1 {
                i.look.storm(&mut d.items, f.storm.0, f.storm.1);
                let e = cam.eye;
                in_storm = p.alive
                    && (e[0] - f.storm.0[0]).powi(2) + (e[2] - f.storm.0[1]).powi(2)
                        > f.storm.1 * f.storm.1;
            }
        }
        if p.alive {
            let kick = (1.0 - (now - p.cast_at) / 180.0).clamp(0.0, 1.0) as f32;
            i.look.wand(&mut d.items, &mut d.lights, &cam, kick);
        }
    }
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
    if let Some(i) = &p.island {
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
        };
        hud::draw(&mut p.g.hud, &i.mini, &view);
        if p.touch && p.alive {
            let own = p.st.frame.as_ref().and_then(|f| f.you.as_ref());
            let me = p.pred.body.p;
            p.pad.can_take =
                p.st.loot
                    .scrolls
                    .iter()
                    .any(|s| (s.3[0] - me[0]).powi(2) + (s.3[2] - me[2]).powi(2) < 2.5);
            p.pad.draw(&mut p.g.hud, own, p.g.css, p.g.scale);
        }
    } else {
        let c = &mut p.g.hud;
        c.text_centred(
            c.w / 2,
            c.h / 2,
            "finding the island...",
            2 * ui,
            pixels::Rgba::rgb(250, 246, 236),
        );
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
        let r = Renderer::new(&g.device, &g.queue, g.format());
        let session = kit::Session::load();
        let link = kit::Link::open("wandfall", session.hello(&session.name(), false), false);
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
                cool: 0,
                hands,
                link,
                session,
                version: kit::Version::watch(VERSION_EVERY),
                last: kit::now(),
                fps: 60.0,
                perf: query("perf=1"),
                touch: kit::touch(),
            })
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
