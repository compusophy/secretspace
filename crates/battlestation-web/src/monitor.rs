//! What the monitor shows, drawn pixel by pixel: a wallpaper, a bar with
//! the time, the terminal's window (its lines in colour, the caret
//! blinking), green rain when the terminal asks for it, and the mouse's
//! arrow.

use battlestation::laws::SCREEN_PX;
use battlestation::term::{Ink, Mode, Term};
use pixels::{Canvas, Rect, Rgba};

/// The terminal's window and its text.
pub const WIN: (i32, i32, i32, i32) = (14, 20, 452, 240);
const BAR: i32 = 12;
const LINE: i32 = 9;
const PAD: i32 = 6;

/// Columns and rows of text the window holds.
pub fn grid() -> (usize, usize) {
    let (_, _, w, h) = WIN;
    (
        ((w - 2 * PAD) / pixels::font::CELL_W) as usize,
        ((h - BAR - 2 * PAD) / LINE) as usize,
    )
}

pub fn ink(i: Ink) -> Rgba {
    match i {
        Ink::Text => Rgba::rgb(222, 226, 238),
        Ink::Dim => Rgba::rgb(122, 130, 156),
        Ink::Prompt => Rgba::rgb(126, 232, 166),
        Ink::Accent => Rgba::rgb(122, 190, 255),
        Ink::Good => Rgba::rgb(150, 244, 180),
        Ink::Bad => Rgba::rgb(255, 126, 126),
        Ink::Warm => Rgba::rgb(255, 202, 122),
    }
}

const ARROW: [&str; 14] = [
    "X.........",
    "XX........",
    "X#X.......",
    "X##X......",
    "X###X.....",
    "X####X....",
    "X#####X...",
    "X######X..",
    "X#######X.",
    "X####XXXX.",
    "X#XX#X....",
    "XX..X#X...",
    "X...X#X...",
    ".....XX...",
];

pub struct Screen {
    pub c: Canvas,
    wall: Canvas,
    /// Where the arrow is (pixels).
    pub cursor: (f32, f32),
    /// Each column of rain: how far down its head is, how fast it falls.
    rain: Vec<(f32, f32)>,
}

impl Default for Screen {
    fn default() -> Screen {
        Screen::new()
    }
}

impl Screen {
    pub fn new() -> Screen {
        let (w, h) = SCREEN_PX;
        let cols = (w / pixels::font::CELL_W) as usize;
        Screen {
            c: Canvas::new(w, h),
            wall: wallpaper(w, h),
            cursor: (w as f32 * 0.7, h as f32 * 0.55),
            rain: (0..cols)
                .map(|k| {
                    let r = hash((k as u32).wrapping_mul(7919).wrapping_add(13));
                    (-((r % 40) as f32) * LINE as f32, 60.0 + (r % 90) as f32)
                })
                .collect(),
        }
    }

    /// Move the arrow by (dx, dy) pixels, kept on the screen.
    pub fn nudge(&mut self, dx: f32, dy: f32) {
        let (w, h) = SCREEN_PX;
        self.cursor.0 = (self.cursor.0 + dx).clamp(0.0, w as f32 - 1.0);
        self.cursor.1 = (self.cursor.1 + dy).clamp(0.0, h as f32 - 1.0);
    }

    /// Draw it all: the terminal, the time, `t` seconds in (for the
    /// caret's blink and the rain), `dt` since last.
    pub fn draw(&mut self, term: &Term, clock: &str, t: f32, dt: f32) {
        if term.mode == Mode::Matrix {
            self.matrix(t, dt);
        } else {
            self.c.data.copy_from_slice(&self.wall.data);
            self.bar(clock);
            self.window(term, t);
        }
        self.arrow();
    }

    fn bar(&mut self, clock: &str) {
        let w = self.c.w;
        self.c.fill_rect(0, 0, w, BAR, Rgba(5, 6, 12, 215));
        self.c.circle(8.0, 6.0, 2.5, Rgba::rgb(126, 232, 166));
        self.c
            .text(15, 3, "battlestation", 1, Rgba::rgb(200, 204, 220));
        self.c.text(100, 3, "terminal", 1, Rgba(200, 204, 220, 150));
        let cw = pixels::text_width(clock, 1);
        self.c
            .text(w - cw - 6, 3, clock, 1, Rgba::rgb(222, 226, 238));
    }

    fn window(&mut self, term: &Term, t: f32) {
        let (x, y, w, h) = WIN;
        let c = &mut self.c;
        // Its shadow, its body, its title.
        c.round_rect(
            Rect::new(x as f32 + 2.0, y as f32 + 4.0, w as f32, h as f32),
            6.0,
            Rgba(0, 0, 0, 90),
        );
        c.round_rect(
            Rect::new(x as f32, y as f32, w as f32, h as f32),
            5.0,
            Rgba(11, 13, 20, 244),
        );
        c.fill_rect(x + 3, y + BAR, w - 6, 1, Rgba(255, 255, 255, 14));
        for (k, col) in [
            Rgba::rgb(255, 96, 92),
            Rgba::rgb(255, 189, 68),
            Rgba::rgb(0, 202, 78),
        ]
        .into_iter()
        .enumerate()
        {
            c.circle((x + 9 + k as i32 * 9) as f32, (y + 6) as f32, 2.6, col);
        }
        c.text_centred(
            x + w / 2,
            y + 3,
            "you@battlestation: ~",
            1,
            Rgba(200, 204, 220, 170),
        );
        let (cols, rows) = grid();
        let v = term.view(cols, rows);
        let (tx, ty) = (x + PAD, y + BAR + PAD);
        for (r, line) in v.lines.iter().enumerate() {
            let mut cx = tx;
            for (s, i) in &line.0 {
                cx += c.text(cx, ty + r as i32 * LINE, s, 1, ink(*i));
            }
        }
        if let Some((row, col)) = v.caret {
            if (t * 1.9).fract() < 0.6 {
                let (cx, cy) = (tx + col as i32 * 6, ty + row as i32 * LINE - 1);
                c.fill_rect(cx, cy, 6, LINE, Rgba(126, 232, 166, 200));
            }
        }
    }

