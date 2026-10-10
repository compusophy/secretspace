//! The island map in the top right corner: the island seen from above,
//! drawn once at the size it is shown (again only when that changes),
//! with the storm's circles and you over it.

use pixels::{Canvas, Rgba};
use wandfall::laws::{MAP_HALF, SEA};
use wandfall::map::Map;
use wandfall::places::Place;
use wandfall::proto::Frame;

use super::{GOLD, INK, STORM};

/// Pixels a side of the island map, at the ui scale.
const MINI: i32 = 96;

/// Where the map goes: its left, its top, its side.
pub fn spot(w: i32, h: i32, ui: i32) -> (i32, i32, i32) {
    let s = (MINI * ui).min(w / 4).min(h / 4).max(8);
    (w - s - 10 * ui, 24 * ui, s)
}

/// The island seen from above, kept at the size last asked for.
#[derive(Default)]
pub struct Mini(Option<Canvas>);

impl Mini {
    /// The island at `s` pixels a side, drawn now if it is not already.
    pub fn at(&mut self, map: &Map, s: i32) -> &Canvas {
        if self.0.as_ref().is_none_or(|c| c.w != s) {
            self.0 = Some(island(map, s));
        }
        self.0.as_ref().expect("drawn")
    }
}

/// The island seen from above, `s` pixels a side.
fn island(map: &Map, s: i32) -> Canvas {
    let mut c = Canvas::new(s, s);
    let at = |k: i32| (k as f32 + 0.5) / s as f32 * 2.0 * MAP_HALF - MAP_HALF;
    for j in 0..s {
        for i in 0..s {
            let h = map.height(at(i), at(j));
            let col = if h < SEA {
                Rgba::rgb(40, 80, 120)
            } else if h < SEA + 0.6 {
                Rgba::rgb(190, 172, 120)
            } else {
                let k = ((h - SEA) / 12.0).clamp(0.0, 1.0);
                Rgba::rgb(70, 120, 60).mix(Rgba::rgb(140, 150, 90), k)
            };
            c.pixel(i, j, col);
        }
    }
    let px = |v: f32| (v + MAP_HALF) / (2.0 * MAP_HALF) * s as f32;
    // Marks as big as they were on the first, 96-pixel map.
    let k = s as f32 / MINI as f32;
    // The launch runes: gold points.
    for q in &map.pads {
        c.circle(px(q[0]), px(q[2]), 1.6 * k, Rgba::rgb(255, 200, 110));
    }
    // The places: the Spire's plaza and tower, the circle, the rift, the
    // grove, the causeway.
    for p in &map.pois {
        let (x, y) = (px(p.x), px(p.z));
        match p.place {
            Place::Spire => {
                c.circle(x, y, 4.0 * k, Rgba::rgb(196, 188, 176));
                c.circle(x, y, 1.8 * k, Rgba::rgb(120, 80, 220));
            }
            Place::Circle => c.ring(x, y, 3.0 * k, 1.2 * k, Rgba::rgb(120, 225, 255)),
            Place::Rift => {
                c.circle(x, y, 3.6 * k, Rgba::rgb(40, 22, 22));
                c.circle(x, y, 1.6 * k, Rgba::rgb(255, 110, 40));
            }
            Place::Grove => c.circle(x, y, 2.6 * k, Rgba::rgb(170, 120, 255)),
            Place::Causeway => {
                c.circle(x, y, 3.2 * k, Rgba::rgb(64, 64, 72));
                c.circle(x, y, 1.2 * k, Rgba::rgb(150, 255, 214));
            }
        }
    }
    c
}

/// The map drawn at `spot`: the island, in a match the storm's circles,
/// and where you are (`me`: feet and facing).
pub(super) fn draw(
    c: &mut Canvas,
    mini: &Canvas,
    (mx, my, s): (i32, i32, i32),
    f: Option<&Frame>,
    me: Option<([f32; 3], f32)>,
    ui: i32,
) {
    c.blit(mini, mx, my, 0.0);
    let to = |x: f32, z: f32| {
        (
            mx as f32 + (x + MAP_HALF) / (2.0 * MAP_HALF) * s as f32,
            my as f32 + (z + MAP_HALF) / (2.0 * MAP_HALF) * s as f32,
        )
    };
    let k = s as f32 / (2.0 * MAP_HALF);
    if let Some(f) = f {
        // A circle bigger than the island is not drawn past the map.
        let fits = |r: f32| r < MAP_HALF * 1.05;
        if fits(f.storm.1) {
            let (sx, sy) = to(f.storm.0[0], f.storm.0[1]);
            c.ring(sx, sy, f.storm.1 * k, 1.5, STORM);
        }
        if fits(f.next.1) {
            let (nx, ny) = to(f.next.0[0], f.next.0[1]);
            c.ring(nx, ny, f.next.1 * k, 1.0, INK.fade(0.8));
        }
    }
    if let Some((p, yaw)) = me {
        let (px, py) = to(p[0], p[2]);
        c.circle(px, py, 2.5 * ui as f32, GOLD);
        c.line(
            px,
            py,
            px + yaw.cos() * 7.0 * ui as f32,
            py + yaw.sin() * 7.0 * ui as f32,
            ui as f32,
            GOLD,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_map_is_drawn_once_a_size() {
        let map = Map::new(7);
        let mut m = Mini::default();
        let a = m.at(&map, 48).data.clone();
        assert_eq!(m.at(&map, 48).data, a, "kept");
        assert_eq!(m.at(&map, 64).w, 64, "drawn again at a new size");
        // Sea at the corners, land in the middle.
        let c = m.at(&map, 64);
        let px = |x: i32, y: i32| c.data[((y * 64 + x) * 4) as usize..][..3].to_vec();
        assert_eq!(px(0, 0), vec![40, 80, 120]);
        assert_ne!(px(32, 32), vec![40, 80, 120]);
    }
}
