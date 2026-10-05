//! The picture: the island's light as one soft field, its motes as points
//! of colour (one hue per lineage), its four edges as portals, and around
//! it what this tab knows of the world beyond.

use std::f64::consts::TAU;

use space::laws::{H, LIGHT_CAP, SUN_MAX, W};
use space::net::Portal;
use wasm_bindgen::Clamped;
use web_sys::{CanvasRenderingContext2d as Ctx, HtmlCanvasElement, ImageData};

use crate::app::{hue, App, TICK_MS};
use crate::place::clock;

/// Where the island sits on the screen.
#[derive(Clone, Copy)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub cell: f64,
}

impl Frame {
    pub fn w(&self) -> f64 {
        self.cell * W as f64
    }
    pub fn h(&self) -> f64 {
        self.cell * H as f64
    }
    /// Screen point of a cell-space point.
    pub fn at(&self, cx: f64, cy: f64) -> (f64, f64) {
        (self.x + cx * self.cell, self.y + cy * self.cell)
    }
}

/// Fit the island in the window, leaving room for the portals' labels and
/// the text around it.
pub fn layout(w: f64, h: f64) -> Frame {
    let narrow = w < 700.0;
    let (side, top, bottom) = if narrow {
        (14.0, 96.0, 190.0)
    } else {
        (110.0, 92.0, 150.0)
    };
    let cell = ((w - 2.0 * side) / W as f64)
        .min((h - top - bottom) / H as f64)
        .max(4.0)
        .floor();
    let (iw, ih) = (cell * W as f64, cell * H as f64);
    Frame {
        x: ((w - iw) / 2.0).floor(),
        y: (top + ((h - top - bottom - ih) / 2.0).max(0.0)).floor(),
        cell,
    }
}

fn hsl(h: f64, s: f64, l: f64, a: f64) -> String {
    format!("hsla({h:.0},{s:.0}%,{l:.0}%,{a:.3})")
}

