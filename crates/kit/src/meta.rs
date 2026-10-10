//! The menu every game shares, over its own picture: Esc opens it (or a
//! game's own menu button, on a phone), Esc again goes back to the game.
//! Back to the game first, then the game's own entries, a word to us
//! (feedback, written here and sent with what the page knows), and at the
//! bottom, leaving the game: the only way out, so nothing on screen during
//! play invites leaving.
//!
//! The game draws it into its own pixel layer (`draw`), hands it clicks
//! (`click`) and Esc and Enter (`escape`, `enter`), and does what it picks.

use pixels::{fit_scale, text_width, wrap, Canvas, Rect, Rgba};

use crate::TextField;

/// The most feedback says (characters).
pub const FEEDBACK_MOST: u32 = 1200;
/// How long the thanks show before the menu comes back (ms).
const THANKS: f64 = 1800.0;

const SHADE: Rgba = Rgba(4, 6, 14, 150);
const PANEL: Rgba = Rgba(14, 16, 30, 238);
const BUTTON: Rgba = Rgba(255, 255, 255, 20);
const LIT: Rgba = Rgba(255, 214, 128, 46);
const INK: Rgba = Rgba::rgb(244, 241, 255);
const DIM: Rgba = Rgba(244, 241, 255, 140);
const GOLD: Rgba = Rgba::rgb(255, 214, 128);
const AWAY: Rgba = Rgba::rgb(255, 150, 140);

/// What the player picked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// Back to the game (the menu has closed).
    Resume,
    /// The game's own entry, by its place in the list it gave.
    Game(usize),
    /// Leave the game (`shell::exit`, once the game has done its part).
    Exit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Main,
    Feedback,
    /// A panel of the game's own is up over it (its settings, ...).
    Game,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spot {
    Pick(Pick),
    Feedback,
    Send,
    Back,
}

pub struct Meta {
    open: bool,
    page: Page,
    /// The game's name, over the menu.
    title: &'static str,
    field: TextField,
    spots: Vec<(Rect, Spot)>,
    panel: Rect,
    /// The last report: when, and whether it went out.
    sent: Option<(f64, bool)>,
    /// The field is to take the keys once it is placed (a hidden field
    /// cannot).
    focus: bool,
    /// What the game adds to a report (`key: value` lines); set it when
    /// the menu opens.
    pub context: String,
}

