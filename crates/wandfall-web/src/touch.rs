//! Playing with fingers: the left half of the screen is a stick (put a
//! thumb down anywhere and push), the right half turns the view, and
//! buttons on the right cast: the wand (held, and dragged it turns the
//! view too), jump, duck and aim (two toggles), and the four spells with
//! their icons and cooldowns. The menu button sits in the top left
//! corner, there even when you are out.

use kit::input::Kind;
use pixels::{Canvas, Rect, Rgba};
use wandfall::laws::TICK_HZ;
use wandfall::loot::cooldown;
use wandfall::motion::{cast, keys, Body};
use wandfall::proto::Own;

use crate::bar::{icon, KEYS};

/// Radians a CSS pixel of drag turns the view.
const LOOK: f32 = 0.0065;
/// CSS pixels before the stick moves you, and how far it reaches.
const DEAD: f64 = 12.0;
const REACH: f64 = 60.0;
/// CSS pixels round a button that still press it.
const PAD: f64 = 8.0;

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
    look: Option<(i32, At)>,
    /// The fingers on buttons, and where each was last.
    held: Vec<(i32, Button, At)>,
    pub aim: bool,
    /// Crouching (a toggle, as aim is).
    pub crouch: bool,
    asked: u8,
    /// The menu button was tapped (the page takes it).
    pub menu: bool,
}

/// Where the menu button is, in CSS pixels: (x, y, radius).
fn menu_spot() -> (f64, f64, f64) {
    (30.0, 28.0, 18.0)
}