fn mix(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// The light field: one pixel a cell, smoothed as it is scaled up.
fn paint_field(field: &HtmlCanvasElement, app: &App) {
    let Ok(Some(fctx)) = field.get_context("2d") else {
        return;
    };
    let Ok(fctx) = wasm_bindgen::JsCast::dyn_into::<Ctx>(fctx) else {
        return;
    };
    let sun = app.island.sun as f64 / SUN_MAX as f64;
    let mut px = vec![0u8; W * H * 4];
    for (i, &l) in app.island.light().iter().enumerate() {
        let t = (l as f64 / LIGHT_CAP as f64).powf(0.85);
        let glow = t * (0.18 + 0.62 * sun);
        // Night is blue; light is amber.
        let (br, bg, bb) = (
            mix(10.0, 16.0, sun),
            mix(14.0, 13.0, sun),
            mix(30.0, 14.0, sun),
        );
        px[i * 4] = mix(br, 255.0, glow) as u8;
        px[i * 4 + 1] = mix(bg, 160.0, glow * 0.92) as u8;
        px[i * 4 + 2] = mix(bb, 64.0, glow * 0.8) as u8;
        px[i * 4 + 3] = 255;
    }
    if let Ok(img) = ImageData::new_with_u8_clamped_array_and_sh(Clamped(&px), W as u32, H as u32) {
        let _ = fctx.put_image_data(&img, 0.0, 0.0);
    }
}

fn text(ctx: &Ctx, s: &str, x: f64, y: f64) {
    let _ = ctx.fill_text(s, x, y);
}

const SANS: &str = "ui-sans-serif, system-ui, -apple-system, Segoe UI, sans-serif";
const MONO: &str = "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace";

pub fn frame(ctx: &Ctx, field: &HtmlCanvasElement, app: &App, w: f64, h: f64, now: f64) -> Frame {
    let f = layout(w, h);
    ctx.set_fill_style_str("#07090d");
    ctx.fill_rect(0.0, 0.0, w, h);

    // The land.
    paint_field(field, app);
    ctx.set_image_smoothing_enabled(true);
    ctx.save();
    ctx.begin_path();
    ctx.rect(f.x, f.y, f.w(), f.h());
    ctx.clip();
    ctx.set_filter(&format!("blur({:.1}px)", f.cell * 0.55));
    let pad = f.cell;
    let _ = ctx.draw_image_with_html_canvas_element_and_dw_and_dh(
        field,
        f.x - pad,
        f.y - pad,
        f.w() + 2.0 * pad,
        f.h() + 2.0 * pad,
    );
    ctx.restore();

    portals(ctx, app, f, now);

    // The motes, eased between their last two cells.
    let k = ((now - app.last_tick) / TICK_MS).clamp(0.0, 1.0);
    let ease = k * k * (3.0 - 2.0 * k);
    let r = (f.cell * 0.3).max(1.6);
    let sun = app.island.sun as f64 / SUN_MAX as f64;
    for m in app.island.motes() {
        let (cx, cy) = match app.prev.get(&m.id) {
            Some(&(px, py)) => (
                mix(px as f64, m.x as f64, ease),
                mix(py as f64, m.y as f64, ease),
            ),
            None => (m.x as f64, m.y as f64),
        };
        let (x, y) = f.at(cx + 0.5, cy + 0.5);
        let hu = hue(m.lineage);
        let rich = ((m.balance as f64).sqrt() / 30.0).min(1.0);
        ctx.set_fill_style_str(&hsl(hu, 90.0, 60.0, 0.07 + 0.09 * sun));
        ctx.begin_path();
        let _ = ctx.arc(x, y, r * (1.5 + 0.8 * rich), 0.0, TAU);
        ctx.fill();
        ctx.set_fill_style_str(&hsl(hu, 85.0, mix(48.0, 72.0, rich), 1.0));
        ctx.begin_path();
        let _ = ctx.arc(x, y, r * (0.75 + 0.45 * rich), 0.0, TAU);
        ctx.fill();
        if app.mine.contains(&m.lineage) {
            ctx.set_stroke_style_str("rgba(255,255,255,0.85)");
            ctx.set_line_width(1.0);
            ctx.begin_path();
            let _ = ctx.arc(x, y, r * 1.6, 0.0, TAU);
            ctx.stroke();
        }
        if app.selected == Some(m.id) {
            ctx.set_stroke_style_str("#ffffff");
            ctx.set_line_width(1.5);
            ctx.begin_path();
            let _ = ctx.arc(x, y, r * 2.6 + 2.0 * (now / 300.0).sin().abs(), 0.0, TAU);
            ctx.stroke();
        }
    }

    // Crossings: leaving flies out, arriving ripples in.
    for fl in &app.flyers {
        let t = ((now - fl.born) / 900.0).clamp(0.0, 1.0);
        let d = t * 3.0;
        let (dx, dy) = match fl.side {
            0 => (0.0, -d),
            1 => (d, 0.0),
            2 => (0.0, d),
            _ => (-d, 0.0),
        };
        let (x, y) = f.at(fl.x + dx, fl.y + dy);
        ctx.set_fill_style_str(&hsl(fl.hue, 90.0, 70.0, 1.0 - t));
        ctx.begin_path();
        let _ = ctx.arc(x, y, r * (1.0 - 0.5 * t), 0.0, TAU);
        ctx.fill();
    }
    for rp in &app.ripples {
        let t = ((now - rp.born) / 1200.0).clamp(0.0, 1.0);
        let (x, y) = f.at(rp.x, rp.y);
        ctx.set_stroke_style_str(&hsl(rp.hue, 90.0, 72.0, 0.6 * (1.0 - t)));
        ctx.set_line_width(1.2);
        ctx.begin_path();
        let _ = ctx.arc(x, y, f.cell * (0.4 + 1.8 * t), 0.0, TAU);
        ctx.stroke();
    }

    hud(ctx, app, f, w, h, now);
    f
}

fn portals(ctx: &Ctx, app: &App, f: Frame, now: f64) {
    let (x0, y0, x1, y1) = (f.x, f.y, f.x + f.w(), f.y + f.h());
    let narrow = f.w() < 600.0;
    for side in 0..4usize {
        let (ax, ay, bx, by) = match side {
            0 => (x0, y0, x1, y0),
            1 => (x1, y0, x1, y1),
            2 => (x0, y1, x1, y1),
            _ => (x0, y0, x0, y1),
        };
        let open = app.net.beyond(side);
        let (style, width) = match (app.net.portals[side], &open) {
            (Portal::Open { .. }, Some((_, p))) => {
                let s = p.hello.sun as f64 / 100.0;
                (
                    format!(
                        "rgba(255,{:.0},{:.0},{:.2})",
                        mix(120.0, 190.0, s),
                        mix(160.0, 90.0, s),
                        0.45 + 0.5 * s
                    ),
                    3.0,
                )
            }
            (Portal::Asking { .. }, _) => (
                format!(
                    "rgba(255,255,255,{:.2})",
                    0.15 + 0.15 * (now / 250.0).sin().abs()
                ),
                1.5,
            ),
            _ => ("rgba(255,255,255,0.10)".to_string(), 1.0),
        };
        ctx.set_stroke_style_str(&style);
        ctx.set_line_width(width);
        ctx.begin_path();
        ctx.move_to(ax, ay);
        ctx.line_to(bx, by);
        ctx.stroke();

        // Who is beyond, and what time it is there.
        let label = match &open {
            Some((_, p)) => format!(
                "{} · {} · sun {}",
                p.hello.place,
                clock(p.hello.tz_min),
                p.hello.sun
            ),
            None if app.net.asleep => "shut for the night".to_string(),
            None => "no one yet".to_string(),
        };
        let dim = if open.is_some() {
            "rgba(255,226,180,0.92)"
        } else {
            "rgba(255,255,255,0.28)"
        };
        ctx.set_fill_style_str(dim);
        ctx.set_font(&format!("12px {SANS}"));
        let arrow = ["↑", "→", "↓", "←"][side];
        let s = format!("{arrow} {label}");
        match side {
            0 => {
                ctx.set_text_align("center");
                ctx.set_text_baseline("bottom");
                text(ctx, &s, (x0 + x1) / 2.0, y0 - 8.0);
            }
            2 => {
                ctx.set_text_align("center");
                ctx.set_text_baseline("top");
                text(ctx, &s, (x0 + x1) / 2.0, y1 + 8.0);
            }
            _ => {
                // Side labels run along the edge.
                ctx.save();
                let (x, rot) = if side == 1 {
                    (x1 + 10.0, TAU / 4.0)
                } else {
                    (x0 - 10.0, -TAU / 4.0)
                };
                let _ = ctx.translate(x, (y0 + y1) / 2.0);
                let _ = ctx.rotate(rot);
                ctx.set_text_align("center");
                ctx.set_text_baseline(if narrow { "middle" } else { "bottom" });
                text(ctx, &s, 0.0, 0.0);
                ctx.restore();
            }
        }
    }
}

fn hud(ctx: &Ctx, app: &App, f: Frame, w: f64, h: f64, now: f64) {
    let isl = &app.island;
    let narrow = w < 700.0;
    let pad = if narrow { 14.0 } else { 24.0 };

    // Who and where.
    ctx.set_text_align("left");
    ctx.set_text_baseline("top");
    ctx.set_fill_style_str("rgba(255,236,210,0.95)");
    ctx.set_font(&format!("600 15px {SANS}"));
    text(ctx, "secretspace", pad, pad);
    ctx.set_font(&format!("12px {SANS}"));
    ctx.set_fill_style_str("rgba(255,255,255,0.55)");
    let sun = if isl.watched || isl.sun > 0 {
        format!("sun {}", isl.sun)
    } else {
        "night".into()
    };
    text(
        ctx,
        &format!(
            "your island · {} {} · {} · {} motes · {} island{} heard",
            app.place.name,
            clock(app.place.tz_min),
            sun,
            isl.motes().len(),
            app.heard(),
            if app.heard() == 1 { "" } else { "s" }
        ),
        pad,
        pad + 22.0,
    );

    // The world as far as this tab can hear: lineages by how many islands
    // they are alive on.
    let base = f.y + f.h() + 34.0;
    ctx.set_font(&format!("11px {SANS}"));
    ctx.set_fill_style_str("rgba(255,255,255,0.38)");
    text(ctx, "ALIVE ON", pad.max(f.x), base);
    let rows = if narrow { 4 } else { 5 };
    for (i, l) in app.world.iter().take(rows).enumerate() {
        let y = base + 18.0 + i as f64 * 18.0;
        let x = pad.max(f.x);
        ctx.set_fill_style_str(&hsl(hue(l.lineage), 85.0, 62.0, 1.0));
        ctx.begin_path();
        let _ = ctx.arc(x + 4.0, y + 7.0, 4.0, 0.0, TAU);
        ctx.fill();
        let yours = app.mine.contains(&l.lineage);
        ctx.set_fill_style_str(if yours {
            "#ffffff"
        } else {
            "rgba(255,255,255,0.78)"
        });
        ctx.set_font(&format!("{}12px {SANS}", if yours { "600 " } else { "" }));
        let by = if l.author == "genesis" {
            String::new()
        } else {
            format!(" by {}", l.author)
        };
        text(
            ctx,
            &format!(
                "{}{by} — {} tab{} · {} motes",
                l.name,
                l.tabs,
                if l.tabs == 1 { "" } else { "s" },
                l.count
            ),
            x + 14.0,
            y,
        );
    }

    // What just happened.
    let fx = if narrow {
        pad
    } else {
        (f.x + f.w()).min(w - pad)
    };
    let fy = if narrow {
        base + 18.0 + rows as f64 * 18.0 + 10.0
    } else {
        base
    };
    ctx.set_text_align(if narrow { "left" } else { "right" });
    for (i, item) in app.feed.iter().rev().enumerate() {
        let age = (now - item.at) / 1000.0;
        let a = (1.0 - (age - 8.0) / 6.0).clamp(0.0, 1.0) * (1.0 - i as f64 * 0.12);
        if a <= 0.0 || (narrow && i >= 3) {
            continue;
        }
        ctx.set_font(&format!("12px {SANS}"));
        ctx.set_fill_style_str(&match item.hue {
            Some(hu) => hsl(hu, 70.0, 75.0, a),
            None => format!("rgba(255,236,210,{a:.2})"),
        });
        text(ctx, &item.text, fx, fy + i as f64 * 18.0);
    }

    // The receipt.
    ctx.set_text_align("center");
    ctx.set_text_baseline("bottom");
    ctx.set_font(&format!("10px {MONO}"));
    ctx.set_fill_style_str("rgba(255,255,255,0.22)");
    text(
        ctx,
        &format!(
            "tick {} · {:016x} · {}",
            isl.tick,
            isl.hash(),
            if isl.conserved() {
                "every erg accounted for"
            } else {
                "LEDGER BROKEN"
            }
        ),
        w / 2.0,
        h - 10.0,
    );

    // Alone: say how to make the world bigger.
    if app.heard() == 1 {
        ctx.set_text_baseline("middle");
        ctx.set_font(&format!("13px {SANS}"));
        ctx.set_fill_style_str(&format!(
            "rgba(255,236,210,{:.2})",
            0.55 + 0.25 * (now / 900.0).sin()
        ));
        let (cx, cy) = (f.x + f.w() / 2.0, f.y + f.h() / 2.0);
        text(ctx, "you are the only island.", cx, cy - 10.0);
        text(
            ctx,
            "open this page in another window — islands link at their edges.",
            cx,
            cy + 10.0,
        );
    }

    // Back from the dark.
    if let Some(s) = &app.summary {
        let age = (now - s.at) / 1000.0;
        let a = (1.0 - (age - 6.0) / 2.0).clamp(0.0, 1.0);
        if a > 0.0 {
            let (cx, cy) = (f.x + f.w() / 2.0, f.y + f.h() * 0.38);
            let bw = 340.0_f64.min(f.w() - 20.0);
            let bh = 30.0 + 20.0 * s.lines.len() as f64;
            ctx.set_fill_style_str(&format!("rgba(7,9,13,{:.2})", 0.82 * a));
            ctx.fill_rect(cx - bw / 2.0, cy - 18.0, bw, bh);
            ctx.set_text_align("center");
            ctx.set_text_baseline("top");
            for (i, line) in s.lines.iter().enumerate() {
                ctx.set_font(&format!("{}13px {SANS}", if i == 0 { "600 " } else { "" }));
                ctx.set_fill_style_str(&format!("rgba(255,236,210,{:.2})", a));
                text(ctx, line, cx, cy - 4.0 + i as f64 * 20.0);
            }
        }
    }
}
