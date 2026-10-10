//! The big ones: a fireball bursting, lightning's warning and its strike,
//! a blink out and in, a gust.

use render::geo::{self, mix, rgb, V3};
use render::{m4, Shape, Spark};
use wandfall::laws::{
    spell, FIREBALL_RADIUS, GUST_RADIUS, LIGHTNING_DELAY, LIGHTNING_RADIUS, TICK_HZ,
};

use super::{
    beam, colour, dir, energy, glint, glow, light, puffs, ring, rnd, runes, shaft, spray, wave,
    Draw, Ray, Spray, WHITE,
};
use crate::look::Look;

/// Where sparks thrown from `at` come down: the ground under it, if it
/// is near the ground (a burst up a tree throws them into the air).
fn floor_under(look: &Look, at: V3) -> Option<f32> {
    let g = look.ground.at(at[0], at[2]);
    (at[1] - g < 1.5).then_some(g + 0.05)
}

/// A fireball bursting at `at`, `t` seconds ago: a white flash, a core
/// of rolling fire burning out fast, tongues of flame billowing out and
/// up and burning down to red, a shockwave along the ground out to where
/// it hurts, debris and embers flung out, then a pall of smoke rolling
/// up.
pub fn explosion(look: &Look, d: &mut Draw, at: V3, t: f32, seed: i32) {
    let c = colour(spell::FIREBALL);
    let r = FIREBALL_RADIUS;
    if !(0.0..=3.2).contains(&t) {
        return;
    }
    let swell = 1.0 - (-t / 0.1).exp();
    let burn = (1.0 - t / 0.8).max(0.0);
    let core = (1.0 - t / 0.35).max(0.0);
    let lift = [at[0], at[1] + t * 0.8, at[2]];
    if t < 0.12 {
        let k = 1.0 - t / 0.12;
        glow(
            d,
            look.orb,
            m4::place(at, 0.0, [r * (0.15 + 0.2 * swell); 3]),
            rgb(255, 245, 220),
            0.7 * k,
        );
        glint(d, at, r * 1.2 * k, rgb(255, 220, 160), k);
    }
    // Its heart, fine-grained so it tears rather than reads as a ball,
    // and gone before the flames are.
    energy(
        look,
        d,
        lift,
        [r * (0.15 + 0.3 * swell); 3],
        (rgb(255, 140, 40), core * 0.85),
        6.0 / r,
        0.2 + 0.5 * core,
    );
    // The fireball itself: tongues of flame thrown out over a ball and
    // rising, each burning down from yellow-white to deep red (they make
    // its edge).
    for k in 0..30 {
        let w = dir(seed, k, false);
        let out = r * (0.15 + 0.55 * rnd(seed, k, 71)) * swell;
        let life = 0.55 + 0.45 * rnd(seed, k, 72);
        let f = 1.0 - t / life;
        if f <= 0.0 {
            continue;
        }
        let col = mix(rgb(255, 225, 150), rgb(200, 45, 12), (1.0 - f).powf(0.7));
        d.sparks.push(Spark {
            p: [
                at[0] + w[0] * out,
                at[1] + w[1] * out * 0.8 + t * (0.8 + 1.2 * rnd(seed, k, 73)),
                at[2] + w[2] * out,
            ],
            size: r * (0.35 + 0.3 * rnd(seed, k, 74)) * (0.6 + 0.4 * swell),
            c: [col[0], col[1], col[2], f.min(1.0) * 0.85],
            shape: Shape::Flame,
            seed: rnd(seed, k, 75),
            ..Default::default()
        });
    }
    // The shockwave, out to where it hurts.
    let rush = (1.0 - t / 0.45).max(0.0);
    let reach = r * (t / 0.35).min(1.0).sqrt();
    wave(
        look,
        d,
        [at[0], at[1] - 0.3, at[2]],
        reach,
        rgb(255, 160, 80),
        rush,
    );
    // Debris, streaking out and falling (onto the ground, cooling there,
    // if it burst on the ground); embers drifting up after.
    spray(
        d,
        at,
        t,
        seed,
        Spray {
            fall: 12.0,
            streak: 0.05,
            floor: floor_under(look, at),
            ..Spray::new(28, 14.0, 0.9, (rgb(255, 230, 160), rgb(200, 40, 10)), 0.14)
        },
    );
    spray(
        d,
        at,
        t,
        seed + 3,
        Spray {
            fall: -1.2,
            up: true,
            ..Spray::new(20, 2.5, 2.2, (rgb(255, 200, 110), rgb(220, 60, 20)), 0.09)
        },
    );
    // A pall of smoke, rolling up and out as the fire burns down.
    puffs(
        d,
        [lift[0], lift[1] + r * 0.35, lift[2]],
        (t - 0.35, seed),
        (14, 3.0),
        ([0.16, 0.14, 0.13], 0.75),
        (r * 0.5, 2.6),
        (r * 0.9, 1.1),
    );
    light(
        d,
        lift,
        r * 3.5,
        c,
        3.2 * burn * burn + 0.15 * (1.0 - t / 3.2),
    );
}

