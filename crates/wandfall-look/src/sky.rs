//! The island's sky at its hour. The island's day turns an hour with
//! every lobby (dawn, a golden afternoon, dusk, a moonlit night, dawn
//! again); the range stays at dusk. `Sky` turns it over a few seconds when
//! the hour changes, and into the storm's violet and out again as you
//! step through its wall.

use render::geo::{self, rgb, V3};
use render::{Grade, Look};
use wandfall::laws::SEA;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hour {
    Dawn,
    Day,
    Dusk,
    Night,
}

/// How long the sky takes to turn to a new hour, and to close in as you
/// step into the storm (s).
const TURN: f32 = 6.0;
const INTO_STORM: f32 = 0.7;

impl Hour {
    /// In the order of the day (`laws::HOURS` of them).
    pub const ALL: [Hour; 4] = [Hour::Dawn, Hour::Day, Hour::Dusk, Hour::Night];

    /// The hour a frame names.
    pub fn from(n: u8) -> Hour {
        Hour::ALL[n as usize % Hour::ALL.len()]
    }

    pub fn name(self) -> &'static str {
        match self {
            Hour::Dawn => "dawn",
            Hour::Day => "day",
            Hour::Dusk => "dusk",
            Hour::Night => "night",
        }
    }

    pub fn named(s: &str) -> Option<Hour> {
        Hour::ALL.into_iter().find(|h| h.name() == s)
    }

    /// The hour's clear sky.
    fn clear(self) -> Look {
        match self {
            Hour::Dusk => dusk(),
            Hour::Dawn => dawn(),
            Hour::Day => day(),
            Hour::Night => night(),
        }
    }

    /// How bright the storm's violet is at this hour.
    fn storm(self) -> f32 {
        match self {
            Hour::Day => 1.25,
            Hour::Night => 0.45,
            _ => 1.0,
        }
    }
}

/// The sky as it turns: the hour it is turning from and to, since when
/// (s), and how far into the storm's look.
#[derive(Clone, Debug, Default)]
pub struct Sky {
    turn: Option<(Hour, Hour, f64)>,
    storm: f32,
    last: f64,
}

impl Sky {
    /// The sky now (`now` in ms): turning to `hour` if that is new (at
    /// once, the first time), closed in if you stand in the storm.
    pub fn look(&mut self, hour: Hour, in_storm: bool, now: f64) -> Look {
        let s = now / 1000.0;
        let (from, to, since) = match self.turn {
            Some((_, to, _)) if to != hour => (to, hour, s),
            Some(t) => t,
            None => (hour, hour, s),
        };
        self.turn = Some((from, to, since));
        let dt = (s - self.last).clamp(0.0, 0.25) as f32;
        self.last = s;
        let aim = if in_storm { 1.0 } else { 0.0 };
        let step = dt / INTO_STORM;
        self.storm += (aim - self.storm).clamp(-step, step);
        let t = smooth(((s - since) as f32 / TURN).clamp(0.0, 1.0));
        let clear = blend(&from.clear(), &to.clear(), t);
        let k = from.storm() + (to.storm() - from.storm()) * t;
        blend(&clear, &storm(&clear, k), smooth(self.storm))
    }
}

