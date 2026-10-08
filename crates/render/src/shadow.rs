//! The sun's shadow: cascades, each an orthographic view from the sun
//! over a stretch of what the eye sees (near ones sharp, far ones wide),
//! fitted to a sphere so they do not shimmer as you turn, and snapped to
//! their texels so they do not crawl as you walk.

use crate::geo::{self, V3};
use crate::{m4, Camera, M4};

/// An orthographic projection to WebGPU's 0..1 depth.
pub fn ortho(r: f32, near: f32, far: f32) -> M4 {
    let mut m = [0.0; 16];
    m[0] = 1.0 / r;
    m[5] = 1.0 / r;
    m[10] = -1.0 / (far - near);
    m[14] = -near / (far - near);
    m[15] = 1.0;
    m
}

/// The cascades' sun-space matrices, for a camera, the way to the sun,
/// where each cascade ends, and the shadow map's size.
pub fn fit(cam: &Camera, sun: V3, ends: &[f32], size: u32) -> Vec<M4> {
    let sun = geo::norm(sun);
    let fwd = cam.forward();
    let ty = (cam.fov / 2.0).tan();
    let tx = ty * cam.aspect;
    let up = if sun[1].abs() > 0.99 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
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
        let (_, s, u) = m4::look([0.0; 3], geo::scale(sun, -1.0), up);
        let texel = 2.0 * r / size as f32;
        let cx = (geo::dot(centre, s) / texel).floor() * texel;
        let cy = (geo::dot(centre, u) / texel).floor() * texel;
        let cz = geo::dot(centre, geo::scale(sun, -1.0));
        let snapped = geo::add(
            geo::add(geo::scale(s, cx), geo::scale(u, cy)),
            geo::scale(sun, -cz),
        );
        let back = 300.0 + r;
        let eye = geo::add(snapped, geo::scale(sun, back));
        let (view_at, _, _) = m4::look(eye, geo::scale(sun, -1.0), up);
        out.push(m4::mul(&ortho(r, 0.0, back + r * 2.0), &view_at));
        near = far;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_the_eye_sees_falls_in_the_first_cascade() {
        let cam = Camera {
            eye: [10.0, 2.0, 5.0],
            ..Camera::default()
        };
        let m = fit(&cam, [0.4, 0.8, 0.2], &[14.0, 48.0, 150.0], 2048);
        assert_eq!(m.len(), 3);
        let f = cam.forward();
        let p = geo::add(cam.eye, geo::scale(f, 6.0));
        let (x, y, w) = m4::project(&m[0], p);
        assert!(
            w > 0.0 && (x / w).abs() <= 1.0 && (y / w).abs() <= 1.0,
            "{x} {y}"
        );
        let z = (0..4)
            .map(|k| m[0][k * 4 + 2] * [p[0], p[1], p[2], 1.0][k])
            .sum::<f32>();
        assert!((0.0..=1.0).contains(&z), "depth in range: {z}");
    }
}
