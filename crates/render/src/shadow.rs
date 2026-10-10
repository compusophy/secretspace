//! The sun's shadow, as seen from the sun: cascades, each an orthographic
//! view over a stretch of what the eye sees (near ones sharp, far ones
//! wide), fitted to a sphere so they do not shimmer as you turn, and
//! snapped to their texels so they do not crawl as you walk; and one view
//! over the whole of what stands still (`whole`), drawn again only when
//! that changes or the sun turns. The map they are drawn into is
//! `shadows`.

use crate::geo::{self, V3};
use crate::{laws, m4, Camera, M4};

/// One layer of the shadow map as the scene reads it: its sun-space
/// matrix, how wide a texel of it is, and how deep it is (metres), so a
/// bias given in metres can be given in its own units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    pub m: M4,
    pub texel: f32,
    pub depth: f32,
}

impl Layer {
    /// Its bias (`laws::SHADOW_BIAS` metres) in its depth's units, and how
    /// far along a surface's normal its points are looked up (metres).
    pub fn bias(&self) -> (f32, f32) {
        (
            laws::SHADOW_BIAS / self.depth,
            laws::SHADOW_NORMAL * self.texel,
        )
    }
}

/// An orthographic projection from the sun, `rx` by `ry` metres either
/// side, `depth` metres deep, to WebGPU's 0..1 depth.
pub fn ortho(rx: f32, ry: f32, depth: f32) -> M4 {
    let mut m = [0.0; 16];
    m[0] = 1.0 / rx;
    m[5] = 1.0 / ry;
    m[10] = -1.0 / depth;
    m[15] = 1.0;
    m
}

/// The sun's frame: which way is across and which up, seen from it, and
/// the up it was made with.
fn frame(sun: V3) -> (V3, V3, V3) {
    let up = if sun[1].abs() > 0.99 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let (_, s, u) = m4::look([0.0; 3], geo::scale(sun, -1.0), up);
    (s, u, up)
}

/// The cascades, for a camera, the way to the sun, where each cascade
/// ends, and the shadow map's size.
pub fn fit(cam: &Camera, sun: V3, ends: &[f32], size: u32) -> Vec<Layer> {
    let sun = geo::norm(sun);
    let fwd = cam.forward();
    let ty = (cam.fov / 2.0).tan();
    let tx = ty * cam.aspect;
    let (s, u, up) = frame(sun);
    let mut near = 0.05;
    let mut out = Vec::with_capacity(ends.len());
    for &far in ends {
        // A sphere about this stretch of the view.
        let mid = (near + far) / 2.0;
        let centre = geo::add(cam.eye, geo::scale(fwd, mid));
        let spread = tx * tx + ty * ty;
        let rf = ((far - mid).powi(2) + far * far * spread).sqrt();
        let rn = ((mid - near).powi(2) + near * near * spread).sqrt();
        let r = (rf.max(rn) * 1.02).ceil();
        // Snap the centre to the map's texels, in the sun's frame.
        let texel = 2.0 * r / size as f32;
        let cx = (geo::dot(centre, s) / texel).floor() * texel;
        let cy = (geo::dot(centre, u) / texel).floor() * texel;
        let cz = geo::dot(centre, geo::scale(sun, -1.0));
        let snapped = geo::add(
            geo::add(geo::scale(s, cx), geo::scale(u, cy)),
            geo::scale(sun, -cz),
        );
        // Back toward the sun far enough to take in what casts into it.
        let back = laws::SHADOW_BACK + r;
        let eye = geo::add(snapped, geo::scale(sun, back));
        let (view_at, _, _) = m4::look(eye, geo::scale(sun, -1.0), up);
        let depth = back + r * 2.0;
        out.push(Layer {
            m: m4::mul(&ortho(r, r, depth), &view_at),
            texel,
            depth,
        });
        near = far;
    }
    out
}