/// Lightning's warning: a circle of runes where it will strike, turning;
/// a ring closing to the middle as the moment comes; crackles at its
/// edge; a faint shaft from the sky.
pub fn mark(look: &Look, d: &mut Draw, at: V3, age: f32, seed: i32) {
    let wait = LIGHTNING_DELAY as f32 * 1000.0 / TICK_HZ as f32;
    if age > wait {
        return;
    }
    let c = colour(spell::LIGHTNING);
    let k = age / wait;
    let r = LIGHTNING_RADIUS;
    let pulse = 0.6 + 0.4 * (age / 50.0).sin();
    runes(look, d, at, r, (c, 0.9 * pulse), age / 400.0);
    wave(
        look,
        d,
        at,
        r * (1.0 - k).max(0.08) * 1.05,
        mix(c, WHITE, 0.4),
        0.9,
    );
    let flick = (age / 60.0) as i32;
    for n in 0..14 {
        let a = rnd(seed + flick, n, 31) * std::f32::consts::TAU;
        let tangent = [-a.sin(), 0.0, a.cos()];
        d.sparks.push(Spark {
            p: [
                at[0] + a.cos() * r,
                at[1] + 0.1 + 0.4 * rnd(seed + flick, n, 32),
                at[2] + a.sin() * r,
            ],
            size: 0.08,
            c: [c[0], c[1], c[2], 1.0],
            v: geo::scale(tangent, 0.4 * (rnd(seed + flick, n, 33) - 0.5)),
            ..Default::default()
        });
    }
    shaft(
        look,
        d,
        (at, [at[0], at[1] + 30.0, at[2]]),
        0.15,
        (c, 0.1 + 0.2 * k),
        Ray::Halo,
    );
    light(d, [at[0], at[1] + 1.0, at[2]], r * 1.6, c, 0.6 + 1.4 * k);
}

/// A jagged path of `n` steps from `top` down to `end`, wandering by up
/// to `wide`, as `flick` says; each step a hot core in a halo.
#[allow(clippy::too_many_arguments)]
pub(crate) fn forks(
    look: &Look,
    d: &mut Draw,
    (top, end): (V3, V3),
    n: i32,
    wide: f32,
    flick: i32,
    (thick, c, f): (f32, V3, f32),
) -> Vec<V3> {
    let mut path = vec![top];
    let mut prev = top;
    for k in 1..=n {
        let u = k as f32 / n as f32;
        let j = |q| (rnd(flick, k, q) - 0.5) * wide * (1.0 - u * 0.8);
        let line = geo::add(top, geo::scale(geo::sub(end, top), u));
        let p = if k == n {
            end
        } else {
            [line[0] + j(1), line[1] + j(3) * 0.3, line[2] + j(2)]
        };
        beam(
            look,
            d,
            (prev, p),
            thick,
            (mix(c, WHITE, 0.85), 1.0),
            Ray::Core,
        );
        beam(look, d, (prev, p), thick * 3.5, (c, f * 0.8), Ray::Halo);
        path.push(p);
        prev = p;
    }
    path
}

/// Lightning striking at `at`, `t` seconds ago: a forked bolt from the
/// sky, flickering; a flash; a blast of light along the ground, sparks
/// thrown up, dust, the circle of runes burning out.
pub fn strike(look: &Look, d: &mut Draw, at: V3, t: f32, seed: i32) {
    let c = colour(spell::LIGHTNING);
    let f = 1.0 - t / 0.6;
    if f <= 0.0 {
        return;
    }
    if t < 0.3 {
        let flick = seed + (t / 0.05) as i32;
        let top = [at[0] + 3.0 * (rnd(flick, 0, 9) - 0.5), at[1] + 45.0, at[2]];
        let path = forks(look, d, (top, at), 16, 4.0, flick, (0.08, c, f));
        // Forks off it, thinner, into the air.
        for b in 0..3 {
            let at = 4 + b as usize * 3 + (rnd(flick, b, 5) * 3.0) as usize;
            let from = path[at.min(path.len() - 2)];
            let w = dir(flick, b + 20, false);
            let end = [
                from[0] + w[0] * 6.0,
                from[1] - 3.0 - 4.0 * rnd(flick, b, 6),
                from[2] + w[2] * 6.0,
            ];
            forks(
                look,
                d,
                (from, end),
                6,
                2.0,
                flick + 31 * (b + 1),
                (0.04, c, f * 0.7),
            );
        }
    }
    if t < 0.08 {
        let k = 1.0 - t / 0.08;
        glint(
            d,
            [at[0], at[1] + 0.5, at[2]],
            9.0 * k,
            mix(c, WHITE, 0.6),
            k,
        );
        glow(
            d,
            look.orb,
            m4::place(at, 0.0, [1.4 * k; 3]),
            mix(c, WHITE, 0.6),
            k,
        );
    }
    // The circle burning out, and a blast of light out to where it
    // hurts.
    let r = LIGHTNING_RADIUS;
    runes(look, d, at, r, (mix(c, WHITE, 0.3), f * f), t);
    let reach = r * (0.4 + 0.6 * (t / 0.25).min(1.0).sqrt());
    wave(look, d, at, reach, mix(c, WHITE, 0.3), f);
    let floor = [at[0], at[1] + 0.08, at[2]];
    spray(
        d,
        floor,
        t,
        seed,
        Spray {
            fall: 14.0,
            streak: 0.04,
            floor: Some(floor[1]),
            ..Spray::new(26, 10.0, 0.6, (WHITE, c), 0.12)
        },
    );
    puffs(
        d,
        floor,
        (t, seed),
        (8, 1.6),
        ([0.45, 0.42, 0.5], 0.45),
        (0.5, 3.0),
        (LIGHTNING_RADIUS * 0.8, 0.5),
    );
    light(d, [at[0], at[1] + 3.0, at[2]], 20.0, c, 7.0 * f * f);
}

