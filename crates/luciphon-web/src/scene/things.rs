//! What stands on the island, in low poly: trees, rocks, crystals, moss,
//! brambles, pillars, the Luciphon itself, and every piece people build;
//! the lights that some of them give; and the moving things' own meshes
//! (a hooded Lumen, a mote, glim on the ground, the hand you see).

use luciphon::tiles::obj;

use super::shapes::{hash, mix, rgb, scale, unit, Geo, V3};

pub const MARBLE: V3 = rgb(226, 220, 206);
pub const GOLD: V3 = rgb(255, 210, 122);
pub const EMBER: V3 = rgb(255, 122, 61);
pub const TEAL: V3 = rgb(95, 211, 200);
const BARK: V3 = rgb(82, 62, 48);
const WOOD: V3 = rgb(126, 96, 70);
const BIRCH: V3 = rgb(222, 218, 204);
const LEAF: V3 = rgb(150, 178, 104);
const OAKLEAF: V3 = rgb(70, 104, 64);
const STONE: V3 = rgb(150, 150, 160);
const CRYSTAL: V3 = rgb(110, 190, 230);
const BRAMBLE: V3 = rgb(110, 50, 80);
const SOIL: V3 = rgb(52, 38, 32);
const WHEAT: V3 = rgb(230, 180, 90);

/// A light something gives: where, how far it reaches, its colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub p: V3,
    pub r: f32,
    pub c: V3,
}

/// The light a tile's object gives, if any (the Luciphon's from its
/// centre tile only).
pub fn light_of(o: u8, x: i32, y: i32) -> Option<Light> {
    let at = |h: f32| [x as f32 + 0.5, h, y as f32 + 0.5];
    Some(match o {
        obj::LUCIPHON if (x, y) == (0, 0) => Light {
            p: at(2.2),
            r: 24.0,
            c: scale(GOLD, 1.15),
        },
        obj::LANTERN => Light {
            p: at(1.6),
            r: 7.0,
            c: scale(GOLD, 1.1),
        },
        obj::HEARTH => Light {
            p: at(0.7),
            r: 8.0,
            c: scale(EMBER, 1.3),
        },
        obj::CRYSTAL => Light {
            p: at(0.7),
            r: 3.5,
            c: scale(CRYSTAL, 0.55),
        },
        obj::GLOWMOSS => Light {
            p: at(0.25),
            r: 2.5,
            c: scale(TEAL, 0.3),
        },
        _ => return None,
    })
}

