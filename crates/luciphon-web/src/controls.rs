//! Two hands to Inputs, 30 a second, in first person. On a desktop the
//! mouse looks (once the page has it), WASD moves, shift sprints, space
//! jumps, Q dashes, a click fires the wand, a held click charges a great
//! beam and a held right button a throw (its range follows the look). On
//! a phone the left thumb is a stick wherever it lands (pushed to its
//! edge, a sprint), the right thumb looks (a tap fires, a still hold
//! charges a great beam), and buttons jump, dash, throw and open the
//! Heart. Looking turns at once, between ticks; every Input carries where
//! you look.

use std::collections::VecDeque;
use std::f32::consts::TAU;

use kit::input::{Hand, Kind};
use luciphon::motion::{Intent, Verb};

use crate::laws::Feel;

/// Ticks a second the page sends Inputs at.
pub const TICK_MS: f64 = 1000.0 / 30.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Jump,
    Dash,
    Throw,
    Heart,
    Bag,
}

pub const BUTTONS: [Button; 5] = [
    Button::Jump,
    Button::Dash,
    Button::Throw,
    Button::Heart,
    Button::Bag,
];

/// Where a phone button is, in CSS pixels: centre and radius.
pub fn button_at(b: Button, css: (f64, f64), f: &Feel) -> (f64, f64, f64) {
    let r = f.button_px / 2.0;
    let (w, h) = css;
    let (x, y) = (w - f.edge_px - r, h - f.edge_px - r);
    match b {
        Button::Jump => (x, y, r * 1.15),
        Button::Dash => (x - 2.4 * r, y + 0.25 * r, r * 0.9),
        Button::Throw => (x - 0.2 * r, y - 2.5 * r, r * 0.9),
        Button::Heart => (w - f.edge_px - r * 0.7, f.edge_px + r * 2.2, r * 0.7),
        Button::Bag => (w - f.edge_px - r * 0.7, f.edge_px + r * 4.0, r * 0.7),
    }
}

/// A heading (65536 a turn) from radians.
pub fn heading(rad: f32) -> u16 {
    (rad.rem_euclid(TAU) / TAU * 65536.0) as u32 as u16
}

