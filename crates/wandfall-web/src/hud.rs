//! The HUD, drawn in pixels over the picture: the crosshair (and a mark
//! when your bolt lands), your health, the match and the storm, a map of
//! the island with the storm's circles, the feed, names over heads, and
//! what to do next (click to play, you are out, who won).

use pixels::{Canvas, Rect, Rgba};
use render::{m4, M4};
use wandfall::laws::{MAP_HALF, MATCH_SIZE, SEA};
use wandfall::loot::max_hp;
use wandfall::map::Map;
use wandfall::places::Place;
use wandfall::proto::{flag, Frame, Own, Seen};

use crate::bar;
use crate::state::{Line, State};

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
    // The places: the Spire's plaza and tower, the circle, the rift, the grove.
    let px = |v: f32| (v + MAP_HALF) / (2.0 * MAP_HALF) * MINI as f32;
    for p in &map.pois {
        let (x, y) = (px(p.x), px(p.z));
        match p.place {
            Place::Spire => {
                c.circle(x, y, 4.0, Rgba::rgb(196, 188, 176));
                c.circle(x, y, 1.8, Rgba::rgb(120, 80, 220));
            }
            Place::Circle => c.ring(x, y, 3.0, 1.2, Rgba::rgb(120, 225, 255)),
            Place::Rift => {
                c.circle(x, y, 3.6, Rgba::rgb(40, 22, 22));
                c.circle(x, y, 1.6, Rgba::rgb(255, 110, 40));
            }
            Place::Grove => c.circle(x, y, 2.6, Rgba::rgb(170, 120, 255)),
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
    /// On the practice range (no match, no storm).
    pub practice: bool,
    /// A wizard under the crosshair, in reach of the Lance.
    pub on_target: bool,
    /// The spellbook is open (it takes the place of the cube underfoot).
    pub book: bool,
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
    // Names over heads, near enough to read; nearest first, and one that
    // would cover a nearer one's is left out.
    if let Some((eye, _)) = v.me.or(Some(([0.0; 3], 0.0))) {
        let mut near: Vec<_> = v
            .others
            .iter()
            .filter(|s| s.flags & flag::ALIVE != 0 && s.id != v.st.you)
            .map(|s| {
                let d = (s.p[0] - eye[0]).powi(2) + (s.p[2] - eye[2]).powi(2);
                (d, s)
            })
            .filter(|(d, _)| v.me.is_none() || *d < 45.0 * 45.0)
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut placed: Vec<Rect> = Vec::new();
        for (_, s) in near {
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
            let tw = pixels::text_width(&name, ui).max(24 * ui);
            let r = Rect::new(
                (sx - tw / 2) as f32,
                (sy - 10 * ui) as f32,
                tw as f32,
                (12 * ui) as f32,
            );
            if placed.iter().any(|p| p.overlaps(&r)) {
                continue;
            }
            placed.push(r);
            c.text_centred(sx, sy - 9 * ui, &name, ui, INK.fade(0.9));
            let bar = 24 * ui;
            let full = max_hp(s.level);
            c.fill_rect(sx - bar / 2, sy, bar, 2 * ui, SHADE);
            let k = (s.hp as i32 * bar / full).min(bar);
            let col = if (s.hp as i32) * 3 < full { RED } else { INK };
            c.fill_rect(sx - bar / 2, sy, k, 2 * ui, col);
        }
    }
    // The numbers your hits do, rising off whom they struck.
    for (k, &(at, to, amount, what)) in v.st.numbers.iter().enumerate() {
        let Some(s) = v.others.iter().find(|s| s.id == to) else {
            continue;
        };
        let age = (v.now - at) as f32 / 900.0;
        let side = if k % 2 == 0 { 0.4 } else { -0.4 };
        let (x, y, ww) = m4::project(&v.vp, [s.p[0] + side, s.p[1] + 2.2 + age * 1.2, s.p[2]]);
        if ww <= 0.1 {
            continue;
        }
        let sx = ((x / ww * 0.5 + 0.5) * w as f32) as i32;
        let sy = ((0.5 - y / ww * 0.5) * h as f32) as i32;
        let big = if amount >= 25 { 3 } else { 2 };
        let col = bar::rgba(crate::fx::colour(what)).mix(INK, 0.25);
        let n = format!("{amount}");
        let fade = 1.0 - age * age;
        c.text_centred(sx + ui, sy + ui, &n, big * ui, SHADE.fade(fade));
        c.text_centred(sx, sy, &n, big * ui, col.fade(fade));
    }
    // The crosshair, and a mark when your bolt lands.
    if v.me.is_some() {
        let cy = h / 2;
        let g = 3 * ui;
        let l = 4 * ui;
        let col = if v.on_target { RED } else { INK.fade(0.85) };
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            c.line(
                (cx + dx * g) as f32,
                (cy + dy * g) as f32,
                (cx + dx * (g + l)) as f32,
                (cy + dy * (g + l)) as f32,
                ui as f32,
                col,
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
        _ if v.practice => {
            if v.touch {
                "practice range - menu: spellbook".to_string()
            } else {
                "practice range - B spellbook - Esc menu".to_string()
            }
        }
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
    // Health (and a ward's shield after it), and the spell bar.
    if let Some(o) = v.own.filter(|_| v.me.is_some()) {
        // Warded: the screen's edge glows (you cannot see your own
        // bubble from inside it).
        if o.shield > 0 {
            let ward = bar::rgba(crate::fx::colour(wandfall::laws::spell::WARD));
            for k in 0..4 {
                let inset = (k * 3 * ui) as f32;
                let a = 0.32 - k as f32 * 0.07;
                let r = Rect::new(inset, inset, w as f32 - 2.0 * inset, h as f32 - 2.0 * inset);
                c.round_rect_line(r, 10.0 * ui as f32, 3.0 * ui as f32, ward.fade(a));
            }
        }
        let full = max_hp(o.level);
        // At the foot of the screen, left (the spells are right, your
        // wizard between); on a touch screen the thumbs own the bottom,
        // and health goes top left.
        let (x, y, bw, bh) = if v.touch {
            (12 * ui, 40 * ui, 120 * ui, 8 * ui)
        } else {
            let l = bar::layout(w, h, ui);
            (14 * ui, l.y + l.s - 11 * ui, (150 * ui).min(w / 3), 11 * ui)
        };
        let u = ui as f32;
        c.round_rect(
            Rect::new(x as f32, y as f32, bw as f32, bh as f32),
            2.0 * u,
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
            2.0 * u,
            col,
        );
        if o.shield > 0 {
            let sw = (o.shield as i32 * bw / full).min(bw - k);
            let ward = bar::rgba(crate::fx::colour(wandfall::laws::spell::WARD)).mix(INK, 0.3);
            c.round_rect(
                Rect::new((x + k) as f32, y as f32, sw as f32, bh as f32),
                2.0 * u,
                ward,
            );
        }
        let hp = format!("{} / {}", o.hp, full);
        let ko = format!("knocked out {}", o.kills);
        if v.touch {
            // Stacked at the top left: health and level, XP, knockouts.
            let line = format!("{hp}   level {}", o.level);
            c.text_shadowed(x, y - 12 * ui, &line, ui, INK);
            c.text_shadowed(x, y + 16 * ui, &ko, ui, GOLD);
        } else {
            c.text_centred(x + bw / 2, y + 2 * ui, &hp, ui, INK);
            let s = (MINI * ui).min(w / 4).min(h / 4);
            let kx = w - 10 * ui - pixels::text_width(&ko, ui);
            c.text_shadowed(kx, 24 * ui + s + 6 * ui, &ko, ui, GOLD);
        }
        if o.body.chill > 0 {
            let cold = bar::rgba(crate::fx::colour(wandfall::laws::spell::FROST));
            c.text_centred(cx, h / 2 + 18 * ui, "chilled", ui, cold);
        }
        bar::draw(c, o, v.st, v.now, ui, (!v.touch, !v.book));
    }
    // The island, the storm's circles, and you.
    let s = (MINI * ui).min(w / 4).min(h / 4);
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
    if f.phase == 1 && !v.practice {
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
    // The feed: under the island map, or (on a touch screen, whose right
    // side is the thumb's) under your health.
    let mut y = if v.touch { 74 * ui } else { my + s + 20 * ui };
    for (at, line) in &v.st.feed {
        let age = v.now - at;
        if age > 8000.0 {
            continue;
        }
        let fade = (1.0 - (age - 6000.0).max(0.0) / 2000.0) as f32;
        match line {
            Line::Text(t) => {
                let tw = pixels::text_width(t, ui);
                let x = if v.touch { 12 * ui } else { w - tw - 10 * ui };
                c.text_shadowed(x, y, t, ui, DIM.fade(fade));
            }
            // A knockout: who, the icon of what did it, whom.
            &Line::Out { by, with, who } => {
                let (a, b) = (v.st.name(by), v.st.name(who));
                let (isz, gap) = (10 * ui, 4 * ui);
                let wa = pixels::text_width(&a, ui);
                let tw = wa + 2 * gap + isz + pixels::text_width(&b, ui);
                let x = if v.touch { 12 * ui } else { w - tw - 10 * ui };
                let you = v.st.you;
                let ca = if by == you { GOLD } else { INK };
                let cb = if who == you { RED } else { DIM };
                c.text_shadowed(x, y, &a, ui, ca.fade(fade));
                let tile = Rect::new(
                    (x + wa + gap) as f32,
                    (y - 2 * ui) as f32,
                    isz as f32,
                    isz as f32,
                );
                bar::icon(c, with, tile);
                c.text_shadowed(x + wa + 2 * gap + isz, y, &b, ui, cb.fade(fade));
            }
        }
        y += 12 * ui;
    }
    // What to do now: the card when you are out or the match is won.
    let mid = h / 2 - 40 * ui;
    let won = f.phase == 2 && f.winner == v.st.you && f.winner != 0;
    if !v.practice && (won || v.st.out.is_some_and(|_| f.phase >= 1)) {
        card(c, v, f, won);
    } else if v.me.is_none() && f.phase == 1 {
        c.text_centred(cx, mid, "a match is on: you join the next one", ui, INK);
    }
    if let Some(name) = &v.watching {
        c.text_centred(cx, h - 30 * ui, &format!("watching {name}"), ui, DIM);
    }
    if let Some(p) = &v.perf {
        let tw = pixels::text_width(p, ui);
        c.text_shadowed(w - tw - 6 * ui, h - 12 * ui, p, ui, DIM);
    }
}

/// How your match went: your place (or victory), what took you, your
/// knockouts and level, and what comes next.
fn card(c: &mut Canvas, v: &View, f: &Frame, won: bool) {
    let (w, h, ui) = (c.w, c.h, v.ui);
    let u = ui as f32;
    let (cw, ch) = ((250 * ui).min(w - 16 * ui), 104 * ui);
    let b = Rect::new(
        ((w - cw) / 2) as f32,
        (h / 2 - ch + 10 * ui) as f32,
        cw as f32,
        ch as f32,
    );
    c.round_rect(b, 8.0 * u, Rgba(8, 10, 22, 215));
    c.round_rect_line(b, 8.0 * u, u, if won { GOLD } else { DIM.fade(0.5) });
    let cx = w / 2;
    let mut y = b.y as i32 + 10 * ui;
    let (by, place) = v.st.out.unwrap_or((0, 1));
    let of = f.entrants.max(MATCH_SIZE as u8);
    let title = if won {
        "victory!".to_string()
    } else {
        format!("#{place} of {of}")
    };
    c.text_centred(cx, y, &title, 3 * ui, if won { GOLD } else { INK });
    y += 28 * ui;
    if won {
        c.text_centred(cx, y, "the last wizard standing", ui, INK);
    } else if by == 0 {
        c.text_centred(cx, y, "the storm took you", ui, INK);
    } else {
        // Knocked out by whom, with what.
        let line = format!("knocked out by {}", v.st.name(by));
        let tw = pixels::text_width(&line, ui) + 14 * ui;
        let x = cx - tw / 2;
        c.text_shadowed(x, y, &line, ui, INK);
        let at = (x + tw - 10 * ui) as f32;
        bar::icon(
            c,
            v.st.out_with,
            Rect::new(at, (y - 2 * ui) as f32, 10.0 * u, 10.0 * u),
        );
    }
    y += 16 * ui;
    if let Some(o) = &v.st.last_own {
        let stats = format!("knockouts {}     level {}", o.kills, o.level);
        c.text_centred(cx, y, &stats, ui, GOLD);
    }
    y += 16 * ui;
    let next = if f.phase == 2 {
        format!("the next match in {}", f.secs)
    } else {
        "the next match starts when this one ends".to_string()
    };
    let k = pixels::fit_scale(&next, cw - 12 * ui, ui);
    c.text_centred(cx, y, &next, k, DIM);
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
