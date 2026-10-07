//! The HUD over the first-person picture, in pixels: the crosshair (and a
//! charge filling round it), your flame and breath, names over the Lumens
//! near you, markers toward the Luciphon, your hearth and every Beacon
//! (held at the screen's edge when they are off it), the phone's stick and
//! buttons, and a flash when you are hit.

use kit::gl::{m4, M4};
use lucilook::palette::{GOLD, INK, RIM};
use luciphon::laws::Laws;
use luciphon::proto::kind;
use pixels::{text_width, Canvas, Rgba};

use crate::controls::{button_at, Button, Controls, BUTTONS};
use crate::laws::Feel;
use crate::state::Thing;

/// Where a world point (x, height, y) lands on the layer, and whether it
/// is in front of you.
pub fn project(vp: &M4, c: &Canvas, p: [f32; 3]) -> Option<(f32, f32)> {
    let (x, y, w) = m4::project(vp, p);
    if w <= 0.05 {
        return None;
    }
    Some((
        (x / w * 0.5 + 0.5) * c.w as f32,
        (0.5 - y / w * 0.5) * c.h as f32,
    ))
}

pub fn crosshair(c: &mut Canvas, u: i32) {
    let (x, y) = (c.w / 2, c.h / 2);
    let k = Rgba(244, 238, 222, 200);
    for (dx, dy, w, h) in [(-5, 0, 3, 1), (3, 0, 3, 1), (0, -5, 1, 3), (0, 3, 1, 3)] {
        c.fill_rect(x + dx * u, y + dy * u, w * u, h * u, k);
    }
}

/// A charge filling round the crosshair: gold in the perfect window,
/// red when held too long.
pub fn charge(c: &mut Canvas, ch: u32, l: &Laws, u: i32) {
    let (x, y) = (c.w as f32 / 2.0, c.h as f32 / 2.0);
    let r = 14.0 * u as f32;
    let k = (ch as f32 / l.charge_max as f32).min(1.0);
    let col = if ch > l.charge_max {
        Rgba(255, 110, 90, 220)
    } else if (l.charge_full..=l.perfect_to).contains(&ch) {
        GOLD
    } else if ch >= l.charge_min {
        Rgba(244, 238, 222, 220)
    } else {
        Rgba(244, 238, 222, 110)
    };
    let n = 40;
    for i in 0..(n as f32 * k) as i32 {
        let a = (i as f32 / n as f32 * 360.0 - 90.0).to_radians();
        c.circle(x + a.cos() * r, y + a.sin() * r, 1.2 * u as f32, col);
    }
}

/// Flame and breath, low in the middle.
pub fn vitals(c: &mut Canvas, flame: f32, breath: f32, u: i32) {
    let w = 110 * u;
    let x = c.w / 2 - w / 2;
    let y = c.h - 18 * u;
    for (k, fill, h, dy) in [
        (flame, Rgba::rgb(255, 160, 80), 4, 0),
        (breath, Rgba::rgb(127, 224, 255), 2, 6),
    ] {
        c.fill_rect(
            x - u,
            y + dy * u - u,
            w + 2 * u,
            (h + 2) * u,
            Rgba(0, 0, 10, 120),
        );
        c.fill_rect(
            x,
            y + dy * u,
            (w as f32 * k.clamp(0.0, 1.0)) as i32,
            h * u,
            fill,
        );
    }
}

/// Names over the Lumens near you, and a sliver of their flame.
pub fn names(c: &mut Canvas, vp: &M4, eye: [f32; 3], things: &[Thing], u: i32) {
    for t in things.iter().filter(|t| t.kind == kind::LUMEN && !t.you) {
        let d = ((t.x - eye[0]).powi(2) + (t.y - eye[2]).powi(2)).sqrt();
        if d > 16.0 || t.name.is_empty() {
            continue;
        }
        let Some((sx, sy)) = project(vp, c, [t.x, t.z + 1.75, t.y]) else {
            continue;
        };
        let a = (255.0 * (1.0 - (d - 10.0).max(0.0) / 6.0)) as u8;
        let w = text_width(&t.name, u);
        c.text_shadowed(
            sx as i32 - w / 2,
            sy as i32 - 8 * u,
            &t.name,
            u,
            Rgba(244, 238, 222, a),
        );
        if d < 9.0 {
            let bw = 24 * u;
            let k = t.flame as f32 / 100.0;
            c.fill_rect(
                sx as i32 - bw / 2,
                sy as i32 + 2 * u,
                bw,
                u,
                Rgba(0, 0, 10, a / 2),
            );
            c.fill_rect(
                sx as i32 - bw / 2,
                sy as i32 + 2 * u,
                (bw as f32 * k) as i32,
                u,
                Rgba(255, 170, 90, a),
            );
        }
    }
}

