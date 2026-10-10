//! The night and the desk's light: the moon through the window, a dark
//! sky over the city, and the lights of the desk's own things (the
//! monitor, the lamp, the strip behind the desk, the tower, the keys,
//! the neon), the colour lights going round the rainbow together.

use battlestation::laws::{DESK_TOP, KEYBOARD_X, KEYBOARD_Z, WALL_N};
use render::geo::{self, V3};
use render::{Grade, Light, Look, Shape, Spark};

use crate::gear::on_screen;
use crate::scene::{BULB, MUG, NEON_AT, STRIP};

/// The neon's pink.
pub const NEON: V3 = [1.0, 0.24, 0.62];

pub fn look() -> Look {
    Look {
        sun_dir: geo::norm([-0.66, 0.36, -0.66]),
        sun: [0.30, 0.36, 0.52],
        sun_size: 0.022,
        sky: [0.025, 0.03, 0.05],
        low: [0.012, 0.01, 0.012],
        zenith: [0.004, 0.008, 0.026],
        horizon: [0.07, 0.06, 0.1],
        deep: [0.02, 0.02, 0.03],
        fog: 0.004,
        fog_falloff: 0.01,
        clouds: 0.2,
        stars: 1.0,
        exposure: 1.2,
        bloom: 0.09,
        vignette: 0.38,
        sea: None,
        water: [0.02, 0.1, 0.16],
        waves: 0.0,
        wind: [0.0, 0.0],
        grade: Grade {
            lift: [0.008, 0.004, 0.018],
            gamma: [1.0, 1.0, 1.0],
            gain: [1.03, 1.0, 1.04],
            saturation: 1.08,
            contrast: 1.06,
        },
        glow: 1.0,
        wet: 0.0,
    }
}

/// A colour from its hue (0 to 1 round the wheel), saturation, value.
pub fn hsv(h: f32, s: f32, v: f32) -> V3 {
    let h = h.rem_euclid(1.0) * 6.0;
    let f = h.fract();
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match h as u32 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

/// The colour lights' colour `x` metres along the desk, `t` seconds in:
/// a slow wave between violet, magenta and cyan.
pub fn rainbow(x: f32, t: f32) -> V3 {
    let k = (x * 0.55 - t * 0.05).rem_euclid(1.0);
    // Round a violet-cyan-magenta arc rather than the whole wheel.
    let h = 0.5 + 0.42 * (0.5 - 0.5 * (k * std::f32::consts::TAU).cos());
    hsv(h, 0.78, 1.0)
}

/// Every light this frame: the screen's (its mean colour), the lamp, the
/// strip, the tower, the keys, the neon.
pub fn lights(t: f32, screen: V3) -> Vec<Light> {
    let mut out = Vec::with_capacity(12);
    out.push(Light {
        p: on_screen([0.0, -0.02, 0.22]),
        r: 1.7,
        c: geo::scale(geo::add(screen, [0.05, 0.06, 0.1]), 2.4),
    });
    out.push(Light {
        p: geo::add(BULB, [0.02, -0.06, 0.02]),
        r: 1.6,
        c: [1.1, 0.74, 0.42],
    });
    let (x0, x1, y) = STRIP;
    for k in 0..4 {
        let x = x0 + (x1 - x0) * (k as f32 + 0.5) / 4.0;
        out.push(Light {
            p: [x, y + 0.05, WALL_N + 0.1],
            r: 0.95,
            c: geo::scale(rainbow(x, t), 0.75),
        });
    }
    out.push(Light {
        p: [0.46, DESK_TOP + 0.25, -0.5],
        r: 0.7,
        c: geo::scale(rainbow(0.6, t), 0.55),
    });
    out.push(Light {
        p: [KEYBOARD_X, DESK_TOP + 0.05, KEYBOARD_Z],
        r: 0.34,
        c: geo::scale(rainbow(KEYBOARD_X, t), 0.35),
    });
    let flicker = 0.92 + 0.08 * ((t * 13.0).sin() * (t * 7.3).sin()).abs();
    out.push(Light {
        p: geo::add(NEON_AT, [0.0, 0.0, 0.18]),
        r: 1.3,
        c: geo::scale(NEON, 0.9 * flicker),
    });
    out
}

/// How bright the neon is (it hums and flickers now and then).
pub fn neon_glow(t: f32) -> f32 {
    let stutter = if (t * 0.37).fract() < 0.015 {
        0.35
    } else {
        1.0
    };
    (2.4 + 0.2 * (t * 13.0).sin()) * stutter
}

/// Steam off the coffee: a few soft puffs rising, each its own.
pub fn steam(t: f32) -> Vec<Spark> {
    (0..6)
        .map(|k| {
            let age = (t * 0.22 + k as f32 / 6.0).fract();
            let sway = (t * 0.9 + k as f32 * 1.7).sin() * 0.012 * age;
            Spark {
                p: [
                    MUG[0] + sway,
                    MUG[1] + 0.1 + age * 0.16,
                    MUG[2] + sway * 0.6,
                ],
                size: 0.02 + age * 0.05,
                c: [0.75, 0.75, 0.8, 0.10 * (1.0 - age) * (age * 6.0).min(1.0)],
                v: [0.0; 3],
                shape: Shape::Smoke,
                seed: k as f32 / 6.0,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_colours_stay_in_gamut() {
        for i in 0..100 {
            let c = rainbow(i as f32 * 0.03 - 1.5, i as f32 * 0.7);
            assert!(c.iter().all(|v| (0.0..=1.0).contains(v)), "{c:?}");
            let h = hsv(i as f32 * 0.137, 0.8, 0.9);
            assert!(h.iter().all(|v| (0.0..=1.0).contains(v)));
        }
        assert_eq!(hsv(0.0, 1.0, 1.0), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn the_lights_are_finite_and_near_the_desk() {
        for l in lights(12.5, [0.2, 0.3, 0.5]) {
            assert!(l.p.iter().chain(&l.c).all(|v| v.is_finite()));
            assert!(battlestation::len(geo::sub(l.p, [0.0, 1.0, -0.3])) < 2.0);
        }
        assert!(steam(3.0).iter().all(|s| s.c[3] >= 0.0 && s.c[3] <= 0.1));
    }
}
