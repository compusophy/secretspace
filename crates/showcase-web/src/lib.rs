//! The engine's showcase: one page that shows what the GPU layer draws.
//! Phase 0: a turning triangle and the pixel layer over it, on WebGPU, or
//! on WebGL2 where WebGPU is missing (or `?gpu=0`). Hooks for tests and
//! captures: `?t=<ms>` stops time there, `?hud=0` hides the layer,
//! `?shot=1` titles the page "shot ready" once the picture is settled,
//! `?perf=1` puts the frame rate and the first frame's time in the title.

pub mod shaders;

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
    /// Render time held still, in ms.
    pub t: Option<f64>,
    pub hud: bool,
    pub shot: bool,
    pub perf: bool,
    pub gpu: bool,
}

impl Hooks {
    pub fn read(query: &str) -> Hooks {
        let on = |k: &str, d: bool| param(query, k).map_or(d, |v| v != "0");
        Hooks {
            t: param(query, "t").and_then(|v| v.parse().ok()),
            hud: on("hud", true),
            shot: on("shot", false),
            perf: on("perf", false),
            gpu: on("gpu", true),
        }
    }
}

/// Frames drawn before `?shot=1` calls the picture settled.
pub const SHOT_AFTER: u32 = 3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hooks_read_the_query() {
        let h = Hooks::read("?t=1500&hud=0&shot=1&gpu=0");
        assert_eq!(h.t, Some(1500.0));
        assert!(!h.hud && h.shot && !h.perf && !h.gpu);
        let d = Hooks::read("");
        assert!(d.t.is_none() && d.hud && !d.shot && d.gpu);
        assert!(Hooks::read("?perf").perf);
    }
}
