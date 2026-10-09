//! What throws the sun's shadow into each cascade. Of everything that
//! never moves, a cascade draws only what could reach into its box, seen
//! from the sun (its bounding sphere within the box, and not beyond its
//! far side, with some slack), and all but the nearest draw the coarser
//! meshes. Laid out again only when a box has
//! moved more than the slack, the sun has turned, or the statics changed.

use crate::buffers::{MeshBuf, Run};
use crate::draw::Renderer;
use crate::{Item, M4};
use gpu::wgpu;

use crate::buffers::Grow;

/// How far past its box a cascade takes what casts (a share of its half
/// width): it is laid out again once its box has moved this far.
const SLACK: f32 = 0.2;

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
        }
    }

    /// The statics, as chosen for the eye, and as made.
    pub fn set(&mut self, chosen: &[Item], all: &[Item]) {
        self.near = chosen.to_vec();
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
    let col =
        |k: usize| (i.model[k].powi(2) + i.model[k + 1].powi(2) + i.model[k + 2].powi(2)).sqrt();
    let scale = col(0).max(col(4)).max(col(8));
    let r = meshes
        .get(i.mesh.0 as usize)
        .and_then(Option::as_ref)
        .map_or(1.0, |b| b.r);
    let k = (m[0] * m[0] + m[4] * m[4] + m[8] * m[8]).sqrt();
    let reach = 1.0 + SLACK + r * scale * k;
    // And not wholly beyond the box's far side (its shadow falls away
    // from the box, not into it).
    let z = m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14];
    let kz = (m[2] * m[2] + m[6] * m[6] + m[10] * m[10]).sqrt();
    let deep = 1.0 + (SLACK / k.max(1e-6) + r * scale) * kz;
    x.abs() <= reach && y.abs() <= reach && z <= deep
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