/// A wizard going (`out`) or arriving at `feet`: a whirl of streaks
/// wheeling up and in (going) or out (arriving), a column of light
/// flaring and gone, glints.
pub fn blink(look: &Look, d: &mut Draw, feet: V3, t: f32, seed: i32, out: bool) {
    let c = colour(spell::BLINK);
    let f = 1.0 - t / 0.5;
    if f <= 0.0 {
        return;
    }
    for n in 0..36 {
        let a0 = rnd(seed, n, 41) * std::f32::consts::TAU;
        let spin = if out { 9.0 } else { -7.0 };
        let a = a0 + t * spin;
        let y = rnd(seed, n, 42) * 2.0 + if out { t * 2.5 } else { 0.0 };
        let r = if out { 0.6 * f + 0.05 } else { 0.3 + t * 2.6 };
        let tangent = [
            -a.sin() * spin * r,
            if out { 2.5 } else { 0.0 },
            a.cos() * spin * r,
        ];
        d.sparks.push(Spark {
            p: [feet[0] + a.cos() * r, feet[1] + y, feet[2] + a.sin() * r],
            size: 0.09,
            c: [c[0], c[1], c[2], f],
            v: geo::scale(tangent, 0.035),
            ..Default::default()
        });
    }
    // A shaft of light, thinning to nothing.
    let col = (1.0 - t / 0.3).max(0.0);
    let top = [feet[0], feet[1] + 4.0, feet[2]];
    shaft(
        look,
        d,
        (feet, top),
        0.55 * col + 0.05,
        (c, col * 0.6),
        Ray::Flow(4.0),
    );
    shaft(
        look,
        d,
        (feet, top),
        0.05 * col,
        (mix(c, WHITE, 0.7), col),
        Ray::Core,
    );
    if !out {
        wave(look, d, feet, 0.4 + t * 3.0, c, f);
    }
    for n in 0..3 {
        let y = feet[1] + 0.5 + rnd(seed, n, 43) * 1.4;
        let a = rnd(seed, n, 44) * std::f32::consts::TAU;
        glint(
            d,
            [feet[0] + a.cos() * 0.4, y, feet[2] + a.sin() * 0.4],
            0.8 * f,
            WHITE,
            f,
        );
    }
    light(d, [feet[0], feet[1] + 1.0, feet[2]], 6.0, c, 2.5 * f);
}

/// Gust: wind wheeling out from `feet` in streaks, a ring along the
/// ground and one higher, dust thrown up.
pub fn gust(look: &Look, d: &mut Draw, feet: V3, (t, seed): (f32, i32)) {
    let c = colour(spell::GUST);
    let f = 1.0 - t / 0.55;
    if f <= 0.0 {
        return;
    }
    let grow = 1.0 - (-t / 0.12).exp();
    let r = GUST_RADIUS * grow;
    wave(look, d, feet, r, c, f);
    ring(
        look,
        d,
        [feet[0], feet[1] + 0.9, feet[2]],
        r * 0.8,
        c,
        f * 0.35,
        -t * 3.0,
    );
    // Streaks of wind: arcs flung outward, wheeling as they go.
    for n in 0..30 {
        let a = n as f32 / 30.0 * std::f32::consts::TAU + rnd(seed, n, 53) + t * 4.0;
        let rr = r * (0.45 + 0.55 * rnd(seed, n, 51));
        let y = feet[1] + 0.2 + 1.7 * rnd(seed, n, 52);
        let v = [
            -a.sin() * 4.0 * rr + a.cos() * 6.0,
            0.0,
            a.cos() * 4.0 * rr + a.sin() * 6.0,
        ];
        d.sparks.push(Spark {
            p: [feet[0] + a.cos() * rr, y, feet[2] + a.sin() * rr],
            size: 0.08,
            c: [c[0], c[1], c[2], f * 0.9],
            v: geo::scale(v, 0.05),
            ..Default::default()
        });
    }
    puffs(
        d,
        [feet[0], feet[1] + 0.2, feet[2]],
        (t, seed),
        (10, 1.2),
        ([0.62, 0.58, 0.5], 0.4),
        (0.5, 2.2),
        (GUST_RADIUS * 0.9, 0.4),
    );
    light(d, [feet[0], feet[1] + 1.0, feet[2]], r + 2.0, c, 1.5 * f);
}
