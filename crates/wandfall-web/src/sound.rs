//! What Wandfall sounds like. Every sound is written by `engine::synth`
//! when the page starts (nothing is loaded), one for each spell's cast
//! and one for its landing, in the spirit of its look: a whoosh and a
//! boom for fire, a zap for the Lance, glass for Frost, a rising hum and
//! a crack of thunder for Lightning, a vwip for Blink, a shimmer (and
//! shattering glass) for the Ward, a chime for Mend, wind for Gust. Then
//! the wand, your hits, being hurt, a knockout, a level, a chest, a
//! scroll. Each is heard from where it happened: quieter far off, to the
//! left or the right.

use engine::synth::{Env, Synth, Wave, RATE};
use kit::audio::Audio;
use render::V3;
use wandfall::laws::spell;
use wandfall::proto::{Ev, Frame, Loot};
use wandfall::world::WAND;

use Wave::{Saw, Sine, Square, Triangle};

const STILL: (f32, f32) = (0.0, 0.0);

/// Every sound, by its number in `audio`.
pub struct Sounds {
    pub audio: Audio,
    wand: usize,
    cast: [usize; 8],
    land: [usize; 8],
    mark: usize,
    shatter: usize,
    hit: usize,
    hurt: usize,
    out: usize,
    level: usize,
    chest: usize,
    take: usize,
    hop: usize,
    thud: usize,
}

fn wand() -> Vec<f32> {
    let mut s = Synth::new(0.14);
    s.tone(
        Square,
        (1400.0, 520.0),
        (0.0, 0.12),
        Env::new(0.5, 0.002, 0.03),
        STILL,
    )
    .tone(
        Sine,
        (2600.0, 1300.0),
        (0.0, 0.08),
        Env::new(0.3, 0.001, 0.02),
        STILL,
    )
    .noise(
        (0.0, 0.05),
        Env::new(0.25, 0.001, 0.015),
        (2000.0, 2000.0),
        (8000.0, 8000.0),
        0.0,
    );
    s.done(0.45)
}

fn cast(sp: u8) -> Vec<f32> {
    match sp {
        spell::FIREBALL => {
            let mut s = Synth::new(0.5);
            s.noise(
                (0.0, 0.5),
                Env::new(1.0, 0.04, 0.12),
                (200.0, 500.0),
                (900.0, 2500.0),
                0.0,
            )
            .tone(
                Saw,
                (110.0, 70.0),
                (0.0, 0.4),
                Env::new(0.4, 0.02, 0.1),
                (18.0, 0.05),
            );
            s.done(0.6)
        }
        spell::LANCE => {
            let mut s = Synth::new(0.45);
            s.tone(
                Saw,
                (2400.0, 700.0),
                (0.0, 0.3),
                Env::new(0.6, 0.002, 0.06),
                (60.0, 0.02),
            )
            .tone(
                Sine,
                (3200.0, 3000.0),
                (0.0, 0.4),
                Env::new(0.4, 0.001, 0.1),
                STILL,
            )
            .noise(
                (0.0, 0.2),
                Env::new(0.4, 0.001, 0.04),
                (3000.0, 1500.0),
                (9000.0, 6000.0),
                0.0,
            );
            s.done(0.6)
        }
        spell::FROST => {
            let mut s = Synth::new(0.5);
            s.noise(
                (0.0, 0.25),
                Env::new(0.5, 0.005, 0.05),
                (3000.0, 2000.0),
                (10000.0, 8000.0),
                0.0,
            );
            for (k, f) in [2600.0, 3500.0, 3100.0, 4200.0, 3800.0, 4900.0]
                .into_iter()
                .enumerate()
            {
                s.tone(
                    Sine,
                    (f, f * 0.98),
                    (k as f32 * 0.03, 0.3),
                    Env::new(0.35, 0.001, 0.05),
                    STILL,
                );
            }
            s.done(0.5)
        }
        spell::LIGHTNING => {
            let mut s = Synth::new(0.25);
            s.noise(
                (0.0, 0.25),
                Env::new(0.6, 0.005, 0.05),
                (1500.0, 1500.0),
                (7000.0, 7000.0),
                0.5,
            )
            .tone(
                Square,
                (900.0, 1800.0),
                (0.0, 0.15),
                Env::new(0.3, 0.002, 0.04),
                STILL,
            );
            s.done(0.4)
        }
        spell::BLINK => {
            let mut s = Synth::new(0.3);
            s.tone(
                Sine,
                (300.0, 2400.0),
                (0.0, 0.16),
                Env::new(0.6, 0.005, 0.06),
                STILL,
            )
            .tone(
                Triangle,
                (2400.0, 900.0),
                (0.12, 0.15),
                Env::new(0.4, 0.002, 0.05),
                STILL,
            )
            .noise(
                (0.0, 0.2),
                Env::new(0.25, 0.01, 0.05),
                (2000.0, 4000.0),
                (6000.0, 10000.0),
                0.0,
            );
            s.done(0.55)
        }
        spell::WARD => {
            let mut s = Synth::new(0.8);
            s.tone(
                Sine,
                (523.0, 523.0),
                (0.0, 0.8),
                Env::new(0.4, 0.04, 0.2),
                (6.0, 0.01),
            )
            .tone(
                Sine,
                (784.0, 784.0),
                (0.0, 0.8),
                Env::new(0.3, 0.06, 0.2),
                (5.0, 0.01),
            )
            .tone(
                Sine,
                (1046.0, 1046.0),
                (0.03, 0.7),
                Env::new(0.2, 0.05, 0.15),
                (7.0, 0.01),
            )
            .noise(
                (0.0, 0.3),
                Env::new(0.15, 0.05, 0.1),
                (3000.0, 3000.0),
                (9000.0, 9000.0),
                0.0,
            );
            s.done(0.5)
        }
        spell::MEND => {
            let mut s = Synth::new(0.9);
            for (k, f) in [659.0, 880.0, 1109.0, 1318.0].into_iter().enumerate() {
                let at = k as f32 * 0.06;
                s.tone(
                    Sine,
                    (f, f),
                    (at, 0.8),
                    Env::new(0.4, 0.004, 0.18),
                    (5.0, 0.004),
                )
                .tone(
                    Triangle,
                    (f / 2.0, f / 2.0),
                    (at, 0.5),
                    Env::new(0.15, 0.004, 0.1),
                    STILL,
                );
            }
            s.done(0.5)
        }
        spell::GUST => {
            let mut s = Synth::new(0.75);
            s.noise(
                (0.0, 0.75),
                Env::new(1.0, 0.08, 0.2),
                (150.0, 400.0),
                (700.0, 2200.0),
                0.0,
            )
            .noise(
                (0.0, 0.5),
                Env::new(0.4, 0.05, 0.12),
                (1000.0, 2500.0),
                (3000.0, 6000.0),
                0.0,
            );
            s.done(0.6)
        }
        _ => wand(),
    }
}

