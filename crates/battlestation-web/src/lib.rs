//! Battlestation's page: a desk at night, seen from the chair, drawn by
//! the engine on WebGPU (how it looks: `battlestation-look`). You are in
//! the chair from the first frame: your keyboard and mouse are the ones
//! on the desk (each key pressed there by the finger that types it, the
//! mouse under the right hand where you point), and the monitor runs
//! compusophyOS (`page/os.rs`), which needs the page isolated (an
//! unisolated one loads itself again, on its own, once).
//!
//! Hooks for tests and captures: `?lean=0..1` leans in, `?tone=0..4` the
//! hands' skin, `?shot=1` titles the page "shot ready" once the picture
//! has settled, `?perf=1` puts the frame rate in the title, `?q=low|
//! medium|high` the quality, `?eye=x,y,z,yaw,pitch` holds the camera
//! (metres, degrees: yaw 0 looks north), `?capture=1` draws off screen
//! and shows it as an image (what a headless browser can photograph),
//! `?os=0` keeps compusophyOS off the monitor (its own terminal instead).
//!
//! Another page may run it as a cartridge (the exported `Cartridge`,
//! `page/cart.rs`): then the monitor's compusophyOS is a module its host
//! loaded for it alone (`opts.osModule`, its files at `opts.os`), so the
//! desk runs inside the OS, and inside itself, each level its own OS.

pub mod sound;

#[cfg(target_arch = "wasm32")]
mod page;

#[cfg(target_arch = "wasm32")]
kit::cartridge!(page::cart);

/// Whether a wheel turn (`wheel`, down positive) goes to the computer on
/// the monitor (pure). Plain, it does (it scrolls there; where nothing
/// takes it, it leans). With Shift it leans you in or out, until you can
/// lean no further that way (`lean`, 0 back in the chair, 1 all the way
/// in): then on into the screen, where a desk on it leans in turn, so a
/// desk inside a desk inside a desk can be dived into level by level.
pub fn wheel_to_screen(shift: bool, wheel: f32, lean: f32) -> bool {
    !shift || (wheel < 0.0 && lean >= 1.0) || (wheel > 0.0 && lean <= 0.0)
}

/// A wheel event's turn, down positive (pure): its vertical one, or with
/// Shift and no vertical turn its sideways one (Chrome on Windows and Linux
/// turns a mouse wheel sideways while Shift is held).
pub fn wheel_turn(dx: f64, dy: f64, shift: bool) -> f64 {
    if dy == 0.0 && shift {
        dx
    } else {
        dy
    }
}

/// Whether Shift is held after a key (pure): a Shift key itself says so by
/// going down or up (a host may not count it in its own `mods`); any other
/// key by its `mods` (Shift 1).
pub fn shift_after(code: &str, down: bool, mods: u8) -> bool {
    match code {
        "ShiftLeft" | "ShiftRight" => down,
        _ => mods & 1 != 0,
    }
}

/// The OS's files as a host names them (`opts.os`): an http(s) address,
/// ending in `/` (one is added); anything else, none.
pub fn os_base(given: Option<&str>) -> Option<String> {
    let s = given?.trim();
    let web = s.starts_with("https://") || s.starts_with("http://");
    if !web || s.len() > 2048 || s.contains(char::is_whitespace) {
        return None;
    }
    Some(match s.ends_with('/') {
        true => s.to_string(),
        false => format!("{s}/"),
    })
}

/// The OS's files beside a cartridge's own: `…/battlestation/pkg/` is
/// `…/battlestation/os/`, where scripts/build-web.sh mirrors them.
pub fn os_beside(base: &str) -> String {
    match base.trim_end_matches('/').rsplit_once('/') {
        Some((up, _)) if !up.is_empty() => format!("{up}/os/"),
        _ => "./os/".into(),
    }
}

/// A query value (`?k=v`), from a query string.
pub fn param(query: &str, k: &str) -> Option<String> {
    query
        .trim_start_matches('?')
        .split('&')
        .find_map(|kv| match kv.split_once('=') {
            Some((a, b)) if a == k => Some(b.to_string()),
            None if kv == k => Some(String::new()),
            _ => None,
        })
}

/// The page's hooks, read from its query string.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hooks {
    pub lean: Option<f32>,
    pub tone: Option<usize>,
    pub shot: bool,
    pub perf: bool,
    /// A held camera: where, and its yaw and pitch (degrees).
    pub eye: Option<[f32; 5]>,
    /// compusophyOS on the monitor (else its own terminal).
    pub os: bool,
}

