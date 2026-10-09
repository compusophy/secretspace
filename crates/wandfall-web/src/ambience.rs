//! The island's sound under everything, going round and round: wind (more
//! of it high up and on a broom), the rift's rumble and crackle near it,
//! the Spire's humming chord near its beacon, the storm's roar as its
//! wall comes near (and inside it). Written by `engine::synth` at start,
//! each eased up and down as you move (`kit::audio`'s hums).

use engine::synth::{Env, Synth, Wave};
use kit::audio::Audio;
use render::V3;

/// Steady: no rise, no fall.
const HELD: Env = Env::new(1.0, 0.0, 1e6);

fn wind() -> Vec<f32> {
    let secs = 9.0;
    let mut s = Synth::new(secs);
    s.noise((0.0, secs), HELD, (180.0, 260.0), (700.0, 1100.0), 0.0)
        .noise(
            (0.0, secs),
            Env::new(0.35, 0.0, 1e6),
            (900.0, 700.0),
            (2600.0, 2000.0),
            0.0,
        );
    // Gusts: the loudness rising and falling, unevenly.
    for (k, v) in s.out.iter_mut().enumerate() {
        let t = k as f32 / engine::synth::RATE as f32;
        let tau = std::f32::consts::TAU;
        let g = 0.45
            + 0.35 * (0.5 + 0.5 * (tau * t / 3.1).sin())
            + 0.2 * (0.5 + 0.5 * (tau * t / 1.7 + 1.0).sin());
        *v *= g;
    }
    s.looped(0.5, 1.5)
}

fn rumble() -> Vec<f32> {
    let secs = 7.0;
    let mut s = Synth::new(secs);
    s.noise((0.0, secs), HELD, (25.0, 30.0), (110.0, 140.0), 0.0)
        .tone(
            Wave::Sine,
            (41.0, 41.0),
            (0.0, secs),
            Env::new(0.5, 0.0, 1e6),
            (0.25, 0.04),
        )
        .noise(
            (0.0, secs),
            Env::new(0.25, 0.0, 1e6),
            (500.0, 500.0),
            (1800.0, 1800.0),
            0.75,
        );
    s.looped(0.6, 1.0)
}

fn chord() -> Vec<f32> {
    let secs = 8.0;
    let mut s = Synth::new(secs);
    for (hz, gain) in [
        (220.0, 0.5),
        (277.2, 0.35),
        (329.6, 0.35),
        (440.0, 0.2),
        (659.3, 0.08),
    ] {
        s.tone(
            Wave::Sine,
            (hz, hz),
            (0.0, secs),
            Env::new(gain, 0.0, 1e6),
            (0.2, 0.004),
        );
    }
    s.noise(
        (0.0, secs),
        Env::new(0.06, 0.0, 1e6),
        (3000.0, 3000.0),
        (7000.0, 7000.0),
        0.0,
    );
    s.looped(0.4, 1.5)
}

fn roar() -> Vec<f32> {
    let secs = 7.0;
    let mut s = Synth::new(secs);
    s.noise((0.0, secs), HELD, (50.0, 60.0), (420.0, 380.0), 0.0)
        .tone(
            Wave::Saw,
            (55.0, 55.0),
            (0.0, secs),
            Env::new(0.12, 0.0, 1e6),
            (0.4, 0.03),
        )
        .noise(
            (0.0, secs),
            Env::new(0.4, 0.0, 1e6),
            (1200.0, 1200.0),
            (4500.0, 4500.0),
            0.85,
        );
    s.looped(0.6, 1.0)
}

/// Where you are, for the island's sound: the ear (where, facing which
/// way), how high over the ground, on a broom, how fast; the rift and the
/// Spire's beacon; the storm's circle if it stands (centre, radius).
pub struct Here {
    pub ear: (V3, f32),
    pub over: f32,
    pub glide: bool,
    pub speed: f32,
    pub rift: Option<V3>,
    pub spire: Option<V3>,
    pub storm: Option<([f32; 2], f32)>,
}

pub struct Ambience {
    wind: usize,
    rift: usize,
    spire: usize,
    storm: usize,
}

impl Ambience {
    pub fn new(audio: &mut Audio) -> Ambience {
        let mut hum = |v: Vec<f32>| {
            let id = audio.add(&v, engine::synth::RATE);
            audio.hum(id)
        };
        Ambience {
            wind: hum(wind()),
            rift: hum(rumble()),
            spire: hum(chord()),
            storm: hum(roar()),
        }
    }

    /// Each eased to where you are (silent when `quiet`: the title).
    pub fn tune(&self, audio: &Audio, h: &Here, quiet: bool) {
        let k = if quiet { 0.0 } else { 1.0 };
        let (e, yaw) = h.ear;
        // How loud something `reach` metres across is from here, and from
        // which side.
        let near = |p: V3, reach: f32| {
            let (dx, dz) = (p[0] - e[0], p[2] - e[2]);
            let d = (dx * dx + dz * dz + (p[1] - e[1]).powi(2)).sqrt();
            let v = (1.0 - d / reach).max(0.0);
            let (s, c) = yaw.sin_cos();
            let pan = if d > 0.5 {
                (dx * -s + dz * c) / d * 0.7
            } else {
                0.0
            };
            (v * v, pan)
        };
        let wind = 0.1
            + (h.over / 30.0).clamp(0.0, 0.3)
            + if h.glide { 0.25 } else { 0.0 }
            + (h.speed / 14.0).clamp(0.0, 1.0) * 0.12;
        audio.tune(self.wind, wind * k, 0.0);
        let (rv, rp) = h.rift.map_or((0.0, 0.0), |p| near(p, 70.0));
        audio.tune(self.rift, 0.9 * rv * k, rp);
        let (sv, sp) = h.spire.map_or((0.0, 0.0), |p| near(p, 55.0));
        audio.tune(self.spire, 0.35 * sv * k, sp);
        let roar = h.storm.map_or(0.0, |(c, r)| {
            let d = (e[0] - c[0]).hypot(e[2] - c[1]);
            if d > r {
                0.9
            } else {
                let v = (1.0 - (r - d) / 45.0).max(0.0);
                0.8 * v * v
            }
        });
        audio.tune(self.storm, roar * k, 0.0);
    }
}