fn land(sp: u8) -> Vec<f32> {
    match sp {
        spell::FIREBALL => {
            let mut s = Synth::new(1.0);
            s.noise(
                (0.0, 1.0),
                Env::new(1.0, 0.003, 0.12),
                (40.0, 30.0),
                (3000.0, 300.0),
                0.15,
            )
            .tone(
                Sine,
                (110.0, 38.0),
                (0.0, 0.6),
                Env::new(1.0, 0.004, 0.15),
                STILL,
            );
            s.done(0.9)
        }
        spell::LANCE => {
            let mut s = Synth::new(0.25);
            s.noise(
                (0.0, 0.25),
                Env::new(1.0, 0.002, 0.05),
                (2500.0, 1500.0),
                (9000.0, 5000.0),
                0.3,
            );
            s.done(0.35)
        }
        spell::FROST => {
            let mut s = Synth::new(0.15);
            s.noise(
                (0.0, 0.12),
                Env::new(1.0, 0.001, 0.02),
                (1500.0, 1500.0),
                (9000.0, 9000.0),
                0.3,
            )
            .tone(
                Sine,
                (4200.0, 3900.0),
                (0.0, 0.12),
                Env::new(0.4, 0.001, 0.03),
                STILL,
            );
            s.done(0.3)
        }
        spell::LIGHTNING => {
            let mut s = Synth::new(1.5);
            s.noise(
                (0.0, 0.12),
                Env::new(1.0, 0.001, 0.03),
                (500.0, 500.0),
                (12000.0, 12000.0),
                0.5,
            )
            .noise(
                (0.02, 1.4),
                Env::new(0.9, 0.02, 0.35),
                (30.0, 25.0),
                (900.0, 150.0),
                0.2,
            )
            .tone(
                Sine,
                (70.0, 35.0),
                (0.0, 0.8),
                Env::new(0.6, 0.01, 0.25),
                STILL,
            );
            s.done(1.0)
        }
        spell::BLINK => {
            let mut s = Synth::new(0.15);
            s.tone(
                Sine,
                (900.0, 1800.0),
                (0.0, 0.12),
                Env::new(0.5, 0.002, 0.03),
                STILL,
            );
            s.done(0.35)
        }
        _ => {
            let mut s = Synth::new(0.1);
            s.tone(
                Sine,
                (1000.0, 1000.0),
                (0.0, 0.05),
                Env::new(0.3, 0.002, 0.01),
                STILL,
            );
            s.done(0.2)
        }
    }
}

