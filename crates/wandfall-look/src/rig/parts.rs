//! A wizard's parts, sculpted: the organic ones (the head and its face,
//! beard and brows; the hands closed about a wand; the boots; the chest,
//! the mantle, the belt and pouch) from distance fields blended like clay
//! and darkened in their creases; the cloth (the robe's panels, the
//! sleeves, the hat) woven as sheets with real folds. Each is made at a
//! level of detail: `q` 1 near, larger far (coarser, fewer triangles).
//! Each part is in its joint's frame as `pose` places it: +x forward, +y
//! up, +z to the right.

use std::f32::consts::{PI, TAU};

use render::geo::{rgb, Geo, V3};
use render::sculpt::{
    both, capsule, carve, cone, ellipsoid, mesh, rbox, sheet, smin, sphere, torus,
};

use super::pose::{ANKLE, FORE, SHIN, THIGH, TIP, UPPER};

pub const SKIN: V3 = rgb(226, 170, 130);
pub const GOLD: V3 = rgb(224, 178, 92);
pub const WHITE: V3 = rgb(250, 250, 252);
const CLOTH: V3 = rgb(50, 44, 58);
const LEATHER: V3 = rgb(96, 62, 38);
const BEARD: V3 = rgb(238, 236, 230);
const HAIR: V3 = rgb(92, 60, 38);
const EYE: V3 = rgb(250, 248, 240);
const IRIS: V3 = rgb(70, 110, 150);
const LINING: V3 = rgb(70, 66, 80);

/// The robe falls this far below the waist.
pub const HEM: f32 = 0.83;

fn mix(a: V3, b: V3, t: f32) -> V3 {
    render::geo::mix(a, b, t.clamp(0.0, 1.0))
}

fn scale(a: V3, k: f32) -> V3 {
    render::geo::scale(a, k)
}

/// Steps for a sheet at this detail.
fn steps(n: usize, q: f32) -> usize {
    ((n as f32 / q).round() as usize).max(4)
}

/// One geometry onto another.
fn join(g: &mut Geo, h: Geo) {
    let base = g.len() as u32;
    g.v.extend(h.v);
    g.i.extend(h.i.into_iter().map(|k| k + base));
}

/// A sculpt meshed: `cell` metres at full detail, creases darkened.
fn carved(
    f: impl Fn(V3) -> f32,
    bounds: (V3, V3),
    cell: f32,
    q: f32,
    paint: impl Fn(V3, V3) -> V3,
) -> Geo {
    mesh(
        &f,
        bounds,
        cell * q,
        &|p, n| (paint(p, n), 0.0),
        (0.55, 0.06),
    )
}

/// As `carved`, its creases only lightly darkened (a face's are small).
fn carved_soft(
    f: impl Fn(V3) -> f32,
    bounds: (V3, V3),
    cell: f32,
    q: f32,
    paint: impl Fn(V3, V3) -> V3,
) -> Geo {
    mesh(
        &f,
        bounds,
        cell * q,
        &|p, n| (paint(p, n), 0.0),
        (0.3, 0.03),
    )
}

// The robe.

/// Its radius going down (0 at the waist, 1 at the hem), flaring.
fn robe_r(v: f32) -> f32 {
    0.255 + 0.19 * v.powf(1.3)
}

/// How far its folds stand out at `v`, at angle `a` (deeper toward the
/// hem, never repeating quite evenly).
fn fold(v: f32, a: f32) -> f32 {
    let amp = 0.003 + 0.034 * v * v;
    amp * ((a * 9.0 + 1.1 * v).sin() + 0.45 * (a * 17.0 + 2.0 - 2.5 * v).sin())
}

/// A quarter of the robe about angle `c` (a little over, so they overlap).
pub fn panel(c: f32, q: f32) -> Geo {
    let span = PI / 4.0 + 0.07;
    let mut g = Geo::default();
    sheet(
        &mut g,
        (steps(20, q), steps(16, q)),
        |u, v| {
            let a = c - span + 2.0 * span * u;
            let r = robe_r(v) + fold(v, a);
            let (s, cc) = a.sin_cos();
            [cc * r, -HEM * v, s * r]
        },
        |u, v| {
            let a = c - span + 2.0 * span * u;
            // The folds' valleys and the waist under the belt, shaded.
            let valley = 0.5 - 0.5 * (a * 9.0 + 1.1 * v).sin();
            scale(WHITE, 1.0 - 0.3 * valley * v - 0.3 * (1.0 - v).powi(6))
        },
    );
    g
}

