//! What sits on the desk and answers you: the keyboard (its case, a cap
//! for each key with its legend, the light under each cap), the mouse,
//! and the monitor. Made here as geometry; the page keeps it on the GPU
//! and moves it.

use battlestation::keys::{self, Key};
use battlestation::laws::{
    BEZEL, CAP_FOOT, CAP_H, CAP_TOP, CASE_H, DESK_TOP, KEYBOARD_RIM, KEYBOARD_X, KEYBOARD_Z, KEY_U,
    LAYOUT_D, LAYOUT_W, PANEL_DEPTH, PLATE, SCREEN_H, SCREEN_TILT, SCREEN_W,
};
use render::geo::{self, Geo, V3};
use render::sculpt;

/// Keycaps (as people pick colours): the letters', the modifiers', the
/// accents'; the legends, lit from under.
pub const CAP: V3 = [0.15, 0.155, 0.175];
pub const CAP_MOD: V3 = [0.075, 0.08, 0.095];
pub const CAP_ACCENT: V3 = [0.86, 0.36, 0.25];
pub const LEGEND: V3 = [1.0, 1.0, 1.0];
pub const LEGEND_GLOW: f32 = 1.4;
/// The case, the plate under the caps.
pub const CASE: V3 = [0.12, 0.125, 0.14];
pub const PLATE_COL: V3 = [0.02, 0.02, 0.025];
/// The mouse, its wheel; the monitor's shell.
pub const MOUSE: V3 = [0.055, 0.058, 0.066];
pub const WHEEL: V3 = [0.02, 0.02, 0.022];
pub const SHELL: V3 = [0.035, 0.036, 0.042];

/// A flat face: its middle, half its sides (`u` across, `v` up it); it
/// faces `u` × `v`.
pub fn face(g: &mut Geo, c: V3, u: V3, v: V3, col: V3, glow: f32) {
    let a = geo::sub(geo::sub(c, u), v);
    let b = geo::sub(geo::add(c, u), v);
    let cc = geo::add(geo::add(c, u), v);
    let d = geo::add(geo::sub(c, u), v);
    g.quad(a, b, cc, d, col, glow);
}

/// The colour of a key's cap.
pub fn cap_colour(k: &Key) -> V3 {
    match k.code {
        "Escape" | "Enter" => CAP_ACCENT,
        c if c.starts_with("Key") || c.starts_with("Digit") || c == "Space" => CAP,
        _ if k.legend.chars().count() == 1 => CAP,
        _ => CAP_MOD,
    }
}

/// The cap's width at its foot and its top (metres), for a key `w` units
/// wide.
pub fn cap_size(w: f32) -> (f32, f32) {
    let foot = w * KEY_U - (KEY_U - CAP_FOOT);
    (foot, foot - (CAP_FOOT - CAP_TOP))
}

/// A keycap `w` units wide, its foot's middle at the origin: a rounded
/// block narrowing toward its top.
pub fn keycap(w: f32, col: V3) -> Geo {
    let (foot, _) = cap_size(w);
    let shrink = CAP_FOOT - CAP_TOP;
    let f = move |p: V3| {
        let k = (p[1] / CAP_H).clamp(0.0, 1.0);
        let hx = (foot - shrink * k) / 2.0;
        let hz = (CAP_FOOT - shrink * k) / 2.0;
        sculpt::rbox(p, [0.0, CAP_H / 2.0, 0.0], [hx, CAP_H / 2.0, hz], 0.0016)
    };
    let m = 0.002;
    let lo = [-foot / 2.0 - m, -m, -CAP_FOOT / 2.0 - m];
    let hi = [foot / 2.0 + m, CAP_H + m, CAP_FOOT / 2.0 + m];
    sculpt::mesh(&f, (lo, hi), 0.0013, &|_, _| (col, 0.0), (0.3, 0.003))
}

/// A key's legend on its cap's top: each lit pixel of the font a small
/// square (a run along a row, one), lit from under.
pub fn legend(g: &mut Geo, text: &str, top: f32) {
    let n = text.chars().count() as f32;
    if n == 0.0 {
        return;
    }
    let px = 0.00068f32.min(top * 0.78 / (n * 6.0 - 1.0));
    let (w, h) = ((n * 6.0 - 1.0) * px, 7.0 * px);
    let (x0, z0, y) = (-w / 2.0, -h / 2.0, CAP_H + 0.00012);
    let up = [0.0, 0.0, -px / 2.0];
    for (i, c) in text.chars().enumerate() {
        for (row, bits) in pixels::font::glyph(c).iter().enumerate() {
            let mut col = 0;
            while col < 5 {
                if bits & (0x10 >> col) == 0 {
                    col += 1;
                    continue;
                }
                let start = col;
                while col < 5 && bits & (0x10 >> col) != 0 {
                    col += 1;
                }
                let x = x0 + (i as f32 * 6.0 + (start + col) as f32 / 2.0) * px;
                let z = z0 + (row as f32 + 0.5) * px;
                let half = (col - start) as f32 * px / 2.0;
                // Facing up: across (+x) crossed with toward the back (-z).
                face(g, [x, y, z], [half, 0.0, 0.0], up, LEGEND, LEGEND_GLOW);
            }
        }
    }
}

