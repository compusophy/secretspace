//! The Spire, built from its laws: its tower (a wall of stone courses
//! as wide as it blocks, corbels under the balcony, a wizard's hat of a
//! roof), the door at the plaza and windows lit up a spiral, the stair
//! and balcony (its decks), the merlons along the balcony and the lamps
//! about the plaza.

use std::f32::consts::TAU;

use render::geo::{self, hash, mix, rgb, scale, unit, Geo, V3};
use render::sculpt::{both, mesh, noise, rbox};
use render::{Mesh, Renderer};

use wandfall::laws::{BALCONY, BALCONY_SPAN, STAIR_FROM, TOWER_HEIGHT, TOWER_RADIUS};
use wandfall::map::Map;
use wandfall::places::Deck;

use super::relics::{GRIME, LICHEN};
use super::{one, smooth, GOLDEN, VIOLET};
use crate::flora::FAR;

/// The tower's foot is this far under the plaza.
const SUNK: f32 = 0.3;
/// A course of its stones, and the joint between them (m); stones a
/// course.
const COURSE: f32 = 0.5;
const JOINT: f32 = 0.045;
const STONES: usize = 28;
/// Steps a turn of the stair, and a post every so many of them.
const STEPS: f32 = 72.0;
const POSTS: usize = 6;

pub(super) struct Spire {
    pub tower: Mesh,
    pub trim: Mesh,
    pub decks: Mesh,
    pub merlon: (Mesh, Mesh),
    pub lamp: Mesh,
}

/// The tower's wall at `y` above its foot: as wide as the tower blocks
/// (`TOWER_RADIUS`) up to the balcony, a little narrower above it.
pub(super) fn wall(y: f32) -> f32 {
    let up = ((y - BALCONY) / (TOWER_HEIGHT - BALCONY)).clamp(0.0, 1.0);
    TOWER_RADIUS - 0.25 * up
}

impl Spire {
    pub(super) fn new(r: &mut Renderer, map: &Map) -> Spire {
        let tower = smooth(r, |g| {
            // Behind the stones, the mortar; under the plaza, a flare.
            let mortar = rgb(62, 56, 60);
            let core = |y: f32| (wall(y) - 0.05, y);
            g.lathe(
                [0.0; 3],
                &[
                    (TOWER_RADIUS + 0.35, 0.0),
                    core(SUNK),
                    core(BALCONY),
                    core(TOWER_HEIGHT),
                ],
                32,
                mortar,
                0.0,
            );
            // A wizard's hat: a wide brim, its tip bent over.
            let felt = rgb(66, 44, 140);
            let h = TOWER_HEIGHT;
            let hat = [
                (5.7, h - 0.2),
                (5.6, h + 0.3),
                (3.6, h + 2.4),
                (2.2, h + 5.4),
                (1.2, h + 8.6),
                (0.55, h + 11.0),
            ];
            g.lathe([0.0; 3], &hat, 24, felt, 0.0);
            g.spike(
                [0.0, h + 10.8, 0.0],
                [2.2, h + 14.6, 0.5],
                0.62,
                10,
                felt,
                0.0,
            );
            g.lathe(
                [0.0, h, 0.0],
                &[(5.66, 0.0), (5.75, 0.25), (5.66, 0.5)],
                24,
                GOLDEN,
                0.2,
            );
            for y in [7.0, 14.5] {
                g.lathe(
                    [0.0, y, 0.0],
                    &[
                        (wall(y) + 0.02, 0.0),
                        (wall(y) + 0.12, 0.2),
                        (wall(y) + 0.02, 0.4),
                    ],
                    32,
                    GOLDEN,
                    0.15,
                );
            }
        });
        let trim = one(r, |g| {
            courses(g);
            corbels(g);
            door(g);
            // A dark frame, and candlelight in it, pointed at the top.
            let window = |g: &mut Geo, a: f32, y: f32, w: f32, h: f32| {
                let (c, s) = (a.cos(), a.sin());
                let pane = |g: &mut Geo, r: f32, w: f32, h: f32, col: V3, glow: f32| {
                    let o = [c * r, y, s * r];
                    let side = [-s * w / 2.0, 0.0, c * w / 2.0];
                    let up = [0.0, h / 2.0, 0.0];
                    let p = |i: f32, j: f32| {
                        geo::add(o, geo::add(geo::scale(side, i), geo::scale(up, j)))
                    };
                    g.quad(
                        p(-1.0, -1.0),
                        p(-1.0, 0.6),
                        p(1.0, 0.6),
                        p(1.0, -1.0),
                        col,
                        glow,
                    );
                    g.tri(p(-1.0, 0.6), p(0.0, 1.0), p(1.0, 0.6), col, glow);
                };
                // Just proud of the stones.
                let r = wall(y) + 0.06;
                pane(g, r, w + 0.3, h + 0.3, rgb(44, 34, 56), 0.0);
                pane(g, r + 0.03, w, h, rgb(255, 178, 90), 3.0);
            };
            for k in 0..13 {
                window(g, k as f32 * 1.05 + 1.0, 4.5 + k as f32 * 1.3, 0.55, 1.1);
            }
            for k in 0..4 {
                window(g, k as f32 / 4.0 * TAU + 0.4, BALCONY + 3.5, 0.6, 1.4);
            }
        });
        let decks = one(r, |g| {
            for d in &map.decks {
                deck(g, d);
            }
        });
        let merlon = (r.mesh(&merlon(1.0)), r.mesh(&merlon(FAR)));
        let lamp = smooth(r, |g| {
            let iron = rgb(46, 42, 58);
            let post = [
                (0.24, 0.0),
                (0.24, 0.5),
                (0.17, 0.58),
                (0.1, 0.66),
                (0.08, 2.5),
                (0.15, 2.6),
                (0.15, 2.7),
                (0.3, 2.86),
                (0.32, 2.9),
            ];
            g.lathe([0.0; 3], &post, 12, iron, 0.0);
            // A cage of four bars about the flame, a cap over it.
            for k in 0..4 {
                let a = k as f32 / 4.0 * TAU + 0.4;
                let foot = [a.cos() * 0.28, 2.88, a.sin() * 0.28];
                g.spike(
                    foot,
                    [a.cos() * 0.06, 3.6, a.sin() * 0.06],
                    0.025,
                    4,
                    iron,
                    0.0,
                );
            }
            g.lathe([0.0, 3.5, 0.0], &[(0.18, 0.0), (0.05, 0.16)], 12, iron, 0.0);
            g.sphere([0.0, 3.15, 0.0], [0.24; 3], (1, 4, 0.0), VIOLET, 2.5);
        });
        Spire {
            tower,
            trim,
            decks,
            merlon,
            lamp,
        }
    }

