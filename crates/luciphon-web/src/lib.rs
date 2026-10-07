//! Luciphon's page, in first person: two hands (`controls`) to Inputs 30
//! times a second, its own Lumen predicted and drawn smoothly between
//! ticks, everyone else 66 ms behind, the island drawn by the GPU
//! (`scene`, WebGL2 from Rust) with a pixel HUD over it (`hud`). `?perf=1`
//! shows frame times.

pub mod bag;
pub mod controls;
pub mod first;
pub mod fx;
pub mod hud;
pub mod laws;
pub mod play;
pub mod scene;
pub mod state;

use std::cell::RefCell;
use std::collections::HashMap;

use engine::who::{clean_name, Seen, Status};
use kit::input::Hand;
use kit::Net;
use lucilook::palette;
use luciphon::build::{act, slot};
use luciphon::combat::{Act, Swing};
use luciphon::proto::{Up, PROTO};
use luciphon::tiles::{obj, Tiles};
use pixels::{Rect, Rgba};
use wasm_bindgen::prelude::*;

use controls::{Controls, TICK_MS};
use first::Panel;
use laws::FEEL;
use scene::{Camera, Frame, Scene};
use state::State;

const TAUGHT: &str = "secretspace/luciphon/taught3d";
const VERSION_EVERY: f64 = 5.0 * 60_000.0;
/// Chunks rebuilt a frame, at most.
const BUILDS: usize = 2;

