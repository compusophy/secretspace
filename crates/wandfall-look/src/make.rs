//! The small ways meshes are made, shared by the island, the wizards and
//! the spells: a mesh flat-shaded (`one`) or shaded smooth (`smooth`),
//! one geometry joined onto another (`join`), and how many steps a sheet
//! takes at a level of detail (`steps`).

use render::geo::Geo;
use render::{Mesh, Renderer};

/// A mesh, flat-shaded.
pub(crate) fn one(r: &mut Renderer, f: impl Fn(&mut Geo)) -> Mesh {
    let mut g = Geo::default();
    f(&mut g);
    r.mesh(&g)
}

/// A mesh whose shared vertices are shaded smooth.
pub(crate) fn smooth(r: &mut Renderer, f: impl Fn(&mut Geo)) -> Mesh {
    let mut g = Geo::default();
    f(&mut g);
    g.smooth();
    r.mesh(&g)
}

/// One geometry onto another.
pub(crate) fn join(g: &mut Geo, h: Geo) {
    let base = g.len() as u32;
    g.v.extend(h.v);
    g.i.extend(h.i.into_iter().map(|k| k + base));
}

/// Steps for a sheet of `n` steps near, at detail `q` (1 near, larger
/// coarser), never fewer than four.
pub(crate) fn steps(n: usize, q: f32) -> usize {
    ((n as f32 / q).round() as usize).max(4)
}