/// Its gold hem, about angle `c`.
pub fn hem(c: f32, q: f32) -> Geo {
    let span = PI / 4.0 + 0.07;
    let mut g = Geo::default();
    sheet(
        &mut g,
        (steps(20, q), 2),
        |u, v| {
            let a = c - span + 2.0 * span * u;
            let w = 0.955 + 0.047 * v;
            let r = robe_r(w) + fold(w, a) + 0.005;
            let (s, cc) = a.sin_cos();
            [cc * r, -HEM * w, s * r]
        },
        |_, v| scale(GOLD, 0.85 + 0.15 * v),
    );
    g
}

// The body.

pub fn torso(q: f32) -> Geo {
    let f = |p: V3| {
        let waist = ellipsoid(p, [0.0, 0.06, 0.0], [0.16, 0.14, 0.2]);
        let ribs = ellipsoid(p, [0.0, 0.25, 0.0], [0.18, 0.17, 0.235]);
        let chest = ellipsoid(p, [0.05, 0.27, 0.0], [0.15, 0.12, 0.2]);
        let shoulders = capsule(p, [-0.01, 0.35, -0.2], [-0.01, 0.35, 0.2], 0.095);
        let d = smin(smin(waist, ribs, 0.08), smin(chest, shoulders, 0.06), 0.06);
        // Where the robe closes down the front, a crease.
        let seam = (1.0 - (p[2] / 0.02).abs()).max(0.0) * (p[0] > 0.0) as i32 as f32;
        d + 0.005 * seam
    };
    carved(
        f,
        ([-0.3, -0.1, -0.36], [0.3, 0.5, 0.36]),
        0.024,
        q,
        |_, _| WHITE,
    )
}

pub fn belt(q: f32) -> Geo {
    let mut g = Geo::default();
    sheet(
        &mut g,
        (steps(32, q), 3),
        |u, v| {
            let a = -u * TAU;
            let k = 1.0 + 0.04 * (PI * v).sin();
            [0.176 * k * a.cos(), -0.005 + 0.07 * v, 0.216 * k * a.sin()]
        },
        |_, _| LEATHER,
    );
    let buckle = |p: V3| rbox(p, [0.18, 0.03, 0.0], [0.012, 0.045, 0.05], 0.01);
    let pouch = |p: V3| {
        let bag = rbox(p, [0.05, -0.06, -0.218], [0.06, 0.07, 0.035], 0.028);
        let flap = rbox(p, [0.05, -0.008, -0.226], [0.066, 0.022, 0.04], 0.014);
        smin(bag, flap, 0.01)
    };
    join(
        &mut g,
        carved(
            buckle,
            ([0.14, -0.03, -0.08], [0.22, 0.09, 0.08]),
            0.008,
            q,
            |_, _| GOLD,
        ),
    );
    join(
        &mut g,
        carved(
            pouch,
            ([-0.03, -0.15, -0.28], [0.13, 0.03, -0.16]),
            0.01,
            q,
            |p, _| scale(LEATHER, if p[1] > -0.03 { 0.95 } else { 0.8 }),
        ),
    );
    g
}

/// A capelet over the shoulders, its edge scalloped.
pub fn mantle(q: f32) -> Geo {
    let mut g = Geo::default();
    sheet(
        &mut g,
        (steps(36, q), steps(10, q)),
        |u, v| {
            let a = u * TAU;
            let edge = mantle_edge(a)[1];
            let low = ((edge - 0.3) / 0.18).clamp(-1.0, 1.0).acos();
            let f = v * low;
            let (sa, ca) = a.sin_cos();
            // Close over the chest and back, out over the shoulders.
            let side = sa * sa;
            [
                -0.01 + 0.24 * f.sin() * ca,
                0.3 + 0.18 * f.cos(),
                (0.3 + 0.02 * side) * f.sin() * sa,
            ]
        },
        |_, v| scale(WHITE, 1.0 - 0.15 * v * v),
    );
    g
}

/// The mantle's edge, out from where it hangs (angle `a`).
fn mantle_edge(a: f32) -> V3 {
    let edge = 0.21 + 0.022 * (a * 6.0).sin();
    let f = ((edge - 0.3) / 0.18).clamp(-1.0, 1.0).acos();
    let (sa, ca) = a.sin_cos();
    let side = sa * sa;
    [
        -0.01 + 0.24 * f.sin() * ca,
        0.3 + 0.18 * f.cos(),
        (0.3 + 0.02 * side) * f.sin() * sa,
    ]
}