fn mark() -> Vec<f32> {
    let mut s = Synth::new(0.85);
    s.tone(
        Saw,
        (70.0, 140.0),
        (0.0, 0.85),
        Env::new(0.5, 0.6, 0.3),
        (9.0, 0.08),
    )
    .noise(
        (0.0, 0.85),
        Env::new(0.3, 0.7, 0.2),
        (800.0, 1500.0),
        (2000.0, 5000.0),
        0.4,
    );
    s.done(0.45)
}

fn shatter() -> Vec<f32> {
    let mut s = Synth::new(0.6);
    s.noise(
        (0.0, 0.5),
        Env::new(1.0, 0.001, 0.08),
        (2000.0, 3000.0),
        (12000.0, 9000.0),
        0.5,
    );
    for (k, f) in [3100.0, 4400.0, 5200.0, 3700.0].into_iter().enumerate() {
        s.tone(
            Sine,
            (f, f * 0.97),
            (k as f32 * 0.02, 0.3),
            Env::new(0.3, 0.001, 0.04),
            STILL,
        );
    }
    s.done(0.55)
}

fn hit() -> Vec<f32> {
    let mut s = Synth::new(0.08);
    s.tone(
        Sine,
        (1800.0, 1500.0),
        (0.0, 0.06),
        Env::new(0.6, 0.001, 0.015),
        STILL,
    )
    .noise(
        (0.0, 0.03),
        Env::new(0.3, 0.001, 0.008),
        (3000.0, 3000.0),
        (9000.0, 9000.0),
        0.0,
    );
    s.done(0.4)
}

fn hurt() -> Vec<f32> {
    let mut s = Synth::new(0.22);
    s.tone(
        Sine,
        (180.0, 60.0),
        (0.0, 0.2),
        Env::new(1.0, 0.002, 0.05),
        STILL,
    )
    .noise(
        (0.0, 0.15),
        Env::new(0.6, 0.002, 0.03),
        (80.0, 80.0),
        (1500.0, 400.0),
        0.0,
    );
    s.done(0.6)
}

fn knockout() -> Vec<f32> {
    let mut s = Synth::new(1.4);
    s.tone(
        Sine,
        (196.0, 194.0),
        (0.0, 1.4),
        Env::new(0.7, 0.003, 0.4),
        STILL,
    )
    .tone(
        Sine,
        (294.0, 292.0),
        (0.0, 1.2),
        Env::new(0.4, 0.003, 0.3),
        STILL,
    )
    .tone(
        Sine,
        (523.0, 520.0),
        (0.0, 0.9),
        Env::new(0.3, 0.003, 0.2),
        STILL,
    )
    .tone(
        Triangle,
        (98.0, 97.0),
        (0.0, 1.0),
        Env::new(0.4, 0.005, 0.3),
        STILL,
    );
    s.done(0.6)
}

fn level() -> Vec<f32> {
    let mut s = Synth::new(1.0);
    for (k, f) in [523.0, 659.0, 784.0, 1046.0].into_iter().enumerate() {
        let at = k as f32 * 0.07;
        let half = if k == 3 { 0.25 } else { 0.08 };
        s.tone(
            Triangle,
            (f, f),
            (at, 0.9 - at),
            Env::new(0.5, 0.005, half),
            (6.0, 0.006),
        )
        .tone(
            Sine,
            (f * 2.0, f * 2.0),
            (at, 0.5),
            Env::new(0.2, 0.005, half * 0.6),
            STILL,
        );
    }
    s.done(0.55)
}

fn chest() -> Vec<f32> {
    let mut s = Synth::new(0.7);
    s.tone(
        Triangle,
        (440.0, 880.0),
        (0.0, 0.15),
        Env::new(0.4, 0.005, 0.05),
        STILL,
    );
    for (k, f) in [1760.0, 2349.0, 2637.0, 3520.0].into_iter().enumerate() {
        s.tone(
            Sine,
            (f, f),
            (0.08 + k as f32 * 0.05, 0.5),
            Env::new(0.3, 0.002, 0.08),
            STILL,
        );
    }
    s.done(0.45)
}

