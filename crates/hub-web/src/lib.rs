//! The front page: a card for every game, each showing how many people
//! are in it right now, and along the bottom how many are online anywhere
//! and how many visits there have ever been. The numbers come live from
//! the server's `/ws/hub`; wyrm's card shows the real game, live (`watch`),
//! Wandfall's its match while one is on (`wand`), and Battlestation's the
//! real desk, compusophyOS on it (`desk`). A card opens its
//! game over the page, full screen at once (`kit::shell`): clicked,
//! tapped, or picked with the keys (arrows or Tab, then Enter); the page
//! rests till the game is left.
//! Every pixel is drawn here, in Rust.

mod backdrop;
mod desk;
mod shelf;
mod tag;
mod wand;
mod watch;

use std::cell::RefCell;

use engine::hub::Stats;
use pixels::{text_width, Canvas, Rect, Rgba};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{KeyboardEvent, PointerEvent, WheelEvent};

use shelf::{fitted, shelf, Shelf};

/// A game on the shelf.
struct Card {
    /// Its room on the server, for the player count; "" if not built yet.
    id: &'static str,
    title: &'static str,
    blurb: [&'static str; 2],
    path: &'static str,
    hue: f32,
}

const CARDS: &[Card] = &[
    Card {
        id: "wyrm",
        title: "WYRM",
        blurb: ["eat the glow, grow long,", "make them run into you"],
        path: "/wyrm/",
        hue: 140.0,
    },
    Card {
        id: "wandfall",
        title: "WANDFALL",
        blurb: ["a wand battle royale:", "be the last one standing"],
        path: "/wandfall/",
        hue: 265.0,
    },
    // No room: each desk is its own.
    Card {
        id: "",
        title: "BATTLESTATION",
        blurb: ["sit at a desk at night,", "a real computer on it"],
        path: "/battlestation/",
        hue: 300.0,
    },
];

const INK: Rgba = Rgba::rgb(244, 241, 255);
const DIM: Rgba = Rgba(244, 241, 255, 150);
const GO: Rgba = Rgba::rgb(87, 227, 137);

struct Hub {
    screen: kit::Screen,
    /// The live numbers (`/ws/hub`); its first connection counts the visit.
    link: kit::Link,
    stats: Option<Stats>,
    pointer: Option<(f32, f32)>,
    /// The page as last laid out.
    shelf: Shelf,
    /// Where each card was drawn last frame, for clicks.
    hits: Vec<(Rect, usize)>,
    /// The card picked with the keys, if any.
    focus: Option<usize>,
    /// wyrm, live, for its card.
    watch: watch::Watch,
    /// Wandfall, live while a match is on, for its card.
    wand: wand::WandWatch,
    /// Battlestation's desk, its computer on, for its card.
    desk: desk::DeskWatch,
    backdrop: backdrop::Backdrop,
    /// How far the page is scrolled.
    scroll: f32,
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

/// The live numbers, as they come.
fn numbers(h: &mut Hub, now: f64) {
    for ev in h.link.poll(now) {
        if let kit::Net::Message(b) = ev {
            if let Some(s) = Stats::decode(&b) {
                h.stats = Some(s);
            }
        }
    }
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

/// A game is open over the page: let its connections go (the player is
/// counted in the game; the cards are not watched unseen). They come back
/// once it is left.
fn rest(h: &mut Hub) {
    // Hung up, not dropped: polled again, it connects again, uncounted.
    h.link.close();
    h.watch.rest();
    h.wand.rest();
}

/// The cards that open a game, in order.
fn playable() -> Vec<usize> {
    (0..CARDS.len())
        .filter(|&i| !CARDS[i].path.is_empty())
        .collect()
}

/// The playable card `step` along from `from` (round the end), or the
/// first (the last, going back) when none is picked.
fn along(from: Option<usize>, step: isize) -> Option<usize> {
    let all = playable();
    let at = from.and_then(|f| all.iter().position(|&i| i == f));
    let k = match at {
        Some(k) => (k as isize + step).rem_euclid(all.len() as isize) as usize,
        None if step < 0 => all.len().checked_sub(1)?,
        None => 0,
    };
    all.get(k).copied()
}

/// The playable card a row down (or up) from `from`, on a shelf `cols`
/// wide; along the row where there is none.
fn row(from: Option<usize>, down: bool, cols: usize) -> Option<usize> {
    let to = from.and_then(|f| {
        if down {
            Some(f + cols)
        } else {
            f.checked_sub(cols)
        }
    });
    match to {
        Some(t) if t < CARDS.len() && !CARDS[t].path.is_empty() => Some(t),
        _ => along(from, if down { 1 } else { -1 }),
    }
}

/// A key on the page: the card picked moves (arrows, Tab), opens (Enter)
/// or is let go (Esc); the page scrolls (Space, Page Up and Down, Home,
/// End). Whether the key was the page's, and the game to open.
fn key(h: &mut Hub, key: &str, shift: bool) -> (bool, Option<&'static str>) {
    let ht = h.screen.px.h as f32;
    let s = h.shelf;
    let focus = match key {
        "Tab" => along(h.focus, if shift { -1 } else { 1 }),
        "ArrowRight" => along(h.focus, 1),
        "ArrowLeft" => along(h.focus, -1),
        "ArrowDown" => row(h.focus, true, s.cols),
        "ArrowUp" => row(h.focus, false, s.cols),
        "Enter" => return (true, h.focus.map(|i| CARDS[i].path)),
        "Escape" => {
            h.focus = None;
            return (true, None);
        }
        " " | "PageDown" | "PageUp" | "Home" | "End" => {
            let page = (ht - s.foot_h - s.gap).max(s.gap);
            let to = match key {
                "PageUp" => h.scroll - page,
                "Home" => 0.0,
                "End" => f32::MAX,
                _ => h.scroll + page,
            };
            h.scroll = to.clamp(0.0, s.max_scroll(ht));
            return (true, None);
        }
        _ => return (false, None),
    };
    h.focus = focus;
    if let Some(i) = focus {
        h.scroll = s.show(i, h.scroll, ht);
    }
    (true, None)
}

fn draw(h: &mut Hub, now: f64) {
    numbers(h, now);
    let css = h.screen.css;
    h.watch.poll(now, css);
    // A watcher's zoom in the game, pulled back a little: a card is small.
    let k = 0.8 * wyrm::laws::view_scale(18.0, css.0 as f32, css.1 as f32) / h.screen.scale as f32;
    let u = h.screen.ui();
    let uf = u as f32;
    let t = (now / 1000.0) as f32;
    let (w, ht) = (h.screen.px.w as f32, h.screen.px.h as f32);
    let s = shelf(w, ht, u, CARDS.len());
    h.shelf = s;
    // The page scrolls under a fixed footer when the cards do not fit.
    let scroll = h.scroll.clamp(0.0, s.max_scroll(ht));
    h.scroll = scroll;
    let pointer = h.pointer;
    let stats = h.stats.clone();
    let up = h.link.up();
    let c = &mut h.screen.px;
    h.backdrop.draw(c, t, scroll, uf);

    // The name, and what this is.
    let tw = text_width(shelf::TITLE, s.title_scale);
    let ty = (s.title_y - scroll) as i32;
    backdrop::wordmark(c, (w as i32 - tw) / 2, ty, s.title_scale, t);
    // Every line is fitted to the page: a phone's buffer may be narrower
    // than its CSS width (`kit::snap`).
    let room = shelf::across(w, u);
    let (line, size) = fitted(&[shelf::TAGLINE], room, u);
    c.text_centred(w as i32 / 2, (s.tag_y - scroll) as i32, line, size, DIM);

    // The cards.
    h.hits.clear();
    let mut hover = false;
    for (i, card) in CARDS.iter().enumerate() {
        let b = s.at(i);
        let b = Rect::new(b.x, b.y - scroll, b.w, b.h);
        let live = !card.path.is_empty();
        let pointed = pointer.is_some_and(|(px, py)| b.contains(px, py) && py < ht - s.foot_h);
        hover |= live && pointed;
        let over = live && (pointed || h.focus == Some(i));
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
        let pv = Rect::new(b.x + pad, b.y + pad, b.w - 2.0 * pad, s.preview_h);
        if card.id == "wyrm" {
            h.watch.draw(c, pv, k, 6.0 * uf, u, now);
        } else if card.id == "wandfall" {
            h.wand.draw(c, pv, u, now);
        } else if card.path == "/battlestation/" {
            h.desk.draw(c, pv, u, now);
        } else {
            soon_preview(c, pv, t, uf, card.hue);
        }
        let (inset, words) = s.words(u);
        let (tx, mut ty) = (b.x + inset, pv.y + pv.h + 8.0 * uf);
        let (title, size) = fitted(&[card.title], words, 2 * u);
        c.text_shadowed(
            tx as i32,
            ty as i32,
            title,
            size,
            if live { INK } else { DIM },
        );
        ty += 19.0 * uf;
        for line in card.blurb {
            let (line, size) = fitted(&[line], words, u);
            c.text(tx as i32, ty as i32, line, size, DIM);
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
            let line = if card.id.is_empty() {
                "your own desk".to_string()
            } else if stats.is_some() {
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
            h.hits.push((b, i));
        } else {
            c.text(tx as i32, status_y, "soon", u, DIM);
        }
    }
    h.screen.cursor(if hover { "pointer" } else { "default" });

    // Along the bottom: everyone, everywhere, ever.
    let c = &mut h.screen.px;
    let foot_y = ht - s.foot_h;
    c.fill_rect(
        0,
        foot_y as i32,
        w as i32,
        s.foot_h as i32 + 1,
        Rgba(7, 10, 18, 235),
    );
    c.hline(0, w as i32, foot_y as i32, Rgba(255, 255, 255, 18));
    let fy = (ht - 13.0 * uf) as i32;
    let dot = (10.0 * uf) as i32;
    let foot = match &stats {
        Some(s) => {
            let (online, visits) = (grouped(s.online as u64), grouped(s.visits));
            // Closer together if that is what fits.
            vec![
                format!("{online} online   {visits} visits"),
                format!("{online} online  {visits} visits"),
            ]
        }
        None if up => vec!["counting...".to_string()],
        None => vec!["connecting...".to_string()],
    };
    let ways: Vec<&str> = foot.iter().map(String::as_str).collect();
    let (foot, size) = fitted(&ways, room - dot, u);
    let fx = (w as i32 - text_width(foot, size) - dot) / 2;
    c.circle(
        fx as f32 + 2.5 * uf,
        fy as f32 + 3.5 * uf,
        2.5 * uf,
        if stats.is_some() { GO } else { DIM },
    );
    c.text(fx + dot, fy, foot, size, DIM);
    let (made, size) = fitted(&shelf::MADE, room, u);
    c.text_centred(
        w as i32 / 2,
        fy - (12.0 * uf) as i32,
        made,
        size,
        Rgba(244, 241, 255, 70),
    );
    h.screen.present();
}

/// Open the game at `path` over the page, and rest.
fn open(path: &str) {
    kit::shell::open(path);
    with(rest);
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let screen = kit::Screen::new("screen");
    let canvas = kit::document()
        .get_element_by_id("screen")
        .ok_or("no #screen")?;
    let s = shelf(
        screen.px.w as f32,
        screen.px.h as f32,
        screen.ui(),
        CARDS.len(),
    );
    HUB.with(|h| {
        *h.borrow_mut() = Some(Hub {
            screen,
            link: kit::Link::open("hub", Vec::new(), false),
            stats: None,
            pointer: None,
            shelf: s,
            hits: Vec::new(),
            focus: None,
            watch: watch::Watch::new(),
            wand: wand::WandWatch::new(),
            desk: desk::DeskWatch::new(),
            backdrop: backdrop::Backdrop::default(),
            scroll: 0.0,
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
            // Battlestation's desk, drawn the same way on its own device,
            // its computer starting.
            if let Ok(off) = gpu::Offscreen::new().await {
                with(|h| h.desk.give(off));
                let os = desk::mount().await;
                with(|h| h.desk.mounted(os));
            }
        });
    }
    kit::report::on_panic();
    kit::shell::listen();
    kit::frames(|now| {
        // A game is open over the page: nothing here to see (the desk's
        // computer rests too).
        let playing = kit::shell::playing();
        with(|h| {
            h.desk.rest(playing);
            if !playing {
                draw(h, now);
            }
        });
    });
    kit::on_resize(|| {
        with(|h| h.screen.fit());
    });
    kit::on(&kit::window(), "keydown", |e| {
        let Ok(e) = e.dyn_into::<KeyboardEvent>() else {
            return;
        };
        if kit::shell::playing() || e.ctrl_key() || e.meta_key() || e.alt_key() {
            return;
        }
        let Some((ours, to)) = with(|h| key(h, &e.key(), e.shift_key())) else {
            return;
        };
        if ours {
            e.prevent_default();
        }
        if let Some(path) = to {
            open(path);
        }
    });
    kit::on(&canvas, "pointerdown", |e| {
        if let Ok(e) = e.dyn_into::<PointerEvent>() {
            with(|h| {
                let (_, y) = h.screen.to_px(e.client_x() as f64, e.client_y() as f64);
                h.press = Some((e.pointer_id(), y, h.scroll, false));
                // A hand on the page: the keys' pick goes.
                h.focus = None;
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
                            let most = h.shelf.max_scroll(h.screen.px.h as f32);
                            h.scroll = (at - (y - from)).clamp(0.0, most);
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
                let most = h.shelf.max_scroll(h.screen.px.h as f32);
                h.scroll = (h.scroll + (e.delta_y() / h.screen.scale) as f32).clamp(0.0, most);
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
                if dragged || y >= h.screen.px.h as f32 - h.shelf.foot_h {
                    return None;
                }
                h.hits
                    .iter()
                    .find(|(b, _)| b.contains(x, y))
                    .map(|&(_, i)| CARDS[i].path)
            })
            .flatten();
            if let Some(path) = to {
                open(path);
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keys_go_round_the_games_only() {
        let wyrm = Some(0);
        let wand = Some(1);
        let desk = Some(2);
        assert_eq!(along(None, 1), wyrm);
        assert_eq!(along(None, -1), desk);
        assert_eq!(along(wyrm, 1), wand);
        // Round the end.
        assert_eq!(along(desk, 1), wyrm);
        // A row down on a shelf one card wide: the next game.
        assert_eq!(row(wyrm, true, 1), wand);
        // Down from the last row: along, round to the first.
        assert_eq!(row(desk, true, 3), wyrm);
        assert!(CARDS[along(wand, 1).unwrap()].path.starts_with('/'));
    }

    #[test]
    fn every_line_fits_a_narrow_phone_whole() {
        // 360 CSS pixels at DPR 3, and the 412 of most Androids at 2.625
        // and 2.75, which the snap makes 361 and 378 buffer pixels.
        for w in [360.0, 361.0, 378.0] {
            let u = 2;
            let s = shelf(w, 800.0, u, CARDS.len());
            let room = shelf::across(w, u);
            assert_eq!(fitted(&[shelf::TAGLINE], room, u).1, u, "{w}");
            let (made, k) = fitted(&shelf::MADE, room, u);
            assert!(k == u && text_width(made, k) <= room, "{w}: {made}");
            // Busy days, too: the dot and both numbers.
            let foot = "123 online  123,456 visits";
            assert!(text_width(foot, u) + 10 * u <= room, "{w}");
            let (_, words) = s.words(u);
            for card in CARDS {
                // A long name (BATTLESTATION) may come a size smaller, still
                // over the words and whole.
                let (title, k) = fitted(&[card.title], words, 2 * u);
                assert!(k > u && text_width(title, k) <= words, "{w}: {title}");
                for line in card.blurb {
                    assert_eq!(fitted(&[line], words, u).1, u, "{w}: {line}");
                }
            }
        }
        // Where the whole line fits it is said whole.
        assert_eq!(
            fitted(&shelf::MADE, shelf::across(412.0, 2), 2).0,
            shelf::MADE[0]
        );
        // Too narrow even for the short one: smaller, never cut.
        let (made, k) = fitted(&shelf::MADE, 200, 2);
        assert!(k == 1 && text_width(made, k) <= 200);
    }
}
