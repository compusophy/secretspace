//! What of the statics is drawn: in the view, only what lies within a
//! cone a little wider than the eye's (laid again once the view turns
//! past the slack, or the eye moves); and what throws the sun's shadow
//! into each cascade. Of everything that
//! never moves, a cascade draws only what could reach into its box, seen
//! from the sun (its bounding sphere within the box, and not beyond its
//! far side, with some slack), and all but the nearest draw the coarser
//! meshes. Laid out again only when a box has
//! moved more than the slack, the sun has turned, or the statics changed.

use crate::buffers::{MeshBuf, Run};
use crate::draw::Renderer;
use crate::geo::{self, V3};
use crate::{Camera, Item, M4};
use gpu::wgpu;

use crate::buffers::Grow;

/// How far past its box a cascade takes what casts (a share of its half
/// width): it is laid out again once its box has moved this far.
const SLACK: f32 = 0.2;
/// How much wider than the eye's the view's cone is (radians), and how
/// far the eye may go before it is laid again (m).
const VIEW_SLACK: f32 = 0.35;
pub(crate) const EYE_SLACK: f32 = 6.0;

pub(crate) struct Cull {
    /// The statics as the eye has them (near or far), and all coarse;
    /// each sorted by mesh.
    near: Vec<Item>,
    coarse: Vec<Item>,
    pub bufs: Vec<Grow>,
    pub runs: Vec<Vec<Run>>,
    /// What moves, the same way, laid out every frame.
    pub moving_bufs: Vec<Grow>,
    pub moving_runs: Vec<Vec<Run>>,
    /// Each cascade's matrix when it was last laid out.
    laid: Vec<Option<M4>>,
    /// The view's cone when it was last laid out: the eye, which way it
    /// looked, its half angle.
    seen: Option<(V3, V3, f32)>,
}

impl Cull {
    pub fn new(device: &wgpu::Device, cascades: usize) -> Cull {
        Cull {
            near: Vec::new(),
            coarse: Vec::new(),
            bufs: (0..cascades)
                .map(|_| Grow::new(device, "casters", wgpu::BufferUsages::VERTEX))
                .collect(),
            runs: (0..cascades).map(|_| Vec::new()).collect(),
            moving_bufs: (0..cascades)
                .map(|_| Grow::new(device, "moving casters", wgpu::BufferUsages::VERTEX))
                .collect(),
            moving_runs: (0..cascades).map(|_| Vec::new()).collect(),
            laid: vec![None; cascades],
            seen: None,
        }
    }

    /// The statics, each at its mesh for the eye at `eye` (near or far),
    /// and all coarse.
    pub fn set(&mut self, all: &[Item], eye: V3) {
        self.near = all
            .iter()
            .map(|i| match i.far {
                Some((m, d)) => {
                    let v = geo::sub([i.model[12], i.model[13], i.model[14]], eye);
                    Item {
                        mesh: if geo::dot(v, v) > d * d { m } else { i.mesh },
                        ..*i
                    }
                }
                None => *i,
            })
            .collect();
        self.near.sort_by_key(|i| i.mesh);
        self.coarse = all
            .iter()
            .map(|i| Item {
                mesh: i.far.map_or(i.mesh, |f| f.0),
                ..*i
            })
            .collect();
        self.coarse.sort_by_key(|i| i.mesh);
        self.laid.iter_mut().for_each(|l| *l = None);
        self.seen = None;
    }

    /// What the view at `cam` could see, if it has turned (or gone) past
    /// what was last laid out; none if it has not.
    pub fn view(&mut self, cam: &Camera, meshes: &[Option<MeshBuf>]) -> Option<Vec<&Item>> {
        let fwd = cam.forward();
        let t = (cam.fov / 2.0).tan();
        let half = (t * t * (1.0 + cam.aspect * cam.aspect)).sqrt().atan();
        if let Some((_, was, h)) = self.seen {
            let turned = geo::dot(was, fwd).clamp(-1.0, 1.0).acos();
            if turned < VIEW_SLACK * 0.9 && (h - half).abs() < 0.02 {
                return None;
            }
        }
        let eye = cam.eye;
        self.seen = Some((eye, fwd, half));
        Some(
            self.near
                .iter()
                .filter(|i| in_view(i, (eye, fwd, half), meshes))
                .collect(),
        )
    }

