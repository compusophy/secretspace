//! The marks spells leave on wizards (a ward's bubble, frost's chill,
//! mending) and on the island (burns, rime), wizards knocked out, and
//! dust where they land.

use render::geo::{self, mix, rgb, V3};
use render::{Decal, Mark, Spark};
use wandfall::laws::spell;
use wandfall::proto::{fx, Seen};

use super::{
    colour, crystal, glint, light, plasma, puffs, ring, rnd, shell, sigil, spray, Draw, Spray,
    WHITE,
};
use crate::look::{Look, GOLD};

const UP: V3 = [0.0, 1.0, 0.0];

/// The marks spells leave on a wizard.
/// `me`: it is you (seen from inside: no bubble about your eyes).
pub fn on_wizard(look: &Look, d: &mut Draw, s: &Seen, t: f32, me: bool) {
    let at = s.p;
    let id = s.id as i32;
    let floor = [at[0], at[1] + 0.06, at[2]];
    if s.fx & fx::SHIELD != 0 {
        let c = colour(spell::WARD);
        sigil(look, d, floor, UP, 1.05, (c, 0.75), t * 0.6);
        if !me {
            let wob = 1.0 + 0.03 * (t * 6.0 + id as f32).sin();
            let mid = [at[0], at[1] + 0.95, at[2]];
            let r = [0.95 * wob, 1.2 * wob, 0.95 * wob];
            plasma(look, d, mid, r, (mix(c, WHITE, 0.2), 0.16), (3.5, 0.0), 1.0);
            shell(look, d, mid, r, c, 0.45);
            light(d, [at[0], at[1] + 1.0, at[2]], 3.5, c, 0.8);
        }
    }
    if s.fx & fx::CHILLED != 0 {
        let c = colour(spell::FROST);
        let ice = mix(c, WHITE, 0.4);
        for k in 0..8 {
            let a = k as f32 / 8.0 * std::f32::consts::TAU + id as f32;
            let up = geo::norm([a.cos() * 0.6, 1.0, a.sin() * 0.6]);
            let base = [at[0] + a.cos() * 0.33, at[1] + 0.05, at[2] + a.sin() * 0.33];
            let len = 0.35 + 0.25 * rnd(id, k, 3);
            crystal(look, d, base, up, (len, 0.09), ice, 0.85);
        }
        puffs(
            d,
            [at[0], at[1] + 0.1, at[2]],
            ((t * 0.8).fract() * 1.5, id + (t * 0.8) as i32),
            (4, 1.5),
            ([0.8, 0.92, 1.0], 0.3),
            (0.4, 1.8),
            (0.5, 0.2),
        );
        for k in 0..3 {
            let u = ((t * 1.3 + rnd(id, k, 4)) % 1.0).abs();
            let a = rnd(id, k + (t * 1.3) as i32, 5) * std::f32::consts::TAU;
            let twinkle = (1.0 - (u * 2.0 - 1.0).abs()).powi(2);
            glint(
                d,
                [
                    at[0] + a.cos() * 0.45,
                    at[1] + 0.2 + 1.4 * u,
                    at[2] + a.sin() * 0.45,
                ],
                0.4,
                WHITE,
                twinkle,
            );
        }
        light(d, [at[0], at[1] + 0.4, at[2]], 2.5, c, 0.7);
    }
    if s.fx & fx::MENDING != 0 {
        let c = colour(spell::MEND);
        let pulse = 0.5 + 0.5 * (t * 5.0).sin();
        sigil(look, d, floor, UP, 0.95, (c, 0.45 + 0.25 * pulse), -t * 0.8);
        for k in 0..14 {
            let rise = (t * 0.8 + k as f32 / 14.0).fract();
            let a = k as f32 / 14.0 * std::f32::consts::TAU + t * 2.0 + rise * 3.0;
            let r = 0.6 - rise * 0.3;
            d.sparks.push(Spark {
                p: [at[0] + a.cos() * r, at[1] + rise * 2.3, at[2] + a.sin() * r],
                size: 0.13,
                c: [c[0], c[1], c[2], 1.0 - rise],
                v: [-a.sin() * 0.15, 0.25, a.cos() * 0.15],
                ..Default::default()
            });
        }
        light(d, [at[0], at[1] + 1.0, at[2]], 4.0, c, 0.9);
    }
}

/// Wizards knocked out: a burst of their colour going up where they fell,
/// a flare, a ring along the ground, a puff of their colour.
pub fn falls(look: &Look, d: &mut Draw, list: &[(f64, V3, u16)], now: f64) {
    for &(when, at, who) in list {
        let t = ((now - when) / 1000.0) as f32;
        let f = 1.0 - t / 1.4;
        if f <= 0.0 {
            continue;
        }
        let c = mix(crate::look::hue(who), WHITE, 0.35);
        let seed = who as i32 ^ when as i32;
        for n in 0..36 {
            let a = rnd(seed, n, 61) * std::f32::consts::TAU + t * 3.0;
            let r = 0.3 + t * (0.6 + rnd(seed, n, 62));
            let y = at[1] + 0.2 + t * 3.5 * rnd(seed, n, 63) + 1.6 * rnd(seed, n, 64) * f;
            d.sparks.push(Spark {
                p: [at[0] + a.cos() * r, y, at[2] + a.sin() * r],
                size: 0.14 * f + 0.04,
                c: [c[0], c[1], c[2], f],
                v: [-a.sin() * 0.2, 0.3, a.cos() * 0.2],
                ..Default::default()
            });
        }
        if t < 0.15 {
            glint(
                d,
                [at[0], at[1] + 1.0, at[2]],
                4.0 * (1.0 - t / 0.15),
                WHITE,
                1.0,
            );
        }
        ring(
            look,
            d,
            [at[0], at[1] + 0.06, at[2]],
            0.5 + t * 4.0,
            c,
            f,
            0.0,
        );
        spray(
            d,
            [at[0], at[1] + 1.0, at[2]],
            t,
            seed,
            Spray {
                fall: 9.0,
                streak: 0.04,
                ..Spray::new(16, 7.0, 0.8, (WHITE, GOLD), 0.1)
            },
        );
        puffs(
            d,
            [at[0], at[1] + 0.5, at[2]],
            (t, seed),
            (6, 1.4),
            (geo::scale(c, 0.6), 0.4),
            (0.5, 2.5),
            (0.8, 0.8),
        );
        light(d, [at[0], at[1] + 1.2, at[2]], 8.0, c, 3.0 * f * f);
    }
}

