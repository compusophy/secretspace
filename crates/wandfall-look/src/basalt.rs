//! The causeway's basalt: six-sided columns of stacked drums, a groove at
//! each joint, the colour varying drum to drum; capped by a dished top,
//! moss on some. A column's sides come in a few heights (the nearest
//! stretched to fit, so the joints stay about a metre apart); its cap is
//! never stretched.

use std::f32::consts::TAU;

use render::geo::{hash, mix, rgb, unit, Geo, V3};
use render::{m4, Item, Mesh, Renderer};

use wandfall::map::Prop;

/// The heights the sides are made in (m).
const HEIGHTS: [f32; 7] = [1.5, 2.5, 3.5, 5.0, 7.0, 9.0, 12.5];
/// A column's look is this much of its blocking radius (its corners;
/// the grooves between columns show).
const WIDE: f32 = 0.955;
/// Nearer than this, the drums; farther, a plain prism.
const NEAR: f32 = 40.0;

const DARK: V3 = rgb(50, 50, 56);
const BASALT: V3 = rgb(74, 72, 78);
const TOP: V3 = rgb(92, 92, 96);
const MOSS: V3 = rgb(70, 96, 52);

/// A ring of the six corners, `r` out at height `y`.
fn ring(r: f32, y: f32) -> [V3; 6] {
    std::array::from_fn(|k| {
        let a = TAU / 12.0 + k as f32 * TAU / 6.0;
        [a.cos() * r, y, a.sin() * r]
    })
}

/// The sides between two rings, facing out.
fn band(g: &mut Geo, lo: [V3; 6], hi: [V3; 6], col: V3) {
    for k in 0..6 {
        let j = (k + 1) % 6;
        g.quad(lo[k], hi[k], hi[j], lo[j], col, 0.0);
    }
}

/// Sides `h` tall, unit wide (to the corners): drums from 1.2 m to 2.6 m,
/// each a little narrower or wider, chamfered into a fine groove at each
/// joint.
fn sides(g: &mut Geo, h: f32, seed: u32) {
    let (mut y, mut k) = (0.0, 0);
    while y < h - 0.05 {
        let u = |i| unit(hash(k, i, seed));
        let len = (1.2 + 1.4 * u(1)).min(h - y);
        let len = if h - (y + len) < 0.6 { h - y } else { len };
        let w = 0.985 + 0.015 * u(2);
        let c = mix(DARK, BASALT, 0.6 + 0.4 * u(3));
        let lip = 0.03f32.min(len / 4.0);
        let rings = [
            ring(w * 0.97, y),
            ring(w, y + lip),
            ring(w, y + len - lip),
            ring(w * 0.97, y + len),
        ];
        for r in rings.windows(2) {
            band(g, r[0], r[1], c);
        }
        y += len;
        k += 1;
    }
}

/// The cap, at height 0: a chamfer up from the sides' last groove to a
/// rim, dished a little toward its middle; moss over part of it if
/// `moss`.
fn cap(g: &mut Geo, moss: bool) {
    let rim = ring(0.93, 0.03);
    band(g, ring(0.97, 0.0), rim, mix(DARK, BASALT, 0.8));
    let inner = ring(0.8, 0.04);
    band(g, rim, inner, TOP);
    let mid = [0.0, 0.015, 0.0];
    for k in 0..6 {
        let j = (k + 1) % 6;
        let c = if moss && k < 2 { MOSS } else { TOP };
        g.tri(mid, inner[j], inner[k], c, 0.0);
    }
}

pub(crate) struct Basalt {
    /// Each height's drums, and a plain prism as tall (far off, and for
    /// the shadow).
    sides: Vec<(Mesh, Mesh, f32)>,
    caps: [Mesh; 2],
}

impl Basalt {
    pub(crate) fn new(r: &mut Renderer) -> Basalt {
        let sides = HEIGHTS
            .iter()
            .enumerate()
            .map(|(k, &h)| {
                let mut g = Geo::default();
                sides(&mut g, h, 70 + k as u32);
                let mut plain = Geo::default();
                band(&mut plain, ring(1.0, 0.0), ring(1.0, h), BASALT);
                (r.mesh(&g), r.mesh(&plain), h)
            })
            .collect();
        let caps = [false, true].map(|moss| {
            let mut g = Geo::default();
            cap(&mut g, moss);
            r.mesh(&g)
        });
        Basalt { sides, caps }
    }

    /// The column `p` (the `k`th thing on the island), as statics.
    pub(crate) fn put(&self, statics: &mut Vec<Item>, p: &Prop, k: usize) {
        let h = hash(k as i32, 9, 13);
        // Turned a sixth at a time: the same lattice, a different face.
        let yaw = p.yaw + (h % 6) as f32 * TAU / 6.0;
        let wide = p.r * WIDE;
        let &(near, far, tall) = self
            .sides
            .iter()
            .min_by(|a, b| (a.2 / p.h).ln().abs().total_cmp(&(b.2 / p.h).ln().abs()))
            .unwrap();
        let at = [p.x, p.y, p.z];
        statics.push(
            Item::new(near, m4::place(at, yaw, [wide, p.h / tall, wide]))
                .far(far, NEAR)
                .rough(0.8)
                .detail(0.3),
        );
        let top = [p.x, p.y + p.h, p.z];
        let moss = (h >> 4).is_multiple_of(3);
        statics.push(
            Item::new(
                self.caps[moss as usize],
                m4::place(top, yaw, [wide, 1.0, wide]),
            )
            .rough(0.85)
            .detail(0.35),
        );
    }

    pub(crate) fn meshes(&self) -> impl Iterator<Item = Mesh> + '_ {
        self.sides.iter().flat_map(|s| [s.0, s.1]).chain(self.caps)
    }
}
