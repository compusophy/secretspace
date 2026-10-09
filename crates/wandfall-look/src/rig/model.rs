//! A wizard's parts as meshes, each hung from its joint as `pose` places
//! it: a robe in four panels hinged at the waist (each tinted the
//! wizard's colour, a gold hem under it), a chest, belt and pouch, a
//! mantle with a high collar behind, a head (bearded or not), a pointed
//! hat whose tip is its own part (it sways), bell sleeves on upper arms
//! and forearms, hands, a wand, legs, and boots with curled toes.
//! White parts take the wizard's colour; the rest keep their own.

use std::f32::consts::{FRAC_PI_4, PI, TAU};

use render::geo::{rgb, Geo, V3};
use render::{Mesh, Renderer};

use super::pose::{ANKLE, FORE, SHIN, THIGH, TIP, UPPER};

pub const SKIN: V3 = rgb(214, 160, 120);
pub const GOLD: V3 = rgb(214, 170, 90);
const WHITE: V3 = rgb(248, 248, 252);
const CLOTH: V3 = rgb(58, 50, 62);
const LEATHER: V3 = rgb(86, 56, 36);

fn smooth(r: &mut Renderer, f: impl Fn(&mut Geo)) -> Mesh {
    let mut g = Geo::default();
    f(&mut g);
    g.smooth();
    r.mesh(&g)
}

fn flat(r: &mut Renderer, f: impl Fn(&mut Geo)) -> Mesh {
    let mut g = Geo::default();
    f(&mut g);
    r.mesh(&g)
}

/// Part of a surface turned about the upright: `profile` (radius,
/// height) swept from `a0` to `a1` radians (+x toward +z), `n` steps.
fn sector(g: &mut Geo, profile: &[(f32, f32)], (a0, a1): (f32, f32), n: usize, col: V3) {
    let base = g.len() as u32;
    for &(r, y) in profile {
        for k in 0..=n {
            let a = a0 + (a1 - a0) * k as f32 / n as f32;
            let (s, c) = a.sin_cos();
            g.vertex([c * r, y, s * r], [c, 0.0, s], col, 0.0);
        }
    }
    let w = (n + 1) as u32;
    for j in 0..profile.len().saturating_sub(1) as u32 {
        for k in 0..n as u32 {
            let (a, b) = (base + j * w + k, base + j * w + k + 1);
            let (c, d) = (a + w, b + w);
            g.index(a, c, b);
            g.index(b, c, d);
        }
    }
}

/// Rings stacked up, each (radius, height, lean back), joined: a tube
/// that bends.
fn bent(g: &mut Geo, rings: &[(f32, f32, f32)], n: usize, col: V3) {
    let base = g.len() as u32;
    for &(r, y, back) in rings {
        for k in 0..n {
            let a = k as f32 / n as f32 * TAU;
            let (s, c) = a.sin_cos();
            g.vertex([c * r - back, y, s * r], [c, 0.0, s], col, 0.0);
        }
    }
    let n = n as u32;
    for j in 0..rings.len().saturating_sub(1) as u32 {
        for k in 0..n {
            let (a, b) = (base + j * n + k, base + j * n + (k + 1) % n);
            let (c, d) = (a + n, b + n);
            g.index(a, c, b);
            g.index(b, c, d);
        }
    }
}

pub struct Model {
    pub panels: [Mesh; 4],
    pub hems: [Mesh; 4],
    pub torso: Mesh,
    pub belt: Mesh,
    pub mantle: Mesh,
    pub collar: Mesh,
    pub head: [Mesh; 2],
    pub hat: Mesh,
    pub hat_tip: Mesh,
    pub upper: Mesh,
    pub fore: Mesh,
    pub cuff: Mesh,
    pub hand: Mesh,
    pub wand: Mesh,
    pub thigh: Mesh,
    pub shin: Mesh,
    pub boot: Mesh,
    pub broom: Mesh,
    pub orb: Mesh,
    pub ball: Mesh,
}

/// The robe's quarters about the waist (front left, front right, back
/// right, back left), a little overlapped.
pub const QUARTERS: [f32; 4] = [-FRAC_PI_4, FRAC_PI_4, 3.0 * FRAC_PI_4, -3.0 * FRAC_PI_4];

