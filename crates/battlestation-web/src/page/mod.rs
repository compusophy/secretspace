//! The page itself: start, the frame loop, sitting down and standing up,
//! the shared menu.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use battlestation::hands::Hands;
use battlestation::keys;
use battlestation::term::Term;
use look::desk::Desk;
use look::monitor::Screen;
use render::{Quality, Renderer};
use wasm_bindgen::prelude::*;

use crate::{sound, Hooks};

mod input;
mod os;
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
    /// The modifiers held (as `os::mods` counts them).
    mods: u8,
    repeat: bool,
}

/// What the browser told the page between frames.
#[derive(Default)]
struct Heard {
    keys: Vec<KeyEv>,
    wheel: f32,
    /// Shift was held on the wheel (it leans then, whatever is under it).
    wheel_shift: bool,
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
    /// compusophyOS, once mounted; why not, if it could not be.
    os: Option<os::Os>,
    os_said: String,
    r: Renderer,
    desk: Desk,
    hands: Hands,
    term: Term,
    screen: Screen,
    /// When the page opened (ms): the computer's uptime.
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
    /// When the menu last opened (ms).
    menu_at: f64,
    /// The canvas's CSS cursor as last set.
    cursor: &'static str,
    /// The sound has been woken (a person has acted).
    woke: bool,
    touch: bool,
    hooks: Hooks,
    frames: u32,
    last: f64,
    fps: f64,
    hint_until: f64,
    /// Whether the browser isolated the page (else the computer's
    /// programs cannot run).
    isolated: bool,
}

thread_local! {
    static PAGE: RefCell<Option<Page>> = const { RefCell::new(None) };
    /// Whether the keys are the desk's (no menu): then the
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
        let pointer = match out.canvas() {
            Some(c) => {
                input::listen(&heard, c);
                kit::input::Hands::attach(c)
            }
            None => kit::input::Hands::detached(),
        };
        let now = kit::now();
        let lean = hooks.lean.unwrap_or(0.0);
        Page {
            os: None,
            os_said: String::new(),
            r,
            desk,
            hands: Hands::new(),
            term: Term::new(),
            screen: Screen::new(),
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
            menu_at: 0.0,
            cursor: "",
            woke: false,
            touch,
            hooks,
            frames: 0,
            last: now,
            fps: 60.0,
            hint_until: now + 9000.0,
            isolated: isolated(),
            out,
        }
    }

    fn frame(&mut self, now: f64) {
        let dt = ((now - self.last) / 1000.0).clamp(0.0, 0.1) as f32;
        if now > self.last {
            self.fps += (1000.0 / (now - self.last).max(1.0) - self.fps) * 0.05;
        }
        self.last = now;
        input::input(self, now);
        let playing = !self.meta.is_open();
        CAPTURE.with(|c| c.set(playing));
        // Your own arrow is hidden at the desk (its arrow is on the
        // monitor, where yours points); over the menu it shows.
        let cursor = if playing { "none" } else { "default" };
        if cursor != self.cursor {
            if let Some(c) = self.out.canvas() {
                let _ = c.style().set_property("cursor", cursor);
            }
            self.cursor = cursor;
        }
        view::frame(self, now, dt);
    }
}

impl Page {
    /// A person acted: the sound may start (a browser lets it only now).
    fn wake(&mut self) {
        self.sounds.audio.wake();
        if !self.woke {
            self.woke = true;
            self.sounds.hum();
        }
    }

    /// Whether the monitor shows compusophyOS (once it has mounted).
    fn on_os(&self) -> bool {
        self.os.is_some()
    }

    /// Whether compusophyOS is still starting (the monitor says so).
    fn booting(&self) -> bool {
        self.hooks.os && self.os.is_none() && self.os_said.is_empty()
    }

    fn items(&self) -> Vec<String> {
        vec![
            "hands: next skin tone".into(),
            if self.sounds.audio.muted {
                "sound: off".into()
            } else {
                "sound: on".into()
            },
        ]
    }

    fn open_menu(&mut self) {
        self.meta.context = format!(
            "fps: {:.0}\ngpu: {}\nran: {}\nos: {}",
            self.fps,
            self.out.caps().adapter,
            self.term.ran(),
            if self.os.is_some() {
                "mounted"
            } else {
                &self.os_said
            }
        );
        self.meta.show();
        self.menu_at = kit::now();
    }

    fn resume(&mut self) {
        self.meta.hide();
    }

    fn picked(&mut self, pick: kit::meta::Pick) {
        match pick {
            kit::meta::Pick::Resume => self.resume(),
            kit::meta::Pick::Game(0) => self.desk.next_tone(&mut self.r),
            kit::meta::Pick::Game(_) => self.sounds.audio.muted = !self.sounds.audio.muted,
            kit::meta::Pick::Exit => kit::shell::exit(),
        }
    }
}

/// The query mark of a page loaded again to be isolated.
const AGAIN: &str = "again";

/// Whether the browser isolated this page (COOP+COEP): compusophyOS runs
/// its programs only on an isolated page.
fn isolated() -> bool {
    js_sys::Reflect::get(&kit::window(), &"crossOriginIsolated".into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// An unisolated page: load it again, once, on its own (a front page
/// loaded before it was isolated cannot isolate its frames; a stale copy
/// may lack the headers). Whether it is going.
fn again(query: &str) -> bool {
    if crate::param(query, AGAIN).is_some() {
        return false;
    }
    let w = kit::window();
    let path = w.location().pathname().unwrap_or_default();
    let sep = if query.is_empty() { "?" } else { "&" };
    let to = format!("{path}{query}{sep}{AGAIN}=1");
    let top = w.top().ok().flatten().unwrap_or(w);
    top.location().replace(&to).is_ok()
}

/// An isolated page loaded again: its address without the mark.
fn unmark(query: &str) {
    if crate::param(query, AGAIN).is_none() {
        return;
    }
    let w = kit::window();
    let rest: Vec<&str> = query
        .trim_start_matches('?')
        .split('&')
        .filter(|kv| kv.split('=').next() != Some(AGAIN))
        .collect();
    let path = w.location().pathname().unwrap_or_default();
    let to = match rest.join("&") {
        q if q.is_empty() => path,
        q => format!("{path}?{q}"),
    };
    if let Ok(h) = w.history() {
        let _ = h.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&to));
    }
}

fn say(text: &str) {
    if let Some(b) = kit::document().body() {
        b.set_inner_html(&format!(
            "<p style=\"color:#f4eede;font:16px sans-serif;padding:24px\">{text}</p>"
        ));
    }
}

// The page's own start, called by its index.html after init(): never on
// init, so a host importing this module as a cartridge is left alone
// (the start-guard rule, which scripts/caps.sh holds).
#[wasm_bindgen]
pub fn page() {
    let query = kit::window().location().search().unwrap_or_default();
    let hooks = Hooks::read(&query);
    kit::report::on_panic();
    if hooks.os {
        if isolated() {
            unmark(&query);
        } else if again(&query) {
            return;
        }
    }
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
        // The computer boots in the background, to its welcome.
        if hooks.os {
            wasm_bindgen_futures::spawn_local(async {
                let os = os::mount().await;
                PAGE.with(|p| {
                    if let Some(p) = p.borrow_mut().as_mut() {
                        match os {
                            Ok(o) => p.os = Some(o),
                            Err(e) => p.os_said = e,
                        }
                    }
                })
            });
        }
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
