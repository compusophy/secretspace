//! The room around the desk, at night: walls, a window onto a city far
//! below, the desk and its mat, a lamp, the tower with its fans, a light
//! strip behind the desk, a neon sign, a mug and a plant. What never
//! moves is made once here (`statics`); what glows and changes colour is
//! told apart (`Glows`) so the page can tint it each frame.

use battlestation::laws::{
    CEILING, DESK_DEPTH, DESK_HALF, DESK_THICK, DESK_TOP, PAD, WALL_N, WALL_S, WALL_SIDE,
};
use render::geo::{self, hash, unit, Geo, V3};
use render::sculpt;

use crate::gear::face;

pub const FLOOR: V3 = [0.075, 0.07, 0.08];
pub const WALL: V3 = [0.15, 0.16, 0.2];
pub const ACCENT_WALL: V3 = [0.09, 0.1, 0.16];
pub const DESK: V3 = [0.17, 0.115, 0.085];
pub const MAT: V3 = [0.04, 0.042, 0.05];
pub const FRAME: V3 = [0.62, 0.62, 0.64];
pub const TOWER: V3 = [0.045, 0.047, 0.055];
pub const NIGHT_BLOCK: V3 = [0.035, 0.04, 0.055];

/// The window in the back wall: x from, x to, y from, y to.
pub const WINDOW: [f32; 4] = [-1.45, -0.75, 1.0, 1.95];
/// The lamp's bulb, the tower's fans (their middles), the neon sign's
/// middle, the mug's middle.
pub const BULB: V3 = [-0.47, 1.07, -0.40];
pub const FANS: [V3; 3] = [
    [0.615, DESK_TOP + 0.10, -0.268],
    [0.615, DESK_TOP + 0.23, -0.268],
    [0.615, DESK_TOP + 0.36, -0.268],
];
pub const NEON_AT: V3 = [0.64, 1.34, WALL_N + 0.012];
pub const MUG: V3 = [-0.43, DESK_TOP, -0.13];
/// The light strip on the wall behind the desk: its ends and height.
pub const STRIP: (f32, f32, f32) = (-0.8, 0.8, DESK_TOP + 0.035);
pub const STRIP_PARTS: usize = 10;

/// A box from `lo` to `hi`, every face out.
pub fn cuboid(g: &mut Geo, lo: V3, hi: V3, col: V3, glow: f32) {
    let c = geo::scale(geo::add(lo, hi), 0.5);
    let h = geo::scale(geo::sub(hi, lo), 0.5);
    let (x, y, z) = ([h[0], 0.0, 0.0], [0.0, h[1], 0.0], [0.0, 0.0, h[2]]);
    let neg = |v: V3| geo::scale(v, -1.0);
    face(g, geo::add(c, x), neg(z), y, col, glow);
    face(g, geo::sub(c, x), z, y, col, glow);
    face(g, geo::add(c, y), x, neg(z), col, glow);
    face(g, geo::sub(c, y), x, z, col, glow);
    face(g, geo::add(c, z), x, y, col, glow);
    face(g, geo::sub(c, z), neg(x), y, col, glow);
}

