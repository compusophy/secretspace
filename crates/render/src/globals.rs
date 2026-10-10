//! The scene's globals, as the shaders read them (`shaders::common`'s
//! `Globals`): the view, the sun's shadow (its cascades and the island's
//! layer), the eye and its axes, the look (sun, sky, air, sea), the
//! lights' grid, the screen, the terrain, the wind, the water, the grass.

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

/// The globals' bytes for frame `f` seen at `fov` on a screen of `size`,
/// under the sun's shadow `sun`; the grid, the tier, the terrain's place
/// and the grass's side as the renderer has them.
pub(crate) fn bytes(
    f: &Frame,
    fov: f32,
    size: (u32, u32),
    sun: &Sun,
    (grid, quality, terrain, side): (&Grid, Quality, [f32; 4], u32),
) -> Vec<u8> {
    let cascades = sun.cascades;
    let (vp, right, up): (M4, _, _) = f.cam.matrices(fov);
    let fwd = f.cam.forward();
    let t = (fov / 2.0).tan();
    let l: &Look = &f.look;
    let lin = |c: [f32; 3]| c.map(|v| v.max(0.0).powf(2.2));
    let mut b = Vec::with_capacity(shaders::GLOBALS as usize);
    put_f32s(&mut b, &vp);
    for c in 0..3 {
        put_f32s(&mut b, cascades.get(c).map_or(&m4::ID, |l| &l.m));
    }
    put_f32s(&mut b, sun.island.as_ref().map_or(&m4::ID, |l| &l.0.m));
    let v4 = |b: &mut Vec<u8>, v: [f32; 3], w: f32| put_f32s(b, &[v[0], v[1], v[2], w]);
    v4(&mut b, f.cam.eye, f.time.rem_euclid(laws::TIME_WRAP));
    v4(&mut b, fwd, t * f.cam.aspect);
    v4(&mut b, right, t);
    v4(&mut b, up, size.1 as f32 / 2.0 / t);
    v4(&mut b, geo::norm(l.sun_dir), (l.sun_size / 2.0).cos());
    v4(&mut b, l.sun, l.stars);
    v4(&mut b, l.sky, l.fog);
    v4(&mut b, l.low, l.exposure);
    v4(&mut b, l.zenith, l.clouds);
    v4(&mut b, l.horizon, l.fog_falloff);
    v4(&mut b, l.deep, l.sea.unwrap_or(-1000.0));
    let g = grid;
    put_f32s(&mut b, &[g.origin[0], g.origin[1], g.cell, g.n as f32]);
    let n = cascades.len() as f32;
    put_f32s(
        &mut b,
        &[
            size.0 as f32,
            size.1 as f32,
            1.0 / quality.shadow_size as f32,
            n,
        ],
    );
    // Where each cascade ends (past the last, its end again); each
    // layer's bias and how far out along its normal it is looked up; the
    // island's layer.
    let ends = laws::CASCADES[(quality.cascades.clamp(1, 3) - 1) as usize];
    let end = |c: usize| ends[c.min(ends.len() - 1)];
    put_f32s(&mut b, &[end(0), end(1), end(2), laws::SHADOW_STRENGTH]);
    let biases: Vec<(f32, f32)> = (0..3)
        .map(|c| cascades.get(c).map_or((0.0, 0.0), Layer::bias))
        .chain([sun.island.map_or((0.0, 0.0), |l| l.0.bias())])
        .collect();
    put_f32s(&mut b, &biases.iter().map(|b| b.0).collect::<Vec<_>>());
    put_f32s(&mut b, &biases.iter().map(|b| b.1).collect::<Vec<_>>());
    match sun.island {
        Some((_, layer)) => put_f32s(&mut b, &[layer as f32, 1.0, 0.0, 0.0]),
        None => put_f32s(&mut b, &[0.0; 4]),
    }
    put_f32s(&mut b, &terrain);
    put_f32s(&mut b, &[l.wind[0], l.wind[1], l.wet, l.glow]);
    v4(&mut b, lin(l.water), l.waves);
    let q = quality;
    put_f32s(
        &mut b,
        &[
            q.grass_spacing,
            side as f32,
            q.grass_reach,
            if side > 0 { 1.0 } else { 0.0 },
        ],
    );
    b
}
