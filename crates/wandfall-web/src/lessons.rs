//! The range's first lessons, a step at a time under the top line, each
//! ticked off as you do it: move, jump, hop as you land, jump again in the
//! air, climb onto a rock, take a launch rune, sprint, slide, hit a dummy
//! with your wand, cast a spell, pick up a spell cube, open the spellbook. Shown once (remembered); Enter (or a
//! tap on it) skips a step; the range's menu starts them again.

use pixels::{wrap, Canvas, Rect, Rgba};
use wandfall::laws::HOP_WINDOW;
use wandfall::motion::Body;
use wandfall::proto::Own;

const KEY: &str = "wandfall.lessons";
const INK: Rgba = Rgba::rgb(250, 246, 236);
const DIM: Rgba = Rgba::rgb(190, 196, 214);
const GOLD: Rgba = Rgba::rgb(255, 214, 128);
const GREEN: Rgba = Rgba::rgb(140, 230, 120);
const PANEL: Rgba = Rgba(10, 12, 26, 200);
/// How long a step shows ticked before the next (ms); how long the last
/// word stays.
const TICKED: f64 = 900.0;
const LAST: f64 = 6000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Move,
    Jump,
    Hop,
    Air,
    Climb,
    Launch,
    Sprint,
    Slide,
    Wand,
    Cast,
    Cube,
    Book,
}

const STEPS: [Step; 12] = [
    Step::Move,
    Step::Jump,
    Step::Hop,
    Step::Air,
    Step::Climb,
    Step::Launch,
    Step::Sprint,
    Step::Slide,
    Step::Wand,
    Step::Cast,
    Step::Cube,
    Step::Book,
];

impl Step {
    /// What to do, and a word on why (with keys, or with fingers).
    fn say(self, touch: bool) -> (&'static str, &'static str) {
        match (self, touch) {
            (Step::Move, false) => ("move: W A S D", "look about with the mouse"),
            (Step::Move, true) => (
                "move: push a thumb on the left",
                "drag on the right to look about",
            ),
            (Step::Jump, false) => ("jump: space", "one jump a press: holding it does not hop"),
            (Step::Jump, true) => ("jump: tap jump", "one jump a tap"),
            (Step::Hop, false) => (
                "hop: space again just as you land",
                "a timed hop keeps your speed and adds to it",
            ),
            (Step::Hop, true) => (
                "hop: jump again just as you land",
                "a timed hop keeps your speed and adds to it",
            ),
            (Step::Air, false) => (
                "jump again in the air",
                "it turns you the way you steer, for some stamina",
            ),
            (Step::Air, true) => (
                "tap jump again in the air",
                "it turns you the way you steer, for some stamina",
            ),
            (Step::Climb, _) => (
                "climb: jump at a rock or a pillar",
                "push toward its top and you pull yourself up",
            ),
            (Step::Launch, _) => (
                "launch: step on a rune of light",
                "gold on your map: it throws you up to glide",
            ),
            (Step::Sprint, false) => (
                "sprint: hold shift",
                "it tires you; a breath brings it back",
            ),
            (Step::Sprint, true) => (
                "sprint: push the stick all the way",
                "it tires you; a breath brings it back",
            ),
            (Step::Slide, false) => (
                "slide: crouch (C) at a sprint",
                "a slide keeps your speed: jump out of it",
            ),
            (Step::Slide, true) => (
                "slide: duck at a sprint",
                "a slide keeps your speed: jump out of it",
            ),
            (Step::Wand, false) => (
                "hit a dummy with your wand: click",
                "your wand never runs dry",
            ),
            (Step::Wand, true) => (
                "hit a dummy with your wand: hold cast",
                "your wand never runs dry",
            ),
            (Step::Cast, false) => (
                "cast a spell: Q or E",
                "spells are the big moments: each has a cooldown",
            ),
            (Step::Cast, true) => (
                "cast a spell: tap its icon",
                "spells are the big moments: each has a cooldown",
            ),
            (Step::Cube, _) => (
                "pick up a spell cube: walk into one",
                "cubes teach you spells and bring XP",
            ),
            (Step::Book, false) => (
                "open your spellbook: B",
                "put the spells you know in your four slots",
            ),
            (Step::Book, true) => (
                "open your spellbook from the menu",
                "put the spells you know in your four slots",
            ),
        }
    }
}