impl Hooks {
    pub fn read(query: &str) -> Hooks {
        let on = |k: &str, d: bool| param(query, k).map_or(d, |v| v != "0");
        Hooks {
            lean: param(query, "lean")
                .and_then(|v| v.parse::<f32>().ok())
                .map(|v| v.clamp(0.0, 1.0)),
            tone: param(query, "tone")
                .and_then(|v| v.parse::<usize>().ok())
                .map(|v| v.min(look::body::TONES.len() - 1)),
            shot: on("shot", false),
            perf: on("perf", false),
            os: on("os", true),
            eye: param(query, "eye").and_then(|v| {
                let n: Vec<f32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
                <[f32; 5]>::try_from(n).ok()
            }),
        }
    }
}

/// Frames drawn before `?shot=1` calls the picture settled.
pub const SHOT_AFTER: u32 = 24;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hooks_read_the_query() {
        let h = Hooks::read("?lean=0.5&tone=9&shot=1");
        assert!(h.shot && !h.perf);
        assert_eq!(h.lean, Some(0.5));
        assert_eq!(h.tone, Some(look::body::TONES.len() - 1));
        let d = Hooks::read("");
        assert!(d.lean.is_none() && !d.shot);
        assert!(d.os && !Hooks::read("?os=0").os);
        assert_eq!(
            Hooks::read("?eye=0,1,0.2,10,-30").eye,
            Some([0.0, 1.0, 0.2, 10.0, -30.0])
        );
    }

    #[test]
    fn the_wheel_scrolls_leans_and_dives() {
        // Plain: the computer's, however you lean.
        for lean in [0.0, 0.5, 1.0] {
            assert!(wheel_to_screen(false, -100.0, lean));
            assert!(wheel_to_screen(false, 100.0, lean));
        }
        // Shift: the lean's while it can move that way.
        assert!(!wheel_to_screen(true, -100.0, 0.0));
        assert!(!wheel_to_screen(true, -100.0, 0.88));
        assert!(!wheel_to_screen(true, 100.0, 1.0));
        assert!(!wheel_to_screen(true, 100.0, 0.12));
        // All the way in, on into the screen; all the way out, likewise.
        assert!(wheel_to_screen(true, -100.0, 1.0));
        assert!(wheel_to_screen(true, 100.0, 0.0));
        // The notches add up to all the way in exactly (clamped).
        let mut lean = 0.0f32;
        for _ in 0..12 {
            lean = (lean + battlestation::laws::LEAN_NOTCH).clamp(0.0, 1.0);
        }
        assert!(wheel_to_screen(true, -1.0, lean));
    }

    #[test]
    fn shift_turns_the_wheel_sideways_and_is_heard_by_its_key() {
        assert_eq!(wheel_turn(0.0, -100.0, false), -100.0);
        assert_eq!(wheel_turn(-100.0, 0.0, true), -100.0);
        assert_eq!(wheel_turn(-100.0, 0.0, false), 0.0);
        assert_eq!(wheel_turn(30.0, -100.0, true), -100.0);
        // The Shift key itself, whatever the mods say.
        assert!(shift_after("ShiftLeft", true, 0));
        assert!(!shift_after("ShiftRight", false, 1));
        // Any other key by its mods.
        assert!(shift_after("KeyA", true, 1) && !shift_after("KeyA", true, 2));
    }

    #[test]
    fn the_os_comes_from_where_the_host_says_or_beside() {
        assert_eq!(
            os_base(Some("https://compusophy.com/")).as_deref(),
            Some("https://compusophy.com/")
        );
        assert_eq!(
            os_base(Some(" http://127.0.0.1:8790/battlestation/os")).as_deref(),
            Some("http://127.0.0.1:8790/battlestation/os/")
        );
        for bad in [
            "",
            "./os/",
            "/os/",
            "javascript:x",
            "file:///os/",
            "https://a b/",
        ] {
            assert_eq!(os_base(Some(bad)), None, "{bad}");
        }
        assert_eq!(os_base(None), None);
        assert_eq!(
            os_beside("https://secretspace.compusophy.com/battlestation/pkg/"),
            "https://secretspace.compusophy.com/battlestation/os/"
        );
        assert_eq!(
            os_beside("http://127.0.0.1:8787/battlestation/pkg"),
            "http://127.0.0.1:8787/battlestation/os/"
        );
        assert_eq!(os_beside(""), "./os/");
        assert_eq!(os_beside("/"), "./os/");
    }
}