/// The object on tile (x, y) into `g`; `hue` is its owner's colour, for
/// pieces people built.
pub fn object(g: &mut Geo, o: u8, x: i32, y: i32, hue: V3) {
    let h = |k: u32| unit(hash(x, y, k));
    let natural = matches!(
        o & !obj::BARE,
        obj::BIRCH | obj::OAK | obj::ROCK | obj::CRYSTAL | obj::GLOWMOSS | obj::BRAMBLE
    );
    let (jx, jz) = if natural {
        ((h(1) - 0.5) * 0.24, (h(2) - 0.5) * 0.24)
    } else {
        (0.0, 0.0)
    };
    let c = [x as f32 + 0.5 + jx, 0.0, y as f32 + 0.5 + jz];
    let at = |dy: f32| [c[0], dy, c[2]];
    let rot = h(3) * std::f32::consts::TAU;
    let s = 0.85 + 0.3 * h(4);
    let seed = hash(x, y, 9);
    let bare = o & obj::BARE != 0;
    match o & !obj::BARE {
        obj::BIRCH if bare => g.column(c, 5, (0.1, 0.09), 0.35, rot, BIRCH, 0.0, true),
        obj::BIRCH => {
            g.column(c, 5, (0.09, 0.06), 2.3 * s, rot, BIRCH, 0.0, false);
            g.blob(
                at(1.95 * s),
                [0.55 * s, 0.5 * s, 0.55 * s],
                seed,
                LEAF,
                0.05,
            );
            g.blob(
                at(2.55 * s),
                [0.38 * s, 0.42 * s, 0.38 * s],
                seed + 1,
                mix(LEAF, GOLD, 0.25),
                0.08,
            );
        }
        obj::OAK if bare => g.column(c, 6, (0.2, 0.17), 0.4, rot, BARK, 0.0, true),
        obj::OAK => {
            // High enough that no eye on the ground is ever in it.
            g.column(c, 6, (0.18, 0.12), 1.8 * s, rot, BARK, 0.0, false);
            g.blob(
                at(2.15 * s),
                [0.95 * s, 0.6 * s, 0.95 * s],
                seed,
                OAKLEAF,
                0.0,
            );
            let (dx, dz) = (rot.cos() * 0.35, rot.sin() * 0.35);
            g.blob(
                [c[0] + dx, 2.6 * s, c[2] + dz],
                [0.55 * s, 0.45 * s, 0.55 * s],
                seed + 1,
                mix(OAKLEAF, LEAF, 0.3),
                0.0,
            );
        }
        obj::ROCK if bare => {
            for k in 0..3 {
                let a = rot + k as f32 * 2.1;
                g.blob(
                    [c[0] + a.cos() * 0.2, 0.06, c[2] + a.sin() * 0.2],
                    [0.11, 0.08, 0.1],
                    seed + k,
                    STONE,
                    0.0,
                );
            }
        }
        obj::ROCK => {
            g.blob(
                at(0.32 * s),
                [0.48 * s, 0.4 * s, 0.44 * s],
                seed,
                STONE,
                0.0,
            );
            g.blob(
                [c[0] + rot.cos() * 0.25, 0.18, c[2] + rot.sin() * 0.25],
                [0.22, 0.18, 0.2],
                seed + 1,
                mix(STONE, BARK, 0.2),
                0.0,
            );
        }
        obj::CRYSTAL => {
            let (n, tall, glow) = if bare {
                (2, 0.25, 0.2)
            } else {
                (3, 1.15, 0.75)
            };
            for k in 0..n {
                let a = rot + k as f32 * 2.3;
                let lean = 0.12 + 0.18 * k as f32;
                let base = [c[0] + a.cos() * 0.12, 0.0, c[2] + a.sin() * 0.12];
                let tip = [
                    base[0] + a.cos() * lean,
                    tall * s * (1.0 - 0.25 * k as f32),
                    base[2] + a.sin() * lean,
                ];
                g.spike(base, tip, 0.12 - 0.02 * k as f32, 5, CRYSTAL, glow);
            }
        }
        obj::GLOWMOSS if bare => {}
        obj::GLOWMOSS => {
            for k in 0..3 {
                let a = rot + k as f32 * 2.0;
                g.blob(
                    [c[0] + a.cos() * 0.22, 0.03, c[2] + a.sin() * 0.22],
                    [0.24, 0.07, 0.22],
                    seed + k,
                    TEAL,
                    0.55,
                );
            }
        }
        obj::BRAMBLE | obj::THORNS => {
            let col = if o == obj::THORNS {
                mix(rgb(120, 100, 80), hue, 0.25)
            } else {
                BRAMBLE
            };
            g.blob(at(0.12), [0.32, 0.14, 0.32], seed, scale(col, 0.7), 0.0);
            for k in 0..7 {
                let a = rot + k as f32 * 0.9;
                let r = 0.1 + 0.2 * unit(hash(x, y, 20 + k));
                let base = [c[0] + a.cos() * r, 0.05, c[2] + a.sin() * r];
                let tip = [
                    base[0] + a.cos() * 0.3,
                    0.45 + 0.25 * h(30 + k),
                    base[2] + a.sin() * 0.3,
                ];
                g.spike(base, tip, 0.05, 4, col, 0.0);
            }
        }
        obj::PILLAR => {
            g.block(
                at(0.0),
                [0.75, 0.2, 0.75],
                0.0,
                MARBLE,
                scale(MARBLE, 0.8),
                0.0,
            );
            g.column(at(0.2), 8, (0.27, 0.24), 2.7, 0.2, MARBLE, 0.02, false);
            g.block(
                at(2.9),
                [0.7, 0.18, 0.7],
                0.0,
                MARBLE,
                scale(MARBLE, 0.85),
                0.0,
            );
        }
        obj::LUCIPHON if (x, y) == (0, 0) => luciphon(g, c),
        obj::LUCIPHON => {}
        obj::HEARTH => {
            for k in 0..9 {
                let a = k as f32 / 9.0 * std::f32::consts::TAU;
                g.blob(
                    [c[0] + a.cos() * 0.4, 0.1, c[2] + a.sin() * 0.4],
                    [0.13, 0.11, 0.13],
                    seed + k,
                    mix(STONE, hue, 0.2),
                    0.0,
                );
            }
            for k in 0..4 {
                let a = rot + k as f32 * 1.6;
                let base = [c[0] + a.cos() * 0.12, 0.0, c[2] + a.sin() * 0.12];
                g.spike(
                    base,
                    [c[0], 0.75 - 0.1 * k as f32, c[2]],
                    0.12,
                    5,
                    mix(EMBER, GOLD, 0.3 * k as f32),
                    1.0,
                );
            }
        }
        obj::WALL => {
            g.block(at(0.0), [1.0, 1.7, 1.0], 0.0, scale(WOOD, 0.8), WOOD, 0.0);
            g.block(at(1.35), [1.02, 0.12, 1.02], 0.0, hue, hue, 0.35);
        }
        obj::DOOR => {
            for dx in [-0.43, 0.43] {
                g.block(
                    [c[0] + dx, 0.0, c[2]],
                    [0.14, 2.0, 0.9],
                    0.0,
                    BARK,
                    BARK,
                    0.0,
                );
            }
            g.block(at(1.85), [1.0, 0.2, 0.9], 0.0, BARK, BARK, 0.0);
            g.block(
                at(0.0),
                [0.72, 1.82, 0.14],
                0.0,
                mix(WOOD, hue, 0.35),
                mix(WOOD, hue, 0.35),
                0.3,
            );
        }
        obj::LANTERN => {
            g.column(c, 4, (0.06, 0.05), 1.45, 0.78, SOIL, 0.0, false);
            g.block(at(1.42), [0.28, 0.3, 0.28], 0.0, GOLD, GOLD, 1.0);
            g.column(
                at(1.72),
                4,
                (0.24, 0.0),
                0.2,
                0.78,
                mix(SOIL, hue, 0.4),
                0.0,
                false,
            );
        }
        obj::PLANTER | obj::SPROUT | obj::RIPE | obj::WILTED => {
            g.block(
                at(0.0),
                [0.92, 0.32, 0.92],
                0.0,
                SOIL,
                mix(WOOD, hue, 0.15),
                0.0,
            );
            let (n, tall, col, glow) = match o {
                obj::SPROUT => (6, 0.28, rgb(110, 170, 80), 0.0),
                obj::RIPE => (9, 0.6, WHEAT, 0.2),
                obj::WILTED => (7, 0.35, rgb(110, 90, 60), 0.0),
                _ => (0, 0.0, SOIL, 0.0),
            };
            for k in 0..n {
                let a = rot + k as f32 * 2.4;
                let r = 0.12 + 0.25 * unit(hash(x, y, 40 + k));
                let base = [c[0] + a.cos() * r, 0.32, c[2] + a.sin() * r];
                let droop = if o == obj::WILTED { 0.2 } else { 0.04 };
                let tip = [
                    base[0] + a.cos() * droop,
                    0.32 + tall,
                    base[2] + a.sin() * droop,
                ];
                g.spike(base, tip, 0.035, 3, col, glow);
            }
        }
        _ => {}
    }
}

