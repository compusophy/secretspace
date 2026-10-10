//! The scene's globals, as the shaders read them (`shaders::common`'s
//! `Globals`, its fields in `shaders::FIELDS`' order): the view, the sun's
//! shadow (its cascades and the island's layer), the eye and its axes, the
//! look (sun, sky, air, sea), the lights' grid, the screen, the terrain,
//! the wind, the water, the grass.

use crate::buffers::put_f32s;
use crate::grid::Grid;
use crate::shadow::Layer;
use crate::{geo, laws, m4, shaders, Frame, Look, Quality, M4};

/// The sun's shadow as the globals carry it: its cascades, and the
/// island's layer and where it is in the map.
pub(crate) struct Sun<'a> {
    pub cascades: &'a [Layer],
    pub island: Option<(Layer, u32)>,
}

/// The globals' fields, by name, for frame `f` seen at `fov` on a screen
/// of `size`, under the sun's shadow `sun`; the grid, the tier, the
/// terrain's place and the grass's side as the renderer has them.
pub(crate) fn fields(
    f: &Frame,
    fov: f32,
    size: (u32, u32),
    sun: &Sun,
    (grid, quality, terrain, side): (&Grid, Quality, [f32; 4], u32),
) -> Vec<(&'static str, Vec<f32>)> {
    let cascades = sun.cascades;
    let (vp, right, up): (M4, _, _) = f.cam.matrices(fov);
    let fwd = f.cam.forward();
    let t = (fov / 2.0).tan();
    let l: &Look = &f.look;
    let lin = |c: [f32; 3]| c.map(|v| v.max(0.0).powf(2.2));
    let v4 = |v: [f32; 3], w: f32| vec![v[0], v[1], v[2], w];
    // The cascades, then the island's layer.
    let mut shadow = Vec::with_capacity(64);
    for c in 0..3 {
        shadow.extend_from_slice(cascades.get(c).map_or(&m4::ID, |l| &l.m));
    }
    shadow.extend_from_slice(sun.island.as_ref().map_or(&m4::ID, |l| &l.0.m));
    // Where each cascade ends (past the last, its end again); each
    // layer's bias and how far out along its normal it is looked up.
    let ends = laws::CASCADES[(quality.cascades.clamp(1, 3) - 1) as usize];
    let end = |c: usize| ends[c.min(ends.len() - 1)];
    let biases: Vec<(f32, f32)> = (0..3)
        .map(|c| cascades.get(c).map_or((0.0, 0.0), Layer::bias))
        .chain([sun.island.map_or((0.0, 0.0), |l| l.0.bias())])
        .collect();
    let g = grid;
    let q = quality;
    vec![
        ("vp", vp.to_vec()),
        ("shadow", shadow),
        ("eye", v4(f.cam.eye, f.time.rem_euclid(laws::TIME_WRAP))),
        ("fwd", v4(fwd, t * f.cam.aspect)),
        ("right", v4(right, t)),
        ("up", v4(up, size.1 as f32 / 2.0 / t)),
        (
            "sun_dir",
            v4(geo::norm(l.sun_dir), (l.sun_size / 2.0).cos()),
        ),
        ("sun", v4(l.sun, l.stars)),
        ("sky", v4(l.sky, l.fog)),
        ("low", v4(l.low, l.exposure)),
        ("zenith", v4(l.zenith, l.clouds)),
        ("horizon", v4(l.horizon, l.fog_falloff)),
        ("deep", v4(l.deep, l.sea.unwrap_or(-1000.0))),
        ("grid", vec![g.origin[0], g.origin[1], g.cell, g.n as f32]),
        (
            "view",
            vec![
                size.0 as f32,
                size.1 as f32,
                1.0 / quality.shadow_size as f32,
                cascades.len() as f32,
            ],
        ),
        (
            "splits",
            vec![end(0), end(1), end(2), laws::SHADOW_STRENGTH],
        ),
        ("bias", biases.iter().map(|b| b.0).collect()),
        ("offset", biases.iter().map(|b| b.1).collect()),
        (
            "island",
            match sun.island {
                Some((_, layer)) => vec![layer as f32, 1.0, 0.0, 0.0],
                None => vec![0.0; 4],
            },
        ),
        ("terrain", terrain.to_vec()),
        ("wind", vec![l.wind[0], l.wind[1], l.wet, l.glow]),
        ("water", v4(lin(l.water), l.waves)),
        (
            "grass",
            vec![
                q.grass_spacing,
                side as f32,
                q.grass_reach,
                if side > 0 { 1.0 } else { 0.0 },
            ],
        ),
    ]
}

/// The globals' bytes (`fields`, laid end to end).
pub(crate) fn bytes(
    f: &Frame,
    fov: f32,
    size: (u32, u32),
    sun: &Sun,
    shared: (&Grid, Quality, [f32; 4], u32),
) -> Vec<u8> {
    let mut b = Vec::with_capacity(shaders::GLOBALS as usize);
    for (_, v) in fields(f, fov, size, sun, shared) {
        put_f32s(&mut b, &v);
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Camera, Look};

    #[test]
    fn the_fields_are_the_shader_s_in_its_order_and_size() {
        let f = Frame {
            cam: Camera::default(),
            look: Look::default(),
            time: 1.0,
            items: &[],
            lights: &[],
            sparks: &[],
            decals: &[],
            view_fov: 0.9,
        };
        let sun = Sun {
            cascades: &[],
            island: None,
        };
        let shared = (&Grid::default(), Quality::HIGH, [0.0; 4], 0);
        let all = fields(&f, 1.2, (640, 360), &sun, shared);
        let names: Vec<&str> = all.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, shaders::FIELDS);
        for (n, v) in &all {
            let want = match *n {
                "vp" => 16,
                "shadow" => 64,
                _ => 4,
            };
            assert_eq!(v.len(), want, "{n}");
        }
        assert_eq!(
            bytes(&f, 1.2, (640, 360), &sun, shared).len() as u64,
            shaders::GLOBALS
        );
    }
}
