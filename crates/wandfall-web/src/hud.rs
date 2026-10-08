//! The HUD, drawn in pixels over the picture: the crosshair (and a mark
//! when your bolt lands), your health, the match and the storm, a map of
//! the island with the storm's circles, the feed, names over heads, and
//! what to do next (click to play, you are out, who won).

use pixels::{Canvas, Rect, Rgba};
use render::{m4, M4};
use wandfall::laws::{MAP_HALF, MATCH_SIZE, SEA};
use wandfall::loot::max_hp;
use wandfall::map::Map;
use wandfall::proto::{flag, Frame, Own, Seen};

use crate::bar;
use crate::state::State;

const INK: Rgba = Rgba::rgb(250, 246, 236);
const DIM: Rgba = Rgba::rgb(200, 206, 220);
const GOLD: Rgba = Rgba::rgb(255, 214, 128);
const RED: Rgba = Rgba::rgb(240, 80, 80);
const STORM: Rgba = Rgba::rgb(190, 110, 255);
const SHADE: Rgba = Rgba(8, 10, 20, 150);
/// Pixels a side of the island map.
const MINI: i32 = 96;

/// The island seen from above, drawn once.
pub fn island(map: &Map) -> Canvas {
    let mut c = Canvas::new(MINI, MINI);
    for j in 0..MINI {
        for i in 0..MINI {
            let at = |k: i32| (k as f32 + 0.5) / MINI as f32 * 2.0 * MAP_HALF - MAP_HALF;
            let h = map.height(at(i), at(j));
            let col = if h < SEA {
                Rgba::rgb(40, 80, 120)
            } else if h < SEA + 0.6 {
                Rgba::rgb(190, 172, 120)
            } else {
                let k = ((h - SEA) / 12.0).clamp(0.0, 1.0);
                Rgba::rgb(70, 120, 60).mix(Rgba::rgb(140, 150, 90), k)
            };
            c.pixel(i, j, col);
        }
    }
    c
}

/// What the HUD shows this frame.
pub struct View<'a> {
    pub st: &'a State,
    pub frame: Option<&'a Frame>,
    pub others: &'a [Seen],
    pub vp: M4,
    /// Where you are and which way you face, and your own state.
    pub me: Option<([f32; 3], f32)>,
    pub own: Option<&'a Own>,
    pub watching: Option<String>,
    pub locked: bool,
    pub in_storm: bool,
    pub now: f64,
    pub ui: i32,
    pub perf: Option<String>,
    pub touch: bool,
}

fn clock(secs: u16) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