/// The Luciphon: a marble shrine on the nine centre tiles, its golden
/// bell hanging lit inside.
fn luciphon(g: &mut Geo, c: V3) {
    g.block(c, [2.9, 0.35, 2.9], 0.0, MARBLE, scale(MARBLE, 0.8), 0.0);
    g.block(
        [c[0], 0.35, c[2]],
        [2.4, 0.15, 2.4],
        0.0,
        MARBLE,
        scale(MARBLE, 0.85),
        0.0,
    );
    for (dx, dz) in [(-1.05, -1.05), (1.05, -1.05), (1.05, 1.05), (-1.05, 1.05)] {
        g.column(
            [c[0] + dx, 0.5, c[2] + dz],
            8,
            (0.2, 0.17),
            3.0,
            0.0,
            MARBLE,
            0.02,
            false,
        );
    }
    g.block(
        [c[0], 3.5, c[2]],
        [2.9, 0.3, 2.9],
        0.0,
        MARBLE,
        scale(MARBLE, 0.85),
        0.0,
    );
    g.column(
        [c[0], 3.8, c[2]],
        4,
        (1.6, 0.0),
        0.9,
        std::f32::consts::FRAC_PI_4,
        mix(MARBLE, GOLD, 0.2),
        0.05,
        false,
    );
    // The bell, open below, and its glowing heart.
    g.column(
        [c[0], 1.4, c[2]],
        12,
        (0.78, 0.4),
        1.5,
        0.0,
        GOLD,
        0.85,
        true,
    );
    g.column(
        [c[0], 1.25, c[2]],
        12,
        (0.85, 0.78),
        0.15,
        0.0,
        scale(GOLD, 1.1),
        0.9,
        false,
    );
    g.column(
        [c[0], 2.9, c[2]],
        6,
        (0.06, 0.06),
        0.6,
        0.0,
        scale(GOLD, 0.7),
        0.3,
        false,
    );
    g.blob(
        [c[0], 1.2, c[2]],
        [0.28, 0.28, 0.28],
        3,
        rgb(255, 244, 210),
        1.0,
    );
}