struct Page {
    gl: kit::gl::Gl,
    scene: Scene,
    st: State,
    fx: fx::Fx,
    ctl: Controls,
    hands: kit::input::Hands,
    link: kit::Link,
    session: kit::Session,
    version: kit::Version,
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
    ping_at: f64,
    perf: Option<f64>,
    /// The Heart's wheel is open; Build's pieces are being picked.
    wheel: bool,
    picking: bool,
    wheel_spots: Vec<(Rect, u8)>,
    /// The Underlight's two lights (layer pixels).
    choice: (Rect, Rect),
    /// Since when this page has had no Lumen while joined (it dreams).
    gone_since: f64,
    /// Where you fell from, for the Underlight's camera.
    fell: [f32; 3],
    keys_hidden: bool,
    was_locked: bool,
    swing: f32,
    /// What you have done once, ever (for the hints).
    taught: u8,
    /// Asked for the newest page, being too old.
    asked: bool,
    /// The wand fired since the last frame (drawn at once).
    shots: Vec<Swing>,
    /// Your gear's panel is open; the piece picked in it; where it was drawn.
    bag_open: bool,
    bag_pick: Option<usize>,
    bag_spots: Vec<(Rect, bag::Pick)>,
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

fn send(p: &Page, up: &Up) {
    p.link.send(&up.encode());
}

fn heart(p: &Page, act: u8, arg: u8) {
    send(p, &Up::Heart { act, arg });
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
    if !p.touch {
        kit::input::lock(p.gl.canvas());
    }
}

fn net(p: &mut Page, now: f64) {
    for ev in p.link.poll(now) {
        match ev {
            Net::Up => {
                p.st.connected = true;
                send(
                    p,
                    &Up::Device {
                        w: p.gl.css.0 as u16,
                        h: p.gl.css.1 as u16,
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
                let under = p.st.descent > 0;
                p.st.receive(&b, now, &mut p.fx);
                if p.st.joined && !was {
                    p.joining = false;
                    p.ctl.next_at = now;
                }
                if !under && p.st.descent > 0 {
                    p.fell = p.st.drawn_self(now);
                }
                // Rebirth is the update: a newer page loads at the return.
                if under && p.st.descent == 0 && p.version.newer() {
                    kit::version::reload();
                }
            }
        }
    }
    for ms in p.st.felt.drain(..) {
        kit::vibrate(ms);
    }
    // Too old for the server: ask for the newest page, and load it.
    if p.st.outdated && !p.asked {
        p.asked = true;
        p.version.poll(now, true);
    }
    if p.st.outdated && p.version.newer() {
        kit::version::reload();
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

/// Joined, connected, and no Lumen for a second: asleep.
fn dreaming(p: &Page, now: f64) -> bool {
    p.st.joined
        && p.st.connected
        && p.st.mirror.own.is_none()
        && p.gone_since > 0.0
        && now - p.gone_since > 1000.0
}

fn building(p: &Page) -> bool {
    p.st.mirror.own.is_some_and(|o| o.build != 0)
}

/// A wheel slot, or a piece while picking.
fn pick(p: &mut Page, k: u8) {
    if p.picking {
        p.picking = false;
        if (k as usize) < luciphon::build::PIECES.len() {
            heart(p, act::PIECE, k);
        }
    } else {
        heart(p, act::WHEEL, k);
        p.picking = k == slot::BUILD;
    }
    p.wheel = false;
}

fn key(p: &mut Page, code: &str) {
    if !p.st.joined {
        if code == "Enter" && p.panel == Panel::Title {
            play(p);
        }
        return;
    }
    p.ctl.key(code);
    if let Some(d) = code
        .strip_prefix("Digit")
        .and_then(|d| d.parse::<u8>().ok())
    {
        if (1..=8).contains(&d) && (p.wheel || p.picking) {
            pick(p, d - 1);
        }
        return;
    }
    match code {
        "Tab" | "KeyI" => toggle_bag(p),
        "Escape" if p.bag_open => toggle_bag(p),
        "KeyE" => p.wheel = !p.wheel && !p.picking,
        "KeyB" if building(p) || p.picking => {
            p.picking = false;
            heart(p, act::DONE, 0);
        }
        "KeyB" => pick(p, slot::BUILD),
        "KeyK" => pick(p, slot::KINDLE),
        "KeyR" => pick(p, slot::REKINDLE),
        "KeyT" => pick(p, slot::RECALL),
        "KeyH" => heart(p, act::CHIRP, 0),
        "Slash" => p.keys_hidden = !p.keys_hidden,
        _ => {}
    }
}

/// Open or close your gear; on a desktop the mouse is let go to use it,
/// and taken back after.
fn toggle_bag(p: &mut Page) {
    p.bag_open = !p.bag_open;
    p.bag_pick = None;
    if !p.touch {
        if p.bag_open {
            kit::input::unlock();
        } else {
            kit::input::lock(p.gl.canvas());
        }
    }
}

/// A press on your gear's panel.
fn bag_press(p: &mut Page, x: f32, y: f32) {
    let Some(&(_, pick)) = p.bag_spots.iter().find(|s| s.0.contains(x, y)) else {
        return;
    };
    match pick {
        bag::Pick::Item(k) => p.bag_pick = Some(k),
        bag::Pick::Wear | bag::Pick::Drop => {
            if let Some(k) = p.bag_pick.take() {
                let a = if pick == bag::Pick::Wear {
                    act::EQUIP
                } else {
                    act::DROP
                };
                heart(p, a, k as u8);
            }
        }
        bag::Pick::Craft(id) => heart(p, act::CRAFT, id),
        bag::Pick::Close => toggle_bag(p),
    }
}

/// Whether you stand near a light to craft by: the Luciphon, or your hearth.
fn near_a_light(p: &Page) -> bool {
    let b = &p.st.pred.me.body;
    let (x, y) = (b.x.to_f32(), b.y.to_f32());
    let near = |hx: f32, hy: f32| ((x - hx).powi(2) + (y - hy).powi(2)).sqrt() <= 6.0;
    near(0.5, 0.5)
        || p.st
            .claims
            .get(&b.claim)
            .and_then(|c| c.hearth)
            .is_some_and(|(hx, hy)| near(hx as f32 + 0.5, hy as f32 + 0.5))
}

fn hands(p: &mut Page, now: f64) {
    let playing = p.st.joined && p.st.connected;
    let locked = kit::input::locked();
    if p.was_locked && !locked {
        p.ctl.drop_all();
    }
    p.was_locked = locked;
    for code in p.hands.pressed() {
        key(p, &code);
    }
    for h in p.hands.drain() {
        let down = match h {
            Hand::Finger {
                kind: kit::input::Kind::Down,
                x,
                y,
                ..
            } => Some((x, y)),
            Hand::Button {
                down: true, x, y, ..
            } if !locked => Some((x, y)),
            _ => None,
        };
        if !playing {
            if let Some((x, y)) = down {
                let (x, y) = p.gl.to_px(x, y);
                menu_press(p, x, y);
            }
            continue;
        }
        // In the Underlight, the two lights to return at.
        if p.st.descent > 0 {
            if let Some((x, y)) = down {
                let (x, y) = p.gl.to_px(x, y);
                if p.choice.0.contains(x, y) {
                    heart(p, act::HOME, 0);
                } else if p.choice.1.contains(x, y) {
                    heart(p, act::HOME, 1);
                }
            }
            continue;
        }
        // Your gear's panel takes every press while it is open.
        if p.bag_open {
            if let Some((x, y)) = down {
                let (x, y) = p.gl.to_px(x, y);
                bag_press(p, x, y);
            }
            continue;
        }
        // Dreaming: a touch wakes you where you lay.
        if dreaming(p, now) {
            if down.is_some() {
                send(p, &Up::Join { proto: PROTO });
                p.gone_since = now;
            }
            continue;
        }
        // The wheel takes the next touch: a slot, or closing it.
        if (p.wheel || p.picking) && p.touch {
            if let Some((x, y)) = down {
                let (x, y) = p.gl.to_px(x, y);
                match p.wheel_spots.iter().find(|s| s.0.contains(x, y)) {
                    Some(&(_, k)) => pick(p, k),
                    None => {
                        p.wheel = false;
                        if p.picking {
                            p.picking = false;
                            heart(p, act::DONE, 0);
                        }
                    }
                }
                continue;
            }
        }
        p.ctl.feed(&h, locked, p.gl.css, &FEEL);
    }
    if std::mem::take(&mut p.ctl.want_lock) && playing && !p.touch {
        kit::input::lock(p.gl.canvas());
    }
    if std::mem::take(&mut p.ctl.heart) {
        p.wheel = !p.wheel;
    }
    if std::mem::take(&mut p.ctl.bag) {
        toggle_bag(p);
    }
    p.ctl.tick(now, &FEEL);
    // Inputs, 30 a second.
    if playing {
        if p.ctl.next_at < now - 250.0 {
            p.ctl.next_at = now;
        }
        while now >= p.ctl.next_at {
            let hands = &p.hands;
            let (seq, it) = p.ctl.sample(&|k| hands.held(k), &FEEL);
            send(p, &Up::Input { seq, it });
            if let Some(s) = p.st.push(seq, it, now) {
                p.shots.push(s);
            }
            p.ctl.next_at += TICK_MS;
        }
    }
    if p.taught | p.ctl.done != p.taught {
        p.taught |= p.ctl.done;
        kit::save(TAUGHT, &p.taught.to_string());
    }
}

fn frame(p: &mut Page, now: f64) {
    if p.st.joined && p.st.mirror.own.is_none() {
        if p.gone_since == 0.0 {
            p.gone_since = now;
        }
    } else {
        p.gone_since = 0.0;
    }
    net(p, now);
    // Chunks to build, the nearest first, a few a frame.
    let hues: HashMap<u16, u8> = p.st.claims.iter().map(|(&id, c)| (id, c.hue)).collect();
    for at in std::mem::take(&mut p.st.gone) {
        p.scene.forget(&p.gl.gl, at);
        p.st.stale.remove(&at);
    }
    let mut todo: Vec<(i32, i32)> = p.st.stale.iter().copied().collect();
    let me = p.st.drawn_self(now);
    let (mx, my) = Tiles::chunk_of(me[0] as i32, me[1] as i32);
    todo.sort_by_key(|&(x, y)| (x - mx).pow(2) + (y - my).pow(2));
    for at in todo.into_iter().take(BUILDS) {
        p.st.stale.remove(&at);
        if p.st.mirror.chunks.contains(&at) {
            p.scene.build(&p.gl.gl, &p.st.mirror.tiles, at, &hues);
        }
    }
    hands(p, now);
    if now - p.ping_at > 1000.0 {
        p.ping_at = now;
        let rtt = (p.st.rtt / 4.0).clamp(0.0, 255.0) as u8;
        send(p, &Up::Ping { t: now as u32, rtt });
    }
    p.version.poll(now, false);
    let dt = (now - p.drawn_at).clamp(0.0, 100.0);
    p.st.age(now, dt);
    p.fx.age(now);
    p.drawn_at = now;
    if p.gl.lost() {
        return;
    }
    draw(p, now, dt);
    if let Some(avg) = &mut p.perf {
        let ms = kit::now() - now;
        *avg = *avg * 0.95 + ms * 0.05;
        let me = p.st.drawn_self(now);
        // The nearest monster in sight: how far, which way.
        let near = p
            .st
            .mirror
            .ents
            .values()
            .filter(|e| e.kind == luciphon::proto::kind::BEAST)
            .map(|e| {
                let (dx, dy) = (e.x as f32 / 256.0 - me[0], e.y as f32 / 256.0 - me[1]);
                ((dx * dx + dy * dy).sqrt(), dy.atan2(dx))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let line = format!(
            "{:.2} ms {} verts @{:.1},{:.1} yaw {:.2} beast {:.1},{:.2}",
            *avg,
            p.scene.vertices,
            me[0],
            me[1],
            p.ctl.yaw,
            near.map_or(-1.0, |n| n.0),
            near.map_or(0.0, |n| n.1)
        );
        let c = &mut p.gl.hud;
        c.text_shadowed(4, c.h - 12, &line, 1, palette::INK);
        kit::document().set_title(&line);
    }
    p.gl.present();
}

fn camera(p: &mut Page, now: f64) -> Camera {
    let aspect = p.gl.size.0 as f32 / p.gl.size.1.max(1) as f32;
    let fov = if aspect < 1.0 {
        FEEL.fov_tall
    } else {
        FEEL.fov
    };
    let mut cam = Camera {
        eye: [0.0; 3],
        yaw: p.ctl.yaw,
        pitch: p.ctl.pitch,
        fov,
        aspect,
        far: FEEL.far,
    };
    if !p.st.joined {
        // Before playing: round the Luciphon, slowly.
        let t = (now / 14_000.0) as f32;
        cam.eye = [0.5 + t.cos() * 11.0, 3.4, 0.5 + t.sin() * 11.0];
        cam.yaw = (0.5 - cam.eye[2]).atan2(0.5 - cam.eye[0]);
        cam.pitch = -0.12;
    } else if p.st.descent > 0 {
        // The Underlight: high over where you fell, looking down.
        cam.eye = [p.fell[0], 14.0, p.fell[1]];
        cam.pitch = -1.2;
        cam.yaw = p.ctl.yaw + (now / 9000.0) as f32;
    } else {
        let me = p.st.drawn_self(now);
        let (sx, sy) = p.fx.shake(now);
        let (s, c) = cam.yaw.sin_cos();
        cam.eye = [me[0] - s * sx, me[2] + FEEL.eye + sy, me[1] + c * sx];
    }
    cam
}

fn draw(p: &mut Page, now: f64, dt: f64) {
    let cam = camera(p, now);
    let alive = p.st.joined && p.st.descent == 0 && p.st.pred.ready && p.st.mirror.own.is_some();
    let things = p.st.things(now);
    let own = p.st.mirror.own.unwrap_or_default();
    let l = &p.st.laws;
    let me = p.st.pred.me;
    let hue = scene::chunk::hue(p.st.mirror.ents.get(&p.st.you).map_or(0, |e| e.hue));
    let flame = own.flame as f32 / l.flame.max(1) as f32;

    // The wand: drawn back in a wind-up, kicking when it fires.
    let target = if now - p.st.struck_at < 70.0 {
        1.0
    } else if me.act.act == Act::Windup {
        -0.7
    } else {
        0.0
    };
    let k = 1.0 - (-dt / 40.0).exp() as f32;
    p.swing += (target - p.swing) * k;
    let charging = me.act.act == Act::Charge;
    let hand = alive.then_some(scene::Hand {
        hue,
        flame,
        swing: p.swing,
        charge: if charging {
            (me.act.c as f32 / l.charge_full as f32).min(1.0)
        } else {
            0.0
        },
        perfect: charging && (l.charge_full..=l.perfect_to).contains(&me.act.c),
    });
    // Your own beams, from the wand's tip, as you fired them.
    let at = p.st.drawn_self(now);
    for s in std::mem::take(&mut p.shots) {
        if matches!(s, Swing::Throw { .. }) || !alive {
            continue;
        }
        let big = s != Swing::Strike;
        let len = p.st.own_beam(now, big);
        let a = me.act.aim as f32 / 65536.0 * std::f32::consts::TAU;
        let h = (cam.eye[1] + cam.pitch.tan() * len).clamp(0.15, 4.0);
        let to = [at[0] + a.cos() * len, h, at[1] + a.sin() * len];
        let from = scene::wand(&cam, 1.0).1;
        let hue = p.st.mirror.ents.get(&p.st.you).map_or(0, |e| e.hue);
        p.st.beams.push(state::Beam {
            from,
            to,
            at: now,
            big,
            hue,
        });
        p.fx.burst(to, 6, 2.5, palette::RIM, now);
    }
    let beams =
        p.st.beams
            .iter()
            .map(|b| {
                let life = state::BEAM_MS * if b.big { 2.0 } else { 1.0 };
                let k = (1.0 - (now - b.at) / life).clamp(0.0, 1.0) as f32;
                let c = shapes_mix(scene::chunk::hue(b.hue), b.big);
                let w = if b.big { 0.1 } else { 0.035 } * (0.5 + 0.5 * k);
                (b.from, b.to, w, [c[0], c[1], c[2], k])
            })
            .collect();
    let ghost = (alive && own.build != 0).then(|| {
        let b = &me.body;
        let (gx, gy) = luciphon::build::ghost(b.x, b.y, b.facing);
        let t = p.st.mirror.tiles.get(gx, gy);
        let mine = t.land_of(b.claim) || own.build == obj::HEARTH;
        (
            gx,
            gy,
            !t.void() && !t.solid() && mine,
            own.build == obj::HEARTH,
        )
    });
    // The rings of nodes near you: strike on the ring for double.
    let mut rings = Vec::new();
    if alive {
        let (cx, cy) = (me.body.x.floor(), me.body.y.floor());
        let tick = p.st.tick_now(now);
        for ty in cy - 4..=cy + 4 {
            for tx in cx - 4..=cx + 4 {
                let o = p.st.mirror.tiles.get(tx, ty).obj;
                let Some(idx) = Tiles::index(tx, ty).filter(|_| obj::node(o)) else {
                    continue;
                };
                let ph = luciphon::gather::phase(p.st.seed, idx as u16) as f64;
                let k = (((tick + ph) % 30.0) / 30.0) as f32;
                let near = (1.0 - k).min(k) < 0.08;
                let r = 0.35 + 0.6 * (1.0 - k);
                let a = if near { 0.8 } else { 0.1 + 0.25 * k };
                rings.push(([tx as f32 + 0.5, 0.0, ty as f32 + 0.5], r, a));
            }
        }
    }
    let mut sparks = Vec::new();
    p.fx.points(now, &mut sparks);
    if p.st.descent > 0 {
        // The thread of your path since your last return.
        for &(x, y) in &p.st.path {
            sparks.extend_from_slice(&[x, 0.15, y, 1.0, 0.82, 0.48, 0.8, 0.18]);
        }
    }
    let under = p.st.descent > 0;
    let f = Frame {
        cam,
        things: &things,
        hand,
        ghost,
        rings,
        beams,
        sparks: &sparks,
        you: alive.then_some((hue, scene::flame(flame))),
        under,
        now,
        fog: if under {
            (20.0, 70.0)
        } else {
            (FEEL.fog_near, FEEL.fog_far)
        },
    };
    let vp = p.scene.draw(&p.gl, &f);
    overlay(p, now, &vp, &cam, &things, alive);
}

/// Everything in pixels over the picture.
fn overlay(
    p: &mut Page,
    now: f64,
    vp: &kit::gl::M4,
    cam: &Camera,
    things: &[state::Thing],
    alive: bool,
) {
    let u = p.gl.ui();
    let locked = kit::input::locked();
    p.gl.hud.wipe();
    let own = p.st.mirror.own.unwrap_or_default();
    if alive {
        let c = &mut p.gl.hud;
        let l = &p.st.laws;
        let me = p.st.pred.me;
        hud::names(c, vp, cam.eye, things, u);
        hud::beasts(c, vp, cam.eye, things, u);
        hud::marker(c, vp, [0.5, 4.6, 0.5], palette::GOLD, u);
        if let Some(info) = p.st.claims.get(&me.body.claim) {
            if let Some((hx, hy)) = info.hearth {
                let col = Rgba::hsl(info.hue as f32 / 256.0 * 360.0, 0.7, 0.65);
                hud::marker(c, vp, [hx as f32 + 0.5, 1.4, hy as f32 + 0.5], col, u);
            }
        }
        for &(_, x, y) in &p.st.mirror.far {
            let at = [x as f32 + 0.5, 1.8, y as f32 + 0.5];
            hud::marker(c, vp, at, Rgba(255, 240, 190, 220), u);
        }
        hud::crosshair(c, u);
        if me.act.act == Act::Charge {
            hud::charge(c, me.act.c, l, u);
        }
        let most = l.flame + luciphon::gear::worn(own.gear).flame;
        hud::vitals(
            c,
            own.flame as f32 / most.max(1) as f32,
            me.body.breath as f32 / l.breath as f32,
            u,
        );
        if let Some((t, at)) = &p.st.toast {
            if now - at < 2200.0 {
                let s = pixels::fit_scale(t, c.w - 20, 2 * u);
                c.text_centred(c.w / 2, c.h / 4, t, s, palette::GOLD);
            }
        }
        if own.build != 0 {
            let name = play::PIECES
                .iter()
                .zip(luciphon::build::PIECES)
                .find(|x| x.1 == own.build)
                .map_or("a piece", |x| x.0);
            let line = format!("placing: {name}");
            c.text_centred(c.w / 2, c.h / 2 + 22 * u, &line, u, palette::GOLD);
            if own.channel > 0 {
                let k = 1.0 - own.channel as f32 / l.build_channel.max(1) as f32;
                let (x, y) = (c.w as f32 / 2.0, c.h as f32 / 2.0);
                c.ring(x, y, 6.0 + 10.0 * k, 2.0, palette::GOLD);
            }
        }
    }
    if p.st.joined {
        let build_keys = building(p) || p.picking;
        let c = &mut p.gl.hud;
        let top = format!("{} here, {} awake", p.st.people, p.st.awake);
        c.text_shadowed(6 * u, 6 * u, &top, u, palette::DIM);
        play::bag(c, 6 * u, 18 * u, &own, u);
        if alive && p.touch {
            hud::touch(c, &p.ctl, p.gl.css, p.gl.scale, &FEEL, u);
        }
        if alive && !p.touch {
            if !locked && !p.bag_open {
                let a = (200.0 + 50.0 * (now / 400.0).sin()) as u8;
                let y = c.h / 2 - 30 * u;
                c.text_centred(c.w / 2, y, "click to play", 2 * u, Rgba(255, 210, 122, a));
            }
            if !p.keys_hidden {
                play::keys(c, build_keys, u);
            }
        }
        if alive && (p.wheel || p.picking) {
            let (cx, cy) = (c.w as f32 / 2.0, c.h as f32 / 2.0);
            p.wheel_spots = play::wheel(&mut p.gl.hud, cx, cy, p.picking, !p.touch, u);
        }
        let c = &mut p.gl.hud;
        if alive && !p.wheel && !p.picking && (p.touch || locked) {
            first::hint(c, p.taught, p.touch, u, now);
        }
        if let Some((col, _)) = p.fx.flash {
            hud::flash(c, col);
        }
        if alive && p.bag_open {
            let near = near_a_light(p);
            p.bag_spots = bag::draw(&mut p.gl.hud, &own, p.bag_pick, near, u);
        }
    }
    if p.st.descent > 0 {
        let has =
            p.st.claims
                .get(&own.me.body.claim)
                .is_some_and(|c| c.hearth.is_some());
        let c = &mut p.gl.hud;
        let line = "the underlight";
        c.text_centred(c.w / 2, c.h / 5, line, 2 * u, Rgba(127, 224, 255, 200));
        p.choice = play::choice(c, has, own.home, u);
    }
    if dreaming(p, now) {
        play::dreaming(&mut p.gl.hud, u, now);
    }
    // The Stillness: the world holds; the picture stays, under gold.
    if p.st.joined && !p.st.connected {
        let c = &mut p.gl.hud;
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
        p.spots = first::draw(&mut p.gl.hud, &look, now);
        let place = |r: Rect| Some(p.gl.to_css(r.x, r.y, r.w, r.h));
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

/// A beam's colour: its shooter's hue, mostly white; a great beam gold.
fn shapes_mix(hue: [f32; 3], big: bool) -> [f32; 3] {
    let white = if big {
        [1.0, 0.86, 0.55]
    } else {
        [1.0, 1.0, 1.0]
    };
    scene::shapes::mix(hue, white, 0.6)
}

fn no_webgl() {
    if let Some(b) = kit::document().body() {
        b.set_inner_html(
            "<p style=\"color:#f4eede;font:16px sans-serif;padding:24px\">luciphon is drawn in 3D: it needs a browser with WebGL2.</p>",
        );
    }
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let touch = kit::touch();
    let Some(mut gl) = kit::gl::Gl::new("screen") else {
        no_webgl();
        return Ok(());
    };
    let max = if touch {
        FEEL.max_dpr_touch
    } else {
        FEEL.max_dpr
    };
    gl.fit(352.0, max);
    let scene = match Scene::new(&gl) {
        Ok(s) => s,
        Err(e) => {
            kit::document().set_title(&e);
            no_webgl();
            return Ok(());
        }
    };
    let session = kit::Session::load();
    let name = kit::TextField::new(engine::who::MAX_NAME as u32, "your name");
    name.set_value(&session.name());
    let words = kit::TextField::new(120, "eleven words");
    let link = kit::Link::open("luciphon", session.hello(&session.name(), false), false);
    let hands = kit::input::Hands::attach(gl.canvas());
    let taught = kit::load(TAUGHT).and_then(|t| t.parse().ok()).unwrap_or(0);
    PAGE.with(|p| {
        *p.borrow_mut() = Some(Page {
            gl,
            scene,
            st: State::default(),
            fx: fx::Fx::default(),
            ctl: Controls::default(),
            taught,
            asked: false,
            shots: Vec::new(),
            bag_open: false,
            bag_pick: None,
            bag_spots: Vec::new(),
            hands,
            link,
            session,
            version: kit::Version::watch(VERSION_EVERY),
            name,
            words,
            panel: Panel::Title,
            spots: first::Spots::default(),
            seen: None,
            taken: false,
            renaming: false,
            joining: false,
            restore_bad: false,
            touch,
            drawn_at: 0.0,
            ping_at: 0.0,
            perf: query("perf=1").then_some(0.0),
            wheel: false,
            picking: false,
            wheel_spots: Vec::new(),
            choice: (Rect::default(), Rect::default()),
            gone_since: 0.0,
            fell: [0.0; 3],
            keys_hidden: false,
            was_locked: false,
            swing: 0.0,
        })
    });
    kit::frames(|now| {
        with(|p| frame(p, now));
    });
    kit::on(&kit::window(), "resize", move |_| {
        with(|p| p.gl.fit(352.0, max));
    });
    Ok(())
}
