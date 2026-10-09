//! Playing with fingers: the left half of the screen is a stick (put a
//! thumb down anywhere and push), the right half turns the view, and
//! buttons on the right cast: the wand (held), jump, duck and aim (two
//! toggles), and the four spells with their icons and cooldowns.

use kit::input::Kind;
use pixels::{Canvas, Rect, Rgba};
use wandfall::laws::TICK_HZ;
use wandfall::loot::cooldown;
use wandfall::motion::{cast, keys};
use wandfall::proto::Own;

use crate::bar::{icon, KEYS};

/// Radians a CSS pixel of drag turns the view.
const LOOK: f32 = 0.0065;
/// CSS pixels before the stick moves you, and how far it reaches.
const DEAD: f64 = 12.0;
const REACH: f64 = 60.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Button {
    Fire,
    Jump,
    Crouch,
    Aim,
    Slot(usize),
    Menu,
}

/// A point in CSS pixels.
type At = (f64, f64);

#[derive(Default)]
pub struct Touch {
    /// The stick's finger, where it went down, and where it is.
    stick: Option<(i32, At, At)>,
    look: Option<(i32, (f64, f64))>,
    held: Vec<(i32, Button)>,
    pub aim: bool,
    /// Crouching (a toggle, as aim is).
    pub crouch: bool,
    asked: u8,
    /// The menu button was tapped (the page takes it).
    pub menu: bool,
}

/// Where each button is, in CSS pixels: (button, x, y, radius).
fn layout(w: f64, h: f64) -> Vec<(Button, f64, f64, f64)> {
    let (fx, fy) = (w - 62.0, h - 70.0);
    let mut v = vec![
        (Button::Fire, fx, fy, 38.0),
        (Button::Jump, w - 196.0, h - 38.0, 24.0),
        (Button::Crouch, w - 252.0, h - 34.0, 20.0),
        (Button::Aim, w - 36.0, h - 214.0, 20.0),
    ];
    // The spells in an arc about the wand's button, in reach of a thumb.
    for k in 0..4 {
        let a = (95.0 + 30.0 * k as f64).to_radians();
        v.push((
            Button::Slot(k),
            fx + a.cos() * 92.0,
            fy - a.sin() * 92.0,
            21.0,
        ));
    }
    v.push((Button::Menu, w / 2.0, 58.0, 18.0));
    v
}

impl Touch {
    /// A finger did something; how far the view turns (yaw, pitch).
    pub fn finger(&mut self, id: i32, kind: Kind, x: f64, y: f64, css: (f64, f64)) -> (f32, f32) {
        match kind {
            Kind::Down => {
                let hit = layout(css.0, css.1).into_iter().find(|&(_, bx, by, r)| {
                    (x - bx).powi(2) + (y - by).powi(2) < (r + 8.0).powi(2)
                });
                match hit {
                    Some((b, ..)) => {
                        match b {
                            Button::Aim => self.aim = !self.aim,
                            Button::Crouch => self.crouch = !self.crouch,
                            Button::Slot(k) => self.asked |= cast::SLOT[k],
                            Button::Menu => self.menu = true,
                            _ => {}
                        }
                        self.held.push((id, b));
                    }
                    None if x < css.0 / 2.0 && self.stick.is_none() => {
                        self.stick = Some((id, (x, y), (x, y)))
                    }
                    None => self.look = Some((id, (x, y))),
                }
                (0.0, 0.0)
            }
            Kind::Move => {
                if let Some(s) = self.stick.as_mut().filter(|s| s.0 == id) {
                    s.2 = (x, y);
                }
                match self.look.as_mut().filter(|l| l.0 == id) {
                    Some(l) => {
                        let d = ((x - l.1 .0) as f32 * LOOK, -(y - l.1 .1) as f32 * LOOK);
                        l.1 = (x, y);
                        d
                    }
                    None => (0.0, 0.0),
                }
            }
            Kind::Up | Kind::Cancel => {
                self.held.retain(|h| h.0 != id);
                if self.stick.is_some_and(|s| s.0 == id) {
                    self.stick = None;
                }
                if self.look.is_some_and(|l| l.0 == id) {
                    self.look = None;
                }
                (0.0, 0.0)
            }
            Kind::Hover => (0.0, 0.0),
        }
    }

