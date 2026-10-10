//! Spells in flight: the wand's bolt (a white-hot point drawing a long,
//! thinning streak of light), a fireball (a roiling ball of fire, flame
//! licking off behind it into smoke, embers), frost (cut crystals in mist
//! and glints), and any other bolt.

use render::geo::{self, mix, rgb, V3};
use render::{m4, Shape, Spark};
use wandfall::laws::spell;
use wandfall::proto::BoltSeen;
use wandfall::world::WAND;

use super::{
    colour, crystal, cycle, energy, glint, glow, light, rnd, trail, twinkle, Draw, FOE, WHITE,
};
use crate::look::{Look, GOLD};

/// How near another shard of the same Frost cast must fly (m) for one
/// shard's mist and light to stand for both.
const VOLLEY: f32 = 1.5;

/// Every bolt in flight (`you`: whose wand bolts are gold). A Frost
/// volley's shards fly close together: its mist, glints and light are
/// drawn once for those flying together, not over and over.
pub fn bolts(look: &Look, d: &mut Draw, list: &[BoltSeen], you: u16, t: f32) {
    for b in list {
        let led = b.kind == spell::FROST
            && list.iter().any(|o| {
                o.kind == spell::FROST
                    && o.by == b.by
                    && o.id < b.id
                    && geo::dot(geo::sub(o.p, b.p), geo::sub(o.p, b.p)) < VOLLEY * VOLLEY
            });
        bolt(look, d, b, (b.by == you, led), t);
    }
}

/// A bolt in flight: the wand's (`mine`: yours), or a spell's (`led`: a
/// shard flying with another that draws their mist).
fn bolt(look: &Look, d: &mut Draw, b: &BoltSeen, (mine, led): (bool, bool), t: f32) {
    let speed = geo::dot(b.v, b.v).sqrt().max(1.0);
    let fwd = geo::scale(b.v, 1.0 / speed);
    let id = b.id as i32;
    match b.kind {
        spell::FIREBALL => fireball(look, d, b.p, fwd, id, t),
        spell::FROST => frost(look, d, b.p, fwd, (id, led), t),
        _ => {
            let c = if b.kind != WAND {
                colour(b.kind)
            } else if mine {
                GOLD
            } else {
                FOE
            };
            // A white-hot point.
            glow(
                d,
                look.orb,
                m4::place(b.p, 0.0, [0.06; 3]),
                mix(c, WHITE, 0.8),
                1.0,
            );
            // Its streak, where it was a moment ago: a long, thin one in
            // its colour, a short white one in it, thinning behind.
            d.sparks.push(Spark {
                p: b.p,
                size: 0.16,
                c: [c[0], c[1], c[2], 1.0],
                v: geo::scale(fwd, 4.0),
                ..Default::default()
            });
            d.sparks.push(Spark {
                p: b.p,
                size: 0.09,
                c: [1.0, 1.0, 1.0, 1.0],
                v: geo::scale(fwd, 1.4),
                ..Default::default()
            });
            for (k, (back, size)) in [(0.4, 0.13), (0.9, 0.09), (1.5, 0.06)]
                .into_iter()
                .enumerate()
            {
                d.sparks.push(Spark {
                    p: geo::add(b.p, geo::scale(fwd, -back)),
                    size,
                    c: [c[0], c[1], c[2], 0.75 - 0.2 * k as f32],
                    v: geo::scale(fwd, 0.5),
                    ..Default::default()
                });
            }
            let flick = 0.8 + 0.2 * (t * 41.0 + id as f32).sin();
            glint(d, b.p, 0.6 * flick, mix(c, WHITE, 0.3), 0.9);
            // A few motes shed behind it.
            for k in 0..6 {
                let (u, life) = cycle(t, 9.0, k as f32 / 6.0 + rnd(id, k, 1));
                let back = geo::scale(fwd, -u * 1.8);
                let j = |q| (rnd(id, k * 7 + life, q) - 0.5) * 0.35 * u;
                d.sparks.push(Spark {
                    p: [
                        b.p[0] + back[0] + j(2),
                        b.p[1] + back[1] + j(3),
                        b.p[2] + back[2] + j(4),
                    ],
                    size: 0.07 * (1.0 - u) + 0.02,
                    c: [c[0], c[1], c[2], 1.0 - u],
                    ..Default::default()
                });
            }
            light(d, b.p, 6.0, c, 0.9);
        }
    }
}