    pub(super) fn meshes(&self) -> [Mesh; 6] {
        [
            self.tower,
            self.trim,
            self.decks,
            self.merlon.0,
            self.merlon.1,
            self.lamp,
        ]
    }
}

/// The tower's stones: a course every `COURSE`, each stone its own shade
/// (darker and greener toward the ground), set half a stone round from
/// the course under it, the mortar dark between them.
fn courses(g: &mut Geo) {
    let n = ((TOWER_HEIGHT - SUNK) / COURSE) as i32;
    for k in 0..n {
        let (y0, y1) = (
            SUNK + k as f32 * COURSE + JOINT / 2.0,
            SUNK + (k + 1) as f32 * COURSE - JOINT / 2.0,
        );
        let half = if k % 2 == 0 { 0.0 } else { 0.5 };
        for j in 0..STONES {
            let u = |i: u32| unit(hash(k, j as i32, 900 + i));
            let r = wall((y0 + y1) / 2.0) + 0.005 + 0.012 * u(0);
            let gap = JOINT / 2.0 / r;
            let a0 = (j as f32 + half) / STONES as f32 * TAU + gap;
            let a1 = (j as f32 + half + 1.0) / STONES as f32 * TAU - gap;
            let low = (1.0 - (y0 - SUNK) / 2.5).clamp(0.0, 1.0);
            let c = scale(rgb(160, 152, 142), 0.84 + 0.24 * u(1));
            let c = mix(mix(c, GRIME, 0.55 * low), LICHEN, 0.25 * low * u(2));
            // Two faces a stone, so it follows the curve.
            let mid = (a0 + a1) / 2.0;
            let at = |a: f32, y: f32| [a.cos() * r, y, a.sin() * r];
            for (b0, b1) in [(a0, mid), (mid, a1)] {
                g.quad(at(b1, y0), at(b0, y0), at(b0, y1), at(b1, y1), c, 0.0);
            }
        }
    }
}

