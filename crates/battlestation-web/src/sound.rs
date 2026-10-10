//! The desk's sounds, written from numbers (`engine::synth`): a key's
//! clack (four, so a run of typing is never one sound again and again),
//! the deeper thock of the long keys, a key coming back up, the mouse's
//! click, and the tower's fans humming under it all.

use engine::synth::{Env, Synth, Wave};

/// A key's clack, its pitch `k` (about 1).
pub fn clack(k: f32) -> Vec<f32> {
    let mut s = Synth::new(0.09);
    s.noise(
        (0.0, 0.03),
        Env::new(1.0, 0.0006, 0.005),
        (2200.0 * k, 1800.0 * k),
        (9000.0, 6000.0),
        0.0,
    );
    s.tone(
        Wave::Sine,
        (420.0 * k, 260.0 * k),
        (0.0, 0.05),
        Env::new(0.45, 0.001, 0.009),
        (0.0, 0.0),
    );
    s.noise(
        (0.003, 0.05),
        Env::new(0.5, 0.002, 0.012),
        (220.0, 180.0),
        (1500.0 * k, 900.0 * k),
        0.0,
    );
    s.done(0.8)
}

/// A long key (space, enter, shift, backspace): lower, rounder.
pub fn thock() -> Vec<f32> {
    let mut s = Synth::new(0.14);
    s.noise(
        (0.0, 0.04),
        Env::new(0.8, 0.0008, 0.006),
        (1400.0, 1000.0),
        (6000.0, 4000.0),
        0.0,
    );
    s.tone(
        Wave::Sine,
        (190.0, 120.0),
        (0.0, 0.1),
        Env::new(0.7, 0.002, 0.02),
        (0.0, 0.0),
    );
    s.noise(
        (0.006, 0.08),
        Env::new(0.45, 0.003, 0.02),
        (120.0, 90.0),
        (800.0, 500.0),
        0.0,
    );
    s.done(0.85)
}

/// A key let go: a light tick.
pub fn up() -> Vec<f32> {
    let mut s = Synth::new(0.04);
    s.noise(
        (0.0, 0.03),
        Env::new(0.6, 0.0005, 0.004),
        (3000.0, 2500.0),
        (9000.0, 7000.0),
        0.0,
    );
    s.done(0.5)
}

/// The mouse's button.
pub fn click() -> Vec<f32> {
    let mut s = Synth::new(0.04);
    s.noise(
        (0.0, 0.02),
        Env::new(1.0, 0.0004, 0.003),
        (3500.0, 3000.0),
        (11000.0, 9000.0),
        0.0,
    );
    s.tone(
        Wave::Triangle,
        (2400.0, 1900.0),
        (0.0, 0.012),
        Env::new(0.3, 0.0005, 0.003),
        (0.0, 0.0),
    );
    s.done(0.6)
}

/// The tower's fans: a low airy hum to loop.
pub fn fans() -> Vec<f32> {
    let mut s = Synth::new(3.0);
    s.noise(
        (0.0, 3.0),
        Env::new(1.0, 0.01, 1e3),
        (60.0, 60.0),
        (420.0, 420.0),
        0.0,
    );
    s.tone(
        Wave::Sine,
        (118.0, 118.0),
        (0.0, 3.0),
        Env::new(0.12, 0.01, 1e3),
        (0.3, 0.01),
    );
    s.looped(0.6, 0.25)
}

/// Which sound a key makes going down.
pub fn long_key(code: &str) -> bool {
    matches!(
        code,
        "Space" | "Enter" | "Backspace" | "ShiftLeft" | "ShiftRight" | "CapsLock" | "Tab"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sound_is_short_and_bounded() {
        let all = [clack(0.9), clack(1.1), thock(), up(), click(), fans()];
        for s in &all {
            assert!(!s.is_empty());
            assert!(s.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
            assert!(s.iter().any(|x| x.abs() > 0.1), "audible");
        }
        assert!(all[0].len() < 22050 / 5, "a clack is short");
    }
}
