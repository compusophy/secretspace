//! The front page: a card for every game, each showing how many people
//! are in it right now, and along the bottom how many are online anywhere
//! and how many visits there have ever been. The numbers come live from
//! the server's `/ws/hub`; wyrm's card shows the real game, live (`watch`),
//! and Wandfall's its match while one is on (`wand`).
//! Every pixel is drawn here, in Rust.

mod luci;
mod wand;
mod watch;

use std::cell::RefCell;

use engine::hub::Stats;
use pixels::{fit_scale, text_width, Canvas, Rect, Rgba};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{PointerEvent, WheelEvent};

/// A game on the shelf.
struct Card {
    /// Its room on the server, for the player count; "" if not built yet.
    id: &'static str,
    title: &'static str,
    blurb: [&'static str; 2],
    path: &'static str,
    hue: f32,
    /// Kept off the public shelf (`?all=1` shows it) until it is ready.
    hidden: bool,
}

const CARDS: &[Card] = &[
    Card {
        id: "wyrm",
        title: "WYRM",
        blurb: ["eat the glow, grow long,", "make them run into you"],
        path: "/wyrm/",
        hue: 140.0,
        hidden: false,
    },
    Card {
        id: "luciphon",
        title: "LUCIPHON",
        blurb: ["carry your light out,", "knock them off the edge"],
        path: "/luciphon/",
        hue: 42.0,
        hidden: true,
    },
    Card {
        id: "wandfall",
        title: "WANDFALL",
        blurb: ["a wand battle royale:", "be the last one standing"],
        path: "/wandfall/",
        hue: 265.0,
        hidden: false,
    },
    Card {
        id: "",
        title: "AND THEN",
        blurb: ["one more after that", "made of rust too"],
        path: "",
        hue: 20.0,
        hidden: false,
    },
];

const BG: Rgba = Rgba::rgb(7, 10, 18);
const INK: Rgba = Rgba::rgb(244, 241, 255);
const DIM: Rgba = Rgba(244, 241, 255, 150);
const GO: Rgba = Rgba::rgb(87, 227, 137);

struct Hub {
    screen: kit::Screen,
    socket: Option<kit::Socket>,
    /// The first connection counts the visit.
    counted: bool,
    retry_at: f64,
    retries: u32,
    up: bool,
    stats: Option<Stats>,
    pointer: Option<(f32, f32)>,
    /// Where each card was drawn last frame, for clicks.
    hits: Vec<(Rect, usize)>,
    /// wyrm, live, for its card.
    watch: watch::Watch,
    /// Luciphon, live, while its card is shown (`?all=1` for now).
    luci: Option<luci::LuciWatch>,
    /// Wandfall, live while a match is on, for its card.
    wand: wand::WandWatch,
    /// The cards on the shelf, by index into CARDS.
    shown: Vec<usize>,
    /// How far the page is scrolled, and the most it can be.
    scroll: f32,
    max_scroll: f32,
    /// A press in progress: its pointer, where it started, the scroll
    /// then, and whether it has moved enough to be a drag, not a tap.
    press: Option<(i32, f32, f32, bool)>,
}

thread_local! {
    static HUB: RefCell<Option<Hub>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Hub) -> R) -> Option<R> {
    HUB.with(|h| h.try_borrow_mut().ok()?.as_mut().map(f))
}

fn connect(h: &mut Hub) {
    let url = kit::room_url("hub", if h.counted { "" } else { "v=1" });
    h.counted = true;
    h.socket = kit::Socket::open(
        &url,
        || {
            with(|h| {
                h.up = true;
                h.retries = 0;
            });
        },
        |bytes| {
            if let Some(s) = Stats::decode(&bytes) {
                with(|h| h.stats = Some(s));
            }
        },
        || {
            with(|h| {
                h.up = false;
                h.socket = None;
                h.retries += 1;
                h.retry_at = kit::now() + 500.0 * 2f64.powi(h.retries.min(5) as i32);
            });
        },
    );
}

