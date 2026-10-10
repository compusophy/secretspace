//! The page itself: start, the frame loop, sitting down and standing up,
//! the shared menu.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use battlestation::demo::Demo;
use battlestation::hands::Hands;
use battlestation::keys;
use battlestation::term::Term;
use look::desk::Desk;
use look::monitor::Screen;
use render::{Quality, Renderer};
use wasm_bindgen::prelude::*;

use crate::{sound, Hooks};

mod input;
mod out;
mod view;

use out::Out;

const MIN_SHORT: f64 = 352.0;
const MAX_DPR: f64 = 1.5;

/// A key as the page heard it.
struct KeyEv {
    code: String,
    key: String,
    down: bool,
    ctrl: bool,
    repeat: bool,
}

/// What the browser told the page between frames.
#[derive(Default)]
struct Heard {
    keys: Vec<KeyEv>,
    wheel: f32,
    /// The page lost the keyboard (another window): let every key go.
    blur: bool,
}

struct Sounds {
    audio: kit::audio::Audio,
    clack: [usize; 4],
    thock: usize,
    up: usize,
    click: usize,
    fans: usize,
    hum: Option<usize>,
    n: usize,
}

impl Sounds {
    fn new() -> Sounds {
        let mut audio = kit::audio::Audio::new();
        let rate = engine::synth::RATE;
        let clack = [0.92, 0.98, 1.04, 1.1].map(|k| audio.add(&sound::clack(k), rate));
        Sounds {
            clack,
            thock: audio.add(&sound::thock(), rate),
            up: audio.add(&sound::up(), rate),
            click: audio.add(&sound::click(), rate),
            fans: audio.add(&sound::fans(), rate),
            audio,
            hum: None,
            n: 0,
        }
    }

    /// A key going down (`code`, `x` metres across the desk) or up.
    fn key(&mut self, code: &str, x: f32, down: bool) {
        let pan = ((x - battlestation::laws::KEYBOARD_X) / 0.4).clamp(-0.6, 0.6);
        self.n = self.n.wrapping_add(1);
        let wobble = 0.96 + (self.n * 7 % 9) as f32 * 0.01;
        if !down {
            self.audio.play(self.up, 0.10, pan, wobble);
        } else if sound::long_key(code) {
            self.audio.play(self.thock, 0.36, pan, wobble);
        } else {
            self.audio.play(self.clack[self.n % 4], 0.3, pan, wobble);
        }
    }

    fn click(&mut self) {
        self.audio.play(self.click, 0.25, 0.55, 1.0);
    }

    fn hum(&mut self) {
        if self.hum.is_none() {
            let h = self.audio.hum(self.fans);
            self.audio.tune(h, 0.05, 0.45);
            self.hum = Some(h);
        }
    }
}

struct Page {
    out: Out,
    r: Renderer,
    desk: Desk,
    hands: Hands,
    term: Term,
    screen: Screen,
    demo: Option<Demo>,
    seated: bool,
    /// When you sat down (ms).
    since: f64,
    pointer: kit::input::Hands,
    heard: Rc<RefCell<Heard>>,
    meta: kit::meta::Meta,
    sounds: Sounds,
    /// Leaning in: where it is, how fast, where the wheel wants it.
    lean: (f32, f32),
    lean_to: f32,
    /// The head following the cursor: yaw and pitch, each with its speed.
    head: [f32; 4],
    /// Each key's light after a press (1 just pressed).
    heat: Vec<f32>,
    /// Keys (or "#mouse") to let go of later: when (ms), which.
    ups: Vec<(f64, &'static str)>,
    field: Option<kit::TextField>,
    /// The last finger on the glass (CSS pixels), and when it went down.
    finger: Option<(f64, f64, f64, f64)>,
    /// Where the mouse was last (CSS pixels).
    last_mouse: Option<(f64, f64)>,
    /// Mouse moves to pass over (see `input::pointer`).
    skip: u8,
    /// When the menu last opened (ms).
    menu_at: f64,
    /// The canvas's CSS cursor as last set.
    cursor: &'static str,
    was_locked: bool,
    touch: bool,
    hooks: Hooks,
    frames: u32,
    last: f64,
    fps: f64,
    hint_until: f64,
}

thread_local! {
    static PAGE: RefCell<Option<Page>> = const { RefCell::new(None) };
    /// Whether the keys are the desk's (seated, no menu): then the
    /// browser's own uses of them are held back.
    static CAPTURE: Cell<bool> = const { Cell::new(false) };
}

impl Page {
    fn new(mut out: Out, hooks: Hooks, query: &str) -> Page {
        let touch = kit::touch();
        let q = Quality::pick(query, out.caps().software, touch);
        let q = Quality {
            grass_spacing: 0.0,
            grass_reach: 0.0,
            ..q
        };
        let mut r = Renderer::new(out.device(), out.queue(), out.format(), q);
        let desk = Desk::new(&mut r, out.device(), out.format(), hooks.tone.unwrap_or(2));
        out.fit(MIN_SHORT, MAX_DPR);
        let heard = Rc::new(RefCell::new(Heard::default()));
        input::listen(&heard, out.canvas());
        let pointer = kit::input::Hands::attach(out.canvas());
        let now = kit::now();
        let lean = hooks.lean.unwrap_or(0.0);
        Page {
            r,
            desk,
            hands: Hands::new(),
            term: Term::new(),
            screen: Screen::new(),
            demo: (hooks.demo && !hooks.sit).then(|| Demo::new(now as u32 ^ 0x5eed)),
            seated: hooks.sit,
            since: now,
            pointer,
            heard,
            meta: kit::meta::Meta::new("battlestation"),
            sounds: Sounds::new(),
            lean: (lean, 0.0),
            lean_to: lean,
            head: [0.0; 4],
            heat: vec![0.0; keys::layout().len()],
            ups: Vec::new(),
            field: touch.then(|| kit::TextField::new(64, "type")),
            finger: None,
            last_mouse: None,
            skip: 0,
            menu_at: 0.0,
            cursor: "",
            was_locked: false,
            touch,
            hooks,
            frames: 0,
            last: now,
            fps: 60.0,
            hint_until: 0.0,
            out,
        }
    }