/// The high collar standing behind the head, lined.
pub fn collar(q: f32) -> Geo {
    let mut g = Geo::default();
    let at = |u: f32, v: f32, inset: f32| {
        let a = (95.0 + 170.0 * u).to_radians();
        let r = 0.13 + 0.11 * v * v - inset;
        [-0.03 - 0.05 * v + a.cos() * r, 0.4 + 0.15 * v, a.sin() * r]
    };
    sheet(
        &mut g,
        (steps(16, q), steps(5, q)),
        |u, v| at(1.0 - u, v, 0.0),
        |_, v| scale(WHITE, 0.85 + 0.15 * v),
    );
    sheet(
        &mut g,
        (steps(16, q), steps(5, q)),
        |u, v| at(u, v, 0.006),
        |_, v| scale(WHITE, 0.55 + 0.25 * v),
    );
    g
}

/// The gold edge of the mantle.
pub fn trim(q: f32) -> Geo {
    let mut g = Geo::default();
    sheet(
        &mut g,
        (steps(48, q), 4),
        |u, v| {
            let a = u * TAU;
            let e = mantle_edge(a);
            let out = render::geo::norm([e[0] + 0.01, 0.0, e[2]]);
            let w = -v * TAU;
            [
                e[0] + 0.011 * w.cos() * out[0],
                e[1] + 0.011 * w.sin(),
                e[2] + 0.011 * w.cos() * out[2],
            ]
        },
        |_, _| GOLD,
    );
    g
}

// The head.

/// A head, bearded (an old wizard) or not (a younger one).
pub fn head(beard: bool, q: f32) -> Geo {
    let eyes = [[0.128, 0.182, -0.058], [0.128, 0.182, 0.058]];
    let beard_f = move |p: V3| {
        if !beard {
            // A short goatee.
            return ellipsoid(p, [0.155, 0.045, 0.0], [0.05, 0.06, 0.045]);
        }
        let b = smin(
            ellipsoid(p, [0.11, 0.05, 0.0], [0.095, 0.13, 0.125]),
            cone(p, [0.12, 0.0, 0.0], [0.2, -0.27, 0.0], 0.1, 0.02),
            0.06,
        );
        let mo = smin(
            capsule(p, [0.195, 0.108, 0.0], [0.17, 0.088, -0.08], 0.02),
            capsule(p, [0.195, 0.108, 0.0], [0.17, 0.088, 0.08], 0.02),
            0.01,
        );
        // Strands.
        smin(b, mo, 0.02) + 0.004 * (p[2] * 80.0 + p[1] * 12.0).sin()
    };
    let brows = |p: V3| {
        let one = |s: f32| {
            capsule(
                p,
                [0.15, 0.226, 0.03 * s],
                [0.14, 0.236, 0.095 * s],
                if beard { 0.019 } else { 0.012 },
            )
        };
        one(-1.0).min(one(1.0))
    };
    let hair = move |p: V3| {
        if beard {
            // Long and white, falling behind, clear of the face.
            let fall = smin(
                ellipsoid(p, [-0.04, 0.15, 0.0], [0.15, 0.16, 0.165]),
                cone(p, [-0.06, 0.12, 0.0], [-0.1, -0.16, 0.0], 0.13, 0.07),
                0.05,
            );
            let strands = 0.004 * (p[2].atan2(p[0]) * 28.0).sin();
            return carve(
                fall,
                rbox(p, [0.2, 0.15, 0.0], [0.17, 0.25, 0.25], 0.06),
                0.05,
            ) + strands;
        }
        let back = ellipsoid(p, [-0.03, 0.2, 0.0], [0.15, 0.17, 0.165]);
        // Clear of the face.
        carve(back, rbox(p, [0.2, 0.2, 0.0], [0.12, 0.2, 0.2], 0.05), 0.04)
    };
    let face = |p: V3| {
        let skull = ellipsoid(p, [0.0, 0.18, 0.0], [0.155, 0.18, 0.148]);
        let jaw = ellipsoid(p, [0.05, 0.1, 0.0], [0.12, 0.1, 0.122]);
        let neck = capsule(p, [-0.01, -0.05, 0.0], [0.0, 0.1, 0.0], 0.064);
        let mut d = smin(smin(skull, jaw, 0.05), neck, 0.04);
        d = smin(
            d,
            capsule(p, [0.128, 0.216, -0.075], [0.128, 0.216, 0.075], 0.028),
            0.03,
        );
        for z in [-0.072, 0.072] {
            d = smin(d, sphere(p, [0.115, 0.13, z], 0.045), 0.03);
        }
        let nose = smin(
            cone(p, [0.148, 0.19, 0.0], [0.205, 0.128, 0.0], 0.024, 0.023),
            sphere(p, [0.2, 0.128, 0.0], 0.026),
            0.01,
        );
        d = smin(d, nose, 0.02);
        for e in eyes {
            d = carve(d, sphere(p, [e[0] + 0.025, e[1], e[2]], 0.031), 0.012);
            d = smin(
                d,
                ellipsoid(p, [0.0, 0.17, e[2] * 2.75], [0.03, 0.052, 0.02]),
                0.015,
            );
        }
        d
    };
    let eye_f = move |p: V3| {
        eyes.iter()
            .map(|e| sphere(p, *e, 0.03))
            .fold(f32::MAX, f32::min)
    };
    let f = move |p: V3| {
        let d = smin(face(p), beard_f(p).min(brows(p)), 0.012);
        smin(d, hair(p), 0.02).min(eye_f(p))
    };
    let tuft = if beard { BEARD } else { HAIR };
    carved_soft(
        f,
        ([-0.24, -0.34, -0.24], [0.3, 0.42, 0.24]),
        0.0115,
        q,
        move |p, _| {
            let e = eyes
                .iter()
                .min_by(|a, b| sphere(p, **a, 0.0).total_cmp(&sphere(p, **b, 0.0)))
                .copied()
                .unwrap_or(eyes[0]);
            if eye_f(p) < 0.003 {
                let k = (p[0] - e[0]) / 0.03;
                return if k > 0.93 {
                    rgb(16, 18, 24)
                } else if k > 0.8 {
                    IRIS
                } else {
                    EYE
                };
            }
            let near = beard_f(p).min(brows(p)).min(hair(p));
            if near < 0.004 {
                return tuft;
            }
            // Rosier at the nose and cheeks.
            let rosy = (1.0 - sphere(p, [0.2, 0.128, 0.0], 0.0) / 0.05).max(0.0)
                + 0.5 * (1.0 - sphere(p, [0.13, 0.13, 0.072 * p[2].signum()], 0.0) / 0.05).max(0.0);
            mix(SKIN, rgb(220, 130, 110), 0.35 * rosy)
        },
    )
}