fn hop() -> Vec<f32> {
    let mut s = Synth::new(0.22);
    s.noise(
        (0.0, 0.2),
        Env::new(0.8, 0.01, 0.05),
        (500.0, 900.0),
        (1800.0, 3200.0),
        0.0,
    )
    .tone(
        Sine,
        (220.0, 420.0),
        (0.0, 0.12),
        Env::new(0.3, 0.005, 0.04),
        STILL,
    );
    s.done(0.35)
}

fn thud() -> Vec<f32> {
    let mut s = Synth::new(0.3);
    s.tone(
        Sine,
        (130.0, 45.0),
        (0.0, 0.2),
        Env::new(1.0, 0.002, 0.05),
        STILL,
    )
    .noise(
        (0.0, 0.18),
        Env::new(0.7, 0.002, 0.04),
        (60.0, 60.0),
        (900.0, 300.0),
        0.2,
    );
    s.done(0.6)
}

fn take() -> Vec<f32> {
    let mut s = Synth::new(0.35);
    s.tone(
        Sine,
        (660.0, 1320.0),
        (0.0, 0.12),
        Env::new(0.5, 0.003, 0.04),
        STILL,
    )
    .tone(
        Sine,
        (1320.0, 1320.0),
        (0.08, 0.25),
        Env::new(0.3, 0.002, 0.06),
        STILL,
    );
    s.done(0.45)
}

impl Default for Sounds {
    fn default() -> Sounds {
        Sounds::new()
    }
}

impl Sounds {
    pub fn new() -> Sounds {
        let mut audio = Audio::new();
        audio.muted = kit::load("wandfall.muted").as_deref() == Some("1");
        let mut add = |v: Vec<f32>| audio.add(&v, RATE);
        let wand = add(wand());
        let order = [
            spell::FIREBALL,
            spell::LANCE,
            spell::FROST,
            spell::LIGHTNING,
            spell::BLINK,
            spell::WARD,
            spell::MEND,
            spell::GUST,
        ];
        let mut cast_ = [0; 8];
        let mut land_ = [0; 8];
        for sp in order {
            cast_[sp as usize] = add(cast(sp));
            land_[sp as usize] = add(land(sp));
        }
        Sounds {
            wand,
            cast: cast_,
            land: land_,
            mark: add(mark()),
            shatter: add(shatter()),
            hit: add(hit()),
            hurt: add(hurt()),
            out: add(knockout()),
            level: add(level()),
            chest: add(chest()),
            take: add(take()),
            hop: add(hop()),
            thud: add(thud()),
            audio,
        }
    }

    pub fn mute(&mut self, on: bool) {
        self.audio.muted = on;
        kit::save("wandfall.muted", if on { "1" } else { "0" });
    }

    fn at(&self, id: usize, gain: f32, p: V3, ear: (V3, f32), speed: f32) {
        let (e, yaw) = ear;
        let (dx, dz) = (p[0] - e[0], p[2] - e[2]);
        let d = (dx * dx + dz * dz + (p[1] - e[1]).powi(2)).sqrt();
        if d > 90.0 {
            return;
        }
        let near = 1.0 / (1.0 + d / 10.0).powf(1.3);
        let (s, c) = yaw.sin_cos();
        let pan = if d > 0.5 {
            (dx * -s + dz * c) / d * 0.8
        } else {
            0.0
        };
        self.audio.play(id, gain * near, pan, speed);
    }

    /// Your own wand, or someone's at `p` (heard from `ear`: where you
    /// are and which way you face).
    pub fn wand(&self, p: Option<V3>, ear: (V3, f32), jitter: f32) {
        let speed = 1.0 + (jitter - 0.5) * 0.12;
        match p {
            None => self.audio.play(self.wand, 0.5, 0.15, speed),
            Some(p) => self.at(self.wand, 0.45, p, ear, speed * 0.9),
        }
    }

    /// A spell cast: yours (`None`), or someone's at `p`.
    pub fn cast(&self, sp: u8, p: Option<V3>, ear: (V3, f32)) {
        let Some(&id) = self.cast.get(sp as usize) else {
            return;
        };
        match p {
            None => self.audio.play(id, 0.8, 0.0, 1.0),
            Some(p) => self.at(id, 0.9, p, ear, 1.0),
        }
    }

    pub fn land(&self, sp: u8, p: V3, ear: (V3, f32)) {
        if let Some(&id) = self.land.get(sp as usize) {
            self.at(id, 1.0, p, ear, 1.0);
        }
    }

    /// Lightning's warning; a ward shattering.
    pub fn mark(&self, p: V3, ear: (V3, f32)) {
        self.at(self.mark, 1.0, p, ear, 1.0);
    }