/// A solid of six faces from its corners (the foot's four, then the
/// top's in the same order round), each face turned out.
fn solid(g: &mut Geo, c: [V3; 8], col: V3) {
    let mid = c
        .iter()
        .fold([0.0; 3], |m, &p| geo::add(m, geo::scale(p, 0.125)));
    for f in [
        [0, 1, 2, 3],
        [4, 5, 6, 7],
        [0, 1, 5, 4],
        [1, 2, 6, 5],
        [2, 3, 7, 6],
        [3, 0, 4, 7],
    ] {
        g.tri_out(c[f[0]], c[f[1]], c[f[2]], mid, col, 0.0);
        g.tri_out(c[f[0]], c[f[2]], c[f[3]], mid, col, 0.0);
    }
}

/// Stone brackets under the balcony, holding it out from the wall: deep
/// at the wall, tapering out to its edge.
fn corbels(g: &mut Geo) {
    let n = 15;
    let under = BALCONY - 0.6;
    for k in 0..n {
        let a = STAIR_FROM + (0.03 + (BALCONY_SPAN - 0.06) * k as f32 / (n - 1) as f32) * TAU;
        let (s, c) = a.sin_cos();
        let at = |r: f32, y: f32, w: f32| [c * r - s * w, y, s * r + c * w];
        let (r0, r1, w) = (TOWER_RADIUS - 0.02, TOWER_RADIUS + 1.5, 0.17);
        let col = scale(rgb(150, 142, 136), 0.9 + 0.15 * unit(hash(k, 7, 61)));
        solid(
            g,
            [
                at(r0, under - 1.1, -w),
                at(r1, under - 0.25, -w),
                at(r1, under - 0.25, w),
                at(r0, under - 1.1, w),
                at(r0, under, -w),
                at(r1, under, -w),
                at(r1, under, w),
                at(r0, under, w),
            ],
            col,
        );
    }
}

/// The door at the plaza: a stone frame, a threshold, a dark wooden door
/// banded with iron.
fn door(g: &mut Geo) {
    let r = TOWER_RADIUS;
    let stone = rgb(172, 164, 156);
    let wood = rgb(52, 34, 30);
    let iron = rgb(40, 38, 46);
    g.block(
        [r + 0.12, SUNK, 0.0],
        [0.6, 0.1, 2.3],
        0.0,
        stone,
        scale(stone, 0.8),
        0.0,
    );
    g.block(
        [r - 0.02, SUNK + 0.1, 0.0],
        [0.16, 3.0, 1.6],
        0.0,
        wood,
        wood,
        0.0,
    );
    for y in [1.0, 2.3] {
        g.block(
            [r + 0.07, SUNK + y, 0.0],
            [0.04, 0.08, 1.6],
            0.0,
            iron,
            iron,
            0.0,
        );
    }
    for z in [-0.96f32, 0.96] {
        g.block(
            [r + 0.04, SUNK + 0.1, z],
            [0.34, 3.2, 0.32],
            0.0,
            stone,
            scale(stone, 0.85),
            0.0,
        );
    }
    g.block(
        [r + 0.04, SUNK + 3.3, 0.0],
        [0.34, 0.42, 2.24],
        0.0,
        stone,
        scale(stone, 0.85),
        0.0,
    );
}

/// A merlon on the balcony's edge, 1.2 tall (where feet stand on it):
/// a block of stone, its edges chipped, lichen on its top.
fn merlon(q: f32) -> Geo {
    let f = |p: V3| {
        let d = rbox(p, [0.0, 0.6, 0.0], [0.25, 0.6, 0.425], 0.05);
        let chips = 0.05 * noise([p[0] * 3.3, p[1] * 3.3, p[2] * 3.3 + 9.0]).max(0.0)
            + 0.012 * noise(scale(p, 9.0));
        both(d + chips, p[1] - 1.2, 0.02)
    };
    mesh(
        &f,
        ([-0.4, -0.1, -0.55], [0.4, 1.3, 0.55]),
        0.05 * q,
        &|p, n| {
            let c = scale(rgb(108, 102, 104), 0.85 + 0.2 * noise(scale(p, 1.7)));
            let up = ((n[1] - 0.5) / 0.35).clamp(0.0, 1.0);
            (mix(c, LICHEN, 0.55 * up), 0.0)
        },
        (0.5, 0.1),
    )
}