    fn frame(&mut self, now: f64) {
        let dt = ((now - self.last) / 1000.0).clamp(0.0, 0.1) as f32;
        if now > self.last {
            self.fps += (1000.0 / (now - self.last).max(1.0) - self.fps) * 0.05;
        }
        self.last = now;
        input::input(self, now, dt);
        let playing = self.seated && !self.meta.is_open();
        CAPTURE.with(|c| c.set(playing));
        // Your own arrow is hidden only while you sit at the desk (its
        // arrow is on the monitor); over a menu, or standing, it shows.
        let cursor = if playing { "none" } else { "default" };
        if cursor != self.cursor {
            let _ = self.out.canvas().style().set_property("cursor", cursor);
            self.cursor = cursor;
        }
        view::frame(self, now, dt);
    }
}

impl Page {
    /// Sit down: the demo stops (a clean terminal), the mouse is taken
    /// (`grab`, from a press) and the sound wakes.
    fn sit(&mut self, grab: bool) {
        self.sounds.audio.wake();
        self.sounds.hum();
        if !self.seated {
            self.seated = true;
            self.since = kit::now();
            self.hint_until = kit::now() + 9000.0;
            if self.demo.take().is_some() {
                for k in keys::layout() {
                    self.hands.key(k.code, false);
                }
                self.hands.button(0, false);
                self.hands.button(2, false);
                self.ups.clear();
                self.term = Term::new();
            }
        }
        if grab && !self.touch {
            kit::input::play(self.out.canvas(), !self.hooks.windowed);
        }
    }

    /// Get up: give the mouse, the keys and the screen back.
    fn stand(&mut self) {
        self.seated = false;
        kit::input::release();
        if let Some(f) = &self.field {
            f.blur();
            f.place(None);
        }
    }

    fn items(&self) -> Vec<String> {
        vec![
            "hands: next skin tone".into(),
            if self.sounds.audio.muted {
                "sound: off".into()
            } else {
                "sound: on".into()
            },
            "get up from the desk".into(),
        ]
    }

    fn open_menu(&mut self) {
        kit::input::unlock();
        self.meta.context = format!(
            "seated: {}\nfps: {:.0}\ngpu: {}\nran: {}",
            self.seated,
            self.fps,
            self.out.caps().adapter,
            self.term.ran()
        );
        self.meta.show();
        self.menu_at = kit::now();
    }

    fn resume(&mut self) {
        self.meta.hide();
        if self.seated && !self.touch {
            kit::input::play(self.out.canvas(), !self.hooks.windowed);
        }
    }

    fn picked(&mut self, pick: kit::meta::Pick) {
        match pick {
            kit::meta::Pick::Resume => self.resume(),
            kit::meta::Pick::Game(0) => self.desk.next_tone(&mut self.r),
            kit::meta::Pick::Game(1) => self.sounds.audio.muted = !self.sounds.audio.muted,
            kit::meta::Pick::Game(_) => {
                self.meta.hide();
                self.stand();
            }
            kit::meta::Pick::Exit => kit::shell::exit(),
        }
    }
}

fn say(text: &str) {
    if let Some(b) = kit::document().body() {
        b.set_inner_html(&format!(
            "<p style=\"color:#f4eede;font:16px sans-serif;padding:24px\">{text}</p>"
        ));
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    let query = kit::window().location().search().unwrap_or_default();
    let hooks = Hooks::read(&query);
    wasm_bindgen_futures::spawn_local(async move {
        if !gpu::offered() {
            // (`?capture=1` needs it too.)
            say("battlestation is drawn with WebGPU, which this browser does not offer yet. Chrome, Edge and Safari 26 have it.");
            return;
        }
        let capture = crate::param(&query, "capture").is_some_and(|v| v != "0");
        let out = match Out::open("screen", capture, MIN_SHORT, MAX_DPR).await {
            Ok(o) => o,
            Err(e) => {
                say(&format!("battlestation could not start WebGPU here ({e})."));
                return;
            }
        };
        let page = Page::new(out, hooks, &query);
        PAGE.with(|p| *p.borrow_mut() = Some(page));
        kit::frames(|now| {
            PAGE.with(|p| {
                if let Some(p) = p.borrow_mut().as_mut() {
                    p.frame(now);
                }
            })
        });
        kit::on(&kit::window(), "resize", |_| {
            PAGE.with(|p| {
                if let Some(p) = p.borrow_mut().as_mut() {
                    p.out.fit(MIN_SHORT, MAX_DPR);
                }
            })
        });
    });
}
