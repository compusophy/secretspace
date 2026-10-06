//! The picture: a dark field, glowing food, striped snakes eased between
//! the server's frames, and the boards around the edge.

use std::collections::VecDeque;
use std::f64::consts::TAU;

use game::laws::{food_radius, radius, view_scale};
use game::mirror::Seen;
use web_sys::CanvasRenderingContext2d as Ctx;

use crate::state::State;

const SANS: &str = "ui-sans-serif, system-ui, -apple-system, Segoe UI, sans-serif";

fn hsl(h: u8, s: f64, l: f64, a: f64) -> String {
    format!(
        "hsla({:.0},{s:.0}%,{l:.0}%,{a:.3})",
        h as f64 * 360.0 / 256.0
    )
}

/// The point `i` places along a body, between its points.
fn along(body: &VecDeque<(f32, f32)>, i: f32) -> (f32, f32) {
    let n = body.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let i = i.max(0.0);
    let k = (i.floor() as usize).min(n - 1);
    let t = i - k as f32;
    let a = body[k];
    let b = body[(k + 1).min(n - 1)];
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

/// Body points as they should be drawn now: each point slides from where
/// it was toward where the server put it.
fn eased(s: &Seen, alpha: f32) -> Vec<(f32, f32)> {
    let lag = (1.0 - alpha) * s.moved as f32;
    (0..s.body.len())
        .map(|k| along(&s.body, k as f32 + lag))
        .collect()
}

pub struct View {
    pub w: f64,
    pub h: f64,
    pub scale: f64,
    pub cx: f64,
    pub cy: f64,
}

impl View {
    fn at(&self, p: (f32, f32)) -> (f64, f64) {
        (
            (p.0 as f64 - self.cx) * self.scale + self.w / 2.0,
            (p.1 as f64 - self.cy) * self.scale + self.h / 2.0,
        )
    }
    fn sees(&self, p: (f32, f32), pad: f64) -> bool {
        let (x, y) = self.at(p);
        x > -pad && y > -pad && x < self.w + pad && y < self.h + pad
    }
}

pub fn frame(ctx: &Ctx, st: &mut State, w: f64, h: f64, now: f64, steer: Option<f32>) {
    let alpha = st.alpha(now);
    let m = &st.mirror;
    let you = m.snakes.get(&m.you);

    // Where to look: your head, or the snake the server shows.
    let target = match you {
        Some(s) => along(&s.body, (1.0 - alpha) * s.moved as f32),
        None => m
            .snakes
            .values()
            .min_by(|a, b| {
                let d = |s: &Seen| {
                    let p = s.body.front().copied().unwrap_or_default();
                    (p.0 - m.centre.0).powi(2) + (p.1 - m.centre.1).powi(2)
                };
                d(a).total_cmp(&d(b))
            })
            .map(|s| along(&s.body, (1.0 - alpha) * s.moved as f32))
            .unwrap_or(m.centre),
    };
    let r = you.map_or(18.0, |s| radius(s.mass as f32));
    let want_zoom = view_scale(r, w as f32, h as f32);
    if st.zoom == 0.0 {
        st.zoom = want_zoom;
        st.camera = target;
    }
    st.zoom += (want_zoom - st.zoom) * 0.05;
    if you.is_some() {
        st.camera = target;
    } else {
        st.camera.0 += (target.0 - st.camera.0) * 0.12;
        st.camera.1 += (target.1 - st.camera.1) * 0.12;
    }
    let v = View {
        w,
        h,
        scale: st.zoom as f64,
        cx: st.camera.0 as f64,
        cy: st.camera.1 as f64,
    };

    ground(ctx, &v, st.arena);
    food(ctx, st, &v, now);

    // Smaller snakes under bigger ones; yours on top of its size.
    let mut order: Vec<&Seen> = m.snakes.values().collect();
    order.sort_by_key(|s| (s.mass, s.id == m.you));
    for s in order {
        snake(ctx, &v, s, alpha, s.id == m.you, steer, now);
    }

    bursts(ctx, st, &v, now);
    hud(ctx, st, &v, now);
}

/// A burst: a ring flying out and sparks, where a snake ran into someone.
fn bursts(ctx: &Ctx, st: &State, v: &View, now: f64) {
    for b in &st.bursts {
        let t = ((now - b.at) / 700.0).clamp(0.0, 1.0);
        let (x, y) = v.at((b.x, b.y));
        let r = b.r as f64 * v.scale;
        let fade = 1.0 - t;
        ctx.set_stroke_style_str(&hsl(b.hue, 100.0, 70.0, 0.8 * fade));
        ctx.set_line_width(3.0 * fade + 1.0);
        ctx.begin_path();
        let _ = ctx.arc(x, y, r * (1.0 + 5.0 * t), 0.0, TAU);
        ctx.stroke();
        ctx.set_fill_style_str(&hsl(b.hue, 100.0, 75.0, fade));
        for k in 0..12 {
            let a = k as f64 / 12.0 * TAU + b.at % 1.0;
            let d = r * (1.0 + 7.0 * t);
            ctx.begin_path();
            let _ = ctx.arc(
                x + a.cos() * d,
                y + a.sin() * d,
                (r * 0.25 * fade).max(1.0),
                0.0,
                TAU,
            );
            ctx.fill();
        }
    }
}

fn ground(ctx: &Ctx, v: &View, arena: f32) {
    ctx.set_fill_style_str("#070a12");
    ctx.fill_rect(0.0, 0.0, v.w, v.h);

    // A faint grid, so motion reads even on empty ground.
    let step = 64.0;
    let (x0, y0) = (v.cx - v.w / 2.0 / v.scale, v.cy - v.h / 2.0 / v.scale);
    let (x1, y1) = (v.cx + v.w / 2.0 / v.scale, v.cy + v.h / 2.0 / v.scale);
    ctx.set_stroke_style_str("rgba(120,150,255,0.06)");
    ctx.set_line_width(1.0);
    ctx.begin_path();
    let mut x = (x0 / step).floor() * step;
    while x <= x1 {
        let (sx, _) = v.at((x as f32, 0.0));
        ctx.move_to(sx, 0.0);
        ctx.line_to(sx, v.h);
        x += step;
    }
    let mut y = (y0 / step).floor() * step;
    while y <= y1 {
        let (_, sy) = v.at((0.0, y as f32));
        ctx.move_to(0.0, sy);
        ctx.line_to(v.w, sy);
        y += step;
    }
    ctx.stroke();

    // Beyond the edge is death: darken it, and draw the edge glowing.
    let (ox, oy) = v.at((0.0, 0.0));
    let rr = arena as f64 * v.scale;
    ctx.save();
    ctx.begin_path();
    ctx.rect(0.0, 0.0, v.w, v.h);
    let _ = ctx.arc(ox, oy, rr, 0.0, TAU);
    ctx.set_fill_style_str("rgba(40,0,10,0.72)");
    ctx.fill_with_canvas_winding_rule(web_sys::CanvasWindingRule::Evenodd);
    ctx.restore();
    for (wd, a) in [(14.0, 0.08), (6.0, 0.2), (2.0, 0.9)] {
        ctx.set_stroke_style_str(&format!("rgba(255,60,90,{a})"));
        ctx.set_line_width(wd);
        ctx.begin_path();
        let _ = ctx.arc(ox, oy, rr, 0.0, TAU);
        ctx.stroke();
    }
}

fn food(ctx: &Ctx, st: &State, v: &View, now: f64) {
    for (id, p) in &st.mirror.food {
        if !v.sees((p.x, p.y), 20.0) {
            continue;
        }
        let (x, y) = v.at((p.x, p.y));
        let pulse = 1.0 + 0.18 * ((now / 380.0) + (*id % 97) as f64).sin();
        let r = food_radius(p.value) as f64 * v.scale * pulse;
        ctx.set_fill_style_str(&hsl(p.hue, 100.0, 60.0, 0.16));
        ctx.begin_path();
        let _ = ctx.arc(x, y, r * 2.2, 0.0, TAU);
        ctx.fill();
        ctx.set_fill_style_str(&hsl(p.hue, 100.0, 66.0, 0.95));
        ctx.begin_path();
        let _ = ctx.arc(x, y, r, 0.0, TAU);
        ctx.fill();
    }
    // Food on its way into a mouth.
    for g in &st.gulps {
        let Some(s) = st.mirror.snakes.get(&g.by) else {
            continue;
        };
        let head = s.body.front().copied().unwrap_or_default();
        let t = ((now - g.at) / 250.0).clamp(0.0, 1.0) as f32;
        let p = (
            g.pellet.x + (head.0 - g.pellet.x) * t,
            g.pellet.y + (head.1 - g.pellet.y) * t,
        );
        let (x, y) = v.at(p);
        let r = food_radius(g.pellet.value) as f64 * v.scale * (1.0 - t as f64);
        ctx.set_fill_style_str(&hsl(g.pellet.hue, 100.0, 70.0, 1.0 - t as f64 * 0.5));
        ctx.begin_path();
        let _ = ctx.arc(x, y, r.max(0.5), 0.0, TAU);
        ctx.fill();
    }
}

fn snake(ctx: &Ctx, v: &View, s: &Seen, alpha: f32, mine: bool, steer: Option<f32>, now: f64) {
    let pts = eased(s, alpha);
    if pts.is_empty() || !pts.iter().step_by(4).any(|&p| v.sees(p, 80.0)) {
        return;
    }
    let r = radius(s.mass as f32) as f64 * v.scale;
    let screen: Vec<(f64, f64)> = pts.iter().map(|&p| v.at(p)).collect();
    let path = |from: usize, to: usize| {
        ctx.begin_path();
        ctx.move_to(screen[from].0, screen[from].1);
        for p in &screen[from + 1..=to] {
            ctx.line_to(p.0, p.1);
        }
        if from == to {
            ctx.line_to(screen[from].0 + 0.01, screen[from].1);
        }
    };
    let last = screen.len() - 1;
    // A ghost (just arrived) is see-through and shimmers.
    ctx.set_global_alpha(if s.ghost {
        0.35 + 0.2 * (now / 120.0).sin()
    } else {
        1.0
    });
    ctx.set_line_cap("round");
    ctx.set_line_join("round");

    // Boosting: a glow around the whole body.
    if s.boosting {
        let flicker = 0.25 + 0.1 * (now / 60.0).sin();
        ctx.set_stroke_style_str(&hsl(s.hue, 100.0, 65.0, flicker));
        ctx.set_line_width(r * 3.2);
        path(0, last);
        ctx.stroke();
    }
    // Outline, then stripes from the tail up so the head sits on top.
    ctx.set_stroke_style_str(&hsl(s.hue, 70.0, 18.0, 1.0));
    ctx.set_line_width(r * 2.0 + 3.0);
    path(0, last);
    ctx.stroke();
    ctx.set_line_width(r * 2.0);
    const BAND: u32 = 5;
    let mut end = last;
    loop {
        // The band that point `end` belongs to, by its place on the body.
        let n = s.seq.wrapping_sub(end as u32);
        let start = end.saturating_sub((BAND - 1 - n % BAND) as usize);
        let light = (n / BAND).is_multiple_of(2);
        ctx.set_stroke_style_str(&hsl(s.hue, 88.0, if light { 60.0 } else { 50.0 }, 1.0));
        path(start, end.max(start));
        if start > 0 {
            ctx.line_to(screen[start - 1].0, screen[start - 1].1);
        }
        ctx.stroke();
        if start == 0 {
            break;
        }
        end = start - 1;
    }

    // Eyes, looking where it heads (yours: where you point).
    let (hx, hy) = screen[0];
    let look = if mine {
        steer.unwrap_or(s.angle) as f64
    } else {
        s.angle as f64
    };
    let face = s.angle as f64;
    for side in [-1.0, 1.0] {
        let a = face + side * 0.75;
        let (ex, ey) = (hx + a.cos() * r * 0.55, hy + a.sin() * r * 0.55);
        ctx.set_fill_style_str("#ffffff");
        ctx.begin_path();
        let _ = ctx.arc(ex, ey, r * 0.38, 0.0, TAU);
        ctx.fill();
        ctx.set_fill_style_str("#0b0d14");
        ctx.begin_path();
        let _ = ctx.arc(
            ex + look.cos() * r * 0.15,
            ey + look.sin() * r * 0.15,
            r * 0.2,
            0.0,
            TAU,
        );
        ctx.fill();
    }

    ctx.set_global_alpha(1.0);

    // Names over other snakes.
    if !mine {
        ctx.set_font(&format!("600 {:.0}px {SANS}", (11.0 + r * 0.3).min(18.0)));
        ctx.set_text_align("center");
        ctx.set_text_baseline("bottom");
        ctx.set_fill_style_str("rgba(255,255,255,0.7)");
        let _ = ctx.fill_text(&s.name, hx, hy - r - 6.0);
    }
}

fn hud(ctx: &Ctx, st: &State, v: &View, now: f64) {
    let narrow = v.w < 640.0;
    let pad = if narrow { 12.0 } else { 18.0 };

    // You.
    ctx.set_text_align("left");
    ctx.set_text_baseline("top");
    if st.playing() {
        ctx.set_fill_style_str("rgba(255,255,255,0.95)");
        ctx.set_font(&format!("700 {}px {SANS}", if narrow { 20 } else { 26 }));
        let _ = ctx.fill_text(&format!("{}", st.score()), pad, pad);
        ctx.set_font(&format!("12px {SANS}"));
        ctx.set_fill_style_str("rgba(255,255,255,0.55)");
        let rank = if st.board.rank > 0 {
            format!("rank {} of {}", st.board.rank, st.board.snakes)
        } else {
            String::new()
        };
        let _ = ctx.fill_text(
            &format!("length · {rank}"),
            pad,
            pad + if narrow { 26.0 } else { 32.0 },
        );
    }
    ctx.set_font(&format!("12px {SANS}"));
    ctx.set_fill_style_str("rgba(140,255,170,0.75)");
    let people = st.board.people;
    let line = if st.connected {
        format!(
            "● {people} {} playing",
            if people == 1 { "person" } else { "people" }
        )
    } else {
        "○ connecting…".to_string()
    };
    let _ = ctx.fill_text(&line, pad, pad + if narrow { 44.0 } else { 52.0 });

    // "you ate noodle!"
    if let Some(t) = &st.toast {
        let age = now - t.at;
        let a = (1.0 - (age - 1400.0) / 800.0).clamp(0.0, 1.0);
        let lift = (age / 2200.0) * 20.0;
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");
        ctx.set_font(&format!("800 {}px {SANS}", if narrow { 24 } else { 34 }));
        ctx.set_fill_style_str(&format!("rgba(255,224,102,{a:.2})"));
        let _ = ctx.fill_text(&t.text, v.w / 2.0, v.h * 0.3 - lift);
        ctx.set_text_align("left");
        ctx.set_text_baseline("top");
    }

    // The leaderboard.
    let lw = if narrow { 150.0 } else { 200.0 };
    let lx = v.w - pad - lw;
    let rows = if narrow { 5 } else { 10 };
    ctx.set_fill_style_str("rgba(10,14,26,0.55)");
    ctx.fill_rect(
        lx - 10.0,
        pad - 8.0,
        lw + 20.0,
        30.0 + 18.0 * rows.min(st.board.top.len()) as f64,
    );
    ctx.set_font(&format!("700 12px {SANS}"));
    ctx.set_fill_style_str("rgba(255,255,255,0.8)");
    let _ = ctx.fill_text("leaderboard", lx, pad);
    let you_name = st.mirror.snakes.get(&st.mirror.you).map(|s| s.name.clone());
    for (i, l) in st.board.top.iter().take(rows).enumerate() {
        let y = pad + 22.0 + i as f64 * 18.0;
        let me = st.board.rank as usize == i + 1 && you_name.as_deref() == Some(l.name.as_str());
        ctx.set_fill_style_str(&hsl(l.hue, 90.0, 60.0, 1.0));
        ctx.begin_path();
        let _ = ctx.arc(lx + 4.0, y + 7.0, 4.0, 0.0, TAU);
        ctx.fill();
        ctx.set_font(&format!("{}12px {SANS}", if me { "700 " } else { "" }));
        ctx.set_fill_style_str(if me {
            "#ffffff"
        } else {
            "rgba(255,255,255,0.75)"
        });
        ctx.set_text_align("left");
        let name: String = l.name.chars().take(if narrow { 11 } else { 16 }).collect();
        let _ = ctx.fill_text(&format!("{}. {name}", i + 1), lx + 14.0, y);
        ctx.set_text_align("right");
        let _ = ctx.fill_text(&l.score.to_string(), lx + lw, y);
    }

    // The feed: who ate whom.
    ctx.set_text_align("left");
    ctx.set_font(&format!("12px {SANS}"));
    let fy = if narrow {
        v.h - 150.0
    } else {
        v.h - pad - 18.0 * st.feed.len() as f64
    };
    for (i, (at, line)) in st.feed.iter().enumerate() {
        let a = (1.0 - (now - at - 5000.0) / 2000.0).clamp(0.0, 1.0);
        ctx.set_fill_style_str(&format!("rgba(255,210,150,{:.2})", 0.85 * a));
        let _ = ctx.fill_text(line, pad, fy + i as f64 * 18.0);
    }

    // The minimap.
    let mr = if narrow { 44.0 } else { 64.0 };
    let (mx, my) = (v.w - pad - mr, v.h - pad - mr);
    ctx.set_fill_style_str("rgba(10,14,26,0.6)");
    ctx.begin_path();
    let _ = ctx.arc(mx, my, mr, 0.0, TAU);
    ctx.fill();
    ctx.set_stroke_style_str("rgba(255,60,90,0.5)");
    ctx.set_line_width(1.0);
    ctx.stroke();
    for &(x, y, size) in &st.board.dots {
        let (dx, dy) = (mx + x as f64 / 127.0 * mr, my + y as f64 / 127.0 * mr);
        ctx.set_fill_style_str("rgba(255,255,255,0.5)");
        ctx.begin_path();
        let _ = ctx.arc(dx, dy, (size as f64 / 14.0).clamp(1.2, 3.5), 0.0, TAU);
        ctx.fill();
    }
    if st.playing() {
        let (cx, cy) = (
            mx + st.camera.0 as f64 / st.arena as f64 * mr,
            my + st.camera.1 as f64 / st.arena as f64 * mr,
        );
        ctx.set_fill_style_str("#7dffb0");
        ctx.begin_path();
        let _ = ctx.arc(cx, cy, 3.5, 0.0, TAU);
        ctx.fill();
    }
}