/// A key's cap with its legend.
pub fn key_mesh(k: &Key) -> Geo {
    let mut g = keycap(k.w, cap_colour(k));
    legend(&mut g, k.legend, cap_size(k.w).1);
    g
}

/// Where a key's cap stands (the middle of its foot) at rest.
pub fn cap_at(k: &Key) -> V3 {
    let t = k.top();
    [t[0], keys::cap_foot(t[2]), t[2]]
}

/// The light under a cap: a square a unit across, lit (the page tints it).
pub fn underglow() -> Geo {
    let mut g = Geo::default();
    let h = KEY_U * 0.42;
    face(
        &mut g,
        [0.0; 3],
        [h, 0.0, 0.0],
        [0.0, 0.0, -h],
        [1.0; 3],
        1.0,
    );
    g
}

/// The keyboard's case: a low wedge rising toward the back, its rim
/// rounded, the plate sunk inside it.
pub fn case() -> Geo {
    let hw = LAYOUT_W * KEY_U / 2.0 + KEYBOARD_RIM;
    let hd = LAYOUT_D * KEY_U / 2.0 + KEYBOARD_RIM;
    let (iw, id) = (
        LAYOUT_W * KEY_U / 2.0 + 0.0015,
        LAYOUT_D * KEY_U / 2.0 + 0.0015,
    );
    let f = move |p: V3| {
        let rise = keys::rise(p[2]);
        let h = (CASE_H + rise) / 2.0;
        let shell = sculpt::rbox(
            p,
            [KEYBOARD_X, DESK_TOP + h, KEYBOARD_Z],
            [hw, h, hd],
            0.004,
        );
        let well = sculpt::rbox(
            p,
            [KEYBOARD_X, DESK_TOP + PLATE + rise + 0.02, KEYBOARD_Z],
            [iw, 0.02, id],
            0.0005,
        );
        sculpt::carve(shell, well, 0.0012)
    };
    let lo = [
        KEYBOARD_X - hw - 0.004,
        DESK_TOP - 0.002,
        KEYBOARD_Z - hd - 0.004,
    ];
    let hi = [
        KEYBOARD_X + hw + 0.004,
        DESK_TOP + 0.05,
        KEYBOARD_Z + hd + 0.004,
    ];
    let paint = move |p: V3, n: V3| {
        let inside = (p[0] - KEYBOARD_X).abs() < iw - 0.0005
            && (p[2] - KEYBOARD_Z).abs() < id - 0.0005
            && n[1] > 0.7;
        (if inside { PLATE_COL } else { CASE }, 0.0)
    };
    sculpt::mesh(&f, (lo, hi), 0.003, &paint, (0.4, 0.006))
}

/// The mouse, its foot's middle at the origin, its front toward -z: a
/// smooth hump with a split between its buttons, a wheel, and a lit mark
/// at its back (the page tints it).
pub fn mouse() -> Geo {
    use battlestation::laws::{MOUSE_H, MOUSE_L, MOUSE_W};
    let f = |p: V3| {
        let back = sculpt::ellipsoid(
            p,
            [0.0, 0.0, 0.012],
            [MOUSE_W / 2.0, MOUSE_H, MOUSE_L * 0.42],
        );
        let front = sculpt::ellipsoid(
            p,
            [0.0, 0.0, -0.02],
            [MOUSE_W * 0.46, MOUSE_H * 0.8, MOUSE_L * 0.42],
        );
        let body = sculpt::smin(back, front, 0.02);
        let body = sculpt::both(body, -p[1], 0.003);
        let split = sculpt::rbox(p, [0.0, MOUSE_H, -0.04], [0.0005, 0.012, 0.03], 0.0);
        let body = sculpt::carve(body, split, 0.001);
        let wheel = sculpt::rbox(
            p,
            [0.0, MOUSE_H * 0.86, -0.026],
            [0.0034, 0.0062, 0.0075],
            0.003,
        );
        body.min(wheel)
    };
    let m = 0.006;
    let lo = [-MOUSE_W / 2.0 - m, -0.002, -MOUSE_L / 2.0 - m];
    let hi = [MOUSE_W / 2.0 + m, MOUSE_H + m, MOUSE_L / 2.0 + m];
    let paint = |p: V3, _n: V3| {
        if (p[2] + 0.026).abs() < 0.0085 && p[0].abs() < 0.0045 && p[1] > MOUSE_H * 0.8 {
            (WHEEL, 0.0)
        } else if p[2] > 0.034 && p[0].abs() < 0.008 && p[1] > MOUSE_H * 0.45 {
            ([1.0; 3], 0.7)
        } else {
            (MOUSE, 0.0)
        }
    };
    sculpt::mesh(&f, (lo, hi), 0.002, &paint, (0.35, 0.006))
}

