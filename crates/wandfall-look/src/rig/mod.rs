//! A wizard, jointed and alive, in four parts:
//!
//! - `gait`: how it moves, as watched (eased by springs): speed and
//!   heading off its aim, its hips turning toward where it goes, its
//!   stride and phase (advanced by distance, so its feet stay planted),
//!   lean, bank, the drag of its robe and the lag of its hat; rising and
//!   falling, jumps made in the air; its feet held as it turns in place;
//! - `pose`: its skeleton each frame: feet placed through stance and
//!   swing (and leaping, tucking and reaching down in the air), knees
//!   found by two-bone reach, the chest keeping its aim, arms swinging
//!   against the legs, the wand arm rising to cast, the robe's panels
//!   following the thighs; sitting on a broom; falling;
//! - `model`: its parts as meshes, each hung from its joint, in full near,
//!   coarser a little way off and coarser still far off (`parts` sculpts
//!   them);
//! - `math`: the matrices, the reach and the springs they share.
//!
//! The model faces +x, up is +y, its right is +z (as `m4::place` turns
//! it).

mod gait;
mod math;
mod model;
mod parts;
mod pose;

pub use gait::Anim;
pub use math::Spring;
pub use pose::{Frames, Pose};

use std::f32::consts::FRAC_PI_2;

use render::geo::{self, hash, mix, rgb, unit, V3};
use render::{m4, Item, Light, Material, Mesh, Pass, Renderer, Spark, M4};

use crate::fx::Draw;
use math::{chain, point, rz, tr};
use model::Model;
use pose::HIP;

/// Beyond these distances (m) a wizard is drawn coarser, then coarser
/// still (at 16 m one stands some 90 pixels tall on a 1080p screen).
const LOD: [f32; 2] = [6.0, 16.0];

/// The wizards' colours (an id picks one).
const HUES: [V3; 8] = [
    rgb(70, 110, 230),
    rgb(200, 60, 70),
    // Ivory, not green: a green robe hides in the grass and pines.
    rgb(226, 218, 200),
    rgb(150, 80, 200),
    rgb(220, 140, 40),
    rgb(40, 170, 190),
    rgb(220, 90, 160),
    rgb(120, 120, 130),
];

/// A wizard's colour from its id.
pub fn hue(id: u16) -> V3 {
    HUES[id as usize % HUES.len()]
}

pub struct Rig {
    m: Model,
}

impl Rig {
    pub fn new(r: &mut Renderer) -> Rig {
        Rig { m: Model::new(r) }
    }

    pub fn free(self, r: &mut Renderer) {
        for m in self.m.all() {
            r.free(m);
        }
    }

    /// A wizard at `at` (its feet), facing `yaw` (radians), moving as `a`
    /// says and doing what `p` says, `far` metres from the eye (drawn the
    /// coarser the further); its frames (where its wand's tip is).
    #[allow(clippy::too_many_arguments)]
    pub fn wizard(
        &self,
        d: &mut Draw,
        id: u16,
        at: V3,
        yaw: f32,
        a: &Anim,
        p: &Pose,
        far: f32,
    ) -> Frames {
        let f = pose::wizard(at, yaw, a, p, id);
        self.draw(d, id, &f, p, (far, a.seat.x));
        f
    }

    /// A wizard knocked out `t` seconds ago at `at`, `far` metres from the
    /// eye: its knees give, it topples back (turning a little, a different
    /// way each time) and sinks away.
    pub fn fallen(&self, d: &mut Draw, id: u16, (at, far): (V3, f32), yaw: f32, t: f32) {
        if t > 1.7 {
            return;
        }
        let p = Pose {
            spell: None,
            flash: 0.0,
            arm: 0.0,
            aim: 0.0,
            tip: (rgb(255, 214, 128), 0.0),
            t,
        };
        let twist = (unit(hash(at[0] as i32, at[2] as i32, id as u32)) - 0.5) * 1.6;
        let f = pose::fallen(at, yaw, &p, id, t, twist);
        self.draw(d, id, &f, &p, (far, 0.0));
    }