/// A Lumen standing at the origin facing +x: the robe and hood (white,
/// to take its soul's hue) and the light in it (to take its flame's).
pub fn lumen() -> (Geo, Geo) {
    let mut robe = Geo::default();
    let o = [0.0, 0.0, 0.0];
    let white = [0.92, 0.92, 0.92];
    robe.column(
        o,
        9,
        (0.36, 0.34),
        0.08,
        0.0,
        scale(white, 0.55),
        0.0,
        false,
    );
    robe.column(
        [0.0, 0.08, 0.0],
        9,
        (0.34, 0.17),
        0.88,
        0.0,
        white,
        0.0,
        false,
    );
    robe.blob([0.0, 0.98, 0.0], [0.23, 0.15, 0.23], 1, white, 0.0);
    robe.blob([-0.02, 1.12, 0.0], [0.2, 0.21, 0.2], 2, white, 0.0);
    robe.spike([-0.06, 1.2, 0.0], [-0.22, 1.5, 0.0], 0.1, 5, white, 0.0);
    // The dark of the hood's opening.
    robe.blob(
        [0.12, 1.1, 0.0],
        [0.07, 0.11, 0.12],
        3,
        [0.06, 0.06, 0.08],
        0.0,
    );
    let mut glow = Geo::default();
    for z in [-0.05, 0.05] {
        glow.blob(
            [0.19, 1.12, z],
            [0.03, 0.035, 0.03],
            4,
            [1.0, 1.0, 1.0],
            1.0,
        );
    }
    glow.blob(
        [0.2, 0.62, 0.0],
        [0.06, 0.06, 0.06],
        5,
        [1.0, 1.0, 1.0],
        1.0,
    );
    (robe, glow)
}

/// A mote in flight, glim lying on the ground, and the hand you see: all
/// white, to be tinted.
pub fn mote() -> Geo {
    let mut g = Geo::default();
    g.blob([0.0; 3], [0.12, 0.12, 0.12], 6, [1.0; 3], 1.0);
    g
}