/// The hat: a wide brim that droops and waves, a crown that crumples.
pub fn hat(q: f32) -> Geo {
    let mut g = Geo::default();
    let crown_r = |v: f32, a: f32| 0.21 - 0.065 * v + 0.006 * (a * 5.0 + v * 6.0).sin();
    sheet(
        &mut g,
        (steps(28, q), steps(8, q)),
        |u, v| {
            let a = -u * TAU;
            let r = crown_r(v, a);
            [a.cos() * r, 0.28 + 0.47 * v, a.sin() * r]
        },
        |_, v| scale(WHITE, 0.82 + 0.18 * v),
    );
    let brim = |r: f32, a: f32| {
        let t = (r - 0.18) / 0.32;
        0.285 - 0.04 * t * t + 0.016 * t * (a * 3.0 + 0.6).sin()
    };
    sheet(
        &mut g,
        (steps(36, q), steps(6, q)),
        |u, v| {
            let a = u * TAU;
            let r = 0.18 + 0.32 * v;
            [a.cos() * r, brim(r, a), a.sin() * r]
        },
        |_, v| scale(WHITE, 0.75 + 0.25 * v),
    );
    // Its underside, a little below (seen from under the brim).
    sheet(
        &mut g,
        (steps(36, q), steps(6, q)),
        |u, v| {
            let a = -u * TAU;
            let r = 0.18 + 0.32 * v;
            [a.cos() * r, brim(r, a) - 0.007, a.sin() * r]
        },
        |_, v| scale(WHITE, 0.6 + 0.2 * v),
    );
    // A rolled edge.
    sheet(
        &mut g,
        (steps(36, q), 4),
        |u, v| {
            let a = u * TAU;
            let w = -v * TAU;
            let r = 0.5 + 0.011 * w.cos();
            [a.cos() * r, brim(0.5, a) + 0.011 * w.sin(), a.sin() * r]
        },
        |_, _| WHITE,
    );
    g
}