    /// Lay out what casts into each cascade whose box has moved on.
    pub fn lay(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        cascades: &[M4],
        meshes: &[Option<MeshBuf>],
        bytes: &mut Vec<u8>,
    ) {
        for (c, m) in cascades.iter().enumerate() {
            if self.laid[c].is_some_and(|l| near_enough(&l, m)) {
                continue;
            }
            self.laid[c] = Some(*m);
            let from = if c == 0 { &self.near } else { &self.coarse };
            let mut items: Vec<&Item> = from.iter().filter(|i| reaches(i, m, meshes)).collect();
            bytes.clear();
            self.runs[c] = Renderer::lay(&mut items, bytes, 0);
            self.bufs[c].put(device, queue, bytes);
        }
    }
}

impl Cull {
    /// What moves and is solid, for each cascade: what reaches into it,
    /// at its coarser mesh if it has one.
    pub fn moving(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        cascades: &[M4],
        items: &[&Item],
        meshes: &[Option<MeshBuf>],
        bytes: &mut Vec<u8>,
    ) {
        for (c, m) in cascades.iter().enumerate() {
            let mut list: Vec<Item> = items
                .iter()
                .filter(|i| reaches(i, m, meshes))
                .map(|i| Item {
                    mesh: i.far.map_or(i.mesh, |f| f.0),
                    ..**i
                })
                .collect();
            list.sort_by_key(|i| i.mesh);
            let mut refs: Vec<&Item> = list.iter().collect();
            bytes.clear();
            self.moving_runs[c] = Renderer::lay(&mut refs, bytes, 0);
            self.moving_bufs[c].put(device, queue, bytes);
        }
    }
}

/// Whether `i` could throw a shadow into the box `m` sees (with slack).
fn reaches(i: &Item, m: &M4, meshes: &[Option<MeshBuf>]) -> bool {
    let p = [i.model[12], i.model[13], i.model[14]];
    let x = m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12];
    let y = m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13];
    // Its reach, in the box's units: the mesh's radius, scaled.
    let r = radius(i, meshes);
    let k = (m[0] * m[0] + m[4] * m[4] + m[8] * m[8]).sqrt();
    let reach = 1.0 + SLACK + r * k;
    // And not wholly beyond the box's far side (its shadow falls away
    // from the box, not into it).
    let z = m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14];
    let kz = (m[2] * m[2] + m[6] * m[6] + m[10] * m[10]).sqrt();
    let deep = 1.0 + (SLACK / k.max(1e-6) + r) * kz;
    x.abs() <= reach && y.abs() <= reach && z <= deep
}

/// Whether `i` lies in the cone from `eye` along `fwd`, `half` wide (and
/// the slack), or so near the eye it is drawn anyway.
fn in_view(i: &Item, (eye, fwd, half): (V3, V3, f32), meshes: &[Option<MeshBuf>]) -> bool {
    let v = geo::sub([i.model[12], i.model[13], i.model[14]], eye);
    let d = geo::dot(v, v).sqrt();
    let r = radius(i, meshes);
    if d <= r + EYE_SLACK {
        return true;
    }
    // Its angle off the view, against the cone's, its own size, and how
    // far the eye may go before this is laid again.
    let off = (geo::dot(v, fwd) / d).clamp(-1.0, 1.0).acos();
    let room = half + VIEW_SLACK + (r / d).min(1.0).asin() + (EYE_SLACK / d).min(1.0).asin();
    off <= room
}