    /// Its parts at their frames, the coarser the further they are
    /// (`far` metres off); its broom under it as far as it sits on it
    /// (`seat`).
    fn draw(&self, d: &mut Draw, id: u16, f: &Frames, p: &Pose, (far, seat): (f32, f32)) {
        let m = &self.m.lod[LOD.iter().filter(|&&d| far > d).count()];
        let c = hue(id);
        let robe = mix(c, [1.0; 3], 0.7 * p.flash);
        let lit = 0.6 * p.flash;
        let dark = geo::scale(c, 0.6);
        // Cloth takes the wizard's colour; gold shines; skin and
        // hair wrap the light.
        let mut put = |mesh: Mesh, at: M4, tint: Option<V3>, (mat, rough): (Material, f32)| {
            // Its shadow is drawn coarse (the engine's `far`, for what
            // moves).
            let it = Item::new(mesh, at)
                .far(self.m.coarse(mesh), f32::MAX)
                .rough(rough)
                .material(mat);
            d.items.push(match tint {
                Some(t) => it.tint(t, 1.0).glow(lit),
                None => it,
            });
        };
        let cloth = (Material::Cloth, 0.85);
        let gold = (Material::Plain, 0.35);
        let skin = (Material::Skin, 0.6);
        let leather = (Material::Plain, 0.62);
        for k in 0..4 {
            put(m.panels[k], f.panels[k], Some(robe), cloth);
            put(m.hems[k], f.panels[k], None, gold);
        }
        put(m.torso, f.chest, Some(robe), cloth);
        put(m.belt, f.chest, None, leather);
        put(m.mantle, f.chest, Some(dark), cloth);
        put(m.collar, f.chest, Some(dark), cloth);
        put(m.trim, f.chest, None, gold);
        // Bearded or not, whatever its colour.
        put(m.head[(id as usize / HUES.len()) % 2], f.head, None, skin);
        put(m.hat, f.head, Some(dark), cloth);
        put(m.band, f.head, None, gold);
        put(m.hat_tip, f.hat, Some(dark), cloth);
        for side in 0..2 {
            put(m.thigh, f.thigh[side], None, cloth);
            put(m.shin, f.shin[side], None, cloth);
            put(m.boot, f.boot[side], None, leather);
            put(m.upper, f.upper[side], Some(robe), cloth);
            put(m.fore, f.fore[side], Some(robe), cloth);
            put(m.cuff, f.fore[side], None, gold);
        }
        put(m.hand, f.fore[0], None, skin);
        put(m.wand, f.fore[1], None, leather);
        let tip = f.tip();
        let (col, flare) = p.tip;
        let k = 0.05 + 0.07 * flare;
        d.items.push(
            Item::new(self.m.orb, m4::place(tip, 0.0, [k; 3]))
                .tint(col, 1.0)
                .glow(1.0)
                .pass(Pass::Glow),
        );
        if flare > 0.05 {
            d.lights.push(Light {
                p: tip,
                r: 3.0 + 5.0 * flare,
                c: geo::scale(col, 2.5 * flare),
            });
        }
        if seat > 0.05 {
            self.broom(d, f.root, c, (p.t, seat), id);
        }
    }

    /// The broomstick under a wizard `seat` of the way sat on it (rising
    /// into place as it gets on, dropping away as it gets off), sparks
    /// trailing from its bristles.
    fn broom(&self, d: &mut Draw, root: M4, c: V3, (t, seat): (f32, f32), id: u16) {
        let k = seat.clamp(0.0, 1.0);
        let below = 0.7 * (1.0 - k);
        let m = chain(&[
            root,
            tr([0.08, HIP - 0.1 - below, 0.0]),
            rz(-FRAC_PI_2 + 0.08),
        ]);
        d.items.push(Item::new(self.m.broom, m).rough(0.85));
        let tail = point(&m, [0.0, -1.42, 0.0]);
        let back = geo::norm(geo::sub(tail, point(&m, [0.0, 0.0, 0.0])));
        let glow = mix(c, [1.0; 3], 0.5);
        for n in 0..14 {
            let u = ((t * 1.7 + n as f32 / 14.0) % 1.0).abs();
            let j = |q| (unit(hash(id as i32, n, q)) - 0.5) * 0.5 * u;
            d.sparks.push(Spark {
                p: [
                    tail[0] + back[0] * u * 2.2 + j(1),
                    tail[1] + back[1] * u * 2.2 + j(2) + u * 0.4,
                    tail[2] + back[2] * u * 2.2 + j(3),
                ],
                size: 0.12 * (1.0 - u) + 0.03,
                c: [glow[0], glow[1], glow[2], (1.0 - u) * k],
                ..Default::default()
            });
        }
        d.lights.push(Light {
            p: tail,
            r: 4.0,
            c: geo::scale(glow, 0.8 * k),
        });
    }
}
