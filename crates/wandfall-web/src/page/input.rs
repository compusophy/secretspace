//! Your hands: the mouse and fingers turning the view, buttons and keys
//! casting and pressing the menus; each tick's input predicted at once
//! and sent; and the pace kept (a tier down when frames run slow).

use super::*;

/// The most a single mouse move may turn the view (pixels).
const MOUSE_MOST: f64 = 250.0;

pub(super) fn hands(p: &mut Page) {
    let locked = kit::input::locked();
    if locked && !p.was_locked {
        p.skip = 2;
    }
    // The mouse let go while playing (the browser's own Esc, a switch
    // away): the menu, as if Esc had come to the page.
    if p.was_locked
        && !locked
        && !p.touch
        && !p.book
        && !p.meta.is_open()
        && !matches!(p.mode, Mode::Title)
    {
        open_menu(p);
    }
    p.was_locked = locked;
    if upright(p) {
        p.pad.reset();
    }
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
                if p.skip > 0 {
                    p.skip -= 1;
                    continue;
                }
                let (dx, dy) = (
                    dx.clamp(-MOUSE_MOST, MOUSE_MOST),
                    dy.clamp(-MOUSE_MOST, MOUSE_MOST),
                );
                // Slower when zoomed, so the aim holds.
                let k = MOUSE * zoom(p) * p.set.look;
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
                tap(p, lx, ly);
            }
            // The menu button, alive or out (when out, all there is).
            Hand::Finger {
                kind: kit::input::Kind::Down,
                x,
                y,
                ..
            } if p.touch && touch::on_menu(x, y) => p.pad.menu = true,
            // Out, a tap watches the next one still in it.
            Hand::Finger {
                kind: kit::input::Kind::Down,
                ..
            } if p.touch && !p.alive && !matches!(p.mode, Mode::Title) => {
                p.cycle += 1;
            }
            // A tap on the lesson skips it.
            Hand::Finger {
                kind: kit::input::Kind::Down,
                x,
                y,
                ..
            } if p.touch && on_lesson(p, x, y) => {
                p.lessons.skip(kit::now());
            }
            // Held upright, the pad waits (a word says to turn it).
            Hand::Finger { id, kind, x, y, .. } if p.touch && !upright(p) => {
                let (dy, dp) = p.pad.finger(id, kind, x, y, p.g.css);
                let k = zoom(p) * p.set.look;
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
                    tap(p, lx, ly);
                } else {
                    p.firing = down && locked;
                }
            }
            _ => {}
        }
    }
    if p.touch {
        p.aiming = p.pad.aim;
        // Spells tapped go the way keys do: once, with their sound.
        let tapped = p.pad.casts();
        if !p.meta.is_open() {
            p.asked |= tapped;
        }
    } else if !locked {
        p.firing = false;
        p.aiming = false;
    }
    // Out, nothing is cast: a press then is not kept for the next life.
    if !p.alive {
        p.asked = 0;
    }
    if std::mem::take(&mut p.pad.menu) {
        if p.meta.is_open() {
            p.meta.hide();
        } else {
            open_menu(p);
        }
    }
    for code in p.hands.pressed() {
        p.sounds.audio.wake();
        // Writing your name on the title: the keys are the field's (Enter
        // or Esc puts it down).
        if p.name.focused() {
            if matches!(code.as_str(), "Enter" | "Escape") {
                p.name.blur();
            }
            continue;
        }
        // Esc: the shared menu (with the keyboard held, full screen, it
        // comes to the page; a held Esc still leaves full screen).
        if code == "Escape" {
            if p.book {
                act(p, Act::CloseBook);
            } else if p.meta.is_open() {
                if p.meta.escape() == Some(kit::meta::Pick::Resume) {
                    resume(p);
                }
            } else {
                open_menu(p);
            }
            continue;
        }
        // Writing to us: the keys are the field's (Enter sends).
        if p.meta.typing() {
            if code == "Enter" {
                p.meta.enter(kit::now());
            }
            continue;
        }
        if p.meta.is_open() {
            continue;
        }
        if code == "Enter" && matches!(p.mode, Mode::Practice(_)) {
            p.lessons.skip(kit::now());
            continue;
        }
        if code == "KeyM" {
            let on = !p.sounds.audio.muted;
            p.sounds.mute(on);
            continue;
        }
        if code == "KeyB" && !matches!(p.mode, Mode::Title) && (p.alive || p.book) {
            act(p, if p.book { Act::CloseBook } else { Act::Book });
            continue;
        }
        // Out: the next one to watch, or the last.
        if !p.alive && !matches!(p.mode, Mode::Title) {
            match code.as_str() {
                "Space" | "ArrowRight" | "KeyD" => p.cycle += 1,
                "ArrowLeft" | "KeyA" => p.cycle -= 1,
                _ => {}
            }
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
pub(super) fn inputs(p: &mut Page, dt: f64) {
    if p.island.is_none() {
        return;
    }
    p.acc = (p.acc + dt).min(MS_A_TICK * 6.0);
    if matches!(p.mode, Mode::Title) {
        p.acc = 0.0;
        return;
    }
    // With the book open you still move (the mouse is the book's); with
    // the shared menu up you stand.
    let busy = p.meta.is_open();
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
    // Ctrl crouches only while every key comes to the page (full screen,
    // the keyboard held): otherwise Ctrl+W, running, would close the tab.
    let ctrl = kit::input::keys_held() && (held("ControlLeft") || held("ControlRight"));
    if held("KeyC") || ctrl {
        k |= keys::CROUCH;
    }
    if held("ShiftLeft") || held("ShiftRight") {
        k |= keys::SPRINT;
    }
    if p.firing {
        k |= keys::FIRE;
    }
    if p.aiming {
        k |= keys::AIM;
    }
    if p.touch && !busy {
        k |= p.pad.keys();
    }
    while p.acc >= MS_A_TICK {
        p.acc -= MS_A_TICK;
        // Out, or the room not there to hear (your wizard holds still
        // till it is back, not running on alone).
        if !p.alive || p.lost.is_some() {
            continue;
        }
        p.seq = p.seq.wrapping_add(1);
        let asked = std::mem::take(&mut p.asked);
        heard_cast(p, asked);
        let i = Input {
            seq: p.seq,
            yaw: trig::heading(p.aim.0),
            pitch: trig::pitch(p.aim.1),
            keys: k,
            cast: asked,
            view: p.st.view_tick(kit::now()) as u16,
        };
        p.prev = p.pred.body;
        if let Some(island) = &p.island {
            p.pred.push(i, &island.map);
        }
        if matches!(p.mode, Mode::Practice(_)) {
            p.lessons.tick(&p.prev, &p.pred.body, p.seq, kit::now());
        }
        // Feet leaving the ground, and meeting it again.
        let (was, is) = (p.prev, p.pred.body);
        if p.touch {
            p.pad.ease(&was, &is);
        }
        if !was.ground && is.ground {
            let hard = ((-was.v[1] - 3.0) / 17.0).clamp(0.0, 1.0);
            if hard > 0.0 || was.glide {
                let hard = if was.glide { 0.8 } else { hard };
                p.sounds.thud(hard);
                p.st.dust.push((kit::now(), is.p, hard));
            }
        } else if was.ground && !is.ground && is.v[1] > 1.0 {
            p.sounds.hop();
        } else if was.ground && is.glide && !was.glide {
            // Thrown up by a launch rune: a rush of wind.
            p.sounds.cast(spell::GUST, None, p.ear);
        } else if was.mantle == 0 && is.mantle > 0 {
            // Pulling up onto a ledge.
            p.sounds.hop();
        } else if is.walls > was.walls {
            // Off a wall: a hop, and dust kicked from it.
            p.sounds.hop();
            let n = was.wall_n;
            let at = [is.p[0] - n[0] * 0.4, is.p[1] + 0.8, is.p[2] - n[1] * 0.4];
            p.st.dust.push((kit::now(), at, 0.5));
        } else if !was.air_jumped && is.air_jumped {
            // The air jump: a hop, and a puff of magic under the feet.
            p.sounds.hop();
            p.st.dust.push((kit::now(), is.p, 0.6));
        }
        p.cool = p.cool.saturating_sub(1);
        if k & keys::FIRE != 0 && p.cool == 0 && !p.pred.body.glide {
            p.cool = BOLT_COOLDOWN;
            p.sounds.wand(None, p.ear, (p.seq % 5) as f32 / 5.0);
        }
        p.outbox.push(i);
    }
    let out = std::mem::take(&mut p.outbox);
    for chunk in out.chunks(proto::MAX_INPUTS) {
        send(p, &Up::Inputs(chunk.to_vec()));
    }
}

/// A spell asked for (`asked`, a bit a slot) heard at once, if the room
/// will cast it: it is ready, you are not gliding (nothing casts in the
/// air on a broom or a rune's flight), and it was not already asked for
/// in the time the room takes to answer.
fn heard_cast(p: &mut Page, asked: u8) {
    let Some(o) = p.st.frame.as_ref().and_then(|f| f.you.as_ref()) else {
        return;
    };
    if asked == 0 || p.pred.body.glide {
        return;
    }
    let now = kit::now();
    // Inputs the room has not answered yet: the time it takes, near enough.
    let wait = ((p.seq.wrapping_sub(o.seq) as f64 + 2.0) * MS_A_TICK).min(1000.0);
    let ready = |k: usize| asked & cast::SLOT[k] != 0 && o.cds[k] == 0;
    let Some((k, sp)) = (0..4).find_map(|k| Some((k, o.slots[k].filter(|_| ready(k))?.0))) else {
        return;
    };
    if now - p.cast_heard[k] > wait {
        p.cast_heard[k] = now;
        p.sounds.cast(sp, None, p.ear);
    }
}

/// Too slow for this tier (over 2.5 s of play, frames over 30 ms on
/// average): a step down (first the scene's size, then the tier),
/// nothing built again.
pub(super) fn pace(p: &mut Page, dt: f64) {
    if p.fixed || !p.alive || dt <= 0.0 || !p.set.auto() {
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
    repaint(p, q);
}

/// The view turns slower aiming (zoomed in), so the aim holds.
fn zoom(p: &Page) -> f32 {
    if p.aiming {
        camera::FOV_AIM / camera::FOV
    } else {
        1.0
    }
}

/// Whether a finger at (`x`, `y`) CSS pixels is on the lesson's skip
/// chip (the rest of its panel is the stick's and the view's, as ever).
fn on_lesson(p: &Page, x: f64, y: f64) -> bool {
    let (lx, ly) = p.g.to_px(x, y);
    p.lesson_skip.is_some_and(|r| r.contains(lx, ly))
}

/// A press on the menus at (x, y) on the layer: the shared menu first,
/// then the page's own buttons; off them all, in a game, back to it.
fn tap(p: &mut Page, x: f32, y: f32) {
    if p.meta.is_open() && !p.meta.in_game_panel() {
        match p.meta.click(x, y, kit::now()) {
            Some(Some(pick)) => picked(p, pick),
            Some(None) => {}
            None => resume(p),
        }
        return;
    }
    match p.spots.hit(x, y) {
        Some(a) => act(p, a),
        // Off the settings: the menu again.
        None if p.meta.in_game_panel() => p.meta.back(),
        None if p.book => act(p, Act::CloseBook),
        None if !p.touch && !matches!(p.mode, Mode::Title) => grab(p),
        None => {}
    }
}
