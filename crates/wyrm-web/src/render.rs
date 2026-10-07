//! The picture: the world as `look` draws it, around your snake (or the
//! one the server shows), and the boards around the edge.

use look::{hue, View};
use pixels::{text_width, Canvas, Rect, Rgba};
use wyrm::laws::{radius, view_scale};

use crate::state::State;

pub const INK: Rgba = Rgba::rgb(244, 241, 255);
pub const DIM: Rgba = Rgba(244, 241, 255, 150);
pub const GO: Rgba = Rgba::rgb(87, 227, 137);

/// Everything the HUD needs that is not the game's own state.
pub struct Hud {
    /// Text scale.
    pub u: i32,
    /// Where the steering is aimed, for your snake's eyes.
    pub steer: Option<f32>,
    /// The phone's boost button, if there is one, and whether it is held.
    pub boost: Option<(Rect, bool)>,
}

pub fn frame(c: &mut Canvas, st: &mut State, css: (f64, f64), scale: f64, now: f64, hud: &Hud) {
    let alpha = st.alpha(now);
    let m = &st.mirror;
    let you = m.snakes.get(&m.you);

    // Where to look: your head, or near what the server centres on.
    let target = match you {
        Some(s) => look::head(s, alpha),
        None => look::watched(m, alpha),
    };
    let r = you.map_or(18.0, |s| radius(s.mass as f32));
    let want = view_scale(r, css.0 as f32, css.1 as f32) / scale as f32;
    if st.zoom == 0.0 {
        st.zoom = want;
        st.camera = target;
    }
    st.zoom += (want - st.zoom) * 0.05;
    if you.is_some() {
        st.camera = target;
    } else {
        st.camera.0 += (target.0 - st.camera.0) * 0.12;
        st.camera.1 += (target.1 - st.camera.1) * 0.12;
    }
    let v = View {
        w: c.w as f32,
        h: c.h as f32,
        k: st.zoom,
        cx: st.camera.0,
        cy: st.camera.1,
    };
    let scene = look::Scene {
        mirror: &st.mirror,
        alpha,
        arena: st.arena,
        gulps: &st.gulps,
        bursts: &st.bursts,
        steer: hud.steer,
        names: Some(hud.u),
    };
    look::world(c, &v, &scene, now);
    board(c, st, hud, now);
}