/// One view from the sun over every sphere (its middle, its radius):
/// what stands still, the whole island; None when there is nothing.
pub fn whole(spheres: &[(V3, f32)], sun: V3, size: u32) -> Option<Layer> {
    let sun = geo::norm(sun);
    let toward = geo::scale(sun, -1.0);
    let (s, u, up) = frame(sun);
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for &(c, r) in spheres {
        for (k, axis) in [s, u, toward].into_iter().enumerate() {
            let at = geo::dot(c, axis);
            lo[k] = lo[k].min(at - r);
            hi[k] = hi[k].max(at + r);
        }
    }
    if spheres.is_empty() || !lo.iter().zip(&hi).all(|(a, b)| b >= a) {
        return None;
    }
    let (rx, ry) = (
        ((hi[0] - lo[0]) / 2.0).max(1.0),
        ((hi[1] - lo[1]) / 2.0).max(1.0),
    );
    // From a metre before the nearest, at its middle across and up.
    let eye = geo::add(
        geo::add(
            geo::scale(s, (lo[0] + hi[0]) / 2.0),
            geo::scale(u, (lo[1] + hi[1]) / 2.0),
        ),
        geo::scale(toward, lo[2] - 1.0),
    );
    let (view_at, _, _) = m4::look(eye, toward, up);
    let depth = hi[2] - lo[2] + 2.0;
    Some(Layer {
        m: m4::mul(&ortho(rx, ry, depth), &view_at),
        texel: 2.0 * rx.max(ry) / size as f32,
        depth,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where `p` falls in a layer: x, y (-1..1 inside) and depth (0..1).
    fn into(l: &Layer, p: V3) -> V3 {
        let q = [p[0], p[1], p[2], 1.0];
        [0, 1, 2].map(|r| (0..4).map(|k| l.m[k * 4 + r] * q[k]).sum::<f32>())
    }

    #[test]
    fn what_the_eye_sees_falls_in_the_first_cascade() {
        let cam = Camera {
            eye: [10.0, 2.0, 5.0],
            ..Camera::default()
        };
        let m = fit(&cam, [0.4, 0.8, 0.2], &[14.0, 48.0, 150.0], 2048);
        assert_eq!(m.len(), 3);
        let f = cam.forward();
        let p = into(&m[0], geo::add(cam.eye, geo::scale(f, 6.0)));
        assert!(p[0].abs() <= 1.0 && p[1].abs() <= 1.0, "{p:?}");
        assert!((0.0..=1.0).contains(&p[2]), "depth in range: {p:?}");
    }

    #[test]
    fn a_bias_is_the_same_few_centimetres_in_every_cascade() {
        let cam = Camera::default();
        let sun = [0.4, 0.8, 0.2];
        for l in fit(&cam, sun, &[14.0, 48.0, 150.0], 2048) {
            let (bias, _) = l.bias();
            // A point and one a bias nearer the sun: their depths differ
            // by the bias, in the layer's own units.
            let p = geo::add(cam.eye, [3.0, -1.0, 2.0]);
            let q = geo::add(p, geo::scale(geo::norm(sun), laws::SHADOW_BIAS));
            let d = into(&l, p)[2] - into(&l, q)[2];
            assert!((d - bias).abs() < bias * 0.01, "{d} {bias}");
        }
    }

    #[test]
    fn the_whole_view_holds_every_sphere_nearer_than_its_far_side() {
        let spheres = [
            ([0.0, 0.0, 0.0], 60.0),
            ([120.0, 3.0, -80.0], 20.0),
            ([-150.0, 30.0, 140.0], 5.0),
        ];
        for sun in [[0.4, 0.8, 0.2], [-0.6, 0.26, 0.3], [0.0, 1.0, 0.0]] {
            let l = whole(&spheres, sun, 2048).expect("a view");
            for (c, r) in spheres {
                for o in [[r, 0.0, 0.0], [0.0, -r, 0.0], [0.0, 0.0, r]] {
                    let p = into(&l, geo::add(c, o));
                    assert!(p[0].abs() <= 1.0 && p[1].abs() <= 1.0, "{sun:?} {p:?}");
                    assert!((0.0..=1.0).contains(&p[2]), "{sun:?} {p:?}");
                }
            }
            // Toward the sun is nearer (a smaller depth).
            let a = into(&l, [0.0; 3])[2];
            let b = into(&l, geo::scale(geo::norm(sun), 10.0))[2];
            assert!(b < a, "{a} {b}");
        }
        assert!(whole(&[], [0.0, 1.0, 0.0], 2048).is_none());
    }
}
