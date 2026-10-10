//! The scene's globals, as the shaders read them (`shaders::common`'s
//! `Globals`): the view, the sun's cascades, the eye and its axes, the
//! look (sun, sky, air, sea), the lights' grid, the screen, the terrain,
//! the wind, the water, the grass.

use crate::buffers::put_f32s;
use crate::grid::Grid;
use crate::{geo, laws, m4, shaders, Frame, Look, Quality, M4};

/// The globals' bytes for frame `f` seen at `fov` on a screen of `size`,
/// with the sun's `cascades`; the grid, the tier, the terrain's place and
/// the grass's side as the renderer has them.
pub(crate) fn bytes(
    f: &Frame,
    fov: f32,
    size: (u32, u32),
    cascades: &[M4],
    (grid, quality, terrain, side): (&Grid, Quality, [f32; 4], u32),
) -> Vec<u8> {
    let (vp, right, up): (M4, _, _) = f.cam.matrices(fov);
    let fwd = f.cam.forward();
    let t = (fov / 2.0).tan();
    let l: &Look = &f.look;
    let lin = |c: [f32; 3]| c.map(|v| v.max(0.0).powf(2.2));
    let mut b = Vec::with_capacity(shaders::GLOBALS as usize);
    put_f32s(&mut b, &vp);
    for c in 0..3 {
        put_f32s(&mut b, cascades.get(c).unwrap_or(&m4::ID));
    }
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
    let s = laws::CASCADES;
    put_f32s(&mut b, &[s[0], s[1], s[2], laws::SHADOW_STRENGTH]);
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