    pub fn shatter(&self, p: V3, ear: (V3, f32)) {
        self.at(self.shatter, 1.0, p, ear, 1.0);
    }

    /// Your hit landed (higher for a bigger one).
    pub fn hit(&self, amount: u16) {
        let up = (amount as f32 / 40.0).min(1.0);
        self.audio.play(self.hit, 0.6, 0.0, 0.9 + 0.35 * up);
    }

    pub fn hurt(&self) {
        self.audio.play(self.hurt, 0.8, 0.0, 1.0);
    }

    pub fn knockout(&self) {
        self.audio.play(self.out, 0.8, 0.0, 1.0);
    }

    pub fn level(&self) {
        self.audio.play(self.level, 0.8, 0.0, 1.0);
    }

    pub fn chest(&self, p: V3, ear: (V3, f32)) {
        self.at(self.chest, 1.0, p, ear, 1.0);
    }

    pub fn take(&self) {
        self.audio.play(self.take, 0.7, 0.0, 1.0);
    }

    /// Your feet leave the ground; meet it again, `hard` (0 to 1) hard.
    pub fn hop(&self) {
        self.audio.play(self.hop, 0.45, 0.0, 1.0);
    }

    pub fn thud(&self, hard: f32) {
        self.audio
            .play(self.thud, 0.3 + 0.7 * hard, 0.0, 1.1 - 0.25 * hard);
    }

    /// What these events sound like, heard by `me` (`alive`: your own
    /// casts were heard as you cast them) from `ear`.
    pub fn events(&self, list: &[Ev], me: u16, alive: bool, ear: (V3, f32)) {
        let mine = |by: u16| by == me && alive;
        let mut shards = 0;
        for e in list {
            match *e {
                Ev::Cast {
                    by,
                    spell: sp,
                    stage: 0,
                    at,
                } if !mine(by) && sp != spell::LANCE => self.cast(sp, Some(at), ear),
                Ev::Cast {
                    spell: spell::FROST,
                    stage: 1,
                    at,
                    ..
                } => {
                    if shards < 2 {
                        self.land(spell::FROST, at, ear);
                        shards += 1;
                    }
                }
                Ev::Cast {
                    spell: sp,
                    stage: 1,
                    at,
                    ..
                } => self.land(sp, at, ear),
                Ev::Cast {
                    spell: spell::LIGHTNING,
                    stage: 2,
                    at,
                    ..
                } => self.mark(at, ear),
                Ev::Cast {
                    spell: spell::WARD,
                    stage: 2,
                    at,
                    ..
                } => self.shatter(at, ear),
                Ev::Beam { by, from, to, .. } => {
                    if !mine(by) {
                        self.cast(spell::LANCE, Some(from), ear);
                    }
                    self.land(spell::LANCE, to, ear);
                }
                Ev::Hit { by, to, amount, .. } => {
                    if by == me && to != me {
                        self.hit(amount);
                    }
                    if to == me {
                        self.hurt();
                    }
                }
                Ev::Out { who, by, .. } if by == me && who != me => self.knockout(),
                Ev::Level { who, .. } if who == me => self.level(),
                _ => {}
            }
        }
    }

    /// A new frame: others' wands newly fired (a few at most), and a
    /// scroll you took.
    pub fn frame(&self, old: Option<&Frame>, f: &Frame, me: u16, ear: (V3, f32)) {
        let mut n = 0;
        for b in f.bolts.iter().filter(|b| b.kind == WAND && b.by != me) {
            if n < 3 && !old.is_some_and(|o| o.bolts.iter().any(|x| x.id == b.id)) {
                self.wand(Some(b.p), ear, (b.id % 7) as f32 / 7.0);
                n += 1;
            }
        }
        if let (Some(a), Some(b)) = (old.and_then(|o| o.you.as_ref()), f.you.as_ref()) {
            let more = (0..4).any(|k| match (a.slots[k], b.slots[k]) {
                (None, Some(_)) => true,
                (Some(x), Some(y)) => y.0 != x.0 || y.1 > x.1,
                _ => false,
            });
            if more {
                self.take();
            }
        }
    }

    /// The loot changed: a chest opened is heard where it stands.
    pub fn loot(&self, old: &Loot, new: &Loot, ear: (V3, f32)) {
        for &(id, at, open) in &new.chests {
            let was = old.chests.iter().find(|c| c.0 == id);
            if open && was.is_some_and(|c| !c.2) {
                self.chest(at, ear);
            }
        }
    }
}