    fn matrix(&mut self, t: f32, dt: f32) {
        let (w, h) = (self.c.w, self.c.h);
        self.c.clear(Rgba::rgb(0, 5, 2));
        let tick = (t * 12.0) as u32;
        for (k, (head, speed)) in self.rain.iter_mut().enumerate() {
            *head += *speed * dt.clamp(0.0, 0.1);
            if *head > (h + 18 * LINE) as f32 {
                *head = -((hash((k as u32).wrapping_add(tick)) % 30) as f32) * LINE as f32;
            }
            let x = k as i32 * pixels::font::CELL_W;
            let top = (*head / LINE as f32) as i32;
            for n in 0..18 {
                let row = top - n;
                let y = row * LINE;
                if y < -LINE || y > h {
                    continue;
                }
                let seed = (k as u32)
                    .wrapping_mul(131)
                    .wrapping_add((row as u32).wrapping_mul(7))
                    .wrapping_add(tick / 3);
                let glyph = (33 + hash(seed) % 90) as u8 as char;
                let fade = 1.0 - n as f32 / 18.0;
                let col = if n == 0 {
                    Rgba::rgb(210, 255, 220)
                } else {
                    Rgba(40, 230, 90, (fade * 230.0) as u8)
                };
                self.c.text(x, y, &glyph.to_string(), 1, col);
            }
        }
        let note = "any key";
        let nw = pixels::text_width(note, 1);
        self.c
            .text(w - nw - 6, h - 12, note, 1, Rgba(120, 255, 160, 120));
    }

    fn arrow(&mut self) {
        let (x0, y0) = (self.cursor.0 as i32, self.cursor.1 as i32);
        for (r, row) in ARROW.iter().enumerate() {
            for (k, ch) in row.chars().enumerate() {
                let c = match ch {
                    'X' => Rgba::rgb(10, 10, 14),
                    '#' => Rgba::rgb(250, 250, 252),
                    _ => continue,
                };
                self.c.pixel(x0 + k as i32, y0 + r as i32, c);
            }
        }
    }

    /// The picture's mean colour (0 to 1), from a sparse sample: the light
    /// it throws on the desk.
    pub fn mean(&self) -> [f32; 3] {
        let mut s = [0.0f32; 3];
        let mut n = 0.0f32;
        for px in self.c.data.chunks_exact(4).step_by(97) {
            for k in 0..3 {
                s[k] += px[k] as f32 / 255.0;
            }
            n += 1.0;
        }
        s.map(|v| v / n.max(1.0))
    }
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

/// The desktop's wallpaper: night purple into deep blue, a soft glow
/// low on the right, a scatter of faint stars.
fn wallpaper(w: i32, h: i32) -> Canvas {
    let mut c = Canvas::new(w, h);
    for y in 0..h {
        let k = y as f32 / h as f32;
        let col = Rgba::rgb(26, 16, 52).mix(Rgba::rgb(6, 10, 26), k);
        c.fill_rect(0, y, w, 1, col);
    }
    c.glow(
        w as f32 * 0.78,
        h as f32 * 0.82,
        h as f32 * 0.75,
        Rgba(255, 70, 160, 70),
    );
    c.glow(
        w as f32 * 0.2,
        h as f32 * 0.1,
        h as f32 * 0.6,
        Rgba(80, 120, 255, 50),
    );
    for k in 0..90u32 {
        let r = hash(k.wrapping_mul(2654).wrapping_add(7));
        let (x, y) = ((r % w as u32) as i32, ((r >> 12) % h as u32) as i32);
        c.pixel(x, y, Rgba(255, 255, 255, 40 + (r >> 24) as u8 % 80));
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use battlestation::term::Facts;

    #[test]
    fn the_terminal_fits_the_screen() {
        let (cols, rows) = grid();
        assert!(cols >= 70 && rows >= 22, "{cols}x{rows}");
        let (x, y, w, h) = WIN;
        assert!(x + w <= SCREEN_PX.0 && y + h <= SCREEN_PX.1);
    }

    #[test]
    fn it_draws_text_and_rain() {
        let mut s = Screen::new();
        let mut t = Term::new();
        s.draw(&t, "12:34", 0.1, 0.016);
        let lit =
            s.c.data
                .chunks_exact(4)
                .filter(|p| p[0] > 200 && p[1] > 200)
                .count();
        assert!(lit > 200, "text shows: {lit}");
        let f = Facts::default();
        for k in ["m", "a", "t", "r", "i", "x", "Enter"] {
            t.key(k, false, &f);
        }
        for i in 0..60 {
            s.draw(&t, "12:34", i as f32 / 60.0, 1.0 / 60.0);
        }
        let green =
            s.c.data
                .chunks_exact(4)
                .filter(|p| p[1] > 150 && p[0] < 120)
                .count();
        assert!(green > 100, "rain falls: {green}");
        s.nudge(-1e6, 1e6);
        assert_eq!(s.cursor, (0.0, SCREEN_PX.1 as f32 - 1.0));
    }
}