/// Dust thrown up where wizards landed, more the harder; in the shallows,
/// a splash.
pub fn dust(look: &Look, d: &mut Draw, list: &[(f64, V3, f32)], now: f64) {
    let c = rgb(170, 160, 138);
    for &(when, at, hard) in list {
        let t = ((now - when) / 1000.0) as f32;
        let seed = when as i32 ^ (at[0] * 31.0) as i32;
        if at[1] < wandfall::laws::SEA + 0.05 {
            splash(
                look,
                d,
                [at[0], wandfall::laws::SEA, at[2]],
                (t, seed),
                hard,
            );
            continue;
        }
        let n = (4.0 + 8.0 * hard) as i32;
        puffs(
            d,
            [at[0], at[1] + 0.1, at[2]],
            (t, seed),
            (n, 0.8),
            (c, 0.25 + 0.3 * hard),
            (0.25 + 0.2 * hard, 2.2),
            (0.6 + 1.6 * hard, 0.3),
        );
        if hard > 0.5 {
            let f = 1.0 - t / 0.6;
            ring(
                look,
                d,
                [at[0], at[1] + 0.05, at[2]],
                0.4 + t * 4.0,
                c,
                0.3 * f.max(0.0),
                0.0,
            );
        }
    }
}

/// Water thrown up where a wizard came down in the shallows: drops
/// streaking up and falling, spray, a ring spreading on the water.
fn splash(look: &Look, d: &mut Draw, at: V3, (t, seed): (f32, i32), hard: f32) {
    let water = rgb(200, 225, 235);
    spray(
        d,
        at,
        t,
        seed,
        Spray {
            fall: 12.0,
            up: true,
            streak: 0.04,
            ..Spray::new(
                (10.0 + 14.0 * hard) as i32,
                3.5 + 3.0 * hard,
                0.6,
                (WHITE, water),
                0.07,
            )
        },
    );
    puffs(
        d,
        at,
        (t, seed),
        (4, 0.6),
        (water, 0.3 + 0.2 * hard),
        (0.3, 2.0),
        (0.6 + hard, 0.4),
    );
    let f = 1.0 - t / 0.8;
    if f > 0.0 {
        ring(
            look,
            d,
            [at[0], at[1] - 0.2, at[2]],
            0.3 + t * 2.5,
            water,
            0.5 * f,
            0.0,
        );
    }
}

/// The scars spells leave on the island, as decals: a fireball's burn
/// (embers in it a while, a shockwave out of it), lightning's (a flash
/// ringing out), the Lance's small one, frost's rime; fading at the end.
pub fn scars(d: &mut Draw, list: &[(f64, V3, u8)], now: f64) {
    for &(when, at, s) in list {
        let t = ((now - when) / 1000.0) as f32;
        let seed = when as i32 ^ (at[0] * 17.0) as i32;
        let yaw = rnd(seed, 0, 61) * std::f32::consts::TAU;
        let last = crate::state::SCARS_MS as f32 / 1000.0;
        let (r, depth, mark, col, hot, life) = match s {
            spell::FIREBALL => (2.8, 2.2, Mark::Scorch, rgb(255, 110, 30), 4.0, last),
            spell::LIGHTNING => (1.8, 1.6, Mark::Scorch, rgb(150, 190, 255), 1.5, last),
            spell::LANCE => (0.5, 0.8, Mark::Scorch, rgb(255, 210, 120), 1.5, last * 0.5),
            _ => (2.2, 2.0, Mark::Frost, rgb(205, 236, 255), 0.0, 9.0),
        };
        let fade = (1.0 - (t - life * 0.7) / (life * 0.3)).clamp(0.0, 1.0);
        if fade <= 0.0 {
            continue;
        }
        // Rime grows in fast; a burn is there at once.
        let grow = if mark == Mark::Frost {
            (t / 0.25).min(1.0)
        } else {
            1.0
        };
        let heat = if hot > 0.0 {
            (1.0 - t / hot).max(0.0).powi(2)
        } else {
            1.0
        };
        d.decals.push(Decal {
            p: at,
            r: r * (0.6 + 0.4 * grow),
            depth,
            yaw,
            c: [col[0] * heat, col[1] * heat, col[2] * heat, fade],
            mark,
            seed: rnd(seed, 1, 62),
        });
        // A ring of light rushing out over the ground.
        let k = t / 0.45;
        if mark == Mark::Scorch && s != spell::LANCE && k < 1.0 {
            d.decals.push(Decal {
                p: at,
                r: r * (0.6 + 1.6 * k),
                depth,
                yaw,
                c: [col[0], col[1], col[2], (1.0 - k) * 1.5],
                mark: Mark::Ring,
                seed: 0.0,
            });
        }
    }
}
