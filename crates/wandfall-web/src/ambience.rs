//! The island's sound under everything, going round and round: wind (more
//! of it high up and on a broom), the rift's rumble and crackle near it,
//! the Spire's humming chord near its beacon, the storm's roar as its
//! wall comes near (and inside it); crickets at night, birds at dawn and
//! through the afternoon, both hushed high up and in the storm. Written
//! by `engine::synth` at start, each eased up and down as you move
//! (`kit::audio`'s hums).

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

/// A little number from a few, the same every time.
fn rnd(a: u32, b: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9e37_79b9) ^ b.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h & 0xffff) as f32 / 65535.0
}

/// Crickets in the grass: a few of them, each chirping its own pulses at
/// its own pitch and pace.
fn crickets() -> Vec<f32> {
    let secs = 9.0;
    let mut s = Synth::new(secs);
    for c in 0..6u32 {
        let hz = 3900.0 + 1300.0 * rnd(c, 1);
        let every = 0.55 + 0.6 * rnd(c, 2);
        let pulses = 2 + (rnd(c, 3) * 3.0) as u32;
        let gain = 0.25 + 0.35 * rnd(c, 4);
        let mut t = rnd(c, 5) * every;
        let mut n = 0;
        while t < secs - 0.2 {
            for p in 0..pulses {
                s.tone(
                    Wave::Sine,
                    (hz, hz * 0.985),
                    (t + p as f32 * 0.028, 0.02),
                    Env::new(gain, 0.004, 0.008),
                    (0.0, 0.0),
                );
            }
            n += 1;
            t += every * (0.9 + 0.2 * rnd(c, 10 + n));
        }
    }
    // The grass hissing under them.
    s.noise(
        (0.0, secs),
        Env::new(0.04, 0.0, 1e6),
        (2500.0, 2500.0),
        (6000.0, 6000.0),
        0.0,
    );
    s.looped(0.5, 1.0)
}

/// Birds: now and then a phrase of quick whistled notes, each bird its
/// own pitch and song.
fn birds() -> Vec<f32> {
    let secs = 11.0;
    let mut s = Synth::new(secs);
    let mut t = 0.3;
    let mut k = 0u32;
    while t < secs - 1.0 {
        let bird = (rnd(k, 1) * 4.0) as u32;
        let base = 2200.0 + 1400.0 * rnd(bird, 2);
        let notes = 3 + (rnd(k, 3) * 5.0) as u32;
        let gain = 0.2 + 0.3 * rnd(k, 4);
        for n in 0..notes {
            let up = rnd(bird * 31 + n, 5);
            let (a, b) = if n % 2 == 0 {
                (base * (0.9 + 0.3 * up), base * (1.2 + 0.3 * up))
            } else {
                (base * (1.25 + 0.2 * up), base * 0.95)
            };
            s.tone(
                Wave::Sine,
                (a, b),
                (t + n as f32 * 0.09, 0.07),
                Env::new(gain, 0.01, 0.03),
                (28.0, 0.02),
            );
        }
        k += 1;
        t += 0.5 + notes as f32 * 0.09 + 1.6 * rnd(k, 6);
    }
    s.looped(0.45, 0.8)
}

/// Rain: a hiss of many drops, and nearer ones pattering through it.
fn rain() -> Vec<f32> {
    let secs = 8.0;
    let mut s = Synth::new(secs);
    s.noise((0.0, secs), HELD, (1800.0, 1800.0), (7500.0, 7500.0), 0.0)
        .noise(
            (0.0, secs),
            Env::new(0.7, 0.0, 1e6),
            (900.0, 900.0),
            (5000.0, 5000.0),
            0.92,
        )
        .noise(
            (0.0, secs),
            Env::new(0.35, 0.0, 1e6),
            (120.0, 120.0),
            (600.0, 600.0),
            0.0,
        );
    s.looped(0.45, 1.0)
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
    /// How much of the night there is (crickets), and of the morning and
    /// the afternoon (birds), 0 to 1.
    pub night: f32,
    pub birds: f32,
    /// How hard it is raining, 0 to 1.
    pub rain: f32,
}

pub struct Ambience {
    wind: usize,
    rift: usize,
    spire: usize,
    storm: usize,
    crickets: usize,
    birds: usize,
    rain: usize,
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
            crickets: hum(crickets()),
            birds: hum(birds()),
            rain: hum(rain()),
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
        // The island's own sounds, close to the ground and out of the
        // storm.
        let calm = (1.0 - h.over / 25.0).clamp(0.0, 1.0) * (1.0 - roar).max(0.0);
        audio.tune(self.crickets, 0.3 * h.night * calm * k, 0.0);
        audio.tune(self.birds, 0.22 * h.birds * calm * k, 0.0);
        // Rain all round, a little less inside the storm's roar.
        audio.tune(self.rain, 0.5 * h.rain * (1.0 - 0.5 * roar) * k, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_island_sounds_are_there_and_never_clip() {
        for (name, v) in [
            ("wind", wind()),
            ("rumble", rumble()),
            ("chord", chord()),
            ("roar", roar()),
            ("crickets", crickets()),
            ("birds", birds()),
            ("rain", rain()),
        ] {
            let top = v.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            let rms = (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
            assert!(top <= 0.6 && rms > 0.005, "{name}: top {top}, rms {rms}");
            assert!(
                v.len() > engine::synth::RATE as usize * 5,
                "{name}: long enough to loop"
            );
        }
    }
}
