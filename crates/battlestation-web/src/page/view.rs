//! A frame: the screen's picture, the camera from the chair, what moves
//! and glows, the lights; the engine draws the room, the glass the
//! picture on the monitor, and the pixel layer what is said over it.

use battlestation::laws::{
    BREATH, BREATH_HZ, EYE, FOLLOW_PITCH, FOLLOW_YAW, HEAD_OMEGA, LEAN, LEAN_OMEGA, PITCH,
    SCREEN_AT, SCREEN_PX,
};
use battlestation::{damp, V3};
use pixels::Rgba;
use render::geo;
use render::Camera;

use super::input::clock;
use super::Page;
use crate::SHOT_AFTER;

/// Where you look from: the chair, leaned in as far as the wheel says,
/// the head turned a little toward the arrow, breathing.
fn camera(p: &mut Page, t: f32, dt: f32) -> Camera {
    let (w, h) = SCREEN_PX;
    let (cx, cy) = p.screen.cursor;
    let yaw_to = (cx / w as f32 - 0.5) * 2.0 * FOLLOW_YAW;
    let pitch_to = -(cy / h as f32 - 0.5) * 2.0 * FOLLOW_PITCH;
    let [yaw, yv, pitch, pv] = &mut p.head;
    damp(yaw, yv, yaw_to, HEAD_OMEGA, dt);
    damp(pitch, pv, pitch_to, HEAD_OMEGA, dt);
    let (lean, lv) = &mut p.lean;
    damp(lean, lv, p.lean_to, LEAN_OMEGA, dt);
    let lean = p.lean.0.clamp(0.0, 1.0);
    let to = geo::sub(SCREEN_AT, EYE);
    let mut eye: V3 = geo::add(EYE, geo::scale(geo::norm(to), LEAN * lean));
    eye[1] += BREATH * (t * BREATH_HZ * std::f32::consts::TAU).sin();
    let d = geo::sub(SCREEN_AT, eye);
    let at_screen = d[1].atan2((d[0] * d[0] + d[2] * d[2]).sqrt());
    let (css_w, css_h) = p.out.css();
    let aspect = (css_w / css_h.max(1.0)) as f32;
    // A narrow screen widens the view to keep the desk in it.
    let fov = look::desk::fov(aspect);
    if let Some([x, y, z, yaw, pitch]) = p.hooks.eye {
        return Camera {
            eye: [x, y, z],
            yaw: yaw.to_radians() - std::f32::consts::FRAC_PI_2,
            pitch: pitch.to_radians(),
            fov: 0.7,
            aspect,
        };
    }
    Camera {
        eye,
        yaw: -std::f32::consts::FRAC_PI_2 + p.head[0],
        pitch: PITCH + (at_screen - PITCH) * lean + p.head[2],
        fov,
        aspect,
    }
}

pub(super) fn frame(p: &mut Page, now: f64, dt: f32) {
    let t = (now / 1000.0) as f32;
    p.hands.step(dt);
    for h in &mut p.heat {
        *h *= (-dt / 0.35).exp();
    }
    let on_os = p.on_os();
    if p.booting() {
        p.screen.boot("compusophyOS is starting...", t);
    } else if !on_os {
        p.screen.draw(&p.term, &clock(), t, dt);
    }
    let cam = camera(p, t, dt);
    hud(p, now);
    let size = p.out.size();
    let Some(mut target) = p.out.begin() else {
        p.out.idle();
        return;
    };
    if let (true, Some(o)) = (on_os, p.os.as_mut()) {
        o.copy(p.out.device(), p.out.queue(), &mut p.desk.glass);
    }
    let arrow =
        p.os.as_ref()
            .filter(|_| on_os)
            .map(|o| o.arrow(p.screen.cursor));
    let (view, encoder) = target.parts();
    p.desk.draw(
        &mut p.r,
        p.out.queue(),
        (encoder, view, size),
        cam,
        (&p.hands, &p.heat),
        (&p.screen, arrow),
        t,
    );
    p.out.finish(target);
    p.frames += 1;
    let doc = kit::document();
    if p.hooks.shot && p.frames == SHOT_AFTER {
        doc.set_title("shot ready");
    } else if p.hooks.perf && p.frames.is_multiple_of(10) {
        let s = p.r.stats;
        doc.set_title(&format!(
            "battlestation {:.0}fps draws {} tris {}k at {:.0},{:.0} os {}",
            p.fps,
            s.draws,
            s.triangles / 1000,
            p.screen.cursor.0,
            p.screen.cursor.1,
            match (&p.os, p.os_said.as_str()) {
                (Some(_), _) => "on",
                (None, "") => "starting",
                (None, why) => why,
            }
        ));
    }
}

/// What is said over the picture: a hint at first, a word if the browser
/// left the page unisolated (the computer's programs cannot run), the
/// shared menu.
fn hud(p: &mut Page, now: f64) {
    let ui = p.out.ui();
    let unisolated = !p.isolated && p.on_os();
    let (open, touch, hint_until) = (p.meta.is_open(), p.touch, p.hint_until);
    let hud = p.out.hud();
    hud.wipe();
    let w = hud.w;
    if !open {
        let mut said = Vec::new();
        if unisolated {
            let text = "this browser did not isolate the page, so the computer's programs cannot run: reload it";
            said.push((
                pixels::wrap(text, w - 24 * ui, ui),
                Rgba(255, 196, 120, 230),
            ));
        }
        if now < hint_until {
            let fade = ((hint_until - now) / 1500.0).clamp(0.0, 1.0);
            let text = if touch {
                "drag to move the mouse - tap to click and type"
            } else {
                "your mouse and keys are the desk's - shift+wheel leans in - esc for the menu"
            };
            said.push((
                pixels::wrap(text, w - 24 * ui, ui),
                Rgba(244, 241, 255, (fade * 170.0) as u8),
            ));
        }
        let mut y = 8 * ui;
        for (lines, ink) in said {
            for l in &lines {
                hud.text_centred(w / 2, y, l, ui, ink);
                y += 10 * ui;
            }
            y += 4 * ui;
        }
    }
    if p.meta.is_open() {
        let labels = p.items();
        let labels: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        let help = vec![
            "your keys are the desk's keys".to_string(),
            "shift+wheel leans in to read".to_string(),
        ];
        let s = p.out.scale();
        let hud = p.out.hud();
        p.meta.draw(hud, ui, &labels, &help, now, |r| {
            (
                r.x as f64 * s,
                r.y as f64 * s,
                r.w as f64 * s,
                r.h as f64 * s,
            )
        });
    }
}