/// A block of a ring between radii `r`, from heading `a0` to `a1`, from
/// `lo` up to `hi`: its top, outer side, ends and underside.
fn wedge(g: &mut Geo, c: (f32, f32), r: (f32, f32), (a0, a1): (f32, f32), (lo, hi): (f32, f32)) {
    let top = rgb(116, 110, 112);
    let side = rgb(92, 86, 96);
    let p = |rr: f32, a: f32, y: f32| [c.0 + a.cos() * rr, y, c.1 + a.sin() * rr];
    g.quad(
        p(r.0, a0, hi),
        p(r.0, a1, hi),
        p(r.1, a1, hi),
        p(r.1, a0, hi),
        top,
        0.0,
    );
    g.quad(
        p(r.1, a0, lo),
        p(r.1, a0, hi),
        p(r.1, a1, hi),
        p(r.1, a1, lo),
        side,
        0.0,
    );
    g.quad(
        p(r.0, a0, lo),
        p(r.0, a0, hi),
        p(r.1, a0, hi),
        p(r.1, a0, lo),
        side,
        0.0,
    );
    g.quad(
        p(r.0, a1, lo),
        p(r.1, a1, lo),
        p(r.1, a1, hi),
        p(r.0, a1, hi),
        side,
        0.0,
    );
    g.quad(
        p(r.0, a0, lo),
        p(r.1, a0, lo),
        p(r.1, a1, lo),
        p(r.0, a1, lo),
        side,
        0.0,
    );
}

/// Somewhere to stand above the ground, built: a stair of stone steps
/// (each as tall as a step should be, so feet on its ramp are never far
/// off a tread) with a glowing rail on posts along its outer edge, seen
/// from either side; or a ring of slabs.
fn deck(g: &mut Geo, d: &Deck) {
    match *d {
        Deck::Stair {
            x,
            z,
            r,
            y0,
            rise,
            turns,
            from,
        } => {
            let n = (turns * STEPS) as usize;
            let inner = (r.0 - 0.2, r.1);
            let at = |u: f32| from + u * TAU;
            let rr = r.1 - 0.12;
            // The rail follows the ramp feet climb, `RAIL` over it.
            const RAIL: f32 = 0.95;
            let ramp = |u: f32| y0 + u * rise;
            for k in 0..n {
                let (u0, u1) = (k as f32 / STEPS, (k + 1) as f32 / STEPS);
                let y = ramp((u0 + u1) / 2.0);
                wedge(g, (x, z), inner, (at(u0), at(u1)), (y - 0.45, y));
                if k % POSTS == 0 {
                    // From its tread up to the rail.
                    let a = at(u0);
                    let post = [x + a.cos() * rr, y, z + a.sin() * rr];
                    let tall = ramp(u0) + RAIL - y;
                    g.column(post, 4, (0.05, 0.04), tall, a, rgb(46, 40, 58), 0.0, true);
                }
            }
            let pt = |u: f32, dy: f32| {
                let a = at(u);
                [x + a.cos() * rr, ramp(u) + RAIL + dy, z + a.sin() * rr]
            };
            for k in 0..n {
                let (u0, u1) = (k as f32 / STEPS, (k + 1) as f32 / STEPS);
                let (a, b, c, d) = (pt(u0, -0.04), pt(u0, 0.04), pt(u1, 0.04), pt(u1, -0.04));
                g.quad(a, b, c, d, GOLDEN, 1.4);
                g.quad(a, d, c, b, GOLDEN, 1.4);
            }
        }
        Deck::Ring {
            x,
            z,
            r,
            y,
            from,
            span,
        } => {
            let n = (span * 48.0) as usize;
            for k in 0..n {
                let (u0, u1) = (k as f32 / 48.0, (k + 1) as f32 / 48.0);
                wedge(
                    g,
                    (x, z),
                    (r.0 - 0.3, r.1),
                    (from + u0 * TAU, from + u1 * TAU),
                    (y - 0.6, y),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tower_is_drawn_as_wide_as_it_blocks() {
        // Its stones stand where a wizard meets the wall: within a few
        // centimetres of the tower's blocking radius up to the balcony,
        // never more than a hand inside it above.
        let mut g = Geo::default();
        courses(&mut g);
        for v in g.v.chunks(geo::STRIDE) {
            let r = v[0].hypot(v[2]);
            assert!(r < TOWER_RADIUS + 0.03, "{r} at {}", v[1]);
            let most = if v[1] < BALCONY { 0.01 } else { 0.27 };
            assert!(r > TOWER_RADIUS - most, "{r} at {}", v[1]);
        }
        // The door stands on the plaza.
        let mut g = Geo::default();
        door(&mut g);
        let sill =
            g.v.chunks(geo::STRIDE)
                .map(|v| v[1])
                .fold(f32::MAX, f32::min);
        assert!((sill - SUNK).abs() < 1e-4, "the door's sill at {sill}");
    }
}
