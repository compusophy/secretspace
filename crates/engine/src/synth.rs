//! Sound from numbers: a game's sounds are written as a few voices
//! (oscillators sweeping in pitch, noise through a sweeping filter), each
//! shaped by an envelope, mixed and made loud enough. Nothing recorded,
//! nothing loaded: the page builds its sounds when it starts and plays
//! them (`kit::audio`). std only, the same on every machine.

use std::f32::consts::TAU;

/// Samples a second.
pub const RATE: u32 = 22050;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wave {
    Sine,
    Triangle,
    Square,
    Saw,
}

/// How loud a voice is over its life: a rise of `attack` seconds, then a
/// fall that halves every `half` seconds, at `gain` at its height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Env {
    pub gain: f32,
    pub attack: f32,
    pub half: f32,
}

impl Env {
    pub const fn new(gain: f32, attack: f32, half: f32) -> Env {
        Env { gain, attack, half }
    }

    fn at(&self, t: f32) -> f32 {
        if t < self.attack {
            self.gain * t / self.attack.max(1e-6)
        } else {
            self.gain * (-(t - self.attack) / self.half.max(1e-6) * std::f32::consts::LN_2).exp()
        }
    }
}

/// A sound being written.
pub struct Synth {
    pub out: Vec<f32>,
    seed: u32,
}

fn sweep(a: f32, b: f32, k: f32) -> f32 {
    // Pitch moves evenly in octaves, not in hertz.
    a * (b / a.max(1e-3)).powf(k)
}

impl Synth {
    /// `secs` of silence to write into.
    pub fn new(secs: f32) -> Synth {
        Synth {
            out: vec![0.0; (secs * RATE as f32) as usize],
            seed: 0x9e37_79b9,
        }
    }

    fn span(&self, at: f32, len: f32) -> (usize, usize) {
        let a = ((at * RATE as f32) as usize).min(self.out.len());
        let b = (((at + len) * RATE as f32) as usize).min(self.out.len());
        (a, b)
    }

    /// A tone from `at` for `len` seconds, its pitch sweeping from `f.0` to
    /// `f.1` hertz, wobbling by `vib` (rate in hertz, depth as a share).
    pub fn tone(
        &mut self,
        wave: Wave,
        f: (f32, f32),
        (at, len): (f32, f32),
        env: Env,
        vib: (f32, f32),
    ) -> &mut Synth {
        let (a, b) = self.span(at, len);
        let mut phase = 0.0f32;
        for i in a..b {
            let t = (i - a) as f32 / RATE as f32;
            let k = t / len.max(1e-6);
            let hz = sweep(f.0, f.1, k) * (1.0 + vib.1 * (TAU * vib.0 * t).sin());
            phase = (phase + hz / RATE as f32).fract();
            let v = match wave {
                Wave::Sine => (TAU * phase).sin(),
                Wave::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
                Wave::Square => {
                    if phase < 0.5 {
                        0.7
                    } else {
                        -0.7
                    }
                }
                Wave::Saw => (2.0 * phase - 1.0) * 0.8,
            };
            self.out[i] += v * env.at(t);
        }
        self
    }