/// 2413 -> "2,413".
fn grouped(n: u64) -> String {
    let d = n.to_string();
    let mut out = String::new();
    for (i, c) in d.chars().enumerate() {
        if i > 0 && (d.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A game not built yet: a slow shimmer and a question mark.
fn soon_preview(c: &mut Canvas, b: Rect, t: f32, u: f32, hue: f32) {
    c.round_rect(b, 6.0 * u, Rgba::hsl(hue, 0.35, 0.09));
    for k in 0..9 {
        let x = b.x + b.w * (0.15 + 0.7 * ((k as f32 * 1.7 + t * 0.2).sin() * 0.5 + 0.5));
        let y = b.y + b.h * (0.2 + 0.6 * ((k as f32 * 2.3 + t * 0.15).cos() * 0.5 + 0.5));
        c.glow(x, y, 9.0 * u, Rgba::hsl(hue, 0.9, 0.6).fade(0.18));
    }
    let s = (3.0 * u) as i32;
    let w = text_width("?", s);
    c.text_shadowed(
        (b.x + b.w / 2.0) as i32 - w / 2,
        (b.y + b.h / 2.0) as i32 - 3 * s,
        "?",
        s,
        Rgba::hsl(hue, 0.7, 0.75).fade(0.6 + 0.3 * (t * 2.0).sin()),
    );
}

/// Watch wyrm for its card: a watcher is never one of the people there.
fn watch_wyrm(h: &mut Hub) {
    let url = kit::room_url("wyrm", "watch=1");
    h.watch.socket = kit::Socket::open(
        &url,
        || {
            with(|h| {
                h.watch.up = true;
                h.watch.retries = 0;
                // The server sends the part of the arena this screen would
                // show a watcher; the card shows the middle of it.
                let (w, ht) = h.screen.css;
                let screen = wyrm::proto::Up::Screen {
                    w: w as u16,
                    h: ht as u16,
                };
                if let Some(s) = &h.watch.socket {
                    s.send(&screen.encode());
                }
            });
        },
        |bytes| {
            with(|h| h.watch.receive(kit::now(), &bytes));
        },
        || {
            with(|h| h.watch.closed(kit::now()));
        },
    );
}

fn draw(h: &mut Hub, now: f64) {
    if h.socket.is_none() && now >= h.retry_at {
        connect(h);
    }
    if h.watch.socket.is_none() && now >= h.watch.retry_at {
        watch_wyrm(h);
    }
    let css = h.screen.css;
    // A watcher's zoom in the game, pulled back a little: a card is small.
    let k = 0.8 * wyrm::laws::view_scale(18.0, css.0 as f32, css.1 as f32) / h.screen.scale as f32;
    let u = h.screen.ui();
    let uf = u as f32;
    let t = (now / 1000.0) as f32;
    let (w, ht) = (h.screen.px.w as f32, h.screen.px.h as f32);
    let pointer = h.pointer;
    let stats = h.stats.clone();
    let up = h.up;
    let c = &mut h.screen.px;
    c.clear(BG);

    // A slow drift of faint lights behind everything.
    for k in 0..40 {
        let x = ((k * 173 % 1000) as f32 / 1000.0 * w + t * (3.0 + (k % 5) as f32)) % w;
        let y = (k * 337 % 1000) as f32 / 1000.0 * ht;
        c.circle(
            x,
            y,
            0.7 * uf,
            Rgba::hsl((k * 29 % 360) as f32, 0.6, 0.7).fade(0.35),
        );
    }

    // The page scrolls under a fixed footer when the cards do not fit.
    let foot_h = 34.0 * uf;
    let scroll = h.scroll.clamp(0.0, h.max_scroll);
    h.scroll = scroll;

    // The name, each letter its own colour, drifting.
    let narrow = w / uf < 420.0;
    let title = "SECRETSPACE";
    let title_scale = fit_scale(title, (w - 16.0 * uf) as i32, 5 * u);
    let tw = text_width(title, title_scale);
    let mut x = (w as i32 - tw) / 2;
    let ty = (18.0 * uf - scroll) as i32;
    for (i, ch) in title.chars().enumerate() {
        let hue = 140.0 + i as f32 * 14.0 + (t * 40.0);
        let s = ch.to_string();
        x += c.text_shadowed(x, ty, &s, title_scale, Rgba::hsl(hue, 0.75, 0.62));
    }
    let tag = "tiny games, everyone in them";
    let tag_y = ty + 8 * title_scale + (6.0 * uf) as i32;
    c.text_centred(w as i32 / 2, tag_y, tag, u, DIM);

    // The cards: side by side on a wide screen, one under another on a
    // narrow one, the picture shorter.
    h.hits.clear();
    let top = tag_y as f32 + 22.0 * uf;
    let gap = 12.0 * uf;
    let preview_h = if narrow { 64.0 } else { 104.0 } * uf;
    let (cw, ch) = if narrow {
        ((w - 24.0 * uf).min(360.0 * uf), preview_h + 84.0 * uf)
    } else {
        (196.0 * uf, preview_h + 84.0 * uf)
    };
    let shown = h.shown.clone();
    let cols = (((w - 24.0 * uf + gap) / (cw + gap)).floor() as usize).clamp(1, shown.len());
    let grid_w = cols as f32 * cw + (cols - 1) as f32 * gap;
    let x0 = (w - grid_w) / 2.0;
    let mut hover = false;
    for (slot, &i) in shown.iter().enumerate() {
        let card = &CARDS[i];
        let (col, row) = (slot % cols, slot / cols);
        let b = Rect::new(
            x0 + col as f32 * (cw + gap),
            top + row as f32 * (ch + gap),
            cw,
            ch,
        );
        let live = !card.path.is_empty();
        let over = live && pointer.is_some_and(|(px, py)| b.contains(px, py) && py < ht - foot_h);
        hover |= over;
        let lift = if over { -2.0 * uf } else { 0.0 };
        let b = Rect::new(b.x, b.y + lift, b.w, b.h);
        c.round_rect(b, 9.0 * uf, Rgba::rgb(16, 21, 36));
        let edge = if over {
            Rgba::hsl(card.hue, 0.8, 0.6)
        } else {
            Rgba(255, 255, 255, 30)
        };
        c.round_rect_line(
            b,
            9.0 * uf,
            if over { 2.0 } else { 1.0 } * uf.min(1.5),
            edge,
        );

        let pad = 7.0 * uf;
        let pv = Rect::new(b.x + pad, b.y + pad, b.w - 2.0 * pad, preview_h);
        if card.id == "wyrm" {
            h.watch.draw(c, pv, k, 6.0 * uf, u, now);
        } else if let (Some(l), "luciphon") = (h.luci.as_mut(), card.id) {
            l.draw(c, pv, 6.0 * uf, u, now);
        } else if card.id == "wandfall" {
            h.wand.draw(c, pv, u, now);
        } else {
            soon_preview(c, pv, t, uf, card.hue);
        }
        let (tx, mut ty) = (b.x + pad + 2.0 * uf, pv.y + pv.h + 8.0 * uf);
        c.text_shadowed(
            tx as i32,
            ty as i32,
            card.title,
            2 * u,
            if live { INK } else { DIM },
        );
        ty += 19.0 * uf;
        for line in card.blurb {
            c.text(tx as i32, ty as i32, line, u, DIM);
            ty += 10.0 * uf;
        }
        // Who is in it.
        let status_y = (b.y + b.h - 15.0 * uf) as i32;
        if live {
            let n = stats.as_ref().map_or(0, |s| s.players(card.id));
            c.circle(
                tx + 2.5 * uf,
                status_y as f32 + 3.5 * uf,
                2.5 * uf,
                GO.fade(0.6 + 0.4 * (t * 3.0).sin().abs()),
            );
            let line = if stats.is_some() {
                format!("{n} playing")
            } else {
                "...".to_string()
            };
            c.text((tx + 9.0 * uf) as i32, status_y, &line, u, GO);
            let play = if over { "PLAY >" } else { "PLAY" };
            let pw = text_width(play, u);
            c.text(
                (b.x + b.w - pad) as i32 - pw - (2.0 * uf) as i32,
                status_y,
                play,
                u,
                if over { GO } else { DIM },
            );
        } else {
            c.text(tx as i32, status_y, "soon", u, DIM);
        }
        if live {
            h.hits.push((b, i));
        }
    }
    let rows = shown.len().div_ceil(cols) as f32;
    let content = top + scroll + rows * (ch + gap) + 8.0 * uf;
    h.max_scroll = (content + foot_h - ht).max(0.0);
    h.screen.cursor(if hover { "pointer" } else { "default" });

    // Along the bottom: everyone, everywhere, ever.
    let c = &mut h.screen.px;
    c.fill_rect(
        0,
        (ht - foot_h) as i32,
        w as i32,
        foot_h as i32 + 1,
        Rgba(7, 10, 18, 235),
    );
    c.hline(0, w as i32, (ht - foot_h) as i32, Rgba(255, 255, 255, 18));
    let fy = (ht - 13.0 * uf) as i32;
    let foot = match &stats {
        Some(s) => format!(
            "{} online   {} visits",
            grouped(s.online as u64),
            grouped(s.visits)
        ),
        None if up => "counting...".to_string(),
        None => "connecting...".to_string(),
    };
    let fw = text_width(&foot, u) + (10.0 * uf) as i32;
    let fx = (w as i32 - fw) / 2;
    c.circle(
        fx as f32 + 2.5 * uf,
        fy as f32 + 3.5 * uf,
        2.5 * uf,
        if stats.is_some() { GO } else { DIM },
    );
    c.text(fx + (10.0 * uf) as i32, fy, &foot, u, DIM);
    c.text_centred(
        w as i32 / 2,
        fy - (12.0 * uf) as i32,
        "all rust - no javascript written",
        u,
        Rgba(244, 241, 255, 70),
    );
    h.screen.present();
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let screen = kit::Screen::new("screen");
    let canvas = kit::document()
        .get_element_by_id("screen")
        .ok_or("no #screen")?;
    // A card not yet on the public shelf shows with ?all=1, in place of
    // the "next game" it is.
    let all = kit::window()
        .location()
        .search()
        .is_ok_and(|q| q.trim_start_matches('?').split('&').any(|kv| kv == "all=1"));
    let shown: Vec<usize> = (0..CARDS.len())
        .filter(|&i| !CARDS[i].hidden || all)
        .filter(|&i| !(all && CARDS[i].title == "NEXT GAME"))
        .collect();
    HUB.with(|h| {
        *h.borrow_mut() = Some(Hub {
            screen,
            socket: None,
            counted: false,
            retry_at: 0.0,
            retries: 0,
            up: false,
            stats: None,
            pointer: None,
            hits: Vec::new(),
            watch: watch::Watch::new(),
            luci: all.then(luci::LuciWatch::new),
            wand: wand::WandWatch::new(),
            shown,
            scroll: 0.0,
            max_scroll: 0.0,
            press: None,
        })
    });
    // Wandfall's card plays its match in 3D where the browser offers
    // WebGPU (off screen, read back into the card); else its map.
    if gpu::offered() {
        wasm_bindgen_futures::spawn_local(async {
            if let Ok(off) = gpu::Offscreen::new().await {
                with(|h| h.wand.give(off));
            }
        });
    }
    kit::frames(|now| {
        with(|h| draw(h, now));
    });
    kit::on(&kit::window(), "resize", |_| {
        with(|h| h.screen.fit());
    });
    kit::on(&canvas, "pointerdown", |e| {
        if let Ok(e) = e.dyn_into::<PointerEvent>() {
            with(|h| {
                let (_, y) = h.screen.to_px(e.client_x() as f64, e.client_y() as f64);
                h.press = Some((e.pointer_id(), y, h.scroll, false));
            });
        }
    });
    kit::on(&canvas, "pointermove", |e| {
        if let Ok(e) = e.dyn_into::<PointerEvent>() {
            with(|h| {
                let (x, y) = h.screen.to_px(e.client_x() as f64, e.client_y() as f64);
                h.pointer = Some((x, y));
                // Dragging scrolls, once it has moved far enough to mean it.
                if let Some((id, from, at, moved)) = h.press {
                    if id == e.pointer_id() {
                        let moved = moved || (y - from).abs() > 6.0;
                        if moved {
                            h.scroll = (at - (y - from)).clamp(0.0, h.max_scroll);
                        }
                        h.press = Some((id, from, at, moved));
                    }
                }
            });
        }
    });
    kit::on(&canvas, "wheel", |e| {
        if let Ok(e) = e.dyn_into::<WheelEvent>() {
            with(|h| {
                h.scroll =
                    (h.scroll + (e.delta_y() / h.screen.scale) as f32).clamp(0.0, h.max_scroll);
            });
        }
    });
    kit::on(&canvas, "pointerleave", |_| {
        with(|h| h.pointer = None);
    });
    kit::on(&canvas, "pointerup", |e| {
        if let Ok(e) = e.dyn_into::<PointerEvent>() {
            let to = with(|h| {
                let (x, y) = h.screen.to_px(e.client_x() as f64, e.client_y() as f64);
                let dragged = h.press.take().is_some_and(|p| p.3);
                if dragged || y >= h.screen.px.h as f32 - 34.0 * h.screen.ui() as f32 {
                    return None;
                }
                h.hits
                    .iter()
                    .find(|(b, _)| b.contains(x, y))
                    .map(|&(_, i)| CARDS[i].path)
            })
            .flatten();
            if let Some(path) = to {
                kit::go(path);
            }
        }
    });
    Ok(())
}
