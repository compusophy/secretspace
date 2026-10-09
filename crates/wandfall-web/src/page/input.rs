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
    p.was_locked = locked;
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
                if let Some(a) = p.spots.hit(lx, ly) {
                    act(p, a);
                }
            }
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
            Hand::Finger { id, kind, x, y, .. } if p.touch => {
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
                    match p.spots.hit(lx, ly) {
                        Some(a) => act(p, a),
                        // Off the buttons, in a game: play (closing the
                        // book if it is open).
                        None if !matches!(p.mode, Mode::Title) => {
                            if p.book {
                                act(p, Act::CloseBook);
                            } else {
                                grab(p);
                            }
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
        // Esc pauses (with the keyboard held it comes to the page instead
        // of letting the mouse go; held, it leaves full screen).
        if code == "Escape" {
            if p.book {
                act(p, Act::CloseBook);
            } else if locked {
                kit::input::unlock();
            }
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
        if code == "KeyB" && !matches!(p.mode, Mode::Title) {
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
    let Some(island) = &p.island else {
        return;
    };
    p.acc = (p.acc + dt).min(MS_A_TICK * 6.0);
    if matches!(p.mode, Mode::Title) {
        p.acc = 0.0;
        return;
    }
    // With the book open you still move (the mouse is the book's).
    let busy = p.paused;
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
    let mut tapped = 0;
    if p.touch && !busy {
        k |= p.pad.keys();
        tapped |= p.pad.casts();
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
            p.sounds.cast(sp, None, p.ear);
        }
        let i = Input {
            seq: p.seq,
            yaw: trig::heading(p.aim.0),
            pitch: trig::pitch(p.aim.1),
            keys: k,
            cast: asked | tapped,
            view: p.st.view_tick(kit::now()) as u16,
        };
        p.prev = p.pred.body;
        p.pred.push(i, &island.map);
        if matches!(p.mode, Mode::Practice(_)) {
            p.lessons.tick(&p.prev, &p.pred.body, p.seq, kit::now());
        }
        // Feet leaving the ground, and meeting it again.
        let (was, is) = (p.prev, p.pred.body);
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

/// Too slow for this tier (over 2.5 s of play, frames over 30 ms on
/// average): one tier down, the island built again on it.
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

/// Whether a finger at (`x`, `y`) CSS pixels is on the lesson's panel.
fn on_lesson(p: &Page, x: f64, y: f64) -> bool {
    let (lx, ly) = p.g.to_px(x, y);
    p.lesson_panel
        .is_some_and(|r| lx >= r.x && lx <= r.x + r.w && ly >= r.y && ly <= r.y + r.h)
}