/// The band about the hat's crown, and its buckle.
pub fn band(q: f32) -> Geo {
    let mut g = Geo::default();
    sheet(
        &mut g,
        (steps(28, q), 2),
        |u, v| {
            let a = -u * TAU;
            let r = 0.21 - 0.065 * (0.01 + 0.12 * v) + 0.008;
            [a.cos() * r, 0.295 + 0.055 * v, a.sin() * r]
        },
        |_, _| LEATHER,
    );
    let buckle = |p: V3| {
        carve(
            rbox(p, [0.212, 0.322, 0.0], [0.01, 0.034, 0.04], 0.008),
            rbox(p, [0.22, 0.322, 0.0], [0.03, 0.018, 0.024], 0.004),
            0.003,
        )
    };
    join(
        &mut g,
        carved(
            buckle,
            ([0.18, 0.27, -0.06], [0.24, 0.37, 0.06]),
            0.005,
            q,
            |_, _| GOLD,
        ),
    );
    g
}

/// The hat's tip, curling back (it sways on its own joint).
pub fn hat_tip(q: f32) -> Geo {
    let mut g = Geo::default();
    sheet(
        &mut g,
        (steps(16, q), steps(10, q)),
        |u, v| {
            let a = -u * TAU;
            let r = 0.15 * (1.0 - v) + 0.006 + 0.005 * (1.0 - v) * (a * 4.0 + v * 9.0).sin();
            let back = 0.17 * v * v;
            [a.cos() * r - back, 0.42 * v, a.sin() * r]
        },
        |_, _| WHITE,
    );
    g
}

// The arms.

/// The upper arm's sleeve, loose.
pub fn upper(q: f32) -> Geo {
    let mut g = Geo::default();
    sheet(
        &mut g,
        (steps(14, q), steps(6, q)),
        |u, v| {
            let a = u * TAU;
            let r = 0.094 + 0.014 * v + 0.006 * (a * 6.0 + v * 3.0).sin();
            [a.cos() * r, -UPPER * v, a.sin() * r]
        },
        |_, v| scale(WHITE, 0.85 + 0.15 * v),
    );
    g
}

/// The forearm's bell sleeve: wide at the cuff, folded, lined.
pub fn fore(q: f32) -> Geo {
    let r =
        |v: f32, a: f32| 0.082 + 0.075 * v * v + (0.003 + 0.012 * v) * (a * 7.0 + v * 2.0).sin();
    let mut g = Geo::default();
    sheet(
        &mut g,
        (steps(18, q), steps(8, q)),
        |u, v| {
            let a = u * TAU;
            let rr = r(v, a);
            [a.cos() * rr, -FORE * v, a.sin() * rr]
        },
        |_, v| scale(WHITE, 0.82 + 0.18 * v),
    );
    // The lining, seen in the open end.
    sheet(
        &mut g,
        (steps(18, q), 3),
        |u, v| {
            let a = -u * TAU;
            let w = 0.7 + 0.3 * v;
            let rr = r(w, a) - 0.008;
            [a.cos() * rr, -FORE * w, a.sin() * rr]
        },
        |_, v| scale(LINING, 0.5 + 0.5 * v),
    );
    g
}

/// The gold edge of the sleeve's open end.
pub fn cuff(q: f32) -> Geo {
    let mut g = Geo::default();
    sheet(
        &mut g,
        (steps(18, q), 4),
        |u, v| {
            let a = u * TAU;
            let w = -v * TAU;
            let rr = 0.157 + 0.012 * (a * 7.0 + 2.0).sin() + 0.009 * w.cos();
            [a.cos() * rr, -FORE + 0.009 * w.sin(), a.sin() * rr]
        },
        |_, _| GOLD,
    );
    g
}