pub fn draw(c: &mut Canvas, mini: &Canvas, v: &View) {
    let ui = v.ui;
    let (w, h) = (c.w, c.h);
    let cx = w / 2;
    let Some(f) = v.frame else {
        c.text_centred(cx, h / 2, "finding the island...", 2 * ui, INK);
        return;
    };
    // Names over heads, near enough to read.
    if let Some((eye, _)) = v.me.or(Some(([0.0; 3], 0.0))) {
        for s in v
            .others
            .iter()
            .filter(|s| s.flags & flag::ALIVE != 0 && s.id != v.st.you)
        {
            let d = ((s.p[0] - eye[0]).powi(2) + (s.p[2] - eye[2]).powi(2)).sqrt();
            if v.me.is_some() && d > 45.0 {
                continue;
            }
            let (x, y, ww) = m4::project(&v.vp, [s.p[0], s.p[1] + 2.5, s.p[2]]);
            if ww <= 0.1 {
                continue;
            }
            let sx = ((x / ww * 0.5 + 0.5) * w as f32) as i32;
            let sy = ((0.5 - y / ww * 0.5) * h as f32) as i32;
            if sx < 0 || sx > w || sy < 0 || sy > h {
                continue;
            }
            let name = format!("{}  {}", v.st.name(s.id), s.level);
            c.text_centred(sx, sy - 9 * ui, &name, ui, INK.fade(0.9));
            let bar = 24 * ui;
            let full = max_hp(s.level);
            c.fill_rect(sx - bar / 2, sy, bar, 2 * ui, SHADE);
            let k = (s.hp as i32 * bar / full).min(bar);
            let col = if (s.hp as i32) * 3 < full { RED } else { INK };
            c.fill_rect(sx - bar / 2, sy, k, 2 * ui, col);
        }
    }
    // The crosshair, and a mark when your bolt lands.
    if v.me.is_some() {
        let cy = h / 2;
        let g = 3 * ui;
        let l = 4 * ui;
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            c.line(
                (cx + dx * g) as f32,
                (cy + dy * g) as f32,
                (cx + dx * (g + l)) as f32,
                (cy + dy * (g + l)) as f32,
                ui as f32,
                INK.fade(0.85),
            );
        }
        if v.now - v.st.hit_at < 220.0 {
            let k = 6 * ui;
            c.line(
                (cx - k) as f32,
                (cy - k) as f32,
                (cx + k) as f32,
                (cy + k) as f32,
                ui as f32 * 1.5,
                GOLD,
            );
            c.line(
                (cx - k) as f32,
                (cy + k) as f32,
                (cx + k) as f32,
                (cy - k) as f32,
                ui as f32 * 1.5,
                GOLD,
            );
        }
    }
    // Hurt, or in the storm: the edges of the screen say so.
    let hurt = (1.0 - (v.now - v.st.hurt_at) / 400.0).clamp(0.0, 1.0) as f32;
    if hurt > 0.0 {
        let r = (w.min(h) as f32) * 0.62;
        c.outside_circle(cx as f32, h as f32 / 2.0, r, RED.fade(0.35 * hurt));
    }
    if v.in_storm {
        let r = (w.min(h) as f32) * 0.55;
        c.outside_circle(cx as f32, h as f32 / 2.0, r, STORM.fade(0.4));
        c.text_centred(
            cx,
            h / 2 + 30 * ui,
            "you are in the storm: get inside the circle!",
            ui,
            INK,
        );
    }
    // Top: the match.
    let top = match f.phase {
        0 if f.secs > 0 => format!("the match begins in {}", f.secs),
        0 => "waiting for wizards".to_string(),
        1 => {
            let storm = if f.storm.1 < 0.5 {
                "the storm has closed".to_string()
            } else if f.shrinking {
                format!("the storm closes - {}", clock(f.secs))
            } else {
                format!("the storm moves in {}", clock(f.secs))
            };
            format!(
                "{} of {} alive   {}",
                f.alive,
                f.entrants.max(MATCH_SIZE as u8),
                storm
            )
        }
        _ => format!("{} wins!", v.st.name(f.winner)),
    };
    let k = pixels::fit_scale(&top, w - 20 * ui, 2 * ui);
    c.text_shadowed(cx - pixels::text_width(&top, k) / 2, 8 * ui, &top, k, INK);
    // Health (and a ward's shield over it), and the spell bar.
    if let Some(o) = v.own.filter(|_| v.me.is_some()) {
        let (bw, bh) = (120 * ui, 8 * ui);
        let (x, y) = (12 * ui, h - 20 * ui);
        let full = max_hp(o.level);
        c.round_rect(
            Rect::new(x as f32, y as f32, bw as f32, bh as f32),
            2.0 * ui as f32,
            SHADE,
        );
        let k = (o.hp as i32 * bw / full).min(bw);
        let col = if (o.hp as i32) * 3 < full {
            RED
        } else {
            Rgba::rgb(110, 220, 120)
        };
        c.round_rect(
            Rect::new(x as f32, y as f32, k as f32, bh as f32),
            2.0 * ui as f32,
            col,
        );
        if o.shield > 0 {
            let sw = (o.shield as i32 * bw / full).min(bw);
            c.round_rect(
                Rect::new(x as f32, (y - 3 * ui) as f32, sw as f32, (2 * ui) as f32),
                ui as f32,
                Rgba::rgb(150, 214, 255),
            );
        }
        c.text_shadowed(x, y - 12 * ui, &format!("{} / {}", o.hp, full), ui, INK);
        c.text_shadowed(
            x + bw + 8 * ui,
            y,
            &format!("knocked out {}", o.kills),
            ui,
            GOLD,
        );
        if o.body.root > 0 {
            c.text_centred(
                cx,
                h / 2 + 18 * ui,
                "rooted!",
                2 * ui,
                Rgba::rgb(130, 220, 100),
            );
        }
        bar::draw(c, o, &v.st.loot, v.now, v.st.levelled, ui);
    }
    // The island, the storm's circles, and you.
    let s = (MINI * ui).min(w / 4);
    let (mx, my) = (w - s - 10 * ui, 10 * ui + 14 * ui);
    let scaled = scale_to(mini, s);
    c.blit(&scaled, mx, my, 0.0);
    let to = |x: f32, z: f32| {
        (
            mx as f32 + (x + MAP_HALF) / (2.0 * MAP_HALF) * s as f32,
            my as f32 + (z + MAP_HALF) / (2.0 * MAP_HALF) * s as f32,
        )
    };
    let k = s as f32 / (2.0 * MAP_HALF);
    if f.phase == 1 {
        // A circle bigger than the island is not drawn past the map.
        let fits = |r: f32| r < MAP_HALF * 1.05;
        if fits(f.storm.1) {
            let (sx, sy) = to(f.storm.0[0], f.storm.0[1]);
            c.ring(sx, sy, f.storm.1 * k, 1.5, STORM);
        }
        if fits(f.next.1) {
            let (nx, ny) = to(f.next.0[0], f.next.0[1]);
            c.ring(nx, ny, f.next.1 * k, 1.0, INK.fade(0.8));
        }
    }
    if let Some((p, yaw)) = v.me {
        let (px, py) = to(p[0], p[2]);
        c.circle(px, py, 2.5 * ui as f32, GOLD);
        c.line(
            px,
            py,
            px + yaw.cos() * 7.0 * ui as f32,
            py + yaw.sin() * 7.0 * ui as f32,
            ui as f32,
            GOLD,
        );
    }
    // The feed.
    let mut y = my + s + 8 * ui;
    for (at, line) in &v.st.feed {
        let age = v.now - at;
        if age > 8000.0 {
            continue;
        }
        let fade = (1.0 - (age - 6000.0).max(0.0) / 2000.0) as f32;
        let tw = pixels::text_width(line, ui);
        c.text_shadowed(w - tw - 10 * ui, y, line, ui, DIM.fade(fade));
        y += 10 * ui;
    }
    // What to do now.
    let mid = h / 2 - 40 * ui;
    if let Some((by, place)) = v.st.out.filter(|_| f.phase == 1) {
        let line = if by == 0 {
            format!("the storm took you - #{place}")
        } else {
            format!("{} knocked you out - #{place}", v.st.name(by))
        };
        let k = pixels::fit_scale(&line, w - 20 * ui, 2 * ui);
        c.text_centred(cx, mid, &line, k, INK);
        c.text_centred(
            cx,
            mid + 22 * ui,
            "the next match starts when this one ends",
            ui,
            DIM,
        );
    } else if v.me.is_none() && f.phase == 1 {
        c.text_centred(cx, mid, "a match is on: you join the next one", ui, INK);
    }
    if let Some(name) = &v.watching {
        c.text_centred(cx, h - 30 * ui, &format!("watching {name}"), ui, DIM);
    }
    if !v.locked && v.me.is_some() && !v.touch {
        let b = Rect::new(
            (cx - 120 * ui) as f32,
            (mid - 10 * ui) as f32,
            (240 * ui) as f32,
            (60 * ui) as f32,
        );
        c.round_rect(b, 6.0 * ui as f32, SHADE);
        c.text_centred(cx, mid, "click to play", 2 * ui, GOLD);
        c.text_centred(
            cx,
            mid + 24 * ui,
            "WASD move - space jump - click cast",
            ui,
            INK,
        );
        c.text_centred(cx, mid + 36 * ui, "last wizard standing wins", ui, DIM);
    }
    if let Some(p) = &v.perf {
        let tw = pixels::text_width(p, ui);
        c.text_shadowed(w - tw - 6 * ui, h - 12 * ui, p, ui, DIM);
    }
}

/// The island map at `s` pixels a side (nearest pixel).
fn scale_to(src: &Canvas, s: i32) -> Canvas {
    let mut c = Canvas::new(s, s);
    for y in 0..s {
        for x in 0..s {
            let (sx, sy) = (x * src.w / s, y * src.h / s);
            let i = ((sy * src.w + sx) * 4) as usize;
            let d = &src.data[i..i + 4];
            c.pixel(x, y, Rgba(d[0], d[1], d[2], d[3]));
        }
    }
    c
}