/// The room: floor, ceiling, walls (the back one round the window), the
/// window's frame, sill and blinds.
pub fn room() -> Geo {
    let mut g = Geo::default();
    let t = 0.12;
    let (s, n, w) = (WALL_S, WALL_N, WALL_SIDE);
    cuboid(&mut g, [-w, -t, n], [w, 0.0, s], FLOOR, 0.0);
    cuboid(&mut g, [-w, CEILING, n], [w, CEILING + t, s], WALL, 0.0);
    cuboid(&mut g, [-w - t, 0.0, n], [-w, CEILING, s], WALL, 0.0);
    cuboid(&mut g, [w, 0.0, n], [w + t, CEILING, s], WALL, 0.0);
    cuboid(&mut g, [-w, 0.0, s], [w, CEILING, s + t], WALL, 0.0);
    let [x0, x1, y0, y1] = WINDOW;
    let back = |g: &mut Geo, a: f32, b: f32, lo: f32, hi: f32| {
        cuboid(g, [a, lo, n - t], [b, hi, n], ACCENT_WALL, 0.0)
    };
    back(&mut g, -w, x0, 0.0, CEILING);
    back(&mut g, x1, w, 0.0, CEILING);
    back(&mut g, x0, x1, 0.0, y0);
    back(&mut g, x0, x1, y1, CEILING);
    // The frame, a bar down its middle, the sill.
    let f = 0.03;
    cuboid(
        &mut g,
        [x0 - f, y0 - f, n - 0.02],
        [x1 + f, y0, n + 0.01],
        FRAME,
        0.0,
    );
    cuboid(
        &mut g,
        [x0 - f, y1, n - 0.02],
        [x1 + f, y1 + f, n + 0.01],
        FRAME,
        0.0,
    );
    cuboid(
        &mut g,
        [x0 - f, y0, n - 0.02],
        [x0, y1, n + 0.01],
        FRAME,
        0.0,
    );
    cuboid(
        &mut g,
        [x1, y0, n - 0.02],
        [x1 + f, y1, n + 0.01],
        FRAME,
        0.0,
    );
    let mid = (x0 + x1) / 2.0;
    cuboid(
        &mut g,
        [mid - 0.012, y0, n - 0.07],
        [mid + 0.012, y1, n - 0.05],
        FRAME,
        0.0,
    );
    cuboid(
        &mut g,
        [x0 - 0.06, y0 - 0.05, n - 0.02],
        [x1 + 0.06, y0 - f, n + 0.07],
        FRAME,
        0.0,
    );
    // Blinds, half down: slats with gaps the moon comes through.
    for k in 0..9 {
        let y = y1 - 0.025 - k as f32 * 0.042;
        cuboid(
            &mut g,
            [x0, y - 0.004, n + 0.015],
            [x1, y, n + 0.045],
            FRAME,
            0.0,
        );
    }
    g
}

/// The desk: its top, two panels for legs, and the mat on it.
pub fn desk() -> Geo {
    let mut g = Geo::default();
    let (h, d) = (DESK_HALF, DESK_DEPTH);
    cuboid(
        &mut g,
        [-h, DESK_TOP - DESK_THICK, -d],
        [h, DESK_TOP, 0.0],
        DESK,
        0.0,
    );
    for x in [-h + 0.02, h - 0.05] {
        cuboid(
            &mut g,
            [x, 0.0, -d + 0.03],
            [x + 0.03, DESK_TOP - DESK_THICK, -0.03],
            DESK,
            0.0,
        );
    }
    cuboid(
        &mut g,
        [-h + 0.05, 0.35, -d + 0.02],
        [h - 0.05, 0.62, -d + 0.04],
        DESK,
        0.0,
    );
    g
}

pub fn mat() -> Geo {
    let mut g = Geo::default();
    cuboid(
        &mut g,
        [-0.44, DESK_TOP, -0.34],
        [0.47, DESK_TOP + PAD, -0.035],
        MAT,
        0.0,
    );
    g
}

/// A lamp on an arm, its shade over the desk's left, its bulb lit.
pub fn lamp() -> Geo {
    let foot = [-0.71, DESK_TOP, -0.62];
    let knee = [-0.69, 1.22, -0.66];
    let head = [BULB[0] - 0.03, BULB[1] + 0.06, BULB[2] - 0.03];
    let lip = [BULB[0] + 0.01, BULB[1] - 0.04, BULB[2] + 0.01];
    let f = move |p: V3| {
        let base = sculpt::rbox(
            p,
            geo::add(foot, [0.0, 0.009, 0.0]),
            [0.07, 0.009, 0.07],
            0.008,
        );
        let arm =
            sculpt::capsule(p, foot, knee, 0.0065).min(sculpt::capsule(p, knee, head, 0.0065));
        let shade = sculpt::cone(p, head, lip, 0.022, 0.065);
        let hollow = sculpt::cone(
            p,
            geo::add(head, [0.0, -0.01, 0.0]),
            geo::add(lip, [0.0, -0.02, 0.0]),
            0.017,
            0.062,
        );
        let shade = sculpt::carve(shade, hollow, 0.002);
        let bulb = sculpt::sphere(p, BULB, 0.022);
        base.min(arm).min(shade).min(bulb)
    };
    let lo = [-0.79, DESK_TOP - 0.01, -0.73];
    let hi = [-0.38, 1.24, -0.30];
    let paint = |p: V3, _n: V3| {
        if battlestation::len(geo::sub(p, BULB)) < 0.026 {
            ([1.0, 0.86, 0.62], 6.0)
        } else if p[1] > BULB[1] - 0.05 && battlestation::len(geo::sub(p, BULB)) < 0.12 {
            ([0.86, 0.82, 0.74], 0.0)
        } else {
            ([0.05, 0.05, 0.055], 0.0)
        }
    };
    sculpt::mesh(&f, (lo, hi), 0.005, &paint, (0.3, 0.02))
}