/// Where each button is, in CSS pixels: (button, x, y, radius).
fn layout(w: f64, h: f64) -> Vec<(Button, f64, f64, f64)> {
    let (fx, fy) = (w - 62.0, h - 70.0);
    let mut v = vec![
        (Button::Fire, fx, fy, 38.0),
        (Button::Jump, w - 196.0, h - 38.0, 24.0),
        (Button::Crouch, w - 254.0, h - 34.0, 22.0),
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
    let (mx, my, mr) = menu_spot();
    v.push((Button::Menu, mx, my, mr));
    v
}

/// The button a finger at (`x`, `y`) presses: of those whose padded
/// circle holds it, the one whose edge is nearest (so where two pads
/// overlap, the nearer button wins).
fn hit(w: f64, h: f64, x: f64, y: f64) -> Option<Button> {
    let edge = |&(_, bx, by, r): &(Button, f64, f64, f64)| (x - bx).hypot(y - by) - r;
    layout(w, h)
        .into_iter()
        .filter(|b| edge(b) < PAD)
        .min_by(|a, b| edge(a).total_cmp(&edge(b)))
        .map(|b| b.0)
}

/// Whether a finger at (`x`, `y`) CSS pixels is on the menu button.
pub fn on_menu(x: f64, y: f64) -> bool {
    let (mx, my, r) = menu_spot();
    (x - mx).hypot(y - my) < r + PAD
}

impl Touch {
    /// A finger did something; how far the view turns (yaw, pitch).
    pub fn finger(&mut self, id: i32, kind: Kind, x: f64, y: f64, css: (f64, f64)) -> (f32, f32) {
        match kind {
            Kind::Down => {
                match hit(css.0, css.1, x, y) {
                    Some(b) => {
                        match b {
                            Button::Aim => self.aim = !self.aim,
                            Button::Crouch => self.crouch = !self.crouch,
                            // A jump stands you up (out of a slide, too).
                            Button::Jump => self.crouch = false,
                            Button::Slot(k) => self.asked |= cast::SLOT[k],
                            Button::Menu => self.menu = true,
                            Button::Fire => {}
                        }
                        self.held.push((id, b, (x, y)));
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
                // The look finger turns the view, and so does the thumb
                // holding the wand down (aim while you cast).
                let last = match self.look.as_mut().filter(|l| l.0 == id) {
                    Some(l) => Some(&mut l.1),
                    None => self
                        .held
                        .iter_mut()
                        .find(|h| h.0 == id && h.1 == Button::Fire)
                        .map(|h| &mut h.2),
                };
                match last {
                    Some(at) => {
                        let d = ((x - at.0) as f32 * LOOK, -(y - at.1) as f32 * LOOK);
                        *at = (x, y);
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

    /// A tick played (your body before and after): duck lets go where
    /// holding it on would only get in the way, once a slide ends and
    /// when you push to sprint (sprinting stands you up).
    pub fn ease(&mut self, was: &Body, is: &Body) {
        let sprint = self.keys() & keys::SPRINT != 0;
        if self.crouch && ((was.slide && !is.slide) || (sprint && !is.slide && !was.slide)) {
            self.crouch = false;
        }
    }

    /// Every finger and toggle let go (you are out, or somewhere new).
    pub fn reset(&mut self) {
        *self = Touch::default();
    }

    /// The stick and the buttons, over the HUD (`scale` CSS pixels a
    /// layer pixel; words at `ui`).
    pub fn draw(&self, c: &mut Canvas, own: Option<&Own>, css: (f64, f64), scale: f64, ui: i32) {
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
            disc(c, (px(x), px(y), px(r)), held);
            // A word in the button, as big as fits it (to its ring).
            let label = |c: &mut Canvas, t: &str| {
                let k = pixels::fit_scale(t, (2.0 * px(r)) as i32 + 4, ui);
                c.text_centred(px(x) as i32, px(y) as i32 - 3 * k, t, k, ink)
            };
            match b {
                Button::Fire => label(c, "cast"),
                Button::Jump => label(c, "jump"),
                Button::Aim => label(c, "aim"),
                Button::Menu => bars(c, (px(x), px(y), px(r))),
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
                        label(c, &format!("{}", cd.div_ceil(TICK_HZ)));
                    }
                }
            }
        }
    }

    /// Only the menu button (you are out: nothing else to press).
    pub fn draw_menu(c: &mut Canvas, scale: f64) {
        let (x, y, r) = menu_spot();
        let at = ((x / scale) as f32, (y / scale) as f32, (r / scale) as f32);
        disc(c, at, false);
        bars(c, at);
    }
}

/// A button's disc: darker, its ring brighter, while held.
fn disc(c: &mut Canvas, (x, y, r): (f32, f32, f32), held: bool) {
    let ink = Rgba::rgb(250, 246, 236);
    c.circle(x, y, r, Rgba(10, 12, 24, if held { 200 } else { 120 }));
    c.ring(x, y, r, 1.5, ink.fade(if held { 0.9 } else { 0.45 }));
}

/// The menu's mark: three bars.
fn bars(c: &mut Canvas, (x, y, r): (f32, f32, f32)) {
    let ink = Rgba::rgb(250, 246, 236);
    let (half, gap) = (r * 0.45, r * 0.3);
    for k in -1..=1 {
        let yy = y + k as f32 * gap;
        c.line(x - half, yy, x + half, yy, (r * 0.13).max(1.5), ink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearest_button_takes_a_tap_between_two() {
        let (w, h) = (844.0, 390.0);
        let at = |b: Button| {
            let (_, x, y, _) = layout(w, h).into_iter().find(|l| l.0 == b).unwrap();
            (x, y)
        };
        // Just inside jump's padding, but nearer the Tether's edge.
        let (jx, jy) = at(Button::Jump);
        let (fx, fy) = at(Button::Slot(3));
        let mid = |k: f64| (jx + (fx - jx) * k, jy + (fy - jy) * k);
        let (x, y) = mid(0.6);
        assert_eq!(hit(w, h, x, y), Some(Button::Slot(3)));
        let (x, y) = mid(0.35);
        assert_eq!(hit(w, h, x, y), Some(Button::Jump));
        // Neighbouring spells: the one nearer.
        let (ax, ay) = at(Button::Slot(1));
        let (bx, by) = at(Button::Slot(2));
        let (x, y) = (ax + (bx - ax) * 0.6, ay + (by - ay) * 0.6);
        assert_eq!(hit(w, h, x, y), Some(Button::Slot(2)));
        assert_eq!(hit(w, h, 200.0, 200.0), None);
        assert!(on_menu(30.0, 30.0) && !on_menu(200.0, 30.0));
    }

    #[test]
    fn the_wand_thumb_turns_the_view() {
        let css = (844.0, 390.0);
        let mut t = Touch::default();
        let (fx, fy) = (css.0 - 62.0, css.1 - 70.0);
        assert_eq!(t.finger(7, Kind::Down, fx, fy, css), (0.0, 0.0));
        assert!(t.keys() & keys::FIRE != 0, "casting");
        let (yaw, pitch) = t.finger(7, Kind::Move, fx + 20.0, fy - 10.0, css);
        assert!(yaw > 0.0 && pitch > 0.0, "{yaw} {pitch}");
        assert!(t.keys() & keys::FIRE != 0, "still casting");
        t.finger(7, Kind::Up, fx + 20.0, fy - 10.0, css);
        assert_eq!(t.keys() & keys::FIRE, 0);
    }

    #[test]
    fn duck_lets_go_after_a_slide() {
        let mut t = Touch {
            crouch: true,
            ..Touch::default()
        };
        let sliding = Body {
            slide: true,
            crouch: true,
            ..Body::default()
        };
        let done = Body {
            crouch: true,
            ..Body::default()
        };
        t.ease(&sliding, &sliding);
        assert!(t.crouch, "held through the slide");
        t.ease(&sliding, &done);
        assert!(!t.crouch, "up once it ends");
        // A tap on jump stands you up as well.
        t.crouch = true;
        let css = (844.0, 390.0);
        t.finger(1, Kind::Down, css.0 - 196.0, css.1 - 38.0, css);
        assert!(!t.crouch && t.keys() & keys::JUMP != 0);
    }
}
