//! compusophyOS on the monitor: the OS mounted as a cartridge (its files
//! mirrored beside the page under `os/` by scripts/build-web.sh; the page
//! cross-origin isolated, so its programs run), its frames copied onto
//! the monitor's glass as they come, and your keys, mouse and wheel given
//! to it while you sit at the desk. The OS cannot tell: it is the computer
//! it always is, signing in on its welcome, opening windows, running its
//! programs. wasm-bindgen writes the import; no script is written here.
//! Its frames come as pixels (`pixels`), not a canvas copied on the GPU:
//! that copy is not to be had everywhere, and failing it ends a frame.

use battlestation::laws::SCREEN_PX;
use look::glass::Glass;
use render::wgpu;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(raw_module = "../os/os.js")]
extern "C" {
    #[wasm_bindgen(js_name = default)]
    fn os_init() -> js_sys::Promise;

    type Cartridge;
    #[wasm_bindgen(constructor, catch)]
    fn new(w: f32, h: f32, dpr: f32, base: &str, ns: &str) -> Result<Cartridge, JsValue>;
    #[wasm_bindgen(method)]
    fn frames(this: &Cartridge) -> u32;
    #[wasm_bindgen(method)]
    fn width(this: &Cartridge) -> u32;
    #[wasm_bindgen(method)]
    fn height(this: &Cartridge) -> u32;
    #[wasm_bindgen(method)]
    fn pixels(this: &Cartridge, out: &mut [u8]) -> bool;
    #[wasm_bindgen(method)]
    fn pointer(this: &Cartridge, kind: u8, x: f32, y: f32, button: u8) -> bool;
    #[wasm_bindgen(method)]
    fn wheel(this: &Cartridge, x: f32, y: f32, dy: f32) -> bool;
    #[wasm_bindgen(method)]
    fn key(this: &Cartridge, down: bool, code: &str, key: &str, mods: u8, repeat: bool) -> bool;
    #[wasm_bindgen(method)]
    fn text(this: &Cartridge, text: &str);
    #[wasm_bindgen(method)]
    fn typing(this: &Cartridge) -> bool;
}

/// The OS's desktop, in its CSS pixels (one a texel on the glass).
pub(super) const DESKTOP: (f32, f32) = (960.0, 540.0);
/// Where its files are, from the page; its storage's namespace.
const BASE: &str = "./os/";
const NS: &str = "battlestation.";

/// The pointer's kinds, as the cartridge counts them.
pub(super) const DOWN: u8 = 0;
pub(super) const MOVE: u8 = 1;
pub(super) const UP: u8 = 2;

/// The modifier keys held, as the cartridge counts them.
pub(super) fn mods(shift: bool, ctrl: bool, alt: bool, meta: bool, altgr: bool) -> u8 {
    shift as u8 | (ctrl as u8) << 1 | (alt as u8) << 2 | (meta as u8) << 3 | (altgr as u8) << 4
}

pub(super) struct Os {
    c: Cartridge,
    /// Its frames as last copied onto the glass, and its pixels then.
    seen: u32,
    buf: Vec<u8>,
}

/// Waits before each try to mount (ms): the page's own GPU settles
/// first, and a browser short of a WebGL2 context a moment may have one
/// the next. A mount that fails leaves nothing behind, so it may be tried
/// again.
const TRIES: [i32; 4] = [500, 1000, 2500, 5000];

async fn sleep(ms: i32) {
    let p = js_sys::Promise::new(&mut |done, _| {
        let _ = kit::window().set_timeout_with_callback_and_timeout_and_arguments_0(&done, ms);
    });
    let _ = wasm_bindgen_futures::JsFuture::from(p).await;
}

/// Start the OS (it boots to its welcome while the page goes on).
pub(super) async fn mount() -> Result<Os, String> {
    wasm_bindgen_futures::JsFuture::from(os_init())
        .await
        .map_err(|e| format!("the os did not load: {e:?}"))?;
    let mut why = String::new();
    for wait in TRIES {
        sleep(wait).await;
        match Cartridge::new(DESKTOP.0, DESKTOP.1, 1.0, BASE, NS) {
            Ok(c) => {
                return Ok(Os {
                    c,
                    seen: u32::MAX,
                    buf: Vec::new(),
                })
            }
            Err(e) => why = format!("the os did not mount: {e:?}"),
        }
    }
    Err(why)
}

impl Os {
    /// Its newest frame onto the glass, if one has come since the last:
    /// its pixels read back and uploaded (a frame only when the OS draws
    /// one; a still desktop costs nothing).
    pub(super) fn copy(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, glass: &mut Glass) {
        let f = self.c.frames();
        if f == self.seen {
            return;
        }
        let (w, h) = (self.c.width(), self.c.height());
        if w == 0 || h == 0 || w > 8192 || h > 8192 {
            return;
        }
        self.buf.resize((w * h * 4) as usize, 0);
        if self.c.pixels(&mut self.buf) {
            glass.show_os(device, queue, (w, h), &self.buf);
            self.seen = f;
        }
    }

    /// A point of the desk's screen (its pixels) on the OS's desktop.
    pub(super) fn at(cursor: (f32, f32)) -> (f32, f32) {
        (
            cursor.0 / SCREEN_PX.0 as f32 * DESKTOP.0,
            cursor.1 / SCREEN_PX.1 as f32 * DESKTOP.1,
        )
    }

    /// The arrow's point on the glass's picture (its texels).
    pub(super) fn arrow(&self, cursor: (f32, f32)) -> (f32, f32) {
        let (x, y) = Os::at(cursor);
        let k = self.c.width() as f32 / DESKTOP.0;
        (x * k, y * k)
    }

    pub(super) fn pointer(&self, kind: u8, cursor: (f32, f32), button: i16) {
        let (x, y) = Os::at(cursor);
        self.c.pointer(kind, x, y, button.clamp(0, 4) as u8);
    }

    /// The wheel over the desktop: whether the OS took it.
    pub(super) fn wheel(&self, cursor: (f32, f32), dy: f32) -> bool {
        let (x, y) = Os::at(cursor);
        self.c.wheel(x, y, dy)
    }

    /// A key down or up; going down, what it types too, while the OS is
    /// taking text.
    pub(super) fn key(&self, down: bool, (code, key): (&str, &str), mods: u8, repeat: bool) {
        self.c.key(down, code, key, mods, repeat);
        let plain = mods & 0b1010 == 0;
        if down && plain && key.chars().count() == 1 && self.c.typing() {
            self.c.text(key);
        }
    }
}