/// A small plant in a pot.
pub fn plant() -> Geo {
    let at = [-0.52, DESK_TOP, -0.64];
    let mut g = Geo::default();
    g.lathe(
        at,
        &[
            (0.0, 0.0),
            (0.04, 0.0),
            (0.048, 0.07),
            (0.05, 0.075),
            (0.0, 0.075),
        ],
        16,
        [0.82, 0.8, 0.76],
        0.0,
    );
    g.smooth();
    let mut leaves = Geo::default();
    for k in 0..9u32 {
        let a = unit(hash(k as i32, 3, 7)) * std::f32::consts::TAU;
        let tilt = 0.25 + unit(hash(k as i32, 5, 7)) * 0.5;
        let len = 0.07 + unit(hash(k as i32, 9, 7)) * 0.05;
        let dir = geo::norm([a.cos() * tilt, 1.0, a.sin() * tilt]);
        let c = geo::add(geo::add(at, [0.0, 0.08, 0.0]), geo::scale(dir, len * 0.6));
        leaves.sphere(
            c,
            [0.014, len * 0.55, 0.014],
            (1, k, 0.05),
            geo::mix(
                [0.12, 0.32, 0.14],
                [0.2, 0.42, 0.16],
                unit(hash(k as i32, 1, 1)),
            ),
            0.0,
        );
    }
    g.v.extend_from_slice(&leaves.v);
    let base = (g.v.len() - leaves.v.len()) as u32 / geo::STRIDE as u32;
    g.i.extend(leaves.i.iter().map(|i| i + base));
    g
}

/// The mug, and the coffee in it.
pub fn mug() -> Geo {
    let mut g = Geo::default();
    let ink = [0.86, 0.85, 0.82];
    g.lathe(
        MUG,
        &[
            (0.0, 0.0),
            (0.036, 0.0),
            (0.04, 0.006),
            (0.04, 0.095),
            (0.035, 0.095),
            (0.035, 0.012),
            (0.0, 0.012),
        ],
        20,
        ink,
        0.0,
    );
    g.smooth();
    let coffee = [0.10, 0.06, 0.035];
    g.lathe(MUG, &[(0.034, 0.012), (0.034, 0.08)], 20, coffee, 0.0);
    let handle = sculpt::mesh(
        &|p: V3| {
            let q = [p[0] - MUG[0] + 0.045, p[1] - MUG[1] - 0.05, p[2] - MUG[2]];
            // A ring standing up (about z): the torus turned.
            sculpt::torus([q[0], q[2], q[1]], [0.0; 3], 0.022, 0.006)
        },
        (
            geo::add(MUG, [-0.08, 0.015, -0.01]),
            geo::add(MUG, [-0.035, 0.085, 0.01]),
        ),
        0.003,
        &|_, _| (ink, 0.0),
        (0.2, 0.01),
    );
    let base = (g.v.len() / geo::STRIDE) as u32;
    g.v.extend_from_slice(&handle.v);
    g.i.extend(handle.i.iter().map(|i| i + base));
    g
}

/// The tower: a dark case open on the side toward you (glass, below),
/// showing what is inside: the board, the card, the cooler, the memory.
pub fn tower() -> Geo {
    let mut g = Geo::default();
    let (x0, x1, y0, y1, z0, z1) = (0.50, 0.73, DESK_TOP, DESK_TOP + 0.46, -0.72, -0.27);
    let tray = 0.52;
    cuboid(&mut g, [tray, y0, z0], [x1, y1, z1], TOWER, 0.0);
    cuboid(&mut g, [x0, y1 - 0.012, z0], [tray, y1, z1], TOWER, 0.0);
    cuboid(&mut g, [x0, y0, z0], [tray, y0 + 0.02, z1], TOWER, 0.0);
    cuboid(&mut g, [x0, y0, z1 - 0.012], [tray, y1, z1], TOWER, 0.0);
    cuboid(&mut g, [x0, y0, z0], [tray, y1, z0 + 0.012], TOWER, 0.0);
    let board = [0.03, 0.035, 0.045];
    cuboid(
        &mut g,
        [tray - 0.0008, y0 + 0.03, z0 + 0.02],
        [tray, y1 - 0.03, z1 - 0.08],
        board,
        0.0,
    );
    let metal = [0.16, 0.165, 0.18];
    cuboid(
        &mut g,
        [0.505, y0 + 0.15, z0 + 0.06],
        [tray, y0 + 0.19, z1 - 0.04],
        metal,
        0.0,
    );
    cuboid(
        &mut g,
        [0.507, y0 + 0.27, -0.63],
        [tray, y0 + 0.33, -0.57],
        metal,
        0.0,
    );
    for k in 0..4 {
        let z = -0.50 + k as f32 * 0.011;
        cuboid(
            &mut g,
            [0.512, y0 + 0.29, z],
            [tray, y0 + 0.39, z + 0.005],
            board,
            0.0,
        );
    }
    g
}

