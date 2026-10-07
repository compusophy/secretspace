//! The menu over the game: the title, how you died, a name, PLAY.

use pixels::{fit_scale, text_width, wrap, Canvas, Rect, Rgba};

use crate::render::{DIM, GO, INK};
use crate::state::State;

/// Where the menu's parts were drawn, for clicks and the name field.
#[derive(Default, Clone, Copy)]
pub struct Spots {
    pub name: Rect,
    pub play: Rect,
    pub back: Rect,
}

pub struct Look<'a> {
    pub u: i32,
    pub name: &'a str,
    pub editing: bool,
    pub pointer: Option<(f32, f32)>,
    pub touch: bool,
}

pub fn draw(c: &mut Canvas, st: &State, look: &Look, now: f64) -> Spots {
    let u = look.u;
    let uf = u as f32;
    let (w, h) = (c.w as f32, c.h as f32);
    c.fill_rect(0, 0, c.w, c.h, Rgba(7, 10, 18, 150));
    let narrow = w / uf < 420.0;
    let t = (now / 1000.0) as f32;

    // Back to every game.
    let back = Rect::new(
        8.0 * uf,
        8.0 * uf,
        text_width("< ALL GAMES", u) as f32 + 12.0 * uf,
        16.0 * uf,
    );
    let over_back = look.pointer.is_some_and(|(x, y)| back.contains(x, y));
    c.round_rect(
        back,
        4.0 * uf,
        Rgba(255, 255, 255, if over_back { 40 } else { 18 }),
    );
    c.text(
        (back.x + 6.0 * uf) as i32,
        (back.y + 5.0 * uf) as i32,
        "< ALL GAMES",
        u,
        if over_back { INK } else { DIM },
    );

    let panel_w = (230.0 * uf).min(w - 24.0 * uf);
    let x = (w - panel_w) / 2.0;
    let mut y = h * 0.22;

    // The title, a letter at a time in drifting colours.
    let ts = if narrow { 4 * u } else { 6 * u };
    let title = "WYRM";
    let tw = text_width(title, ts);
    let mut tx = (w as i32 - tw) / 2;
    for (i, ch) in title.chars().enumerate() {
        let hue = 140.0 + i as f32 * 25.0 + t * 40.0;
        tx += c.text_shadowed(
            tx,
            y as i32,
            &ch.to_string(),
            ts,
            Rgba::hsl(hue, 0.75, 0.62),
        );
    }
    y += 8.0 * ts as f32 + 6.0 * uf;
    y = lines(
        c,
        w,
        y,
        "eat the glow, grow long, make them run into you",
        u,
        DIM,
    );
    y += 10.0 * uf;

    if let Some(d) = &st.death {
        let by = if d.by.is_empty() {
            "the edge of the world"
        } else {
            &d.by
        };
        let verdict = format!("you ran into {by}");
        c.text_centred(
            w as i32 / 2,
            y as i32,
            &verdict,
            fit_scale(&verdict, (w - 20.0 * uf) as i32, 2 * u),
            INK,
        );
        y += 19.0 * uf;
        c.text_centred(
            w as i32 / 2,
            y as i32,
            &format!("length {}   best {}", d.score, st.best),
            u,
            DIM,
        );
        y += 18.0 * uf;
    }

    // The name.
    let name = Rect::new(x, y, panel_w, 24.0 * uf);
    c.round_rect(name, 6.0 * uf, Rgba(255, 255, 255, 18));
    c.round_rect_line(
        name,
        6.0 * uf,
        1.0,
        if look.editing {
            GO
        } else {
            Rgba(255, 255, 255, 50)
        },
    );
    let shown = if look.name.is_empty() {
        (
            "your name",
            if look.editing {
                Rgba(244, 241, 255, 70)
            } else {
                DIM
            },
        )
    } else {
        (look.name, INK)
    };
    let s = fit_scale(shown.0, (name.w - 12.0 * uf) as i32, 2 * u);
    let nw = text_width(shown.0, s);
    let nx = (name.x + name.w / 2.0) as i32 - nw / 2;
    let ny = (name.y + name.h / 2.0) as i32 - 4 * s + s / 2;
    c.text(nx, ny, shown.0, s, shown.1);
    if look.editing && (now / 500.0) as i64 % 2 == 0 {
        let cx = if look.name.is_empty() {
            (name.x + name.w / 2.0) as i32
        } else {
            nx + nw + s
        };
        c.fill_rect(cx, ny - s / 2, s.max(1), 8 * s, GO);
    }
    y += name.h + 8.0 * uf;

    // PLAY.
    let play = Rect::new(x, y, panel_w, 28.0 * uf);
    let over = look.pointer.is_some_and(|(px, py)| play.contains(px, py));
    c.round_rect(
        play,
        6.0 * uf,
        if over { Rgba::rgb(120, 240, 162) } else { GO },
    );
    let label = if st.death.is_some() {
        "PLAY AGAIN"
    } else {
        "PLAY"
    };
    let lw = text_width(label, s);
    c.text(
        (play.x + play.w / 2.0) as i32 - lw / 2,
        (play.y + play.h / 2.0) as i32 - 4 * s + s / 2,
        label,
        s,
        Rgba::rgb(6, 38, 17),
    );
    y += play.h + 14.0 * uf;

    let how = if look.touch {
        "drag to steer - hold BOOST to go fast"
    } else {
        "mouse steers - hold click or space to boost"
    };
    y = lines(c, w, y, how, u, DIM);
    y += 4.0 * uf;
    let online = if st.connected {
        let n = st.board.people;
        format!("{n} {} here now", if n == 1 { "person" } else { "people" })
    } else {
        "connecting to the arena...".to_string()
    };
    c.text_centred(w as i32 / 2, y as i32, &online, u, Rgba(141, 255, 177, 230));

    Spots { name, play, back }
}

/// Centred text, wrapped to the screen; returns where the next line goes.
fn lines(c: &mut Canvas, w: f32, mut y: f32, s: &str, u: i32, ink: Rgba) -> f32 {
    for line in wrap(s, (w - 20.0 * u as f32) as i32, u) {
        c.text_centred(w as i32 / 2, y as i32, &line, u, ink);
        y += 10.0 * u as f32;
    }
    y
}