pub fn glim() -> Geo {
    let mut g = Geo::default();
    g.spike([0.0, 0.0, 0.0], [0.0, 0.16, 0.0], 0.09, 4, [1.0; 3], 0.9);
    g.spike([0.0, 0.0, 0.0], [0.0, -0.12, 0.0], 0.09, 4, [1.0; 3], 0.9);
    g
}

pub fn hand() -> Geo {
    let mut g = Geo::default();
    // A wisp of your light: a bright core and a flame above it.
    g.blob([0.0; 3], [0.026, 0.026, 0.026], 7, [1.0; 3], 1.0);
    g.spike([0.0, 0.01, 0.0], [0.0, 0.075, 0.0], 0.018, 5, [1.0; 3], 1.0);
    g
}

/// A unit box on the ground (a build ghost), and a flat ring of radius 1
/// (a node's ring, a fan of a strike's reach).
pub fn cube() -> Geo {
    let mut g = Geo::default();
    g.block([0.0; 3], [1.0, 1.0, 1.0], 0.0, [1.0; 3], [1.0; 3], 1.0);
    g
}

pub fn ring() -> Geo {
    let mut g = Geo::default();
    let n = 40;
    for k in 0..n {
        let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
        let p = |a: f32, r: f32| [a.cos() * r, 0.0, a.sin() * r];
        g.quad(
            p(a0, 0.92),
            p(a1, 0.92),
            p(a1, 1.0),
            p(a0, 1.0),
            [1.0; 3],
            1.0,
        );
    }
    g
}

/// A rod from the origin to +x (radius 1 across): a beam, or a wand's
/// shaft, scaled to it.
pub fn rod() -> Geo {
    let mut g = Geo::default();
    let n = 6;
    for k in 0..n {
        let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
        let p = |a: f32, x: f32| [x, a.cos(), a.sin()];
        g.tri_out(
            p(a0, 0.0),
            p(a1, 0.0),
            p(a1, 1.0),
            [0.5, 0.0, 0.0],
            [1.0; 3],
            1.0,
        );
        g.tri_out(
            p(a0, 0.0),
            p(a1, 1.0),
            p(a0, 1.0),
            [0.5, 0.0, 0.0],
            [1.0; 3],
            1.0,
        );
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use luciphon::tiles::obj;

    #[test]
    fn every_object_has_a_shape_and_some_give_light() {
        for o in 1..=17u8 {
            let mut g = Geo::default();
            object(&mut g, o, 3, -4, [1.0, 0.0, 0.0]);
            let empty = o == obj::LUCIPHON;
            assert_eq!(g.is_empty(), empty, "object {o}");
            assert!(g.v.iter().all(|v| v.is_finite()));
        }
        let mut g = Geo::default();
        object(&mut g, obj::LUCIPHON, 0, 0, [1.0; 3]);
        assert!(g.len() > 100);
        assert!(light_of(obj::LUCIPHON, 0, 0).is_some());
        assert!(light_of(obj::LUCIPHON, 1, 0).is_none());
        assert!(light_of(obj::LANTERN, 5, 5).is_some_and(|l| l.p[1] > 1.0));
        let (robe, glow) = lumen();
        assert!(robe.len() > 50 && glow.len() > 10);
    }

    #[test]
    fn no_eye_on_the_ground_is_ever_inside_a_thing() {
        // A body stops 0.35 short of a solid tile, so its eye (1.05 up) is
        // at least 0.85 from the tile's centre: nothing may be there.
        for o in 1..=17u8 {
            for k in 0..40 {
                let mut g = Geo::default();
                object(&mut g, o, k * 7 - 100, k * 3 + 5, [1.0; 3]);
                for v in g.v.chunks(super::super::shapes::STRIDE) {
                    let (dx, dz) = (
                        v[0] - (k * 7 - 100) as f32 - 0.5,
                        v[2] - (k * 3 + 5) as f32 - 0.5,
                    );
                    let near = (dx * dx + dz * dz).sqrt() < 0.84;
                    assert!(near || v[1] < 0.85 || v[1] > 1.3, "object {o}: {v:?}");
                }
            }
        }
    }
}