/// What glows in the tower, facing the glass: the card's edge, the tops
/// of the memory, the cooler's ring.
pub fn tower_lights() -> Geo {
    let mut g = Geo::default();
    let lit = |g: &mut Geo, c: V3, dz: f32, dy: f32| {
        face(g, c, [0.0, 0.0, dz], [0.0, dy, 0.0], [1.0; 3], 1.0)
    };
    lit(&mut g, [0.5045, DESK_TOP + 0.183, -0.48], 0.17, 0.0025);
    for k in 0..4 {
        let z = -0.4975 + k as f32 * 0.011;
        lit(&mut g, [0.5115, DESK_TOP + 0.385, z], 0.0025, 0.004);
    }
    for k in 0..32 {
        let a = k as f32 / 32.0 * std::f32::consts::TAU;
        let c = [
            0.5065,
            DESK_TOP + 0.30 + a.sin() * 0.026,
            -0.6 + a.cos() * 0.026,
        ];
        lit(&mut g, c, 0.0028, 0.0028);
    }
    g
}

/// A fan's ring of light, facing +z, its middle at the origin.
pub fn fan_ring() -> Geo {
    let f = |p: V3| sculpt::torus([p[0], p[2], p[1]], [0.0; 3], 0.054, 0.0035);
    sculpt::mesh(
        &f,
        ([-0.062, -0.062, -0.006], [0.062, 0.062, 0.006]),
        0.0025,
        &|_, _| ([1.0; 3], 1.0),
        (0.0, 0.01),
    )
}

/// The window's pane (faint).
pub fn glass() -> Geo {
    let mut g = Geo::default();
    let [x0, x1, y0, y1] = WINDOW;
    face(
        &mut g,
        [(x0 + x1) / 2.0, (y0 + y1) / 2.0, WALL_N - 0.06],
        [(x1 - x0) / 2.0, 0.0, 0.0],
        [0.0, (y1 - y0) / 2.0, 0.0],
        [0.55, 0.65, 0.8],
        0.0,
    );
    g
}

/// A section of the light strip, a unit long along x (the page places
/// and tints each).
pub fn strip_part() -> Geo {
    let mut g = Geo::default();
    face(
        &mut g,
        [0.5, 0.0, 0.0],
        [0.5, 0.0, 0.0],
        [0.0, 0.006, 0.0],
        [1.0; 3],
        1.0,
    );
    g
}

/// The neon sign's tubes: the word in the pixel font, each lit run one
/// bar, about its middle, facing +z.
pub fn neon(word: &str) -> Geo {
    let mut g = Geo::default();
    let px = 0.0092;
    let n = word.chars().count() as f32;
    let (w, h) = ((n * 6.0 - 1.0) * px, 7.0 * px);
    for (i, c) in word.chars().enumerate() {
        for (row, bits) in pixels::font::glyph(c).iter().enumerate() {
            for col in 0..5 {
                if bits & (0x10 >> col) != 0 {
                    let x = -w / 2.0 + (i as f32 * 6.0 + col as f32 + 0.5) * px;
                    let y = h / 2.0 - (row as f32 + 0.5) * px;
                    face(
                        &mut g,
                        [x, y, 0.0],
                        [px * 0.42, 0.0, 0.0],
                        [0.0, px * 0.42, 0.0],
                        [1.0; 3],
                        1.0,
                    );
                }
            }
        }
    }
    g
}

