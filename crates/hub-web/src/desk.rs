//! Battlestation's card: the desk typing by itself, as pixel art. Its
//! monitor runs the game's own terminal (seen from afar, each letter a
//! dot in its colour); below, the desk from above, where the game's own
//! hands (`battlestation::hands`) type the demo's keys, the keys light as
//! they go down, and the right hand leaves for the mouse and comes back.

use battlestation::demo::{Demo, Ev};
use battlestation::hands::{mouse_for, Hands};
use battlestation::keys::{self, Side};
use battlestation::laws::{CURSOR_SPEED, SCREEN_PX};
use battlestation::term::{Facts, Ink, Term};
use pixels::{Canvas, Rect, Rgba};

const SKIN: Rgba = Rgba::rgb(222, 168, 132);
const SLEEVE: Rgba = Rgba::rgb(22, 24, 34);

/// The desk's part of the world shown from above: x from, to; z from, to.
const AREA: [f32; 4] = [-0.40, 0.46, -0.36, 0.06];

fn ink(i: Ink) -> Rgba {
    match i {
        Ink::Text => Rgba::rgb(210, 214, 228),
        Ink::Dim => Rgba::rgb(110, 118, 146),
        Ink::Prompt => Rgba::rgb(126, 232, 166),
        Ink::Accent => Rgba::rgb(122, 190, 255),
        Ink::Good => Rgba::rgb(150, 244, 180),
        Ink::Bad => Rgba::rgb(255, 126, 126),
        Ink::Warm => Rgba::rgb(255, 202, 122),
    }
}

/// The light strip's and the keys' colour, `x` along, `t` seconds in.
fn rainbow(x: f32, t: f32) -> Rgba {
    Rgba::hsl(
        (x * 0.55 - t * 0.05).rem_euclid(1.0) * 160.0 + 180.0,
        0.85,
        0.6,
    )
}

pub struct DeskWatch {
    demo: Demo,
    term: Term,
    hands: Hands,
    heat: Vec<f32>,
    cursor: (f32, f32),
    last: Option<f64>,
    buf: Canvas,
}

impl Default for DeskWatch {
    fn default() -> DeskWatch {
        DeskWatch::new()
    }
}

impl DeskWatch {
    pub fn new() -> DeskWatch {
        DeskWatch {
            demo: Demo::new(0xde5c),
            term: Term::new(),
            hands: Hands::new(),
            heat: vec![0.0; keys::layout().len()],
            cursor: (SCREEN_PX.0 as f32 * 0.7, SCREEN_PX.1 as f32 * 0.55),
            last: None,
            buf: Canvas::new(1, 1),
        }
    }

    fn step(&mut self, now: f64) {
        let dt = self
            .last
            .map_or(0.0, |l| ((now - l) / 1000.0) as f32)
            .clamp(0.0, 0.1);
        self.last = Some(now);
        let facts = Facts {
            gpu: "",
            when: "",
            up: now / 1000.0,
            screen: SCREEN_PX,
        };
        for ev in self.demo.step(dt) {
            match ev {
                Ev::Key { code, key, down } => {
                    self.hands.key(code, down);
                    if down {
                        if let Some(i) = keys::layout().iter().position(|k| k.code == code) {
                            self.heat[i] = 1.0;
                        }
                        self.term.key(&key, false, &facts);
                    }
                }
                Ev::Move { dx, dy } => {
                    let (w, h) = (SCREEN_PX.0 as f32, SCREEN_PX.1 as f32);
                    self.cursor.0 = (self.cursor.0 + dx * CURSOR_SPEED).clamp(0.0, w - 1.0);
                    self.cursor.1 = (self.cursor.1 + dy * CURSOR_SPEED).clamp(0.0, h - 1.0);
                    self.hands.stir(dx.hypot(dy));
                    let (x, z) = mouse_for(self.cursor, SCREEN_PX);
                    self.hands.put_mouse(x, z);
                }
                Ev::Button { b, down } => self.hands.button(b, down),
            }
        }
        self.hands.step(dt);
        for h in &mut self.heat {
            *h *= (-dt / 0.35).exp();
        }
    }

