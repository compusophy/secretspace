//! A frame: the screen's picture, the camera from the chair, what moves
//! and glows, the lights; the engine draws the room, the glass the
//! picture on the monitor, and the pixel layer what is said over it.

use battlestation::keys::{self, Side};
use battlestation::laws::{
    BREATH, BREATH_HZ, EYE, FOLLOW_PITCH, FOLLOW_YAW, FOV, FOV_MOST, HEAD_OMEGA, LEAN, LEAN_OMEGA,
    PITCH, SCREEN_AT, SCREEN_PX, WALL_N,
};
use battlestation::{damp, V3};
use pixels::Rgba;
use render::geo;
use render::{m4, Camera, Frame, Item, Pass};

use super::input::clock;
use super::{ident, Page};
use crate::{body, gear, light, scene, SHOT_AFTER};

const INK: Rgba = Rgba::rgb(244, 241, 255);
const GREEN: Rgba = Rgba::rgb(126, 232, 166);

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
    let fov = (2.0 * ((0.55f32).tan() / aspect.max(0.1)).atan()).clamp(FOV, FOV_MOST);
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

/// What moves and what glows this frame.
fn items(p: &Page, t: f32) -> Vec<Item> {
    let mut out = Vec::with_capacity(280);
    for (i, k) in keys::layout().iter().enumerate() {
        let at = gear::cap_at(k);
        let hue = light::rainbow(at[0], t);
        let heat = p.heat[i];
        let tint = geo::mix(geo::mix([1.0; 3], hue, 0.8), [1.0; 3], heat * 0.8);
        let down = [at[0], at[1] - p.hands.depth(i), at[2]];
        out.push(
            Item::new(p.m.keys[i], m4::place(down, 0.0, [1.0; 3]))
                .tint(tint, 1.0)
                .glow(heat * 0.25)
                .rough(0.6),
        );
        out.push(
            Item::new(
                p.m.under,
                m4::place([at[0], at[1] - 0.004, at[2]], 0.0, [k.w, 1.0, 1.0]),
            )
            .pass(Pass::Glow)
            .tint(geo::mix(hue, [1.0; 3], heat * 0.5), 1.0)
            .glow(0.2 + heat * 2.2),
        );
    }
    let m = p.hands.mouse.at();
    out.push(
        Item::new(p.m.mouse, m4::place(m, 0.0, [1.0; 3]))
            .tint(geo::mix([1.0; 3], light::rainbow(m[0], t), 0.9), 1.0)
            .rough(0.45),
    );
    for side in [Side::Left, Side::Right] {
        body::items(&p.hands.pose(side), side, &p.m.hands, &mut out);
    }
    let [x, y, z] = gear::screen_axes();
    let mean = p.screen.mean();
    out.push(
        Item::new(p.m.pane, m4::basis(SCREEN_AT, x, y, z))
            .tint(geo::add(mean, [0.04, 0.05, 0.08]), 1.0)
            .glow(1.3),
    );
    for (k, c) in scene::FANS.iter().enumerate() {
        out.push(
            Item::new(p.m.fan, m4::place(*c, 0.0, [1.0; 3]))
                .pass(Pass::Glow)
                .tint(light::rainbow(0.5 + k as f32 * 0.12, t), 1.0)
                .glow(2.2),
        );
    }
    let (x0, x1, sy) = scene::STRIP;
    let len = (x1 - x0) / scene::STRIP_PARTS as f32;
    for k in 0..scene::STRIP_PARTS {
        let sx = x0 + k as f32 * len;
        out.push(
            Item::new(
                p.m.strip,
                m4::place([sx, sy, WALL_N + 0.003], 0.0, [len, 1.0, 1.0]),
            )
            .pass(Pass::Glow)
            .tint(light::rainbow(sx + len / 2.0, t), 1.0)
            .glow(2.6),
        );
    }
    out.push(
        Item::new(p.m.tower, ident())
            .pass(Pass::Glow)
            .tint(light::rainbow(0.6, t), 1.0)
            .glow(2.4),
    );
    out.push(
        Item::new(p.m.neon, m4::place(scene::NEON_AT, 0.0, [1.0; 3]))
            .pass(Pass::Glow)
            .tint(light::NEON, 1.0)
            .glow(light::neon_glow(t)),
    );
    out
}

