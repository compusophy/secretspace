//! The desk typing by itself, before anyone sits down: commands typed
//! key by key (Shift held for capitals), a pause to read, the mouse
//! wandering and clicking, then the next. The page feeds what it does to
//! the hands and the terminal as if it were you.

use crate::keys::{find, for_char, Side};

/// Something the demo does.
#[derive(Clone, Debug, PartialEq)]
pub enum Ev {
    /// A key (`code`) down or up, and what it types (`KeyboardEvent.key`).
    Key {
        code: &'static str,
        key: String,
        down: bool,
    },
    /// The mouse moved (pixels).
    Move { dx: f32, dy: f32 },
    /// A mouse button.
    Button { b: i16, down: bool },
}

const SCRIPT: [&str; 8] = [
    "neofetch",
    "ls",
    "cat notes.txt",
    "fortune",
    "echo Hello from the desk",
    "cowsay battlestation",
    "uptime",
    "clear",
];

pub struct Demo {
    queue: Vec<(f32, Ev)>,
    at: usize,
    time: f32,
    line: usize,
    seed: u32,
}

impl Demo {
    pub fn new(seed: u32) -> Demo {
        let mut d = Demo {
            queue: Vec::new(),
            at: 0,
            time: 0.0,
            line: 0,
            seed: seed | 1,
        };
        d.plan(0.8);
        d
    }

    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    fn key(&mut self, t: f32, code: &'static str, key: &str, hold: f32) {
        self.queue.push((
            t,
            Ev::Key {
                code,
                key: key.to_string(),
                down: true,
            },
        ));
        self.queue.push((
            t + hold,
            Ev::Key {
                code,
                key: key.to_string(),
                down: false,
            },
        ));
    }

    /// The next command, starting `wait` seconds from now.
    fn plan(&mut self, wait: f32) {
        self.queue.clear();
        self.at = 0;
        let text = SCRIPT[self.line % SCRIPT.len()];
        self.line += 1;
        let mut t = self.time + wait;
        for c in text.chars() {
            let Some((code, shift)) = for_char(c) else {
                continue;
            };
            let hold = 0.05 + self.rand() * 0.05;
            if shift {
                // The other hand's Shift.
                let right = find(code).is_some_and(|k| k.finger.side == Side::Right);
                let s = if right { "ShiftLeft" } else { "ShiftRight" };
                self.key(t - 0.06, s, "Shift", hold + 0.12);
            }
            self.key(t, code, &c.to_string(), hold);
            t += 0.07 + self.rand() * 0.13 + if c == ' ' { 0.08 } else { 0.0 };
        }
        t += 0.25;
        self.key(t, "Enter", "Enter", 0.07);
        // Read, then wander with the mouse and click.
        t += 1.2 + self.rand() * 1.5;
        let steps = 40;
        let (ax, ay) = (self.rand() * 2.0 - 1.0, self.rand() * 2.0 - 1.0);
        for i in 0..steps {
            let a = i as f32 / steps as f32 * std::f32::consts::TAU;
            self.queue.push((
                t + i as f32 * 0.03,
                Ev::Move {
                    dx: (a.cos() + ax) * 5.0,
                    dy: (a.sin() * 0.6 + ay) * 4.0,
                },
            ));
        }
        t += steps as f32 * 0.03 + 0.15;
        self.queue.push((t, Ev::Button { b: 0, down: true }));
        self.queue
            .push((t + 0.09, Ev::Button { b: 0, down: false }));
        self.queue.sort_by(|a, b| a.0.total_cmp(&b.0));
    }

    /// What happens in the next `dt` seconds.
    pub fn step(&mut self, dt: f32) -> Vec<Ev> {
        self.time += dt.clamp(0.0, 0.5);
        let mut out = Vec::new();
        while let Some((t, ev)) = self.queue.get(self.at) {
            if *t > self.time {
                break;
            }
            out.push(ev.clone());
            self.at += 1;
        }
        if self.at >= self.queue.len() {
            let wait = 0.6 + self.rand();
            self.plan(wait);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_down_comes_up_and_the_script_goes_round() {
        let mut d = Demo::new(7);
        let mut down = std::collections::HashMap::new();
        let mut typed = String::new();
        for _ in 0..(120 * 60) {
            for ev in d.step(1.0 / 60.0) {
                if let Ev::Key {
                    code,
                    key,
                    down: is,
                } = ev
                {
                    *down.entry(code).or_insert(0) += if is { 1 } else { -1 };
                    if is && key.chars().count() == 1 {
                        typed.push_str(&key);
                    }
                }
            }
        }
        assert!(typed.contains("neofetch") && typed.contains("Hello from the desk"));
        assert!(down.values().all(|n| *n == 0 || *n == 1), "{down:?}");
    }
}
