//! The Spire, built: its tower (a stone shaft, buttressed, a balcony
//! with battlements, a wizard's hat of a roof), the door and windows lit
//! up a spiral, the stair and balcony (its decks), the merlons along the
//! balcony and the lamps about the plaza.

use std::f32::consts::TAU;

use render::geo::{self, rgb, Geo, V3};
use render::{Mesh, Renderer};

use wandfall::map::Map;
use wandfall::places::Deck;

use super::{one, smooth, GOLDEN, VIOLET};

pub(super) struct Spire {
    pub tower: Mesh,
    pub trim: Mesh,
    pub decks: Mesh,
    pub merlon: Mesh,
    pub lamp: Mesh,
}

impl Spire {
    pub(super) fn new(r: &mut Renderer, map: &Map) -> Spire {
        // The Spire's tower: a stone shaft, buttressed, a balcony with
        // battlements, a wizard's hat of a roof; windows lit up a spiral.
        let tower = smooth(r, |g| {
            let pale = rgb(172, 164, 160);
            let dark = rgb(120, 112, 132);
            // Its foot flares a little (the stair starts beside it).
            g.lathe(
                [0.0; 3],
                &[(5.2, 0.0), (5.0, 0.8), (4.7, 4.0), (4.5, 12.0), (4.3, 21.6)],
                24,
                pale,
                0.0,
            );
            for y in [7.0, 14.5] {
                g.lathe(
                    [0.0, y, 0.0],
                    &[(4.62, 0.0), (4.75, 0.2), (4.62, 0.4)],
                    24,
                    GOLDEN,
                    0.15,
                );
            }
            // A shoulder where the shaft narrows (the balcony is a deck).
            g.lathe(
                [0.0; 3],
                &[(4.3, 21.5), (4.5, 22.4), (3.7, 23.0)],
                24,
                dark,
                0.0,
            );
            g.lathe([0.0; 3], &[(3.7, 23.0), (3.5, 30.0)], 20, pale, 0.0);
            // A wizard's hat: a wide brim, its tip bent over.
            let felt = rgb(66, 44, 140);
            let hat = [
                (5.7, 29.8),
                (5.6, 30.3),
                (3.6, 32.4),
                (2.2, 35.4),
                (1.2, 38.6),
                (0.55, 41.0),
            ];
            g.lathe([0.0; 3], &hat, 24, felt, 0.0);
            g.spike([0.0, 40.8, 0.0], [2.2, 44.6, 0.5], 0.62, 10, felt, 0.0);
            g.lathe(
                [0.0, 30.0, 0.0],
                &[(5.66, 0.0), (5.75, 0.25), (5.66, 0.5)],
                24,
                GOLDEN,
                0.2,
            );
        });
        let trim = one(r, |g| {
            // The door, and the windows climbing the shaft.
            g.block(
                [4.75, 1.2, 0.0],
                [0.5, 3.2, 1.8],
                0.0,
                rgb(40, 26, 44),
                rgb(40, 26, 44),
                0.0,
            );
            // A dark frame, and candlelight in it, pointed at the top.
            let window = |g: &mut Geo, a: f32, y: f32, r: f32, w: f32, h: f32| {
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
                pane(g, r, w + 0.3, h + 0.3, rgb(44, 34, 56), 0.0);
                pane(g, r + 0.03, w, h, rgb(255, 178, 90), 3.0);
            };
            for k in 0..13 {
                let y = 4.5 + k as f32 * 1.3;
                // Just proud of the shaft, which narrows as it climbs.
                let r = if y < 12.0 {
                    4.7 - (y - 4.0) * 0.025
                } else {
                    4.5 - (y - 12.0) * 0.0208
                };
                window(g, k as f32 * 1.05 + 1.0, y, r + 0.07, 0.55, 1.1);
            }
            for k in 0..4 {
                window(g, k as f32 / 4.0 * TAU + 0.4, 26.5, 3.68, 0.6, 1.4);
            }
        });
        let decks = one(r, |g| {
            for d in &map.decks {
                deck(g, d);
            }
        });
        let merlon = one(r, |g| {
            let dark = rgb(112, 104, 120);
            g.block(
                [0.0; 3],
                [0.5, 1.2, 0.85],
                0.0,
                rgb(140, 132, 140),
                dark,
                0.0,
            );
        });
        let lamp = one(r, |g| {
            let iron = rgb(46, 42, 58);
            g.column([0.0; 3], 6, (0.2, 0.13), 2.8, 0.0, iron, 0.0, false);
            g.column([0.0, 2.8, 0.0], 6, (0.32, 0.32), 0.08, 0.0, iron, 0.0, true);
            g.sphere([0.0, 3.15, 0.0], [0.26; 3], (1, 4, 0.0), VIOLET, 2.5);
        });
        Spire {
            tower,
            trim,
            decks,
            merlon,
            lamp,
        }
    }

    pub(super) fn meshes(&self) -> [Mesh; 5] {
        [self.tower, self.trim, self.decks, self.merlon, self.lamp]
    }
}

/// A block of a ring between radii `r`, from heading `a0` to `a1`, from
/// `lo` up to `hi`: its top, outer side, ends and underside.
fn wedge(g: &mut Geo, c: (f32, f32), r: (f32, f32), (a0, a1): (f32, f32), (lo, hi): (f32, f32)) {
    let top = rgb(150, 142, 148);
    let side = rgb(104, 96, 110);
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

/// Somewhere to stand above the ground, built: a stair of stone steps with
/// a glowing rail on posts along its outer edge, or a ring of slabs.
fn deck(g: &mut Geo, d: &Deck) {
    const STEPS: f32 = 36.0;
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
            for k in 0..n {
                let (u0, u1) = (k as f32 / STEPS, (k + 1) as f32 / STEPS);
                let y = y0 + (u0 + u1) / 2.0 * rise;
                wedge(g, (x, z), inner, (at(u0), at(u1)), (y - 0.45, y));
                if k % 3 == 0 {
                    let a = at(u0);
                    let post = [x + a.cos() * (r.1 - 0.12), y, z + a.sin() * (r.1 - 0.12)];
                    g.column(post, 4, (0.05, 0.04), 0.95, a, rgb(46, 40, 58), 0.0, true);
                }
            }
            // The rail: a thin gold band, a step at a time.
            let rr = r.1 - 0.12;
            let pt = |u: f32, dy: f32| {
                let a = at(u);
                [
                    x + a.cos() * rr,
                    y0 + u * rise + 0.95 + dy,
                    z + a.sin() * rr,
                ]
            };
            for k in 0..n {
                let (u0, u1) = (k as f32 / STEPS, (k + 1) as f32 / STEPS);
                g.quad(
                    pt(u0, -0.04),
                    pt(u0, 0.04),
                    pt(u1, 0.04),
                    pt(u1, -0.04),
                    GOLDEN,
                    1.4,
                );
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