impl Model {
    pub fn new(r: &mut Renderer) -> Model {
        let span = |c: f32| (c - FRAC_PI_4 - 0.07, c + FRAC_PI_4 + 0.07);
        // To the ankle, flaring as it falls.
        let robe = [
            (0.255, 0.0),
            (0.285, -0.15),
            (0.33, -0.38),
            (0.39, -0.62),
            (0.43, -0.8),
            (0.435, -0.83),
        ];
        Model {
            panels: QUARTERS.map(|c| smooth(r, |g| sector(g, &robe, span(c), 6, WHITE))),
            hems: QUARTERS.map(|c| {
                smooth(r, |g| {
                    sector(g, &[(0.427, -0.785), (0.438, -0.835)], span(c), 6, GOLD)
                })
            }),
            torso: smooth(r, |g| {
                let profile = [
                    (0.24, 0.0),
                    (0.25, 0.12),
                    (0.275, 0.26),
                    (0.26, 0.35),
                    (0.15, 0.44),
                    (0.0, 0.47),
                ];
                g.lathe([0.0; 3], &profile, 18, WHITE, 0.0);
            }),
            belt: flat(r, |g| {
                let brown = rgb(90, 62, 40);
                g.column(
                    [0.0, -0.01, 0.0],
                    18,
                    (0.282, 0.282),
                    0.08,
                    0.0,
                    brown,
                    0.0,
                    false,
                );
                g.block(
                    [0.28, -0.015, 0.0],
                    [0.04, 0.09, 0.1],
                    0.0,
                    GOLD,
                    GOLD,
                    0.15,
                );
                // A pouch at the left hip.
                g.block(
                    [0.06, -0.09, -0.28],
                    [0.13, 0.13, 0.07],
                    0.0,
                    LEATHER,
                    LEATHER,
                    0.0,
                );
            }),
            mantle: smooth(r, |g| {
                let profile = [
                    (0.29, 0.0),
                    (0.315, 0.08),
                    (0.28, 0.17),
                    (0.16, 0.26),
                    (0.0, 0.28),
                ];
                g.lathe([0.0, 0.2, 0.0], &profile, 18, WHITE, 0.0);
            }),
            collar: smooth(r, |g| {
                // A high collar behind the head, and a gold edge to the mantle.
                sector(
                    g,
                    &[(0.18, 0.4), (0.22, 0.5), (0.27, 0.6)],
                    (PI * 0.55, PI * 1.45),
                    8,
                    rgb(120, 110, 128),
                );
                g.lathe(
                    [0.0, 0.19, 0.0],
                    &[(0.292, 0.0), (0.322, 0.03)],
                    18,
                    GOLD,
                    0.1,
                );
            }),
            head: [true, false].map(|beard| {
                smooth(r, |g| {
                    g.sphere(
                        [0.0, 0.14, 0.0],
                        [0.19, 0.21, 0.19],
                        (2, 4, 0.02),
                        SKIN,
                        0.0,
                    );
                    for z in [-0.06f32, 0.06] {
                        g.sphere([0.172, 0.185, z], [0.024; 3], (1, 5, 0.0), WHITE, 0.0);
                        let dark = rgb(30, 40, 60);
                        g.sphere([0.19, 0.185, z], [0.012; 3], (1, 5, 0.0), dark, 0.0);
                        let (a, b) = ([0.17, 0.23, z - 0.03], [0.17, 0.235, z + 0.03]);
                        g.spike(a, b, 0.012, 4, WHITE, 0.0);
                        // Ears.
                        g.sphere(
                            [0.0, 0.15, z * 3.1],
                            [0.04, 0.06, 0.025],
                            (1, 6, 0.0),
                            SKIN,
                            0.0,
                        );
                    }
                    g.sphere(
                        [0.2, 0.12, 0.0],
                        [0.045, 0.04, 0.035],
                        (1, 6, 0.0),
                        SKIN,
                        0.0,
                    );
                    if beard {
                        g.spike([0.1, 0.08, 0.0], [0.24, -0.3, 0.0], 0.13, 10, WHITE, 0.0);
                        g.sphere(
                            [0.16, 0.07, 0.0],
                            [0.08, 0.05, 0.13],
                            (1, 7, 0.0),
                            WHITE,
                            0.0,
                        );
                    } else {
                        let hair = rgb(70, 50, 36);
                        g.sphere(
                            [-0.06, 0.12, 0.0],
                            [0.15, 0.16, 0.17],
                            (2, 9, 0.1),
                            hair,
                            0.0,
                        );
                    }
                })
            }),
            hat: smooth(r, |g| {
                let profile = [
                    (0.49, 0.0),
                    (0.49, 0.03),
                    (0.27, 0.06),
                    (0.2, 0.25),
                    (0.145, 0.45),
                ];
                g.lathe([0.0, 0.28, 0.0], &profile, 20, WHITE, 0.0);
                g.lathe(
                    [0.0, 0.335, 0.0],
                    &[(0.235, 0.0), (0.22, 0.06)],
                    20,
                    GOLD,
                    0.1,
                );
            }),
            hat_tip: smooth(r, |g| {
                let rings: Vec<(f32, f32, f32)> = (0..=7)
                    .map(|i| {
                        let t = i as f32 / 7.0;
                        (0.145 * (1.0 - t) + 0.004, 0.42 * t, 0.16 * t * t)
                    })
                    .collect();
                bent(g, &rings, 12, WHITE);
            }),
            upper: smooth(r, |g| {
                let profile = [(0.085, 0.0), (0.095, UPPER * 0.5), (0.105, UPPER)];
                g.lathe([0.0, -UPPER, 0.0], &profile, 10, WHITE, 0.0);
            }),
            fore: smooth(r, |g| {
                // A bell sleeve, wide at the cuff, open (its lip turns in
                // to the wrist, so no lid shows).
                let profile = [
                    (0.055, 0.03),
                    (0.145, 0.0),
                    (0.125, 0.06),
                    (0.095, 0.18),
                    (0.085, FORE),
                ];
                g.lathe([0.0, -FORE, 0.0], &profile, 12, WHITE, 0.0);
            }),
            cuff: smooth(r, |g| {
                g.lathe(
                    [0.0, -FORE, 0.0],
                    &[(0.13, 0.025), (0.148, 0.0), (0.142, 0.03)],
                    12,
                    GOLD,
                    0.1,
                );
            }),
            hand: smooth(r, |g| {
                g.sphere(
                    [0.0, -FORE - 0.05, 0.0],
                    [0.058, 0.065, 0.05],
                    (1, 8, 0.0),
                    SKIN,
                    0.0,
                );
                g.sphere(
                    [0.04, -FORE - 0.04, 0.035],
                    [0.025; 3],
                    (1, 8, 0.0),
                    SKIN,
                    0.0,
                );
            }),
            wand: smooth(r, |g| {
                g.sphere(
                    [0.0, -FORE - 0.05, 0.0],
                    [0.058, 0.065, 0.05],
                    (1, 8, 0.0),
                    SKIN,
                    0.0,
                );
                let wood = rgb(104, 70, 46);
                let profile = [(0.011, 0.0), (0.017, 0.22), (0.025, TIP - FORE - 0.02)];
                g.lathe([0.0, -TIP, 0.0], &profile, 8, wood, 0.0);
                g.lathe(
                    [0.0, -FORE - 0.13, 0.0],
                    &[(0.03, 0.0), (0.028, 0.03)],
                    8,
                    GOLD,
                    0.1,
                );
            }),
            thigh: smooth(r, |g| {
                let profile = [(0.07, 0.0), (0.085, THIGH * 0.6), (0.095, THIGH)];
                g.lathe([0.0, -THIGH, 0.0], &profile, 10, CLOTH, 0.0);
            }),
            shin: smooth(r, |g| {
                g.lathe(
                    [0.0, -SHIN, 0.0],
                    &[(0.058, 0.0), (0.072, SHIN)],
                    10,
                    CLOTH,
                    0.0,
                );
            }),
            boot: smooth(r, |g| {
                g.lathe(
                    [0.0, -0.02, 0.0],
                    &[(0.078, 0.0), (0.084, 0.15), (0.097, 0.18)],
                    10,
                    LEATHER,
                    0.0,
                );
                // The foot, its toe curling up to a point.
                g.sphere(
                    [0.04, -ANKLE + 0.045, 0.0],
                    [0.13, 0.05, 0.065],
                    (2, 3, 0.02),
                    LEATHER,
                    0.0,
                );
                g.spike(
                    [0.13, -ANKLE + 0.03, 0.0],
                    [0.27, -ANKLE + 0.1, 0.0],
                    0.045,
                    8,
                    LEATHER,
                    0.0,
                );
                g.sphere(
                    [0.27, -ANKLE + 0.1, 0.0],
                    [0.016; 3],
                    (1, 2, 0.0),
                    GOLD,
                    0.1,
                );
            }),
            broom: smooth(r, |g| {
                let wood = rgb(120, 86, 56);
                let straw = rgb(206, 170, 96);
                let profile = [(0.035, 0.0), (0.03, 1.8), (0.018, 1.86)];
                g.lathe([0.0, -0.9, 0.0], &profile, 8, wood, 0.0);
                let bristles = [(0.03, 0.0), (0.2, 0.14), (0.18, 0.38), (0.06, 0.55)];
                g.lathe([0.0, -1.45, 0.0], &bristles, 12, straw, 0.0);
                g.lathe(
                    [0.0, -0.95, 0.0],
                    &[(0.065, 0.0), (0.06, 0.08)],
                    12,
                    GOLD,
                    0.1,
                );
            }),
            orb: smooth(r, |g| {
                g.sphere([0.0; 3], [1.0; 3], (2, 9, 0.0), rgb(255, 246, 225), 1.0)
            }),
            ball: smooth(r, |g| {
                g.sphere([0.0; 3], [1.0; 3], (2, 3, 0.0), [1.0; 3], 0.0)
            }),
        }
    }

    pub fn all(&self) -> Vec<Mesh> {
        let mut v = Vec::new();
        v.extend(self.panels);
        v.extend(self.hems);
        v.extend(self.head);
        v.extend([
            self.torso,
            self.belt,
            self.mantle,
            self.collar,
            self.hat,
            self.hat_tip,
            self.upper,
            self.fore,
            self.cuff,
            self.hand,
            self.wand,
            self.thigh,
            self.shin,
            self.boot,
            self.broom,
            self.orb,
            self.ball,
        ]);
        v
    }
}
