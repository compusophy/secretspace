//! The ground at the places: how it is painted (paving about the Spire,
//! the circle's green, the rift's scorched bowl and lava, the grove's
//! violet, the causeway's black gravel) and where grass grows.

use std::f32::consts::TAU;

use render::geo::{hash, mix, rgb, unit, V3};

use wandfall::map::{smooth as ease, Map};
use wandfall::places::Place;

use super::{EMBER, GOLDEN};

/// How the ground is painted at (x, z): a colour and how much of it
/// (above 1, it glows).
pub(super) fn paint(map: &Map, x: f32, z: f32) -> (V3, f32) {
    for p in &map.pois {
        let d = p.dist(x, z);
        if d > p.r * 1.3 {
            continue;
        }
        let a = (z - p.z).atan2(x - p.x);
        return match p.place {
            Place::Spire => {
                // Paving in rings, a gold inlay about the tower.
                if (d - 10.0).abs() < 0.6 {
                    return (GOLDEN, 1.5);
                }
                let ring = (d / 2.4) as i32;
                let n = 10 + ring * 4;
                let sector = ((a / TAU + 0.5) * n as f32) as i32;
                let c = mix(
                    rgb(150, 144, 136),
                    rgb(186, 178, 166),
                    unit(hash(ring, sector, 3)),
                );
                (c, 0.95 * (1.0 - ease((d - p.r + 1.0) / 3.0)))
            }
            Place::Circle => (rgb(96, 112, 84), 0.4 * (1.0 - ease((d - p.r * 0.6) / 6.0))),
            Place::Rift => {
                // Lava where the gate stands, and cracks running out.
                let veins = (0..5).any(|k| {
                    let va = unit(hash(k, 7, map.seed as u32)) * TAU;
                    let (s, c) = va.sin_cos();
                    let (dx, dz) = (x - p.x, z - p.z);
                    let along = dx * c + dz * s;
                    along > 0.0 && along < p.r * 0.75 && (dz * c - dx * s).abs() < 0.7
                });
                if d < 3.8 || veins {
                    (EMBER, 1.0 + 1.6 * (1.0 - d / p.r).max(0.2))
                } else {
                    let c = mix(
                        rgb(22, 14, 14),
                        rgb(54, 30, 24),
                        unit(hash(x as i32, z as i32, 9)),
                    );
                    (c, 1.0 - ease((d - p.r * 0.8) / (p.r * 0.45)))
                }
            }
            Place::Grove => (rgb(120, 96, 170), 0.5 * (1.0 - ease((d - p.r * 0.7) / 5.0))),
            // Black gravel and broken basalt.
            Place::Causeway => {
                let c = mix(
                    rgb(52, 52, 56),
                    rgb(88, 86, 82),
                    unit(hash((x * 1.7) as i32, (z * 1.7) as i32, 4)),
                );
                (c, 0.9 * (1.0 - ease((d - p.r * 0.7) / (p.r * 0.45))))
            }
        };
    }
    ([1.0; 3], 0.0)
}

/// How much grass grows at (x, z).
pub(super) fn lush(map: &Map, x: f32, z: f32) -> f32 {
    let mut k: f32 = 1.0;
    for p in &map.pois {
        let d = p.dist(x, z);
        let (keep, edge) = match p.place {
            Place::Spire => (0.0, p.r + 1.0),
            Place::Rift => (0.0, p.r * 1.15),
            Place::Circle => (0.55, p.r * 0.8),
            Place::Grove => (0.2, p.r * 0.9),
            Place::Causeway => (0.3, p.r * 0.85),
        };
        let f = ease((d - edge) / 3.0);
        k = k.min(keep + (1.0 - keep) * f);
    }
    k
}