/// How far `i` reaches from its middle: its mesh's radius, scaled.
fn radius(i: &Item, meshes: &[Option<MeshBuf>]) -> f32 {
    let col =
        |k: usize| (i.model[k].powi(2) + i.model[k + 1].powi(2) + i.model[k + 2].powi(2)).sqrt();
    let scale = col(0).max(col(4)).max(col(8));
    let r = meshes
        .get(i.mesh.0 as usize)
        .and_then(Option::as_ref)
        .map_or(1.0, |b| b.r);
    r * scale
}

/// Whether a box at `m` is still inside the one laid out at `l` with its
/// slack: turned no further, and moved less than the slack.
fn near_enough(l: &M4, m: &M4) -> bool {
    let turned = [0, 1, 4, 5, 8, 9]
        .iter()
        .any(|&k| (l[k] - m[k]).abs() > 1e-4 * l[k].abs().max(1e-3));
    !turned && (l[12] - m[12]).abs() < SLACK && (l[13] - m[13]).abs() < SLACK
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{shadow, Camera};

    #[test]
    fn a_box_moved_less_than_its_slack_is_not_laid_again() {
        let cam = Camera::default();
        let a = shadow::fit(&cam, [0.4, 0.8, 0.2], &[14.0], 2048)[0];
        let near = Camera {
            eye: [0.5, 0.0, 0.0],
            ..cam
        };
        let b = shadow::fit(&near, [0.4, 0.8, 0.2], &[14.0], 2048)[0];
        assert!(near_enough(&a, &b), "half a metre: no");
        let far = Camera {
            eye: [10.0, 0.0, 0.0],
            ..cam
        };
        let c = shadow::fit(&far, [0.4, 0.8, 0.2], &[14.0], 2048)[0];
        assert!(!near_enough(&a, &c), "ten metres: yes");
        let d = shadow::fit(&cam, [0.5, 0.8, 0.2], &[14.0], 2048)[0];
        assert!(!near_enough(&a, &d), "the sun turned: yes");
    }

    #[test]
    fn the_view_keeps_what_is_ahead_and_near_and_leaves_what_is_behind() {
        let meshes: Vec<Option<MeshBuf>> = Vec::new();
        let at = |p: V3| Item::new(crate::Mesh(0), crate::m4::place(p, 0.0, [1.0; 3]));
        let cone = ([0.0; 3], [1.0, 0.0, 0.0], 0.7);
        assert!(in_view(&at([50.0, 0.0, 10.0]), cone, &meshes), "ahead");
        assert!(
            in_view(&at([-3.0, 0.0, 0.0]), cone, &meshes),
            "behind, near"
        );
        assert!(!in_view(&at([-50.0, 0.0, 5.0]), cone, &meshes), "behind");
        assert!(!in_view(&at([0.0, 0.0, 60.0]), cone, &meshes), "beside");
    }

    #[test]
    fn what_could_shadow_the_box_is_kept_and_what_could_not_is_left() {
        let cam = Camera::default();
        let sun = crate::geo::norm([0.4, 0.3, 0.2]);
        let m = shadow::fit(&cam, sun, &[14.0], 2048)[0];
        let mid = crate::geo::add(cam.eye, crate::geo::scale(cam.forward(), 7.0));
        let at = |p: crate::geo::V3| Item::new(crate::Mesh(0), crate::m4::place(p, 0.0, [1.0; 3]));
        let meshes: Vec<Option<MeshBuf>> = Vec::new();
        // Between the sun and the box, far off: it may shadow it.
        let toward = at(crate::geo::add(mid, crate::geo::scale(sun, 120.0)));
        assert!(reaches(&toward, &m, &meshes));
        // Beyond the box, away from the sun: it cannot.
        let away = at(crate::geo::add(mid, crate::geo::scale(sun, -120.0)));
        assert!(!reaches(&away, &m, &meshes));
        // Off to its side: it cannot.
        let side = crate::geo::norm(crate::geo::cross(sun, [0.0, 1.0, 0.0]));
        let off = at(crate::geo::add(mid, crate::geo::scale(side, 120.0)));
        assert!(!reaches(&off, &m, &meshes));
    }
}