fn board(c: &mut Canvas, st: &State, hud: &Hud, now: f64) {
    let u = hud.u;
    let uf = u as f32;
    let (w, h) = (c.w, c.h);
    let pad = (8.0 * uf) as i32;
    let narrow = (w as f32 / uf) < 420.0;

    // You.
    if st.playing() {
        c.text_shadowed(pad, pad, &st.score().to_string(), 3 * u, INK);
        let rank = match (st.board.rank, narrow) {
            (0, _) => "length".to_string(),
            (r, true) => format!("#{r} of {}", st.board.snakes),
            (r, false) => format!("length   rank {r} of {}", st.board.snakes),
        };
        c.text_shadowed(pad, pad + 25 * u, &rank, u, DIM);
    }
    let people = st.board.people;
    let line = if st.connected {
        format!(
            "{people} {} here",
            if people == 1 { "person" } else { "people" }
        )
    } else {
        "connecting...".to_string()
    };
    let ly = pad + 37 * u;
    c.circle(
        pad as f32 + 2.5 * uf,
        ly as f32 + 3.5 * uf,
        2.5 * uf,
        if st.connected { GO } else { DIM },
    );
    c.text_shadowed(pad + 9 * u, ly, &line, u, Rgba(141, 255, 177, 210));

    // The leaderboard.
    // On a phone it is the top few, narrow, so it never covers the left.
    let (rows, lw, chars, head) = if narrow {
        (3, 78 * u, 6, "TOP")
    } else {
        (10, 118 * u, 11, "LEADERBOARD")
    };
    let lx = w - pad - lw;
    let n = rows.min(st.board.top.len());
    c.round_rect(
        Rect::new(
            (lx - 6 * u) as f32,
            (pad - 5 * u) as f32,
            (lw + 12 * u) as f32,
            (17 + 11 * n as i32) as f32 * uf,
        ),
        5.0 * uf,
        Rgba(10, 14, 26, 150),
    );
    c.text(lx, pad, head, u, Rgba(244, 241, 255, 200));
    let you_name = st
        .mirror
        .snakes
        .get(&st.mirror.you)
        .map(|s| s.name.as_str());
    for (i, l) in st.board.top.iter().take(rows).enumerate() {
        let y = pad + (13 + 11 * i as i32) * u;
        let me = st.board.rank as usize == i + 1 && you_name == Some(l.name.as_str());
        c.circle(
            lx as f32 + 2.5 * uf,
            y as f32 + 3.5 * uf,
            2.5 * uf,
            Rgba::hsl(hue(l.hue), 0.9, 0.6),
        );
        let name: String = l.name.chars().take(chars).collect();
        let ink = if me { INK } else { Rgba(244, 241, 255, 190) };
        c.text(lx + 8 * u, y, &format!("{}.{name}", i + 1), u, ink);
        let sc = l.score.to_string();
        c.text(lx + lw - text_width(&sc, u), y, &sc, u, ink);
    }

    // The feed: who ate whom.
    let feed_y =
        h - pad - (11 * st.feed.len() as i32 + if hud.boost.is_some() { 80 } else { 0 }) * u;
    for (i, (at, line)) in st.feed.iter().enumerate() {
        let a = (1.0 - (now - at - 5000.0) / 2000.0).clamp(0.0, 1.0) as f32;
        c.text_shadowed(
            pad,
            feed_y + 11 * i as i32 * u,
            line,
            u,
            Rgba(255, 210, 150, 220).fade(a),
        );
    }

    // The minimap.
    let mr = if narrow { 30.0 } else { 44.0 } * uf;
    let (mx, my) = (w as f32 - pad as f32 - mr, h as f32 - pad as f32 - mr);
    c.circle(mx, my, mr, Rgba(10, 14, 26, 170));
    c.ring(mx, my, mr, 1.0, Rgba(255, 60, 90, 130));
    for &(x, y, size) in &st.board.dots {
        let (dx, dy) = (mx + x as f32 / 127.0 * mr, my + y as f32 / 127.0 * mr);
        c.circle(
            dx,
            dy,
            (size as f32 / 16.0).clamp(0.8, 2.6) * uf,
            Rgba(255, 255, 255, 140),
        );
    }
    if st.playing() {
        let (cx, cy) = (
            mx + st.camera.0 / st.arena * mr,
            my + st.camera.1 / st.arena * mr,
        );
        c.circle(cx, cy, 2.6 * uf, Rgba::rgb(125, 255, 176));
    }

    // The phone's boost button.
    if let (Some((b, held)), true) = (hud.boost, st.playing()) {
        let (cx, cy, r) = (b.x + b.w / 2.0, b.y + b.h / 2.0, b.w / 2.0);
        c.circle(
            cx,
            cy,
            r,
            if held {
                Rgba(87, 227, 137, 90)
            } else {
                Rgba(255, 255, 255, 25)
            },
        );
        c.ring(cx, cy, r, 2.0, Rgba(255, 255, 255, 110));
        let tw = text_width("BOOST", u);
        c.text(cx as i32 - tw / 2, cy as i32 - 3 * u, "BOOST", u, INK);
    }

    // "you ate noodle!"
    if let Some(t) = &st.toast {
        let age = now - t.at;
        let a = (1.0 - (age - 1400.0) / 800.0).clamp(0.0, 1.0) as f32;
        let lift = (age / 2200.0 * 12.0) as i32 * u;
        let s = if narrow { 2 * u } else { 3 * u };
        c.text_centred(
            w / 2,
            h * 3 / 10 - lift,
            &t.text,
            s,
            Rgba(255, 224, 102, 255).fade(a),
        );
    }
}