/// What the page knows this frame, for the lessons to watch.
pub struct Watch<'a> {
    pub now: f64,
    pub at: [f32; 3],
    pub own: Option<&'a Own>,
    /// When your wand last struck someone, and when you last cast a
    /// spell (ms).
    pub hit_at: f64,
    pub cast_at: f64,
    pub book: bool,
}

pub struct Lessons {
    /// The step now (STEPS.len(): all done), or none (not showing).
    at: Option<usize>,
    from: [f32; 3],
    since: f64,
    /// When this step was done (ticked), if it was.
    ticked: Option<f64>,
    /// The tick you last landed, and ticks at a sprint.
    landed: Option<u16>,
    sprint: u32,
    /// Where things stood when this step began: hits, casts, learning.
    base: (f64, f64, u32),
}

/// How much you have learned: spells known and XP, as one number.
fn learned(own: Option<&Own>) -> u32 {
    own.map_or(0, |o| {
        o.book.iter().map(|&r| r as u32).sum::<u32>() * 1000 + o.level as u32 * 256 + o.xp as u32
    })
}

impl Lessons {
    /// Starting, unless they were done before.
    pub fn new() -> Lessons {
        let done = kit::load(KEY).is_some_and(|v| v == "done");
        Lessons {
            at: (!done).then_some(0),
            from: [0.0; 3],
            since: 0.0,
            ticked: None,
            landed: None,
            sprint: 0,
            base: (0.0, 0.0, 0),
        }
    }

    /// From the first step again.
    pub fn again(&mut self) {
        self.at = Some(0);
        self.ticked = None;
        self.since = 0.0;
    }

    pub fn showing(&self) -> bool {
        self.at.is_some()
    }

    fn step(&self) -> Option<Step> {
        self.at.and_then(|k| STEPS.get(k).copied())
    }

    fn tick_off(&mut self, now: f64) {
        if self.ticked.is_none() && self.step().is_some() {
            self.ticked = Some(now);
        }
    }

    /// The step skipped.
    pub fn skip(&mut self, now: f64) {
        self.tick_off(now);
    }

    /// A tick of input applied: your body before and after (`seq` its
    /// number).
    pub fn tick(&mut self, was: &Body, is: &Body, seq: u16, now: f64) {
        let Some(step) = self.step() else {
            return;
        };
        if !was.ground && is.ground {
            self.landed = Some(seq);
        }
        let took_off = was.ground && !is.ground && is.v[1] > 1.0;
        let hop = took_off
            && self
                .landed
                .is_some_and(|l| seq.wrapping_sub(l) <= HOP_WINDOW as u16 + 1);
        self.sprint = if is.sprint { self.sprint + 1 } else { 0 };
        let done = match step {
            Step::Jump => took_off,
            Step::Hop => hop,
            Step::Air => !was.air_jumped && is.air_jumped,
            Step::Climb => was.mantle == 0 && is.mantle > 0,
            Step::Launch => was.ground && !was.glide && is.glide,
            Step::Sprint => self.sprint > 30,
            Step::Slide => is.slide,
            _ => false,
        };
        if done {
            self.tick_off(now);
        }
    }

    /// The frame: steps watched, a ticked one followed by the next.
    pub fn frame(&mut self, w: &Watch) {
        let Some(k) = self.at else {
            return;
        };
        if self.since == 0.0 {
            self.begin(w);
        }
        if let Some(t) = self.ticked {
            if w.now - t > TICKED {
                self.at = Some(k + 1);
                self.ticked = None;
                self.begin(w);
                if k + 1 == STEPS.len() {
                    kit::save(KEY, "done");
                }
            }
            return;
        }
        let Some(step) = self.step() else {
            if w.now - self.since > LAST {
                self.at = None;
            }
            return;
        };
        let moved = (w.at[0] - self.from[0]).hypot(w.at[2] - self.from[2]);
        let done = match step {
            Step::Move => moved > 3.0,
            Step::Wand => w.hit_at > self.base.0,
            Step::Cast => w.cast_at > self.base.1,
            Step::Cube => learned(w.own) > self.base.2,
            Step::Book => w.book,
            _ => false,
        };
        if done {
            self.tick_off(w.now);
        }
    }