/// A fireball: a white-hot heart in a ball of rolling fire, a wider haze
/// of flame about it; flame licking off behind it, cooling to red and
/// rising into smoke; embers spat out.
fn fireball(look: &Look, d: &mut Draw, p: V3, fwd: V3, id: i32, t: f32) {
    let c = colour(spell::FIREBALL);
    let roar = 1.0 + 0.07 * (t * 29.0 + id as f32).sin();
    glow(
        d,
        look.orb,
        m4::place(p, 0.0, [0.17; 3]),
        rgb(255, 240, 200),
        1.0,
    );
    energy(
        look,
        d,
        p,
        [0.36 * roar; 3],
        (rgb(255, 150, 50), 1.0),
        4.5,
        1.2,
    );
    energy(
        look,
        d,
        p,
        [0.58 * roar; 3],
        (rgb(255, 80, 20), 0.5),
        3.0,
        0.1,
    );
    // Flame behind it: each tongue keeps its own noise (its seed) as
    // it streams back, so it flickers rather than swaps.
    for (u, life) in trail(16, 14.0, t) {
        let back = geo::scale(fwd, -u * 2.6);
        let j = |q| (rnd(id, life, q) - 0.5) * 0.5 * u;
        let col = mix(rgb(255, 210, 120), rgb(230, 50, 15), u);
        d.sparks.push(Spark {
            p: [
                p[0] + back[0] + j(1),
                p[1] + back[1] + j(2) + u * u * 0.6,
                p[2] + back[2] + j(3),
            ],
            size: 1.0 * (1.0 - u * 0.6),
            c: [col[0], col[1], col[2], (1.0 - u) * 0.75],
            shape: Shape::Flame,
            seed: rnd(id, life, 4),
            ..Default::default()
        });
    }
    // Smoke rolling off further back.
    for (u, life) in trail(7, 6.0, t) {
        let back = geo::scale(fwd, -2.0 - u * 3.0);
        let j = |q| (rnd(id, life, q) - 0.5) * 0.8 * u;
        d.sparks.push(Spark {
            p: [
                p[0] + back[0] + j(5),
                p[1] + back[1] + j(6) + u * 0.9,
                p[2] + back[2] + j(7),
            ],
            size: 0.7 + u * 0.9,
            c: [0.22, 0.2, 0.19, 0.45 * (1.0 - u) * u.min(0.25) * 4.0],
            shape: Shape::Smoke,
            seed: rnd(id, life, 8),
            ..Default::default()
        });
    }
    // Embers, streaking.
    for k in 0..8 {
        let (u, life) = cycle(t, 5.0, rnd(id, k, 9));
        let w = super::dir(id, k * 7 + life, false);
        let at = geo::add(
            geo::add(p, geo::scale(fwd, -u * 1.6)),
            geo::scale(w, u * 0.9),
        );
        d.sparks.push(Spark {
            p: at,
            size: 0.06,
            c: [1.0, 0.75, 0.35, 1.0 - u],
            v: geo::scale(geo::add(w, geo::scale(fwd, -1.0)), 0.25),
            ..Default::default()
        });
    }
    light(d, p, 13.0, c, 1.8);
}

/// Frost: a cut crystal, a glow within it, three smaller ones about it,
/// a trail of mist and glints.
/// (`led`: another shard of its cast flies beside it and draws the
/// mist, glints and light for both.)
fn frost(look: &Look, d: &mut Draw, p: V3, fwd: V3, (id, led): (i32, bool), t: f32) {
    let c = colour(spell::FROST);
    let ice = mix(c, WHITE, 0.35);
    crystal(look, d, p, fwd, (0.6, 0.1), ice, 1.0);
    glow(d, look.orb, m4::place(p, 0.0, [0.07; 3]), ice, 1.0);
    let (s, o) = super::across(fwd);
    for k in 0..3 {
        let a = k as f32 / 3.0 * std::f32::consts::TAU + t * 7.0;
        let off = geo::add(geo::scale(s, a.cos() * 0.16), geo::scale(o, a.sin() * 0.16));
        let at = geo::add(geo::add(p, off), geo::scale(fwd, -0.18));
        crystal(look, d, at, fwd, (0.3, 0.05), ice, 0.8);
    }
    if led {
        return;
    }
    for (u, life) in trail(9, 10.0, t) {
        let back = geo::scale(fwd, -u * 2.4);
        let j = |q| (rnd(id, life, q) - 0.5) * 0.45 * u;
        d.sparks.push(Spark {
            p: [
                p[0] + back[0] + j(1),
                p[1] + back[1] + j(2),
                p[2] + back[2] + j(3),
            ],
            size: 0.35 + 0.45 * u,
            c: [0.75, 0.9, 1.0, 0.3 * (1.0 - u)],
            shape: Shape::Smoke,
            seed: rnd(id, life, 4),
            ..Default::default()
        });
    }
    for k in 0..5 {
        let (u, life) = cycle(t, 3.0, rnd(id, k, 5));
        let back = geo::scale(fwd, -u * 2.0);
        let j = |q| (rnd(id, k * 7 + life, q) - 0.5) * 0.5;
        glint(
            d,
            [
                p[0] + back[0] + j(6),
                p[1] + back[1] + j(7),
                p[2] + back[2] + j(8),
            ],
            0.35,
            WHITE,
            twinkle(u),
        );
    }
    light(d, p, 4.5, c, 0.9);
}