/// Monsters near you: their names, and their health once hurt.
pub fn beasts(c: &mut Canvas, vp: &M4, eye: [f32; 3], things: &[Thing], u: i32) {
    for t in things.iter().filter(|t| t.kind == kind::BEAST) {
        let d = ((t.x - eye[0]).powi(2) + (t.y - eye[2]).powi(2)).sqrt();
        if d > 14.0 {
            continue;
        }
        let k = (t.state & 7) as usize;
        let Some(b) = luciphon::laws::BEASTS.get(k) else {
            continue;
        };
        let top = [0.95, 1.4, 2.9][k.min(2)];
        let Some((sx, sy)) = project(vp, c, [t.x, top, t.y]) else {
            continue;
        };
        let a = (255.0 * (1.0 - (d - 9.0).max(0.0) / 5.0)) as u8;
        let w = text_width(b.name, u);
        let col = if t.state & 8 != 0 {
            Rgba(255, 110, 80, a)
        } else {
            Rgba(200, 180, 240, a)
        };
        c.text_shadowed(sx as i32 - w / 2, sy as i32 - 8 * u, b.name, u, col);
        if t.flame < 100 {
            let bw = 30 * u;
            let k = t.flame as f32 / 100.0;
            c.fill_rect(
                sx as i32 - bw / 2,
                sy as i32 + 2 * u,
                bw,
                2 * u,
                Rgba(0, 0, 10, a / 2),
            );
            c.fill_rect(
                sx as i32 - bw / 2,
                sy as i32 + 2 * u,
                (bw as f32 * k) as i32,
                2 * u,
                Rgba(220, 70, 90, a),
            );
        }
    }
}

/// A marker toward a world point: where it is if you can see it, else at
/// the screen's edge on its side.
pub fn marker(c: &mut Canvas, vp: &M4, p: [f32; 3], col: Rgba, u: i32) {
    let (x, y, w) = m4::project(vp, p);
    let (cx, cy) = (c.w as f32 / 2.0, c.h as f32 / 2.0);
    let m = 10.0 * u as f32;
    let on = w > 0.05 && (x / w).abs() < 0.95 && (y / w).abs() < 0.95;
    let (sx, sy) = if on {
        (
            (x / w * 0.5 + 0.5) * c.w as f32,
            (0.5 - y / w * 0.5) * c.h as f32,
        )
    } else {
        // Behind you, its side flips.
        let (mut dx, mut dy) = (x, -y);
        if w <= 0.0 {
            dx = -dx;
            dy = -dy;
        }
        let k = ((cx - m) / dx.abs().max(1e-4)).min((cy - m) / dy.abs().max(1e-4));
        (cx + dx * k, cy + dy * k)
    };
    let r = 3.0 * u as f32;
    c.glow_add(
        sx as i32,
        sy as i32,
        (6 * u).max(4),
        Rgba(col.0, col.1, col.2, 90),
    );
    c.circle(sx, sy - r, r * 0.6, col);
    c.line(sx - r, sy - r * 0.2, sx, sy + r, u as f32, col);
    c.line(sx + r, sy - r * 0.2, sx, sy + r, u as f32, col);
}

/// The phone's stick and buttons.
pub fn touch(c: &mut Canvas, ctl: &Controls, css: (f64, f64), scale: f64, f: &Feel, u: i32) {
    let s = scale as f32;
    if let Some((ox, oy, x, y)) = ctl.stick() {
        let (ox, oy) = (ox as f32 / s, oy as f32 / s);
        c.ring(ox, oy, f.stick_px as f32 / s, 1.5, Rgba(255, 255, 255, 60));
        let (dx, dy) = (x as f32 / s - ox, y as f32 / s - oy);
        let d = (dx * dx + dy * dy).sqrt();
        let max = f.stick_px as f32 / s;
        let k = if d > max { max / d } else { 1.0 };
        c.circle(
            ox + dx * k,
            oy + dy * k,
            9.0 * u as f32,
            Rgba(255, 255, 255, 110),
        );
    } else {
        let y = c.h - 30 * u;
        c.text_shadowed(10 * u, y, "move", u, Rgba(244, 238, 222, 70));
    }
    for b in BUTTONS {
        let (x, y, r) = button_at(b, css, f);
        let (x, y, r) = (x as f32 / s, y as f32 / s, r as f32 / s);
        let held = ctl.pressed(b);
        let fill = if held {
            Rgba(255, 210, 122, 120)
        } else {
            Rgba(11, 13, 26, 110)
        };
        c.circle(x, y, r, fill);
        c.ring(
            x,
            y,
            r,
            1.5,
            Rgba(255, 210, 122, if held { 230 } else { 120 }),
        );
        let label = match b {
            Button::Jump => "jump",
            Button::Dash => "dash",
            Button::Throw => "throw",
            Button::Heart => "heart",
            Button::Bag => "bag",
        };
        let w = text_width(label, u);
        c.text(
            x as i32 - w / 2,
            y as i32 - 3 * u,
            label,
            u,
            if held { GOLD } else { INK },
        );
    }
}

/// A red flash at the edges, when you are hit.
pub fn flash(c: &mut Canvas, col: Rgba) {
    let band = (c.w.min(c.h) / 6).max(4);
    for k in 0..band {
        let a = (col.3 as i32 * (band - k) / band) as u8;
        let edge = Rgba(col.0, col.1, col.2, a);
        c.fill_rect(k, k, c.w - 2 * k, 1, edge);
        c.fill_rect(k, c.h - 1 - k, c.w - 2 * k, 1, edge);
        c.fill_rect(k, k + 1, 1, c.h - 2 * k - 2, edge);
        c.fill_rect(c.w - 1 - k, k + 1, 1, c.h - 2 * k - 2, edge);
    }
    let _ = RIM;
}