    /// For `?perf=1`: the stick (finger, push) and the buttons held.
    pub fn debug(&self) -> String {
        let push = self.stick.map_or(0.0, |(_, o, n)| {
            ((n.0 - o.0).powi(2) + (n.1 - o.1).powi(2)).sqrt()
        });
        format!(
            "stick {:.0} held {} keys {}",
            push,
            self.held.len(),
            self.keys()
        )
    }

    fn holding(&self, b: Button) -> bool {
        self.held.iter().any(|h| h.1 == b)
    }

    /// The keys the fingers hold now: the stick pushed to its edge
    /// forward sprints.
    pub fn keys(&self) -> u16 {
        let mut k = 0;
        if let Some((_, o, n)) = self.stick {
            let (dx, dy) = (n.0 - o.0, n.1 - o.1);
            let len = (dx * dx + dy * dy).sqrt();
            if len > DEAD {
                let (ux, uy) = (dx / len, dy / len);
                if uy < -0.38 {
                    k |= keys::FWD;
                    if len > REACH * 0.9 && uy < -0.8 {
                        k |= keys::SPRINT;
                    }
                }
                if uy > 0.38 {
                    k |= keys::BACK;
                }
                if ux < -0.38 {
                    k |= keys::LEFT;
                }
                if ux > 0.38 {
                    k |= keys::RIGHT;
                }
            }
        }
        if self.holding(Button::Fire) {
            k |= keys::FIRE;
        }
        if self.holding(Button::Jump) {
            k |= keys::JUMP;
        }
        if self.aim {
            k |= keys::AIM;
        }
        if self.crouch {
            k |= keys::CROUCH;
        }
        k
    }

    /// Spells tapped since the last call.
    pub fn casts(&mut self) -> u8 {
        std::mem::take(&mut self.asked)
    }

    /// The stick and the buttons, over the HUD (`scale` CSS pixels a
    /// layer pixel).
    pub fn draw(&self, c: &mut Canvas, own: Option<&Own>, css: (f64, f64), scale: f64) {
        let px = |v: f64| (v / scale) as f32;
        let ink = Rgba::rgb(250, 246, 236);
        if let Some((_, o, n)) = self.stick {
            c.ring(px(o.0), px(o.1), px(REACH), 2.0, ink.fade(0.35));
            let (dx, dy) = (n.0 - o.0, n.1 - o.1);
            let len = (dx * dx + dy * dy).sqrt().max(1.0);
            let k = len.min(REACH) / len;
            c.circle(px(o.0 + dx * k), px(o.1 + dy * k), px(22.0), ink.fade(0.45));
        }
        for (b, x, y, r) in layout(css.0, css.1) {
            let held = self.holding(b)
                || (b == Button::Aim && self.aim)
                || (b == Button::Crouch && self.crouch);
            c.circle(
                px(x),
                px(y),
                px(r),
                Rgba(10, 12, 24, if held { 200 } else { 120 }),
            );
            c.ring(
                px(x),
                px(y),
                px(r),
                1.5,
                ink.fade(if held { 0.9 } else { 0.45 }),
            );
            let label = |c: &mut Canvas, t: &str| {
                c.text_centred(px(x) as i32, (px(y) - 3.0) as i32, t, 1, ink)
            };
            match b {
                Button::Fire => label(c, "cast"),
                Button::Jump => label(c, "jump"),
                Button::Aim => label(c, "aim"),
                Button::Menu => label(c, "menu"),
                Button::Crouch => label(c, "duck"),
                Button::Slot(k) => {
                    let Some((sp, rank)) = own.and_then(|o| o.slots[k]) else {
                        label(c, KEYS[k]);
                        continue;
                    };
                    let s = px(r * 1.42);
                    let tile = Rect::new(px(x) - s / 2.0, px(y) - s / 2.0, s, s);
                    icon(c, sp, tile);
                    let cd = own.map_or(0, |o| o.cds[k]) as u32;
                    if cd > 0 {
                        let left = (cd as f32 / cooldown(sp, rank).max(1) as f32).min(1.0);
                        c.sweep(tile, s * 0.2, 1.0 - left, 1.0, Rgba(4, 6, 12, 175));
                        c.text_centred(
                            px(x) as i32,
                            (px(y) - 3.0) as i32,
                            &format!("{}", cd.div_ceil(TICK_HZ)),
                            1,
                            ink,
                        );
                    }
                }
            }
        }
    }
}