/// A hand closed in a loose fist, at the end of the forearm.
fn fist(p: V3) -> f32 {
    let y = -FORE - 0.02;
    let wrist = capsule(p, [0.0, -FORE + 0.02, 0.0], [0.0, y - 0.01, 0.0], 0.03);
    let palm = ellipsoid(p, [0.0, y - 0.045, 0.0], [0.04, 0.055, 0.036]);
    let mut d = smin(wrist, palm, 0.02);
    // Four fingers curled round in front, one under the other.
    for k in 0..4 {
        let fy = y - 0.025 - 0.019 * k as f32;
        let r = 0.012 - 0.0008 * k as f32;
        let a = capsule(p, [0.02, fy, -0.03], [0.045, fy - 0.004, -0.008], r);
        let b = capsule(p, [0.045, fy - 0.004, -0.008], [0.03, fy - 0.008, 0.02], r);
        d = smin(d, smin(a, b, 0.006), 0.012);
    }
    let thumb = capsule(p, [0.0, y - 0.015, 0.03], [0.032, y - 0.03, 0.03], 0.013);
    smin(d, thumb, 0.012)
}

pub fn hand(q: f32) -> Geo {
    carved(
        fist,
        ([-0.07, -FORE - 0.13, -0.07], [0.08, -FORE + 0.06, 0.07]),
        0.0075,
        q,
        |_, _| SKIN,
    )
}

/// The wand hand: the fist closed about a gnarled wand, a gold band at
/// its grip.
pub fn wand(q: f32) -> Geo {
    let stick = |p: V3| {
        let w = cone(
            p,
            [0.012, -FORE - 0.02, 0.0],
            [0.0, -TIP + 0.01, 0.0],
            0.021,
            0.01,
        );
        let knots = sphere(p, [0.008, -FORE - 0.24, 0.004], 0.019).min(sphere(
            p,
            [0.003, -FORE - 0.36, -0.004],
            0.016,
        ));
        smin(w, knots, 0.02) + 0.0018 * (p[1] * 90.0).sin()
    };
    let ring = |p: V3| {
        let s = [p[0] - 0.008, p[1] + FORE + 0.13, p[2]];
        torus(s, [0.0; 3], 0.021, 0.006)
    };
    let f = move |p: V3| fist(p).min(stick(p)).min(ring(p));
    carved(
        f,
        ([-0.07, -TIP - 0.02, -0.07], [0.08, -FORE + 0.06, 0.07]),
        0.0075,
        q,
        move |p, _| {
            if ring(p) < 0.003 {
                GOLD
            } else if fist(p) < stick(p) {
                SKIN
            } else {
                rgb(108, 72, 46)
            }
        },
    )
}

// The legs.

pub fn thigh(q: f32) -> Geo {
    let f = |p: V3| cone(p, [0.0, 0.0, 0.0], [0.0, -THIGH, 0.0], 0.085, 0.07);
    carved(
        f,
        ([-0.12, -THIGH - 0.1, -0.12], [0.12, 0.1, 0.12]),
        0.03,
        q,
        |_, _| CLOTH,
    )
}

pub fn shin(q: f32) -> Geo {
    let f = |p: V3| cone(p, [0.0, 0.0, 0.0], [0.0, -SHIN, 0.0], 0.068, 0.056);
    carved(
        f,
        ([-0.1, -SHIN - 0.09, -0.1], [0.1, 0.09, 0.1]),
        0.03,
        q,
        |_, _| CLOTH,
    )
}

