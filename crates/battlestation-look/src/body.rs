//! The hands as they are drawn: a palm sculpted for each side, a capsule
//! for each segment of each finger (the last with its nail), the wrist
//! and the sleeve over the forearm, placed each frame from the core's
//! `Pose`. A tone is baked into the skin, so changing it remakes them.

use std::f32::consts::FRAC_PI_2;

use battlestation::hands::Pose;
use battlestation::keys::{Side, THUMB};
use battlestation::laws::{FOREARM, RADII, SEGMENTS};
use render::geo::{self, Geo, V3};
use render::{m4, sculpt, Item, Material, Mesh, M4};

/// The sleeve's cloth.
pub const SLEEVE: V3 = [0.075, 0.08, 0.1];

/// Skin tones to pick from (the Esc menu goes round them).
pub const TONES: [V3; 5] = [
    [0.93, 0.75, 0.63],
    [0.84, 0.63, 0.49],
    [0.70, 0.49, 0.36],
    [0.50, 0.33, 0.23],
    [0.33, 0.22, 0.15],
];

/// A capsule `len` long up +y from the origin, `r0` thick at its foot
/// and `r1` at its end; with a nail on its back (+z) if `nail`.
pub fn capsule(len: f32, (r0, r1): (f32, f32), col: V3, nail: Option<V3>) -> Geo {
    let n = 5;
    let mut prof = Vec::with_capacity(2 * n + 2);
    for k in 0..=n {
        let a = k as f32 / n as f32 * FRAC_PI_2;
        prof.push((r0 * a.sin(), -r0 * a.cos()));
    }
    for k in 0..=n {
        let a = k as f32 / n as f32 * FRAC_PI_2;
        prof.push((r1 * a.cos(), len + r1 * a.sin()));
    }
    let mut g = Geo::default();
    g.lathe([0.0; 3], &prof, 12, col, 0.0);
    g.smooth();
    if let Some(c) = nail {
        g.sphere(
            [0.0, len * 0.66, r1 * 0.6],
            [r1 * 0.64, len * 0.36, r1 * 0.45],
            (1, 0, 0.0),
            c,
            0.0,
        );
    }
    g
}

/// The palm of a hand (mesh axes: x to the hand's right, y up its back,
/// z back toward the elbow; the wrist at the origin).
pub fn palm(side: Side, tone: V3) -> Geo {
    let flip = if side == Side::Left { -1.0 } else { 1.0 };
    let f = move |p: V3| {
        // In a right hand's terms: out, up, forward.
        let q = [p[0] * flip, p[1], -p[2]];
        let main = sculpt::rbox(q, [0.002, 0.0, 0.05], [0.033, 0.0095, 0.038], 0.0095);
        let ridge = sculpt::capsule(q, [-0.026, 0.002, 0.083], [0.028, -0.004, 0.072], 0.0095);
        let thenar = sculpt::ellipsoid(q, [-0.023, -0.008, 0.032], [0.017, 0.012, 0.027]);
        let wrist = sculpt::ellipsoid(q, [0.0, -0.002, 0.004], [0.026, 0.0155, 0.022]);
        sculpt::smin(
            sculpt::smin(sculpt::smin(main, ridge, 0.012), thenar, 0.012),
            wrist,
            0.014,
        )
    };
    sculpt::mesh(
        &f,
        ([-0.055, -0.032, -0.105], [0.055, 0.03, 0.03]),
        0.0028,
        &|_, _| (tone, 0.0),
        (0.25, 0.008),
    )
}

/// Every mesh of the hands, as geometry.
pub struct HandGeo {
    pub palm: [Geo; 2],
    /// Finger `d`, segment `s` at `d * 3 + s`.
    pub seg: Vec<Geo>,
    pub arm: Geo,
    pub sleeve: Geo,
}

/// How thick segment `s` of finger `d` is at its foot and its end.
pub fn thickness(d: usize, s: usize) -> (f32, f32) {
    if d == THUMB && s == 0 {
        return (0.0125, 0.0108);
    }
    let r = RADII[d];
    (r * (1.0 - 0.055 * s as f32), r * (0.95 - 0.055 * s as f32))
}

