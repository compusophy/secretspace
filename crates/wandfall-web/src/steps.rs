//! Footsteps: each wizard's feet heard as they come down, in time with
//! the stride it is drawn with, on grass, sand, stone or in the shallows;
//! louder at a sprint, hushed crouching (so creeping up is quiet). Yours
//! are soft and close; others' come from where they are, out to `REACH`.

use std::collections::HashMap;

use engine::synth::{Env, Synth, Wave, RATE};
use kit::audio::Audio;
use render::V3;
use wandfall::laws::SEA;
use wandfall::map::Map;
use wandfall::places::Place;
use wandfall::proto::Seen;

use crate::rig::Anim;
use crate::sound;

/// How far off footsteps are heard (m).
const REACH: f32 = 40.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    Grass,
    Sand,
    Stone,
    Water,
}

/// What is underfoot at `p`.
pub fn ground(map: &Map, p: V3) -> Ground {
    if p[1] < SEA + 0.12 {
        return Ground::Water;
    }
    let under = map.height(p[0], p[2]);
    let paved = map.pois.iter().any(|q| {
        matches!(q.place, Place::Spire | Place::Circle)
            && (p[0] - q.x).hypot(p[2] - q.z) < q.r * 0.45
    });
    if p[1] - under > 0.3 || paved {
        Ground::Stone
    } else if under < SEA + 1.0 {
        Ground::Sand
    } else {
        Ground::Grass
    }
}

fn grass() -> Vec<f32> {
    let mut s = Synth::new(0.16);
    s.noise(
        (0.0, 0.12),
        Env::new(0.9, 0.006, 0.03),
        (700.0, 500.0),
        (4200.0, 3000.0),
        0.55,
    )
    .tone(
        Wave::Sine,
        (110.0, 70.0),
        (0.0, 0.06),
        Env::new(0.35, 0.002, 0.02),
        (0.0, 0.0),
    );
    s.done(0.5)
}

fn sand() -> Vec<f32> {
    let mut s = Synth::new(0.17);
    s.noise(
        (0.0, 0.14),
        Env::new(0.8, 0.008, 0.035),
        (300.0, 250.0),
        (2200.0, 1600.0),
        0.8,
    )
    .tone(
        Wave::Sine,
        (90.0, 60.0),
        (0.0, 0.06),
        Env::new(0.3, 0.002, 0.02),
        (0.0, 0.0),
    );
    s.done(0.45)
}

fn stone() -> Vec<f32> {
    let mut s = Synth::new(0.1);
    s.noise(
        (0.0, 0.05),
        Env::new(0.9, 0.001, 0.01),
        (1200.0, 1000.0),
        (6000.0, 4000.0),
        0.0,
    )
    .tone(
        Wave::Triangle,
        (240.0, 170.0),
        (0.0, 0.05),
        Env::new(0.45, 0.001, 0.012),
        (0.0, 0.0),
    )
    .tone(
        Wave::Sine,
        (95.0, 70.0),
        (0.0, 0.07),
        Env::new(0.4, 0.001, 0.02),
        (0.0, 0.0),
    );
    s.done(0.5)
}

fn water() -> Vec<f32> {
    let mut s = Synth::new(0.26);
    s.noise(
        (0.0, 0.22),
        Env::new(0.8, 0.01, 0.05),
        (400.0, 900.0),
        (2400.0, 1600.0),
        0.35,
    )
    .tone(
        Wave::Sine,
        (280.0, 620.0),
        (0.01, 0.08),
        Env::new(0.25, 0.005, 0.025),
        (0.0, 0.0),
    );
    s.done(0.5)
}

/// The footstep sounds, by their numbers in `audio`.
pub struct Steps {
    ids: [usize; 4],
}

impl Steps {
    pub fn new(audio: &mut Audio) -> Steps {
        let mut add = |v: Vec<f32>| audio.add(&v, RATE);
        Steps {
            ids: [add(grass()), add(sand()), add(stone()), add(water())],
        }
    }

    /// Every foot that came down this frame (`anims` stepped for
    /// `wizards`), heard from `ear`; `you` (alive) heard close.
    pub fn hear(
        &self,
        audio: &Audio,
        map: &Map,
        (wizards, anims): (&[Seen], &HashMap<u16, Anim>),
        you: Option<u16>,
        ear: (V3, f32),
    ) {
        for s in wizards {
            let Some(a) = anims.get(&s.id).filter(|a| a.footfall) else {
                continue;
            };
            let mine = you == Some(s.id);
            if !mine && geo_far(s.p, ear.0) {
                continue;
            }
            let id = self.ids[ground(map, s.p) as usize];
            let loud = (0.45 + 0.55 * (a.speed / 9.0).min(1.0)) * (1.0 - 0.65 * a.crouch);
            // Each step a little different.
            let jitter = ((a.phase * 997.0 + s.id as f32 * 13.7).sin() * 0.5 + 0.5) * 0.2;
            let speed = 0.9 + jitter;
            if mine {
                audio.play(id, 0.3 * loud, 0.0, speed);
            } else {
                sound::heard(audio, id, 0.7 * loud, s.p, ear, speed);
            }
        }
    }
}

fn geo_far(p: V3, e: V3) -> bool {
    let d = [p[0] - e[0], p[1] - e[1], p[2] - e[2]];
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2] > REACH * REACH
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_know_what_is_underfoot() {
        let map = Map::new(0x5eed_0007);
        let spire = map.pois.iter().find(|q| q.place == Place::Spire).unwrap();
        let on = |x: f32, z: f32| ground(&map, [x, map.height(x, z), z]);
        assert_eq!(on(spire.x + 2.0, spire.z), Ground::Stone);
        // Walking out from the middle: grass, then sand, then the sea.
        let mut seen = Vec::new();
        for k in 0..400 {
            let x = 30.0 + k as f32 * 0.5;
            let g = on(x, 30.0);
            if seen.last() != Some(&g) {
                seen.push(g);
            }
        }
        let at = |g| seen.iter().position(|&s| s == g);
        let (lawn, beach, sea) = (at(Ground::Grass), at(Ground::Sand), at(Ground::Water));
        assert!(lawn < beach && beach < sea && lawn.is_some(), "{seen:?}");
        for (name, v) in [
            ("grass", grass()),
            ("sand", sand()),
            ("stone", stone()),
            ("water", water()),
        ] {
            let top = v.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(
                top > 0.3 && top <= 0.5 && v.len() < RATE as usize / 2,
                "{name}: {top}"
            );
        }
    }
}