/// The sky at `hour`, at once; violet and close in the storm.
pub fn sky(hour: Hour, in_storm: bool) -> Look {
    let clear = hour.clear();
    if in_storm {
        storm(&clear, hour.storm())
    } else {
        clear
    }
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// A clear sky closed in by the storm, its violet `k` bright.
fn storm(clear: &Look, k: f32) -> Look {
    Look {
        sky: geo::scale([0.30, 0.18, 0.44], k),
        zenith: geo::scale([0.14, 0.05, 0.26], k),
        horizon: geo::scale([0.42, 0.20, 0.58], k),
        fog: 0.05,
        fog_falloff: 0.002,
        clouds: 0.9,
        grade: Grade {
            saturation: 0.85,
            contrast: 1.2,
            lift: [0.02, 0.0, 0.04],
            ..clear.grade
        },
        ..*clear
    }
}

/// One sky `t` of the way to another.
pub fn blend(a: &Look, b: &Look, t: f32) -> Look {
    if t <= 0.0 {
        return *a;
    }
    if t >= 1.0 {
        return *b;
    }
    let f = |x: f32, y: f32| x + (y - x) * t;
    let v = |x: V3, y: V3| [f(x[0], y[0]), f(x[1], y[1]), f(x[2], y[2])];
    Look {
        sun_dir: geo::norm(v(a.sun_dir, b.sun_dir)),
        sun: v(a.sun, b.sun),
        sun_size: f(a.sun_size, b.sun_size),
        sky: v(a.sky, b.sky),
        low: v(a.low, b.low),
        zenith: v(a.zenith, b.zenith),
        horizon: v(a.horizon, b.horizon),
        deep: v(a.deep, b.deep),
        // The air thickens as a power, so blend it as one.
        fog: a.fog.max(1e-6) * (b.fog.max(1e-6) / a.fog.max(1e-6)).powf(t),
        fog_falloff: f(a.fog_falloff, b.fog_falloff),
        clouds: f(a.clouds, b.clouds),
        stars: f(a.stars, b.stars),
        exposure: f(a.exposure, b.exposure),
        bloom: f(a.bloom, b.bloom),
        vignette: f(a.vignette, b.vignette),
        sea: if t < 0.5 { a.sea } else { b.sea },
        water: v(a.water, b.water),
        waves: f(a.waves, b.waves),
        wind: [f(a.wind[0], b.wind[0]), f(a.wind[1], b.wind[1])],
        grade: Grade {
            lift: v(a.grade.lift, b.grade.lift),
            gamma: v(a.grade.gamma, b.grade.gamma),
            gain: v(a.grade.gain, b.grade.gain),
            saturation: f(a.grade.saturation, b.grade.saturation),
            contrast: f(a.grade.contrast, b.grade.contrast),
        },
    }
}

/// Dusk on a wizard's island: a low amber sun under a violet sky, the
/// first stars out, so lamps, crystals and lava read.
fn dusk() -> Look {
    Look {
        sun_dir: geo::norm([0.6, 0.26, 0.3]),
        sun: [2.2, 1.45, 0.95],
        sun_size: 0.04,
        sky: [0.15, 0.15, 0.29],
        low: [0.08, 0.065, 0.06],
        zenith: [0.05, 0.07, 0.22],
        horizon: [0.42, 0.29, 0.45],
        deep: [0.05, 0.06, 0.10],
        fog: 0.0016,
        fog_falloff: 0.015,
        clouds: 0.55,
        stars: 0.25,
        exposure: 0.85,
        bloom: 0.07,
        vignette: 0.36,
        sea: Some(SEA),
        water: rgb(10, 40, 66),
        waves: 0.4,
        wind: [0.9, 0.35],
        grade: GRADE,
    }
}

/// Dawn: a rose-gold sun just up over the other shore, a pale sky, mist
/// lying low on the ground, the last stars going.
fn dawn() -> Look {
    Look {
        sun_dir: geo::norm([-0.55, 0.2, -0.5]),
        sun: [2.3, 1.5, 1.2],
        sun_size: 0.045,
        sky: [0.19, 0.19, 0.29],
        low: [0.09, 0.075, 0.075],
        zenith: [0.09, 0.13, 0.30],
        horizon: [0.78, 0.50, 0.50],
        deep: [0.07, 0.07, 0.10],
        fog: 0.0034,
        fog_falloff: 0.035,
        clouds: 0.4,
        stars: 0.08,
        exposure: 0.82,
        bloom: 0.08,
        vignette: 0.34,
        water: rgb(20, 50, 74),
        waves: 0.3,
        wind: [-0.5, 0.2],
        grade: Grade {
            lift: [0.012, 0.01, 0.03],
            gain: [1.04, 0.99, 0.97],
            saturation: 1.06,
            ..GRADE
        },
        ..dusk()
    }
}

/// A golden afternoon: the sun high and warm, a blue sky, long clear
/// views; the brightest hour, so the least exposure.
fn day() -> Look {
    Look {
        sun_dir: geo::norm([0.45, 0.62, 0.4]),
        sun: [3.1, 2.7, 2.15],
        sun_size: 0.035,
        sky: [0.30, 0.35, 0.50],
        low: [0.12, 0.11, 0.09],
        zenith: [0.10, 0.23, 0.58],
        horizon: [0.58, 0.64, 0.78],
        deep: [0.08, 0.10, 0.14],
        fog: 0.0012,
        fog_falloff: 0.012,
        clouds: 0.5,
        stars: 0.0,
        exposure: 0.66,
        bloom: 0.05,
        vignette: 0.3,
        water: rgb(14, 60, 88),
        waves: 0.45,
        wind: [1.0, 0.45],
        grade: Grade {
            lift: [0.0, 0.006, 0.016],
            gain: [1.03, 1.0, 0.96],
            saturation: 1.08,
            contrast: 1.08,
            ..GRADE
        },
        ..dusk()
    }
}

/// A moonlit night: a cold moon high over the sea, the sky full of
/// stars; lamps, runes and every spell burn bright against it.
fn night() -> Look {
    Look {
        sun_dir: geo::norm([-0.35, 0.5, 0.55]),
        sun: MOON,
        sun_size: 0.035,
        sky: [0.06, 0.08, 0.18],
        low: [0.03, 0.03, 0.045],
        zenith: [0.008, 0.012, 0.04],
        horizon: [0.05, 0.06, 0.13],
        deep: [0.01, 0.012, 0.025],
        fog: 0.0018,
        fog_falloff: 0.015,
        clouds: 0.3,
        stars: 1.0,
        exposure: 1.45,
        bloom: 0.1,
        vignette: 0.4,
        water: rgb(6, 20, 38),
        waves: 0.35,
        wind: [0.6, -0.3],
        grade: Grade {
            lift: [0.0, 0.01, 0.035],
            gamma: [1.0, 1.0, 1.03],
            gain: [0.98, 1.0, 1.04],
            saturation: 0.95,
            contrast: 1.1,
        },
        ..dusk()
    }
}

/// Moonlight: cold and dim.
const MOON: V3 = [0.34, 0.42, 0.68];

/// The island's grade: shadows a little cool (toward teal and violet),
/// light a little warm, colour a touch richer, and a gentle S-curve.
const GRADE: Grade = Grade {
    lift: [0.0, 0.012, 0.03],
    gamma: [1.0, 1.0, 1.02],
    gain: [1.03, 1.0, 0.95],
    saturation: 1.1,
    contrast: 1.12,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_hour_is_lit_and_named() {
        assert_eq!(Hour::ALL.len(), wandfall::laws::HOURS as usize);
        assert_eq!(
            Hour::from(wandfall::laws::RANGE_HOUR),
            Hour::Dusk,
            "the range is at dusk"
        );
        for h in Hour::ALL {
            assert_eq!(Hour::named(h.name()), Some(h));
            assert!(sky(h, false).sun_dir[1] > 0.1, "{h:?}: the light is up");
        }
    }

    #[test]
    fn the_sky_turns_and_settles() {
        let mut s = Sky::default();
        let night = s.look(Hour::Night, false, 1000.0);
        assert_eq!(night, sky(Hour::Night, false), "at once, the first time");
        let start = s.look(Hour::Dawn, false, 2000.0);
        assert_eq!(start, night, "it turns from where it was");
        let half = s.look(Hour::Dawn, false, 2000.0 + TURN as f64 * 500.0);
        assert!(half.exposure < night.exposure && half.exposure > sky(Hour::Dawn, false).exposure);
        let mut last = half;
        for k in 0..200 {
            last = s.look(
                Hour::Dawn,
                k > 100,
                2000.0 + TURN as f64 * 1000.0 + k as f64 * 50.0,
            );
        }
        assert_eq!(last, sky(Hour::Dawn, true), "settled, in the storm");
    }
}