pub(super) fn frame(p: &mut Page, now: f64, dt: f32) {
    let t = (now / 1000.0) as f32;
    p.hands.step(dt);
    for h in &mut p.heat {
        *h *= (-dt / 0.35).exp();
    }
    p.screen.draw(&p.term, &clock(), t, dt);
    let cam = camera(p, t, dt);
    let items = items(p, t);
    let lights = light::lights(t, p.screen.mean());
    let sparks = light::steam(t);
    let frame = Frame {
        cam,
        look: light::look(),
        time: t,
        items: &items,
        lights: &lights,
        sparks: &sparks,
        decals: &[],
        view_fov: 0.9,
    };
    hud(p, now);
    let size = p.out.size();
    let Some(mut target) = p.out.begin() else {
        p.out.idle();
        return;
    };
    let (view, encoder) = target.parts();
    p.r.draw(encoder, view, size, &frame);
    let (vp, _, _) = cam.matrices(cam.fov);
    p.glass
        .draw(p.out.queue(), encoder, view, &vp, &p.screen.c.data, 1.0);
    p.out.finish(target);
    p.frames += 1;
    let doc = kit::document();
    if p.hooks.shot && p.frames == SHOT_AFTER {
        doc.set_title("shot ready");
    } else if p.hooks.perf && p.frames.is_multiple_of(30) {
        let s = p.r.stats;
        doc.set_title(&format!(
            "battlestation {:.0}fps draws {} tris {}k",
            p.fps,
            s.draws,
            s.triangles / 1000
        ));
    }
}

/// What is said over the picture: how to sit down, a hint once seated,
/// the shared menu.
fn hud(p: &mut Page, now: f64) {
    let ui = p.out.ui();
    let hud = p.out.hud();
    hud.wipe();
    let (w, h) = (hud.w, hud.h);
    if !p.seated && !p.meta.is_open() {
        // A slim bar along the bottom, clear of the hands.
        let go = if p.touch {
            "tap to sit down"
        } else {
            "click to sit down"
        };
        let line = format!("battlestation - {go}");
        let k = pixels::fit_scale(&line, w - 24 * ui, 2 * ui);
        let bar = 8 * k + 10 * ui;
        hud.fill_rect(0, h - bar, w, bar, Rgba(6, 7, 14, 175));
        let tw = pixels::text_width(&line, k);
        let x = (w - tw) / 2;
        let y = h - bar + 5 * ui;
        let lead = pixels::text_width("battlestation - ", k);
        hud.text(x, y, "battlestation - ", k, INK);
        if (now / 600.0) as i64 % 2 == 0 {
            hud.text(x + lead, y, go, k, GREEN);
        }
    } else if p.seated && now < p.hint_until && !p.meta.is_open() {
        let fade = ((p.hint_until - now) / 1500.0).clamp(0.0, 1.0);
        let text = if p.touch {
            "drag to move the mouse - tap to type"
        } else {
            "type anything - move the mouse - the wheel leans in - esc for the menu"
        };
        let lines = pixels::wrap(text, w - 24 * ui, ui);
        let mut y = h - 14 * ui - lines.len() as i32 * 10 * ui;
        for l in &lines {
            hud.text_centred(w / 2, y, l, ui, Rgba(244, 241, 255, (fade * 170.0) as u8));
            y += 10 * ui;
        }
    }
    if p.meta.is_open() {
        let labels = p.items();
        let labels: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        let help = vec![
            "your keys are the desk's keys".to_string(),
            "the wheel leans in to read".to_string(),
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