/// The city far below the window: dark towers, their windows lit here
/// and there (on the sides that face the room).
pub fn city() -> Geo {
    let mut g = Geo::default();
    let street = -45.0;
    let mut k = 0;
    for gx in 0..9 {
        for gz in 0..7 {
            k += 1;
            if unit(hash(gx, gz, 11)) < 0.25 {
                continue;
            }
            let x = -150.0 + gx as f32 * 15.0 + unit(hash(gx, gz, 12)) * 6.0;
            let z = -170.0 + gz as f32 * 18.0 + unit(hash(gx, gz, 13)) * 6.0;
            if x > -25.0 || z > -30.0 {
                continue;
            }
            let (wx, wz) = (
                5.0 + unit(hash(gx, gz, 14)) * 6.0,
                5.0 + unit(hash(gx, gz, 15)) * 6.0,
            );
            let h = 25.0 + unit(hash(gx, gz, 16)).powi(2) * 85.0;
            let top = street + h;
            cuboid(
                &mut g,
                [x, street, z],
                [x + wx, top, z + wz],
                NIGHT_BLOCK,
                0.0,
            );
            // Lit windows on the faces toward the room (+x, +z).
            let floors = (h / 3.4) as i32;
            for f in 0..floors {
                let y = street + 2.0 + f as f32 * 3.4;
                for (side, across) in [(0, wz), (1, wx)] {
                    let cols = (across / 2.6) as i32;
                    for c in 0..cols {
                        let r = unit(hash(k * 97 + side, f * 31 + c, 17));
                        if r > 0.32 {
                            continue;
                        }
                        let warm = unit(hash(k, f * 7 + c, 18)) < 0.7;
                        let col = if warm {
                            [1.0, 0.72, 0.42]
                        } else {
                            [0.62, 0.8, 1.0]
                        };
                        let glow = 2.0 + r * 8.0;
                        let along = 1.3 + c as f32 * 2.6;
                        if side == 0 {
                            face(
                                &mut g,
                                [x + wx + 0.05, y, z + along],
                                [0.0, 0.0, -0.7],
                                [0.0, 0.9, 0.0],
                                col,
                                glow,
                            );
                        } else {
                            face(
                                &mut g,
                                [x + along, y, z + wz + 0.05],
                                [0.7, 0.0, 0.0],
                                [0.0, 0.9, 0.0],
                                col,
                                glow,
                            );
                        }
                    }
                }
            }
            // A red light on the tallest.
            if h > 80.0 {
                face(
                    &mut g,
                    [x + wx / 2.0, top + 0.6, z + wz + 0.02],
                    [0.35, 0.0, 0.0],
                    [0.0, 0.35, 0.0],
                    [1.0, 0.15, 0.1],
                    12.0,
                );
            }
        }
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_room_is_made_and_bounded() {
        for (name, g) in [
            ("room", room()),
            ("desk", desk()),
            ("lamp", lamp()),
            ("plant", plant()),
            ("mug", mug()),
            ("tower", tower()),
            ("tower lights", tower_lights()),
            ("fan", fan_ring()),
            ("neon", neon("secretspace")),
            ("city", city()),
        ] {
            assert!(g.triangles() > 2, "{name} is empty");
            assert!(
                g.triangles() < 200_000,
                "{name}: {} triangles",
                g.triangles()
            );
            assert!(g.v.iter().all(|x| x.is_finite()), "{name}");
            let most = g.i.iter().copied().max().unwrap_or(0) as usize;
            assert!(most < g.len(), "{name}: an index past its vertices");
        }
    }

    #[test]
    fn the_city_is_seen_through_the_window() {
        let g = city();
        let lit = g.v.chunks(geo::STRIDE).filter(|v| v[9] > 1.0).count();
        assert!(lit > 400, "lit windows: {lit}");
        // Some lit window lies along a line from the eye through the window.
        let eye = battlestation::laws::EYE;
        let [x0, x1, y0, y1] = WINDOW;
        let seen = g.v.chunks(geo::STRIDE).filter(|v| v[9] > 1.0).any(|v| {
            let d = geo::sub([v[0], v[1], v[2]], eye);
            let k = (WALL_N - eye[2]) / d[2];
            let (x, y) = (eye[0] + d[0] * k, eye[1] + d[1] * k);
            x > x0 && x < x1 && y > y0 && y < y1
        });
        assert!(seen);
    }
}
