//! Battlestation's page: a desk at night, seen from the chair, drawn by
//! the engine on WebGPU (how it looks: `battlestation-look`). Your keyboard and mouse are the ones on the
//! desk: each key you press is pressed there by the finger that types it,
//! the mouse slides under the right hand, and the monitor runs a terminal
//! that takes what you type. Before you sit down, the desk types by
//! itself.
//!
//! Hooks for tests and captures: `?sit=1` sits you down at once (no
//! demo), `?demo=0` no demo, `?lean=0..1` leans in, `?tone=0..4` the
//! hands' skin, `?shot=1` titles the page "shot ready" once the picture
//! has settled, `?perf=1` puts the frame rate in the title, `?windowed`
//! keeps the page out of full screen, `?q=low|medium|high` the quality,
//! `?eye=x,y,z,yaw,pitch` holds the camera (metres, degrees: yaw 0 looks
//! north), `?capture=1` draws off screen and shows it as an image (what
//! a headless browser can photograph), `?os=0` keeps compusophyOS off
//! the monitor (its own terminal instead), `?lock=0` never takes the
//! mouse (a headless browser moves a locked mouse wrongly).

pub mod sound;

#[cfg(target_arch = "wasm32")]
mod page;

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
    pub sit: bool,
    pub demo: bool,
    pub lean: Option<f32>,
    pub tone: Option<usize>,
    pub shot: bool,
    pub perf: bool,
    pub windowed: bool,
    /// A held camera: where, and its yaw and pitch (degrees).
    pub eye: Option<[f32; 5]>,
    /// compusophyOS on the monitor once you sit (else its own terminal).
    pub os: bool,
    /// Take the mouse when you sit (else it moves the arrow unlocked).
    pub lock: bool,
}

impl Hooks {
    pub fn read(query: &str) -> Hooks {
        let on = |k: &str, d: bool| param(query, k).map_or(d, |v| v != "0");
        Hooks {
            sit: on("sit", false),
            demo: on("demo", true),
            lean: param(query, "lean")
                .and_then(|v| v.parse::<f32>().ok())
                .map(|v| v.clamp(0.0, 1.0)),
            tone: param(query, "tone")
                .and_then(|v| v.parse::<usize>().ok())
                .map(|v| v.min(look::body::TONES.len() - 1)),
            shot: on("shot", false),
            perf: on("perf", false),
            windowed: on("windowed", false),
            os: on("os", true),
            lock: on("lock", true),
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
        let h = Hooks::read("?sit=1&lean=0.5&tone=9&shot=1&windowed");
        assert!(h.sit && h.shot && h.windowed && h.demo && !h.perf);
        assert_eq!(h.lean, Some(0.5));
        assert_eq!(h.tone, Some(look::body::TONES.len() - 1));
        let d = Hooks::read("");
        assert!(!d.sit && d.demo && d.lean.is_none() && !d.windowed);
        assert!(!Hooks::read("?demo=0").demo);
        assert!(d.os && !Hooks::read("?os=0").os);
        assert!(d.lock && !Hooks::read("?lock=0").lock);
        assert_eq!(
            Hooks::read("?eye=0,1,0.2,10,-30").eye,
            Some([0.0, 1.0, 0.2, 10.0, -30.0])
        );
    }
}