    fn begin(&mut self, w: &Watch) {
        self.since = w.now;
        self.from = w.at;
        self.base = (w.hit_at, w.cast_at, learned(w.own));
    }

    /// Drawn under the top line; its panel (to tap, to skip) if any.
    pub fn draw(&self, c: &mut Canvas, ui: i32, touch: bool, now: f64) -> Option<Rect> {
        let k = self.at?;
        let w = c.w;
        let width = (300 * ui).min(w - 16 * ui);
        let (head, why, top) = match STEPS.get(k) {
            Some(s) => {
                let (head, why) = s.say(touch);
                let top = format!("lesson {} of {}", k + 1, STEPS.len());
                (head.to_string(), why.to_string(), top)
            }
            None => (
                "you are ready".to_string(),
                if touch {
                    "menu, then play online: last wizard standing wins".to_string()
                } else {
                    "Esc, then play online: last wizard standing wins".to_string()
                },
                "lessons done".to_string(),
            ),
        };
        let big = if pixels::text_width(&head, 2 * ui) <= width - 12 * ui {
            2 * ui
        } else {
            ui
        };
        let heads = wrap(&head, width - 12 * ui, big);
        let whys = wrap(&why, width - 12 * ui, ui);
        let tall = 12 * ui
            + 10 * ui
            + heads.len() as i32 * 9 * big
            + whys.len() as i32 * 10 * ui
            + 12 * ui;
        // Under the top line; below the map if it would cover it (and the
        // buttons by it, on a phone).
        let map = (96 * ui).min(w / 4).min(c.h / 4);
        let y0 = if (w + width) / 2 > w - map - 14 * ui {
            24 * ui + map + 18 * ui
        } else {
            16 * ui
        };
        let panel = Rect::new(
            ((w - width) / 2) as f32,
            y0 as f32,
            width as f32,
            tall as f32,
        );
        c.round_rect(panel, 5.0 * ui as f32, PANEL);
        let cx = w / 2;
        let mut y = panel.y as i32 + 6 * ui;
        let ticked = self.ticked.is_some();
        let skip = if touch { "tap to skip" } else { "enter: skip" };
        let line = if k < STEPS.len() {
            format!("{top}  -  {skip}")
        } else {
            top
        };
        c.text_centred(cx, y, &line, ui, DIM);
        y += 12 * ui;
        let col = if ticked { GREEN } else { GOLD };
        for (n, l) in heads.iter().enumerate() {
            let text = if ticked && n == 0 {
                format!("done! {l}")
            } else {
                l.clone()
            };
            c.text_centred(cx, y, &text, big, col);
            y += 9 * big;
        }
        y += 2 * ui;
        for l in &whys {
            c.text_centred(cx, y, l, ui, INK.fade(0.85));
            y += 10 * ui;
        }
        // A little bar of the steps done.
        let n = STEPS.len() as i32;
        let gap = 2 * ui;
        let seg = ((width - 24 * ui) - gap * (n - 1)) / n;
        let x0 = cx - (seg * n + gap * (n - 1)) / 2;
        let by = panel.y as i32 + tall - 6 * ui;
        for s in 0..n {
            let on = (s as usize) < k || (s as usize == k && ticked);
            let pulse = if s as usize == k && !ticked {
                0.5 + 0.5 * ((now / 300.0).sin() as f32)
            } else {
                1.0
            };
            let col = if on {
                GREEN
            } else {
                DIM.fade(0.35 + 0.3 * pulse)
            };
            c.fill_rect(x0 + s * (seg + gap), by, seg, 2 * ui, col);
        }
        Some(panel)
    }
}

impl Default for Lessons {
    fn default() -> Self {
        Lessons::new()
    }
}