impl Meta {
    pub fn new(title: &'static str) -> Meta {
        Meta {
            open: false,
            page: Page::Main,
            title,
            field: TextField::prose(FEEDBACK_MOST, "what's on your mind?"),
            spots: Vec::new(),
            panel: Rect::new(0.0, 0.0, 0.0, 0.0),
            sent: None,
            focus: false,
            context: String::new(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Whether the game's own panel is up over it.
    pub fn in_game_panel(&self) -> bool {
        self.open && self.page == Page::Game
    }

    /// Whether a word is being written (keys are the field's, not the
    /// game's).
    pub fn typing(&self) -> bool {
        self.open && self.page == Page::Feedback
    }

    pub fn show(&mut self) {
        self.open = true;
        self.page = Page::Main;
    }

    pub fn hide(&mut self) {
        self.open = false;
        self.page = Page::Main;
        self.field.place(None);
        self.field.blur();
    }

    /// The game puts a panel of its own up (it draws it; Esc or `back`
    /// returns to the menu).
    pub fn game_panel(&mut self) {
        self.open = true;
        self.page = Page::Game;
    }

    /// Back to the menu's first page.
    pub fn back(&mut self) {
        self.page = Page::Main;
        self.field.place(None);
        self.field.blur();
    }

    /// Esc: open the menu, step back a page, or close it (`Resume`).
    pub fn escape(&mut self) -> Option<Pick> {
        if !self.open {
            self.show();
            return None;
        }
        if self.page != Page::Main {
            self.back();
            return None;
        }
        self.hide();
        Some(Pick::Resume)
    }

    /// Enter, while writing: send it.
    pub fn enter(&mut self, now: f64) {
        if self.typing() {
            self.send(now);
        }
    }

    fn send(&mut self, now: f64) {
        let text = self.field.value();
        if text.trim().is_empty() {
            self.field.focus();
            return;
        }
        let ok = crate::report::send("feedback", &self.context, &text);
        if ok {
            self.field.set_value("");
            self.field.blur();
        }
        self.sent = Some((now, ok));
    }

    /// A press at (x, y) on the layer: `None` if it is off the menu (the
    /// game's to use: back to the game, most likely), else what it picked
    /// (`Some(None)`: the menu's own business).
    pub fn click(&mut self, x: f32, y: f32, now: f64) -> Option<Option<Pick>> {
        if !self.open || self.page == Page::Game {
            return None;
        }
        let hit = self
            .spots
            .iter()
            .rev()
            .find(|(r, _)| r.contains(x, y))
            .map(|s| s.1);
        match hit {
            // The game takes it from here (a panel of its own puts the
            // menu back up with `game_panel`).
            Some(Spot::Pick(p)) => {
                self.hide();
                Some(Some(p))
            }
            Some(Spot::Feedback) => {
                self.page = Page::Feedback;
                self.sent = None;
                self.focus = true;
                Some(None)
            }
            Some(Spot::Send) => {
                self.send(now);
                Some(None)
            }
            Some(Spot::Back) => {
                self.back();
                Some(None)
            }
            None if self.panel.contains(x, y) => Some(None),
            None => None,
        }
    }

    /// Draw it over the picture (`c`, text at `ui`), the game's own entries
    /// in `items`, a few lines of help under it; `css` turns a layer box
    /// into a page box (for the writing field).
    pub fn draw(
        &mut self,
        c: &mut Canvas,
        ui: i32,
        items: &[&str],
        help: &[String],
        now: f64,
        css: impl Fn(Rect) -> (f64, f64, f64, f64),
    ) {
        self.spots.clear();
        if !self.open || self.page == Page::Game {
            self.field.place(None);
            return;
        }
        c.fill_rect(0, 0, c.w, c.h, SHADE);
        match self.page {
            Page::Feedback => self.draw_feedback(c, ui, now, css),
            _ => {
                self.field.place(None);
                self.draw_main(c, ui, items, help);
            }
        }
    }

    fn draw_main(&mut self, c: &mut Canvas, ui: i32, items: &[&str], help: &[String]) {
        let u = ui as f32;
        let (w, h) = (c.w as f32, c.h as f32);
        let bw = (210.0 * u).min(w - 32.0 * u);
        let n = items.len() as f32 + 3.0;
        // Buttons as tall as fit, the help let go first (a phone held
        // sideways gets low ones, every one on the screen).
        let fixed = 40.0 * u + 12.0 * u + 20.0 * u;
        let bh = ((h - 16.0 * u - fixed) / n - 6.0 * u).clamp(11.0 * u, 26.0 * u);
        let body = fixed + n * (bh + 6.0 * u);
        let help: &[String] = if body + help.len() as f32 * 10.0 * u <= h - 16.0 * u {
            help
        } else {
            &[]
        };
        let tall = body + help.len() as f32 * 10.0 * u;
        let x = (w - bw) / 2.0;
        self.panel = Rect::new(
            x - 14.0 * u,
            ((h - tall) / 2.0).max(6.0 * u),
            bw + 28.0 * u,
            tall,
        );
        c.round_rect(self.panel, 7.0 * u, PANEL);
        c.round_rect_line(self.panel, 7.0 * u, 1.0, GOLD.fade(0.25));
        let mut y = self.panel.y + 12.0 * u;
        let k = fit_scale(self.title, bw as i32, 2 * ui);
        c.text_centred(c.w / 2, y as i32, self.title, k, GOLD);
        y += 28.0 * u;
        let row = |c: &mut Canvas,
                   spots: &mut Vec<(Rect, Spot)>,
                   y: &mut f32,
                   text: &str,
                   s: Spot,
                   style: Rgba| {
            let b = Rect::new(x, *y, bw, bh);
            button(c, b, text, style, ui);
            spots.push((b, s));
            *y += bh + 6.0 * u;
        };
        row(
            c,
            &mut self.spots,
            &mut y,
            "back to the game",
            Spot::Pick(Pick::Resume),
            GOLD,
        );
        for (i, item) in items.iter().enumerate() {
            row(
                c,
                &mut self.spots,
                &mut y,
                item,
                Spot::Pick(Pick::Game(i)),
                INK,
            );
        }
        row(c, &mut self.spots, &mut y, "feedback", Spot::Feedback, INK);
        y += 12.0 * u;
        row(
            c,
            &mut self.spots,
            &mut y,
            "exit game",
            Spot::Pick(Pick::Exit),
            AWAY,
        );
        for (k, line) in help.iter().enumerate() {
            c.text_centred(c.w / 2, y as i32 + k as i32 * 10 * ui, line, ui, DIM);
        }
    }

    fn draw_feedback(
        &mut self,
        c: &mut Canvas,
        ui: i32,
        now: f64,
        css: impl Fn(Rect) -> (f64, f64, f64, f64),
    ) {
        let u = ui as f32;
        let (w, h) = (c.w as f32, c.h as f32);
        let bw = (300.0 * u).min(w - 32.0 * u);
        let x = (w - bw) / 2.0;
        let ask = "what's broken? what do you love? what do you want?";
        let asks = wrap(ask, bw as i32, ui);
        let box_h = (h * 0.32).clamp(44.0 * u, 110.0 * u);
        let bh = 22.0 * u;
        let tall = 12.0 * u
            + 26.0 * u
            + asks.len() as f32 * 10.0 * u
            + 6.0 * u
            + box_h
            + 8.0 * u
            + bh
            + 22.0 * u;
        self.panel = Rect::new(
            x - 14.0 * u,
            ((h - tall) / 2.0).max(6.0 * u),
            bw + 28.0 * u,
            tall,
        );
        c.round_rect(self.panel, 7.0 * u, PANEL);
        c.round_rect_line(self.panel, 7.0 * u, 1.0, GOLD.fade(0.25));
        let mut y = self.panel.y + 12.0 * u;
        c.text_centred(c.w / 2, y as i32, "feedback", 2 * ui, GOLD);
        y += 26.0 * u;
        for line in &asks {
            c.text_centred(c.w / 2, y as i32, line, ui, DIM);
            y += 10.0 * u;
        }
        y += 6.0 * u;
        // The writing: the field (invisible, so a phone offers its
        // keyboard) over a box showing what it holds.
        let b = Rect::new(x, y, bw, box_h);
        c.round_rect(b, 4.0 * u, Rgba(0, 0, 0, 120));
        let typing = self.field.focused();
        c.round_rect_line(
            b,
            4.0 * u,
            1.0,
            if typing {
                GOLD.fade(0.7)
            } else {
                DIM.fade(0.5)
            },
        );
        let text = self.field.value();
        let pad = 6.0 * u;
        let lines = if text.is_empty() {
            vec![]
        } else {
            wrap(&text, (bw - 2.0 * pad) as i32, ui)
        };
        let fit = ((box_h - 2.0 * pad) / (10.0 * u)).max(1.0) as usize;
        let from = lines.len().saturating_sub(fit);
        for (k, line) in lines[from..].iter().enumerate() {
            c.text(
                (b.x + pad) as i32,
                (b.y + pad) as i32 + k as i32 * 10 * ui,
                line,
                ui,
                INK,
            );
        }
        if text.is_empty() {
            let hint = if typing { "_" } else { "click here and type" };
            c.text((b.x + pad) as i32, (b.y + pad) as i32, hint, ui, DIM);
        } else if typing && ((now / 500.0) as u64).is_multiple_of(2) {
            let last = lines.last().map_or("", |l| l.as_str());
            let k = lines.len().saturating_sub(from + 1) as i32;
            let cx = (b.x + pad) as i32 + text_width(last, ui) + ui;
            c.text(cx, (b.y + pad) as i32 + k * 10 * ui, "_", ui, GOLD);
        }
        let count = format!("{}/{}", text.chars().count(), FEEDBACK_MOST);
        c.text(
            (b.x + b.w - pad) as i32 - text_width(&count, ui),
            (b.y + b.h - pad) as i32 - 7 * ui,
            &count,
            ui,
            DIM.fade(0.6),
        );
        self.field.place(Some(css(b)));
        if std::mem::take(&mut self.focus) {
            self.field.focus();
        }
        y += box_h + 8.0 * u;
        let half = (bw - 8.0 * u) / 2.0;
        let back = Rect::new(x, y, half, bh);
        let send = Rect::new(x + half + 8.0 * u, y, half, bh);
        button(c, back, "back", INK, ui);
        button(c, send, "send", GOLD, ui);
        self.spots.push((back, Spot::Back));
        self.spots.push((send, Spot::Send));
        y += bh + 8.0 * u;
        if let Some((at, ok)) = self.sent {
            let (note, col) = if ok {
                ("sent - thank you!", GOLD)
            } else {
                ("could not send it - try again", AWAY)
            };
            c.text_centred(c.w / 2, y as i32, note, ui, col);
            if ok && now - at > THANKS {
                self.back();
            }
        }
    }
}

fn button(c: &mut Canvas, b: Rect, text: &str, ink: Rgba, ui: i32) {
    let u = ui as f32;
    let lit = ink == GOLD;
    c.round_rect(b, 4.0 * u, if lit { LIT } else { BUTTON });
    c.round_rect_line(b, 4.0 * u, 1.0, ink.fade(0.45));
    let most = if b.h >= 24.0 * u { 2 * ui } else { ui };
    let k = fit_scale(text, b.w as i32 - 8 * ui, most).max(1);
    c.text_centred(
        (b.x + b.w / 2.0) as i32,
        (b.y + b.h / 2.0) as i32 - 3 * k,
        text,
        k,
        ink,
    );
}
