//! A wizard's parts as meshes, each hung from its joint as `pose` places
//! it, made twice: in full near the camera, coarser far off (`parts`
//! sculpts them). The robe in four panels hinged at the waist, each with
//! its gold hem; the chest, belt and pouch; the mantle and its high
//! collar; a head (bearded or not); the hat, its band and its tip (which
//! sways); sleeves and cuffs; a hand and the wand hand; legs and boots.
//! White parts take the wizard's colour; the rest keep their own. And the
//! broomstick and the wand's glowing orb.

use std::f32::consts::FRAC_PI_4;

use render::geo::{rgb, Geo};
use render::{Mesh, Renderer};

use super::parts::{self, GOLD};

/// The robe's quarters about the waist (front left, front right, back
/// right, back left), a little overlapped.
pub const QUARTERS: [f32; 4] = [-FRAC_PI_4, FRAC_PI_4, 3.0 * FRAC_PI_4, -3.0 * FRAC_PI_4];

/// How much coarser the far wizards are (cell size, weave).
pub const FAR: f32 = 2.4;

pub struct Parts {
    pub panels: [Mesh; 4],
    pub hems: [Mesh; 4],
    pub torso: Mesh,
    pub belt: Mesh,
    pub mantle: Mesh,
    pub collar: Mesh,
    pub trim: Mesh,
    pub head: [Mesh; 2],
    pub hat: Mesh,
    pub band: Mesh,
    pub hat_tip: Mesh,
    pub upper: Mesh,
    pub fore: Mesh,
    pub cuff: Mesh,
    pub hand: Mesh,
    pub wand: Mesh,
    pub thigh: Mesh,
    pub shin: Mesh,
    pub boot: Mesh,
}

impl Parts {
    fn new(r: &mut Renderer, q: f32) -> Parts {
        let mut m = |g: Geo| r.mesh(&g);
        Parts {
            panels: QUARTERS.map(|c| m(parts::panel(c, q))),
            hems: QUARTERS.map(|c| m(parts::hem(c, q))),
            torso: m(parts::torso(q)),
            belt: m(parts::belt(q)),
            mantle: m(parts::mantle(q)),
            collar: m(parts::collar(q)),
            trim: m(parts::trim(q)),
            head: [m(parts::head(true, q)), m(parts::head(false, q))],
            hat: m(parts::hat(q)),
            band: m(parts::band(q)),
            hat_tip: m(parts::hat_tip(q)),
            upper: m(parts::upper(q)),
            fore: m(parts::fore(q)),
            cuff: m(parts::cuff(q)),
            hand: m(parts::hand(q)),
            wand: m(parts::wand(q)),
            thigh: m(parts::thigh(q)),
            shin: m(parts::shin(q)),
            boot: m(parts::boot(q)),
        }
    }

    fn all(&self) -> Vec<Mesh> {
        let mut v = Vec::new();
        v.extend(self.panels);
        v.extend(self.hems);
        v.extend(self.head);
        v.extend([
            self.torso,
            self.belt,
            self.mantle,
            self.collar,
            self.trim,
            self.hat,
            self.band,
            self.hat_tip,
            self.upper,
            self.fore,
            self.cuff,
            self.hand,
            self.wand,
            self.thigh,
            self.shin,
            self.boot,
        ]);
        v
    }
}

pub struct Model {
    /// Near and far.
    pub lod: [Parts; 2],
    /// Each near part's coarser twin (for the sun's shadow).
    coarse: std::collections::HashMap<Mesh, Mesh>,
    pub broom: Mesh,
    pub orb: Mesh,
}

fn smooth(r: &mut Renderer, f: impl Fn(&mut Geo)) -> Mesh {
    let mut g = Geo::default();
    f(&mut g);
    g.smooth();
    r.mesh(&g)
}

impl Model {
    pub fn new(r: &mut Renderer) -> Model {
        let lod = [Parts::new(r, 1.0), Parts::new(r, FAR)];
        let coarse = lod[0].all().into_iter().zip(lod[1].all()).collect();
        Model {
            lod,
            coarse,
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
        }
    }

    /// A near part's coarser twin (itself, if it has none).
    pub fn coarse(&self, near: Mesh) -> Mesh {
        self.coarse.get(&near).copied().unwrap_or(near)
    }

    pub fn all(&self) -> Vec<Mesh> {
        let mut v = self.lod[0].all();
        v.extend(self.lod[1].all());
        v.extend([self.broom, self.orb]);
        v
    }
}