    /// Noise from `at` for `len` seconds, kept between `lo` and `hi`
    /// hertz as they sweep from their first to their second; `grain`
    /// (0 to 1) breaks it into crackles.
    pub fn noise(
        &mut self,
        (at, len): (f32, f32),
        env: Env,
        lo: (f32, f32),
        hi: (f32, f32),
        grain: f32,
    ) -> &mut Synth {
        let (a, b) = self.span(at, len);
        let (mut low, mut high) = (0.0f32, 0.0f32);
        let mut gate = 1.0f32;
        for i in a..b {
            let t = (i - a) as f32 / RATE as f32;
            let k = t / len.max(1e-6);
            self.seed ^= self.seed << 13;
            self.seed ^= self.seed >> 17;
            self.seed ^= self.seed << 5;
            let white = (self.seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
            // Two one-pole filters: what is under `hi`, less what is
            // under `lo`.
            let pole = |hz: f32| 1.0 - (-TAU * hz / RATE as f32).exp();
            high += (white - high) * pole(sweep(hi.0, hi.1, k));
            low += (high - low) * pole(sweep(lo.0, lo.1, k));
            if grain > 0.0 && i % (RATE as usize / 400) == 0 {
                gate = if (self.seed >> 8) as f32 / (1u32 << 24) as f32 > grain {
                    1.0
                } else {
                    0.1
                };
            }
            self.out[i] += (high - low) * 2.0 * env.at(t) * gate;
        }
        self
    }

    /// The finished sound to play round and round, its loudest at `peak`:
    /// its last `overlap` seconds faded into its first, so the turn from
    /// end to start cannot be heard (it is that much shorter).
    pub fn looped(self, peak: f32, overlap: f32) -> Vec<f32> {
        let mut v = self.out;
        let top = v.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        if top > 1e-6 {
            let k = peak / top;
            for x in &mut v {
                *x *= k;
            }
        }
        let n = v.len();
        let o = ((overlap * RATE as f32) as usize).min(n / 2);
        for i in 0..o {
            // Equal power: noise keeps its loudness through the fade.
            let k = i as f32 / o as f32;
            let (a, b) = (
                (k * std::f32::consts::FRAC_PI_2).sin(),
                (k * std::f32::consts::FRAC_PI_2).cos(),
            );
            v[i] = v[i] * a + v[n - o + i] * b;
        }
        v.truncate(n - o);
        v
    }

    /// The finished sound, its loudest at `peak`.
    pub fn done(mut self, peak: f32) -> Vec<f32> {
        let top = self.out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        if top > 1e-6 {
            let k = peak / top;
            for v in &mut self.out {
                *v *= k;
            }
        }
        // No click at the end.
        let n = self.out.len();
        let fade = (RATE as usize / 200).min(n);
        for (j, v) in self.out[n - fade..].iter_mut().enumerate() {
            *v *= 1.0 - j as f32 / fade as f32;
        }
        self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_loop_turns_from_its_end_to_its_start_without_a_step() {
        let mut s = Synth::new(2.0);
        s.noise(
            (0.0, 2.0),
            Env::new(0.8, 0.0, 1e6),
            (200.0, 200.0),
            (900.0, 900.0),
            0.0,
        );
        let v = s.looped(0.5, 0.5);
        assert_eq!(v.len(), (1.5 * RATE as f32) as usize);
        let rms = (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
        assert!(rms > 0.05, "{rms}");
        // From its end to its start no bigger a step than within it.
        let most = v
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max);
        assert!((v[0] - v[v.len() - 1]).abs() <= most * 1.01);
    }

    #[test]
    fn a_tone_has_its_pitch_and_dies_away() {
        let mut s = Synth::new(0.5);
        s.tone(
            Wave::Sine,
            (440.0, 440.0),
            (0.0, 0.5),
            Env::new(1.0, 0.01, 0.05),
            (0.0, 0.0),
        );
        let out = s.done(0.9);
        assert_eq!(out.len(), (0.5 * RATE as f32) as usize);
        // Zero crossings in the first tenth: about 2 * 440 / 10.
        let tenth = &out[..RATE as usize / 10];
        let crossings = tenth
            .windows(2)
            .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
            .count();
        assert!((40..=48).contains(&crossings), "{crossings}");
        let loud = |p: &[f32]| p.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(loud(&out[..2000]) > 0.8);
        assert!(loud(&out[out.len() - 2000..]) < 0.01, "and it fades");
    }

    #[test]
    fn noise_keeps_to_its_band_and_is_the_same_every_time() {
        let make = || {
            let mut s = Synth::new(0.3);
            s.noise(
                (0.0, 0.3),
                Env::new(1.0, 0.0, 1.0),
                (500.0, 500.0),
                (2000.0, 2000.0),
                0.0,
            );
            s.done(0.9)
        };
        let a = make();
        assert_eq!(a, make());
        assert!(a.iter().all(|v| v.is_finite() && v.abs() <= 0.9 + 1e-6));
    }
}