    /// Draw the card's picture into `b` (text scale `u`).
    pub fn draw(&mut self, c: &mut Canvas, b: Rect, u: i32, now: f64) {
        self.step(now);
        let t = (now / 1000.0) as f32;
        let (w, h) = (b.w.max(1.0) as i32, b.h.max(1.0) as i32);
        self.buf.resize(w, h);
        let p = &mut self.buf;
        let (wf, hf) = (w as f32, h as f32);
        for y in 0..h {
            let k = y as f32 / hf;
            p.fill_rect(
                0,
                y,
                w,
                1,
                Rgba::rgb(20, 12, 36).mix(Rgba::rgb(8, 8, 18), k),
            );
        }
        // The desk, from its back edge down.
        let desk = (hf * 0.58) as i32;
        p.fill_rect(0, desk, w, h - desk, Rgba::rgb(38, 26, 22));
        p.fill_rect(0, desk, w, 1, Rgba::rgb(70, 50, 42));
        // The light strip behind it, glowing.
        for x in (0..w).step_by(2) {
            let col = rainbow(x as f32 / wf, t);
            p.fill_rect(x, desk - 2, 2, 1, col);
            p.pixel(x, desk - 3, col.fade(0.35));
        }
        monitor(p, &self.term, self.cursor, wf, desk, u, t);
        desk_top(p, &self.hands, &self.heat, (wf, hf), desk, t);
        c.blit(&self.buf, b.x as i32, b.y as i32, 6.0 * u as f32);
    }
}

/// The desk from above: the mat, the keyboard (`heat`: each key's light
/// after a press), the mouse, the hands.
fn desk_top(p: &mut Canvas, hands: &Hands, heat: &[f32], (wf, hf): (f32, f32), desk: i32, t: f32) {
    {
        let top = desk as f32 + 3.0;
        let at = |x: f32, z: f32| {
            (
                (x - AREA[0]) / (AREA[1] - AREA[0]) * wf,
                top + (z - AREA[2]) / (AREA[3] - AREA[2]) * (hf - top),
            )
        };
        let (mx0, my0) = at(-0.44, -0.34);
        let (mx1, my1) = at(0.47, -0.035);
        p.round_rect(
            Rect::new(mx0, my0, mx1 - mx0, my1 - my0),
            2.0,
            Rgba::rgb(14, 15, 20),
        );
        // The keys, each where the game has it, lit when pressed.
        let (kx0, ky0) = at(keys::at(0.0, 0.0)[0], keys::at(0.0, 0.0)[2]);
        let (kx1, ky1) = at(keys::at(18.25, 6.5)[0], keys::at(18.25, 6.5)[2]);
        p.round_rect(
            Rect::new(kx0 - 2.0, ky0 - 2.0, kx1 - kx0 + 4.0, ky1 - ky0 + 4.0),
            2.0,
            Rgba::rgb(30, 31, 40),
        );
        for (i, k) in keys::layout().iter().enumerate() {
            let a = keys::at(k.x + 0.08, k.y + 0.1);
            let z = keys::at(k.x + k.w - 0.08, k.y + 0.9);
            let (x0, y0) = at(a[0], a[2]);
            let (x1, y1) = at(z[0], z[2]);
            let lit = rainbow(a[0], t).mix(Rgba::rgb(255, 255, 255), heat[i] * 0.7);
            let col = Rgba::rgb(28, 28, 38).mix(lit, 0.35 + 0.65 * heat[i]);
            p.fill_rect(
                x0 as i32,
                y0 as i32,
                ((x1 - x0) as i32).max(1),
                ((y1 - y0) as i32).max(1),
                col,
            );
        }
        // The mouse.
        let m = hands.mouse.at();
        let (cx, cy) = at(m[0], m[2]);
        let (mw, mh) = (
            0.064 / (AREA[1] - AREA[0]) * wf,
            0.118 / (AREA[3] - AREA[2]) * (hf - top),
        );
        p.round_rect(
            Rect::new(cx - mw / 2.0, cy - mh / 2.0, mw, mh),
            mw / 2.0,
            Rgba::rgb(20, 20, 26),
        );
        p.pixel(cx as i32, (cy + mh * 0.3) as i32, rainbow(m[0], t));
        // The hands: the sleeve, the palm, each finger root to tip.
        let px_m = wf / (AREA[1] - AREA[0]);
        for side in [Side::Left, Side::Right] {
            let pose = hands.pose(side);
            let (wx, wy) = at(pose.wrist[0], pose.wrist[2]);
            let (ex, ey) = at(pose.elbow[0], pose.elbow[2]);
            p.line(wx, wy, ex, ey + (hf - top), 0.075 * px_m, SLEEVE);
            p.line(
                wx,
                wy,
                ex,
                ey,
                0.045 * px_m,
                SKIN.mix(Rgba::rgb(0, 0, 0), 0.15),
            );
            let mid = |a: usize| pose.fingers[a][0];
            let k = [(mid(1)[0] + mid(4)[0]) / 2.0, (mid(1)[2] + mid(4)[2]) / 2.0];
            let (kx, ky) = at(k[0], k[1]);
            p.line(wx, wy, kx, ky, 0.07 * px_m, SKIN);
            for f in &pose.fingers {
                for s in 0..3 {
                    let (ax, ay) = at(f[s][0], f[s][2]);
                    let (bx, by) = at(f[s + 1][0], f[s + 1][2]);
                    p.line(ax, ay, bx, by, 0.016 * px_m, SKIN);
                }
            }
        }
    }
}

