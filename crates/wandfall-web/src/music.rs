//! The title's and the lobby's music, written at start by `engine::synth`
//! and going round and round: four chords in D minor held soft (D minor,
//! B flat, F, C), a harp-like arpeggio rippling through each, an octave
//! below, and a few bell notes over the second half. Quiet under the
//! title and the lobby; gone in a match and on the range.

use engine::synth::{Env, Synth, Wave};
use kit::audio::Audio;

/// Beats a minute.
const BPM: f32 = 84.0;
/// The chords: their notes (MIDI), two bars each.
const CHORDS: [[u8; 3]; 4] = [[50, 53, 57], [46, 50, 53], [41, 45, 48], [48, 52, 55]];
/// The bells over bars five to eight: (beat, note, beats long).
const BELLS: [(f32, u8, f32); 8] = [
    (16.0, 74, 1.5),
    (17.5, 72, 0.5),
    (18.0, 69, 2.0),
    (22.0, 70, 2.0),
    (24.0, 72, 1.5),
    (25.5, 69, 0.5),
    (26.0, 65, 2.0),
    (29.0, 67, 3.0),
];
/// How long the turn from the end back to the start is (s).
const OVERLAP: f32 = 1.5;

fn hz(m: u8) -> f32 {
    440.0 * 2f32.powf((m as f32 - 69.0) / 12.0)
}

/// The theme, eight bars, to play round and round.
pub fn theme() -> Vec<f32> {
    let beat = 60.0 / BPM;
    let secs = 32.0 * beat + OVERLAP;
    let mut s = Synth::new(secs);
    for (c, notes) in CHORDS.iter().enumerate() {
        let at = c as f32 * 8.0 * beat;
        // The chord held, swelling in and slowly fading.
        for &m in notes {
            s.tone(
                Wave::Sine,
                (hz(m), hz(m)),
                (at, 8.0 * beat + 1.0),
                Env::new(0.16, 1.0, 2.5),
                (0.3, 0.003),
            );
        }
        s.tone(
            Wave::Triangle,
            (hz(notes[0] - 12), hz(notes[0] - 12)),
            (at, 8.0 * beat + 0.6),
            Env::new(0.14, 0.4, 3.0),
            (0.0, 0.0),
        );
        // The arpeggio, in eighths, up and back over two octaves.
        for k in 0..16 {
            let up = [0, 1, 2, 3, 4, 3, 2, 1][k % 8];
            let m = notes[up % 3] + 12 + 12 * (up / 3) as u8;
            s.tone(
                Wave::Triangle,
                (hz(m), hz(m)),
                (at + k as f32 * beat / 2.0, 0.9),
                Env::new(0.1, 0.004, 0.22),
                (5.0, 0.002),
            );
        }
    }
    for (b, m, len) in BELLS {
        let at = b * beat;
        s.tone(
            Wave::Sine,
            (hz(m), hz(m)),
            (at, len * beat + 1.2),
            Env::new(0.12, 0.01, 0.7),
            (5.5, 0.003),
        )
        .tone(
            Wave::Sine,
            (hz(m) * 2.0, hz(m) * 2.0),
            (at, 0.6),
            Env::new(0.03, 0.005, 0.2),
            (0.0, 0.0),
        );
    }
    s.looped(0.42, OVERLAP)
}

pub struct Music {
    hum: usize,
}

impl Music {
    pub fn new(audio: &mut Audio) -> Music {
        let id = audio.add(&theme(), engine::synth::RATE);
        Music { hum: audio.hum(id) }
    }

    /// As loud as `level` (0: gone), eased.
    pub fn tune(&self, audio: &Audio, level: f32) {
        audio.tune(self.hum, level, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_theme_is_eight_bars_and_never_clips() {
        let v = theme();
        let bars = 32.0 * 60.0 / BPM;
        let secs = v.len() as f32 / engine::synth::RATE as f32;
        assert!((secs - bars).abs() < 0.01, "{secs} s, {bars} s");
        let top = v.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        let rms = (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
        assert!(top <= 0.43 && rms > 0.02, "top {top}, rms {rms}");
    }
}