/// The monitor's axes: across, up its face (leaning back), out of it
/// toward you.
pub fn screen_axes() -> [V3; 3] {
    let (s, c) = SCREEN_TILT.sin_cos();
    [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]]
}

/// A point on the monitor (across, up, out from the picture's middle).
pub fn on_screen(p: V3) -> V3 {
    let [x, y, z] = screen_axes();
    let at = battlestation::laws::SCREEN_AT;
    geo::add(
        at,
        geo::add(
            geo::scale(x, p[0]),
            geo::add(geo::scale(y, p[1]), geo::scale(z, p[2])),
        ),
    )
}

/// The monitor's shell about the picture (in its own axes: the picture's
/// middle at the origin, out of it +z), with its back.
pub fn monitor() -> Geo {
    let (hw, hh) = (SCREEN_W / 2.0 + BEZEL, SCREEN_H / 2.0 + BEZEL);
    let chin = 0.006;
    let f = move |p: V3| {
        let panel = sculpt::rbox(
            p,
            [0.0, -chin / 2.0, -PANEL_DEPTH / 2.0 - 0.0008],
            [hw, hh + chin / 2.0, PANEL_DEPTH / 2.0],
            0.004,
        );
        let back = sculpt::rbox(
            p,
            [0.0, -0.03, -PANEL_DEPTH - 0.02],
            [0.19, 0.11, 0.024],
            0.02,
        );
        sculpt::smin(panel, back, 0.02)
    };
    let lo = [-hw - 0.01, -hh - chin - 0.01, -PANEL_DEPTH - 0.06];
    let hi = [hw + 0.01, hh + 0.01, 0.006];
    let paint = move |p: V3, n: V3| {
        // A small light on the chin.
        let led = (p[0] - SCREEN_W * 0.46).abs() < 0.0015
            && (p[1] + hh + chin * 0.5).abs() < 0.0015
            && n[2] > 0.5;
        if led {
            ([0.4, 0.7, 1.0], 3.0)
        } else {
            (SHELL, 0.0)
        }
    };
    sculpt::mesh(&f, (lo, hi), 0.006, &paint, (0.3, 0.01))
}

/// The picture itself as the engine sees it (lit, so it blooms and its
/// light shows): a black pane a hair in front of the shell.
pub fn pane() -> Geo {
    let mut g = Geo::default();
    face(
        &mut g,
        [0.0, 0.0, 0.0004],
        [SCREEN_W / 2.0, 0.0, 0.0],
        [0.0, SCREEN_H / 2.0, 0.0],
        [1.0; 3],
        1.0,
    );
    g
}

/// The monitor's stand: a neck from the back of the shell to a flat foot.
pub fn stand() -> Geo {
    let back = on_screen([0.0, -0.04, -PANEL_DEPTH - 0.045]);
    let z = back[2] - 0.012;
    let f = move |p: V3| {
        let foot = sculpt::rbox(
            p,
            [0.0, DESK_TOP + 0.006, z + 0.01],
            [0.12, 0.006, 0.085],
            0.006,
        );
        let neck = sculpt::rbox(
            p,
            [0.0, (DESK_TOP + back[1]) / 2.0, z],
            [0.024, (back[1] - DESK_TOP) / 2.0, 0.012],
            0.008,
        );
        sculpt::smin(foot, neck, 0.012)
    };
    let lo = [-0.13, DESK_TOP - 0.002, z - 0.09];
    let hi = [0.13, back[1] + 0.01, z + 0.11];
    sculpt::mesh(&f, (lo, hi), 0.005, &|_, _| (SHELL, 0.0), (0.3, 0.01))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_gets_a_cap_with_its_legend() {
        for k in keys::layout() {
            let g = key_mesh(k);
            assert!(g.len() > 30, "{} has a cap", k.code);
            let lit = g.v.chunks(geo::STRIDE).filter(|v| v[9] > 0.0).count();
            assert_eq!(lit > 0, !k.legend.is_empty(), "{} legend", k.code);
            let (foot, _) = cap_size(k.w);
            for v in g.v.chunks(geo::STRIDE) {
                assert!(v[0].abs() <= foot / 2.0 + 0.001, "{} is too wide", k.code);
                assert!(v[1] >= -0.001 && v[1] <= CAP_H + 0.001);
            }
        }
    }

    #[test]
    fn the_desk_gear_is_made() {
        for (name, g) in [
            ("case", case()),
            ("mouse", mouse()),
            ("monitor", monitor()),
            ("stand", stand()),
        ] {
            assert!(g.triangles() > 100, "{name}: {}", g.triangles());
            assert!(g.triangles() < 120_000, "{name}: {}", g.triangles());
        }
    }

    #[test]
    fn the_picture_faces_the_chair() {
        let [_, _, out] = screen_axes();
        let to_eye = geo::norm(geo::sub(
            battlestation::laws::EYE,
            battlestation::laws::SCREEN_AT,
        ));
        assert!(geo::dot(out, to_eye) > 0.95);
    }
}