pub fn hand_geo(tone: V3) -> HandGeo {
    let nail = geo::mix(tone, [1.0, 0.86, 0.84], 0.5);
    let mut seg = Vec::with_capacity(15);
    for (d, lens) in SEGMENTS.iter().enumerate() {
        for (s, &len) in lens.iter().enumerate() {
            seg.push(capsule(
                len,
                thickness(d, s),
                tone,
                (s == 2).then_some(nail),
            ));
        }
    }
    HandGeo {
        palm: [palm(Side::Left, tone), palm(Side::Right, tone)],
        seg,
        arm: capsule(0.07, (0.021, 0.025), tone, None),
        sleeve: capsule(FOREARM + 0.12, (0.036, 0.044), SLEEVE, None),
    }
}

/// The hands' meshes, on the GPU.
#[derive(Clone, Copy, Debug)]
pub struct HandMeshes {
    pub palm: [Mesh; 2],
    pub seg: [Mesh; 15],
    pub arm: Mesh,
    pub sleeve: Mesh,
}

/// A model that stands a +y mesh from `a` toward `b`, its +z toward
/// `up` (as near as it can).
pub fn along(a: V3, b: V3, up: V3) -> M4 {
    let y = geo::norm(geo::sub(b, a));
    let mut z = geo::sub(up, geo::scale(y, geo::dot(up, y)));
    if battlestation::len(z) < 1e-4 {
        z = geo::cross([1.0, 0.0, 0.0], y);
    }
    let z = geo::norm(z);
    let x = geo::cross(y, z);
    m4::basis(a, x, y, z)
}

/// The items that draw one hand in this pose.
pub fn items(p: &Pose, side: Side, m: &HandMeshes, out: &mut Vec<Item>) {
    let skin = |mesh: Mesh, model: M4| Item::new(mesh, model).material(Material::Skin).rough(0.55);
    let [o, up, fwd] = p.axes;
    let right = if side == Side::Left {
        geo::scale(o, -1.0)
    } else {
        o
    };
    let back = geo::scale(fwd, -1.0);
    let k = (side == Side::Right) as usize;
    out.push(skin(m.palm[k], m4::basis(p.wrist, right, up, back)));
    for (d, joints) in p.fingers.iter().enumerate() {
        for s in 0..3 {
            out.push(skin(m.seg[d * 3 + s], along(joints[s], joints[s + 1], up)));
        }
    }
    out.push(skin(m.arm, along(p.wrist, p.elbow, up)));
    let cuff = geo::add(
        p.wrist,
        geo::scale(geo::norm(geo::sub(p.elbow, p.wrist)), 0.055),
    );
    let reach = geo::add(cuff, geo::sub(p.elbow, p.wrist));
    out.push(
        Item::new(m.sleeve, along(cuff, reach, up))
            .material(Material::Cloth)
            .rough(0.95),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use battlestation::hands::Hands;

    #[test]
    fn a_hand_is_eighteen_upright_pieces() {
        let h = Hands::new();
        let meshes = HandMeshes {
            palm: [Mesh(0), Mesh(1)],
            seg: std::array::from_fn(|k| Mesh(2 + k as u32)),
            arm: Mesh(17),
            sleeve: Mesh(18),
        };
        let mut out = Vec::new();
        for side in [Side::Left, Side::Right] {
            items(&h.pose(side), side, &meshes, &mut out);
        }
        assert_eq!(out.len(), 36);
        for it in &out {
            let m = it.model;
            let col = |c: usize| [m[c * 4], m[c * 4 + 1], m[c * 4 + 2]];
            let (x, y, z) = (col(0), col(1), col(2));
            for v in [x, y, z] {
                assert!((battlestation::len(v) - 1.0).abs() < 1e-3, "unit axes");
            }
            // Right-handed: no mirror, so faces keep facing out.
            assert!(geo::dot(geo::cross(x, y), z) > 0.99);
        }
    }

    #[test]
    fn the_hands_are_made() {
        let g = hand_geo(TONES[2]);
        for p in &g.palm {
            assert!(
                p.triangles() > 500 && p.triangles() < 40_000,
                "{}",
                p.triangles()
            );
        }
        assert_eq!(g.seg.len(), 15);
        // A left palm is the right one in a mirror.
        let l = &g.palm[0].v;
        let r = &g.palm[1].v;
        let mean_x = |v: &Vec<f32>| {
            v.chunks(geo::STRIDE).map(|c| c[0]).sum::<f32>() / (v.len() / geo::STRIDE) as f32
        };
        assert!((mean_x(l) + mean_x(r)).abs() < 1e-3);
    }
}