/// A throw's range (1-255) from how far up you look.
pub fn range(pitch: f32, f: &Feel) -> u8 {
    let k = ((pitch - f.throw_low) / (f.throw_high - f.throw_low)).clamp(0.0, 1.0);
    (1.0 + k * 254.0) as u8
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Charge {
    Heavy,
    Throw,
}

#[derive(Clone, Copy, Debug)]
struct Stick {
    id: i32,
    ox: f64,
    oy: f64,
    x: f64,
    y: f64,
}

#[derive(Clone, Copy, Debug)]
struct Look {
    id: i32,
    x: f64,
    y: f64,
    t0: f64,
    moved: f64,
    charging: bool,
}

#[derive(Default)]
pub struct Controls {
    /// Where you look, radians: yaw along the ground (0 east, toward
    /// south), pitch up.
    pub yaw: f32,
    pub pitch: f32,
    pub seq: u16,
    pub next_at: f64,
    verbs: VecDeque<Verb>,
    jump: bool,
    charge: Option<Charge>,
    lmb: Option<f64>,
    stick: Option<Stick>,
    look: Option<Look>,
    held: Vec<(i32, Button)>,
    /// The Heart button was pressed (the page opens the wheel); the bag
    /// button (the page opens your gear).
    pub heart: bool,
    pub bag: bool,
    /// A click while the page did not have the mouse (the page takes it).
    pub want_lock: bool,
    /// Done at least once, for the hints: moved 1, struck 2, dashed 4,
    /// charged 8, jumped 16, sprinted 32.
    pub done: u8,
}

impl Controls {
    fn verb(&mut self, v: Verb) {
        self.verbs.push_back(v);
        while self.verbs.len() > 6 {
            self.verbs.pop_front();
        }
    }

    fn start(&mut self, c: Charge, held_for: u8) {
        if self.charge.is_none() {
            self.charge = Some(c);
            self.verb(Verb::Hold { held_for });
        }
    }

    fn release(&mut self, c: Charge, f: &Feel) {
        if self.charge == Some(c) {
            self.charge = None;
            let range = match c {
                Charge::Heavy => 0,
                Charge::Throw => range(self.pitch, f),
            };
            self.verb(Verb::Release { range });
        }
    }

    fn turn(&mut self, dx: f64, dy: f64, k: f32, f: &Feel) {
        self.yaw = (self.yaw + dx as f32 * k).rem_euclid(TAU);
        self.pitch = (self.pitch - dy as f32 * k).clamp(-f.pitch_max, f.pitch_max);
    }

    /// One hand event. `locked`: the page has the mouse.
    pub fn feed(&mut self, h: &Hand, locked: bool, css: (f64, f64), f: &Feel) {
        match *h {
            Hand::Mouse { dx, dy, .. } if locked => self.turn(dx, dy, f.mouse, f),
            Hand::Mouse { .. } => {}
            Hand::Button {
                button, down, t, ..
            } => match (button, down) {
                (_, true) if !locked => self.want_lock = true,
                (0, true) => self.lmb = Some(t),
                (0, false) => {
                    if self.lmb.take().is_some() {
                        if self.charge == Some(Charge::Heavy) {
                            self.release(Charge::Heavy, f);
                        } else if self.charge.is_none() {
                            self.verb(Verb::Tap);
                        }
                    }
                }
                (2, true) => self.start(Charge::Throw, 0),
                (2, false) => self.release(Charge::Throw, f),
                _ => {}
            },
            Hand::Finger { id, kind, x, y, t } => self.finger(id, kind, (x, y), t, css, f),
        }
    }

    fn finger(
        &mut self,
        id: i32,
        kind: Kind,
        (x, y): (f64, f64),
        t: f64,
        css: (f64, f64),
        f: &Feel,
    ) {
        match kind {
            Kind::Down => {
                for b in BUTTONS {
                    let (bx, by, r) = button_at(b, css, f);
                    if (x - bx).hypot(y - by) <= r * 1.15 {
                        self.held.push((id, b));
                        match b {
                            Button::Jump => self.jump = true,
                            Button::Dash => self.verb(Verb::Flick),
                            Button::Throw => self.start(Charge::Throw, 0),
                            Button::Heart => self.heart = true,
                            Button::Bag => self.bag = true,
                        }
                        return;
                    }
                }
                if x < css.0 / 2.0 {
                    if self.stick.is_none() {
                        self.stick = Some(Stick {
                            id,
                            ox: x,
                            oy: y,
                            x,
                            y,
                        });
                    }
                } else if self.look.is_none() {
                    self.look = Some(Look {
                        id,
                        x,
                        y,
                        t0: t,
                        moved: 0.0,
                        charging: false,
                    });
                }
            }
            Kind::Move => {
                if let Some(s) = self.stick.as_mut().filter(|s| s.id == id) {
                    (s.x, s.y) = (x, y);
                    // Past the run, the stick's centre follows the thumb.
                    let (dx, dy) = (x - s.ox, y - s.oy);
                    let d = dx.hypot(dy);
                    let max = f.stick_px * 1.5;
                    if d > max {
                        s.ox = x - dx / d * max;
                        s.oy = y - dy / d * max;
                    }
                }
                if let Some(l) = self.look.filter(|l| l.id == id) {
                    let (dx, dy) = (x - l.x, y - l.y);
                    self.turn(dx, dy, f.drag, f);
                    if let Some(l) = self.look.as_mut() {
                        l.moved += dx.hypot(dy);
                        (l.x, l.y) = (x, y);
                    }
                }
            }
            Kind::Up | Kind::Cancel => {
                if self.stick.is_some_and(|s| s.id == id) {
                    self.stick = None;
                }
                if let Some(l) = self.look.filter(|l| l.id == id) {
                    self.look = None;
                    if l.charging {
                        self.release(Charge::Heavy, f);
                    } else if kind == Kind::Up
                        && t - l.t0 < f.tap_ms
                        && l.moved < f.tap_px
                        && self.charge.is_none()
                    {
                        self.verb(Verb::Tap);
                    }
                }
                if let Some(k) = self.held.iter().position(|h| h.0 == id) {
                    let (_, b) = self.held.remove(k);
                    if b == Button::Throw {
                        self.release(Charge::Throw, f);
                    }
                }
            }
            Kind::Hover => {}
        }
    }

    /// A key pressed (`KeyboardEvent.code`) that moves you.
    pub fn key(&mut self, code: &str) {
        match code {
            "Space" => self.jump = true,
            "KeyQ" => self.verb(Verb::Flick),
            _ => {}
        }
    }

    /// Time passing: a held press becomes a charge.
    pub fn tick(&mut self, now: f64, f: &Feel) {
        let ticks = |ms: f64| (ms / TICK_MS).round().clamp(0.0, 255.0) as u8;
        if let Some(t0) = self.lmb {
            if self.charge.is_none() && now - t0 >= f.click_hold_ms {
                self.start(Charge::Heavy, ticks(now - t0));
            }
        }
        if let Some(l) = self.look {
            if !l.charging && l.moved < f.tap_px && now - l.t0 >= f.hold_ms && self.charge.is_none()
            {
                self.start(Charge::Heavy, ticks(now - l.t0));
                if let Some(l) = self.look.as_mut() {
                    l.charging = true;
                }
            }
        }
    }

    /// Everything let go at once (the mouse or the window was lost).
    pub fn drop_all(&mut self) {
        self.lmb = None;
        if self.charge.take().is_some() {
            self.verb(Verb::Cancel);
        }
        self.stick = None;
        self.look = None;
        self.held.clear();
    }

    /// The phone's stick, if a thumb is on it: origin and thumb.
    pub fn stick(&self) -> Option<(f64, f64, f64, f64)> {
        self.stick.map(|s| (s.ox, s.oy, s.x, s.y))
    }

    /// Whether this button is held down.
    pub fn pressed(&self, b: Button) -> bool {
        self.held.iter().any(|h| h.1 == b)
    }

    /// The Input for the next tick. `held` says whether a key is down.
    pub fn sample(&mut self, held: &dyn Fn(&str) -> bool, f: &Feel) -> (u16, Intent) {
        let key = |a: &str, b: &str| (held(a) || held(b)) as i32 as f32;
        let ahead = key("KeyW", "ArrowUp") - key("KeyS", "ArrowDown");
        let right = key("KeyD", "ArrowRight") - key("KeyA", "ArrowLeft");
        let (mut way, mut throttle) = (right.atan2(ahead), 0u8);
        let mut sprint = held("ShiftLeft") || held("ShiftRight");
        if ahead != 0.0 || right != 0.0 {
            throttle = 255;
        }
        if let Some(s) = self.stick {
            let (dx, dy) = (s.x - s.ox, s.y - s.oy);
            let d = dx.hypot(dy);
            if d > f.walk_px {
                way = (dx as f32).atan2(-dy as f32);
                // Pushed out to the edge: a sprint.
                sprint |= d >= f.sprint_px;
                throttle = if d >= f.stick_px {
                    255
                } else {
                    (1.0 + (d - f.walk_px) / (f.stick_px - f.walk_px) * 253.0) as u8
                };
            }
        }
        let verb = self.verbs.pop_front().unwrap_or_default();
        let jump = std::mem::take(&mut self.jump);
        self.done |= (throttle > 0) as u8
            | match verb {
                Verb::Tap => 2,
                Verb::Flick => 4,
                Verb::Hold { .. } => 8,
                _ => 0,
            }
            | (jump as u8) << 4
            | ((sprint && throttle == 255) as u8) << 5;
        self.seq = self.seq.wrapping_add(1);
        let it = Intent {
            heading: heading(self.yaw + way),
            throttle,
            verb,
            aim: heading(self.yaw),
            jump,
            sprint,
        };
        (self.seq, it)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::laws::FEEL;

    const CSS: (f64, f64) = (390.0, 844.0);

    fn none(_: &str) -> bool {
        false
    }

    fn finger(id: i32, kind: Kind, x: f64, y: f64, t: f64) -> Hand {
        Hand::Finger { id, kind, x, y, t }
    }

    #[test]
    fn keys_move_you_relative_to_where_you_look() {
        let mut c = Controls::default();
        let w = |k: &str| k == "KeyW";
        assert_eq!(c.sample(&w, &FEEL).1.heading, 0);
        let d = |k: &str| k == "KeyD";
        assert_eq!(c.sample(&d, &FEEL).1.heading, 16384);
        // Looking south, forward is south.
        c.yaw = TAU / 4.0;
        let (_, it) = c.sample(&w, &FEEL);
        assert_eq!((it.heading, it.throttle, it.aim), (16384, 255, 16384));
        let (_, it) = c.sample(&none, &FEEL);
        assert_eq!(it.throttle, 0);
        c.key("Space");
        assert!(c.sample(&none, &FEEL).1.jump);
        let run = |k: &str| k == "KeyW" || k == "ShiftLeft";
        assert!(c.sample(&run, &FEEL).1.sprint);
        c.key("KeyQ");
        assert_eq!(c.sample(&none, &FEEL).1.verb, Verb::Flick);
        assert!(!c.sample(&none, &FEEL).1.jump);
    }

    #[test]
    fn a_click_strikes_and_a_held_click_charges() {
        let mut c = Controls::default();
        let click = |down, t| Hand::Button {
            button: 0,
            down,
            x: 0.0,
            y: 0.0,
            t,
        };
        // Without the mouse, a click only asks for it.
        c.feed(&click(true, 0.0), false, CSS, &FEEL);
        assert!(c.want_lock);
        c.feed(
            &Hand::Mouse {
                x: 0.0,
                y: 0.0,
                dx: 100.0,
                dy: 0.0,
            },
            true,
            CSS,
            &FEEL,
        );
        assert!(c.yaw > 0.2);
        c.feed(&click(true, 10.0), true, CSS, &FEEL);
        c.feed(&click(false, 60.0), true, CSS, &FEEL);
        assert_eq!(c.sample(&none, &FEEL).1.verb, Verb::Tap);
        c.feed(&click(true, 100.0), true, CSS, &FEEL);
        c.tick(400.0, &FEEL);
        assert!(matches!(
            c.sample(&none, &FEEL).1.verb,
            Verb::Hold { held_for: 9 }
        ));
        c.feed(&click(false, 700.0), true, CSS, &FEEL);
        assert_eq!(c.sample(&none, &FEEL).1.verb, Verb::Release { range: 0 });
        // A right hold throws, farther the higher you look.
        let right = |down| Hand::Button {
            button: 2,
            down,
            x: 0.0,
            y: 0.0,
            t: 0.0,
        };
        c.pitch = FEEL.throw_high;
        c.feed(&right(true), true, CSS, &FEEL);
        c.feed(&right(false), true, CSS, &FEEL);
        assert!(matches!(c.sample(&none, &FEEL).1.verb, Verb::Hold { .. }));
        assert_eq!(c.sample(&none, &FEEL).1.verb, Verb::Release { range: 255 });
    }

    #[test]
    fn two_thumbs_move_look_strike_and_charge() {
        let mut c = Controls::default();
        // Left thumb: pushed up is forward.
        c.feed(&finger(1, Kind::Down, 80.0, 600.0, 0.0), false, CSS, &FEEL);
        c.feed(&finger(1, Kind::Move, 80.0, 500.0, 20.0), false, CSS, &FEEL);
        let (_, it) = c.sample(&none, &FEEL);
        assert_eq!((it.heading, it.throttle), (0, 255));
        // Right thumb: a drag looks, a tap strikes.
        c.feed(&finger(2, Kind::Down, 300.0, 400.0, 0.0), false, CSS, &FEEL);
        c.feed(
            &finger(2, Kind::Move, 340.0, 400.0, 30.0),
            false,
            CSS,
            &FEEL,
        );
        c.feed(&finger(2, Kind::Up, 340.0, 400.0, 60.0), false, CSS, &FEEL);
        assert!(c.yaw > 0.2);
        assert_eq!(c.sample(&none, &FEEL).1.verb, Verb::None);
        c.feed(
            &finger(3, Kind::Down, 300.0, 400.0, 100.0),
            false,
            CSS,
            &FEEL,
        );
        c.feed(&finger(3, Kind::Up, 300.0, 400.0, 150.0), false, CSS, &FEEL);
        assert_eq!(c.sample(&none, &FEEL).1.verb, Verb::Tap);
        // Held still, a charge; let go, a heavy.
        c.feed(
            &finger(4, Kind::Down, 300.0, 400.0, 200.0),
            false,
            CSS,
            &FEEL,
        );
        c.tick(600.0, &FEEL);
        assert!(matches!(c.sample(&none, &FEEL).1.verb, Verb::Hold { .. }));
        c.feed(&finger(4, Kind::Up, 300.0, 400.0, 900.0), false, CSS, &FEEL);
        assert_eq!(c.sample(&none, &FEEL).1.verb, Verb::Release { range: 0 });
        // The buttons: jump and dash.
        let (jx, jy, _) = button_at(Button::Jump, CSS, &FEEL);
        c.feed(&finger(5, Kind::Down, jx, jy, 1000.0), false, CSS, &FEEL);
        let (dx, dy, _) = button_at(Button::Dash, CSS, &FEEL);
        c.feed(&finger(6, Kind::Down, dx, dy, 1000.0), false, CSS, &FEEL);
        let (_, it) = c.sample(&none, &FEEL);
        assert!(it.jump && it.verb == Verb::Flick);
        // Pushed to its edge, the stick sprinted too.
        assert_eq!(c.done, 63);
    }
}