/// A boot: its shaft folded over at the top, the foot, the toe curling
/// up to a gold point, a darker sole.
pub fn boot(q: f32) -> Geo {
    let tip = [0.285, 0.03, 0.0];
    let f = move |p: V3| {
        let shaft = cone(p, [0.0, -0.03, 0.0], [0.0, 0.16, 0.0], 0.072, 0.086);
        let fold = torus(p, [0.0, 0.165, 0.0], 0.086, 0.024);
        let foot = ellipsoid(p, [0.055, -0.045, 0.0], [0.135, 0.056, 0.07]);
        let toe = smin(
            cone(p, [0.13, -0.06, 0.0], [0.235, -0.035, 0.0], 0.05, 0.03),
            cone(p, [0.235, -0.035, 0.0], tip, 0.03, 0.012),
            0.02,
        );
        let d = smin(smin(shaft, fold, 0.02), smin(foot, toe, 0.04), 0.05);
        // Flat on the sole.
        both(d, -(p[1] + ANKLE), 0.006).min(sphere(p, tip, 0.017))
    };
    carved(
        f,
        ([-0.12, -ANKLE - 0.02, -0.12], [0.32, 0.22, 0.12]),
        0.013,
        q,
        move |p, _| {
            if sphere(p, tip, 0.017) < 0.003 {
                GOLD
            } else if p[1] < -ANKLE + 0.018 {
                scale(LEATHER, 0.55)
            } else if p[1] > 0.14 {
                scale(LEATHER, 1.15)
            } else {
                LEATHER
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The share of `g`'s triangles (wound as drawn) facing away from the
    /// upright through `axis(y)`, of those that are not flat.
    fn facing_out(g: &Geo, axis: impl Fn(f32) -> f32) -> f32 {
        let st = render::geo::STRIDE;
        let p = |k: u32| {
            let o = k as usize * st;
            [g.v[o], g.v[o + 1], g.v[o + 2]]
        };
        let (mut out, mut all) = (0, 0);
        for t in g.i.chunks(3) {
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            let n = render::geo::norm(render::geo::cross(
                render::geo::sub(b, a),
                render::geo::sub(c, a),
            ));
            if n[1].abs() > 0.7 || n.iter().any(|x| x.is_nan()) {
                continue;
            }
            let m = [
                (a[0] + b[0] + c[0]) / 3.0,
                (a[1] + b[1] + c[1]) / 3.0,
                (a[2] + b[2] + c[2]) / 3.0,
            ];
            let r = [m[0] - axis(m[1]), 0.0, m[2]];
            all += 1;
            if render::geo::dot(n, r) > 0.0 {
                out += 1;
            }
        }
        out as f32 / all.max(1) as f32
    }

    #[test]
    fn its_cloth_faces_out_so_it_is_seen() {
        // The opaque pass drops faces turned away: a sheet wound inward
        // shows the head through the hat.
        let none = |_: f32| 0.0;
        for (name, g, axis) in [
            ("panel", panel(0.8, 1.0), none as fn(f32) -> f32),
            ("hem", hem(0.8, 1.0), none),
            ("hat", hat(1.0), none),
            ("upper", upper(1.0), none),
            ("mantle", mantle(1.0), |_| -0.01),
            ("tip", hat_tip(1.0), |y| -0.17 * (y / 0.42).powi(2)),
        ] {
            let k = facing_out(&g, axis);
            assert!(k > 0.9, "{name}: {k} of it faces out");
        }
        // The cuff is a ring of tube: out from the tube's own middle.
        let g = cuff(1.0);
        let st = render::geo::STRIDE;
        let p = |k: u32| {
            let o = k as usize * st;
            [g.v[o], g.v[o + 1], g.v[o + 2]]
        };
        let out =
            g.i.chunks(3)
                .filter(|t| {
                    let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                    let n = render::geo::cross(render::geo::sub(b, a), render::geo::sub(c, a));
                    let m = [
                        (a[0] + b[0] + c[0]) / 3.0,
                        (a[1] + b[1] + c[1]) / 3.0,
                        (a[2] + b[2] + c[2]) / 3.0,
                    ];
                    let flat = (m[0] * m[0] + m[2] * m[2]).sqrt();
                    let mid = [m[0] / flat * 0.157, -FORE, m[2] / flat * 0.157];
                    render::geo::dot(n, render::geo::sub(m, mid)) > 0.0
                })
                .count();
        assert!(
            out * 10 > g.triangles() * 9,
            "the cuff: {out} of {}",
            g.triangles()
        );
        // The belt's and the hat band's rings (not their buckles).
        for (name, whole, n) in [("belt", belt(1.0), 32), ("band", band(1.0), 28)] {
            let ring = Geo {
                v: whole.v.clone(),
                i: whole.i[..n * 2 * 3 * 2].to_vec(),
            };
            assert!(facing_out(&ring, |_| 0.0) > 0.9, "{name}");
        }
    }

    #[test]
    fn a_wizard_is_detailed_near_and_light_far() {
        let parts = |q: f32| {
            let mut n = 0;
            for c in super::super::model::QUARTERS {
                n += panel(c, q).triangles() + hem(c, q).triangles();
            }
            for g in [
                torso(q),
                belt(q),
                mantle(q),
                collar(q),
                trim(q),
                head(true, q),
                hat(q),
                band(q),
                hat_tip(q),
                thigh(q),
                shin(q),
                boot(q),
                upper(q),
                fore(q),
                cuff(q),
                hand(q),
                wand(q),
            ] {
                assert!(!g.is_empty());
                n += g.triangles();
            }
            n
        };
        let (near, far) = (parts(1.0), parts(2.4));
        println!("triangles: near {near}, far {far}");
        assert!(near < 45_000 && far < near / 3, "near {near}, far {far}");
    }
}