/// The monitor, seen straight on, its terminal from afar: every letter a
/// dot in its colour, the caret blinking, the arrow.
fn monitor(p: &mut Canvas, term: &Term, cursor: (f32, f32), wf: f32, desk: i32, u: i32, t: f32) {
    let sw = (wf * 0.5) as i32;
    let sh = (sw * 9 / 16).min(desk - 10);
    let sx = (wf as i32 - sw) / 2;
    let sy = (desk - sh) / 2 - 2;
    p.fill_rect(
        sx + sw / 2 - 2,
        sy + sh,
        4,
        desk - sy - sh,
        Rgba::rgb(16, 16, 22),
    );
    p.glow(
        (sx + sw / 2) as f32,
        (sy + sh / 2) as f32,
        sw as f32 * 0.8,
        Rgba(90, 120, 255, 26),
    );
    p.fill_rect(sx - 2, sy - 2, sw + 4, sh + 4, Rgba::rgb(14, 14, 20));
    p.fill_rect(sx, sy, sw, sh, Rgba::rgb(10, 12, 22));
    let cell = if u >= 2 { 2 } else { 1 };
    let (cols, rows) = (
        (sw / (cell * 2)) as usize,
        (sh / (cell * 2)).max(1) as usize - 1,
    );
    let v = term.view(cols.max(1), rows.max(1));
    for (r, line) in v.lines.iter().enumerate() {
        let mut col = 0;
        for (s, i) in &line.0 {
            for ch in s.chars() {
                if ch != ' ' {
                    p.fill_rect(
                        sx + 1 + col * cell * 2,
                        sy + 2 + r as i32 * cell * 2,
                        cell,
                        cell,
                        ink(*i),
                    );
                }
                col += 1;
            }
        }
    }
    if let Some((r, c)) = v.caret {
        if (t * 1.9).fract() < 0.6 {
            p.fill_rect(
                sx + 1 + c as i32 * cell * 2,
                sy + 2 + r as i32 * cell * 2,
                cell * 2,
                cell,
                Rgba::rgb(126, 232, 166),
            );
        }
    }
    let (ax, ay) = (
        sx + (cursor.0 / SCREEN_PX.0 as f32 * sw as f32) as i32,
        sy + (cursor.1 / SCREEN_PX.1 as f32 * sh as f32) as i32,
    );
    p.fill_rect(ax, ay, cell, cell * 2, Rgba::rgb(250, 250, 252));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_card_types_by_itself() {
        let mut d = DeskWatch::new();
        let mut c = Canvas::new(200, 120);
        for f in 0..(20 * 60) {
            d.draw(
                &mut c,
                Rect::new(4.0, 4.0, 182.0, 104.0),
                1,
                f as f64 * 1000.0 / 60.0,
            );
        }
        assert!(d.term.ran() >= 1, "the demo ran a command");
        let skin = c
            .data
            .chunks_exact(4)
            .filter(|p| p[0] > 180 && p[1] > 130 && p[1] < 190 && p[2] < 160)
            .count();
        assert!(skin > 50, "the hands show: {skin}");
    }
}
