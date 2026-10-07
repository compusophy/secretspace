//! The arena's regulars: bots that graze, dodge bodies and the edge, and,
//! if they are bold enough, cut across a smaller snake's path.

use crate::laws::ARENA;
use crate::world::World;
use engine::rng::Rng;

pub struct Bot {
    /// 0..=255: how readily it hunts other snakes.
    pub nerve: u8,
}

pub const NAMES: &[&str] = &[
    "noodle",
    "zigzag",
    "sir slithers",
    "loopy",
    "spaghetti",
    "mamba",
    "wiggles",
    "danger noodle",
    "hissy fit",
    "sssam",
    "linguine",
    "boa",
    "kaa",
    "slinky",
    "squiggle",
    "nagini",
    "jormungandr",
    "ouroboros",
    "spinach",
    "pickle",
    "neon",
    "bean",
    "comet",
    "glowworm",
    "orbit",
    "pixel",
    "gecko",
    "ramen",
    "tofu",
    "yoyo",
    "mochi",
    "zappa",
    "rizzo",
    "taco",
    "pretzel",
    "ziggy",
];

/// A new bot, and a name no one in the arena has.
pub fn recruit(rng: &mut Rng, snakes: &[crate::world::Snake]) -> (String, Bot) {
    let start = rng.below(NAMES.len() as u64) as usize;
    let name = (0..NAMES.len())
        .map(|k| NAMES[(start + k) % NAMES.len()])
        .find(|n| !snakes.iter().any(|s| s.name == *n))
        .map_or_else(
            || format!("{} {}", NAMES[start], rng.below(90) + 10),
            str::to_string,
        );
    (
        name,
        Bot {
            nerve: rng.below(256) as u8,
        },
    )
}

fn wrap(a: f32) -> f32 {
    let mut a = a % std::f32::consts::TAU;
    if a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    if a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

/// Where bot `i` heads this tick, and whether it boosts.
pub fn think(w: &World, i: usize) -> (f32, bool) {
    let s = &w.snakes[i];
    let nerve = s.bot.as_ref().map_or(0, |b| b.nerve);
    let mut rng = Rng::new(((w.tick as u64) << 16) ^ s.id as u64 ^ 0x5eed_b075);
    let (hx, hy) = s.head();
    let r = s.radius();
    let (fc, fs) = (s.angle.cos(), s.angle.sin());

    // Meander when nothing better calls.
    let t = w.tick as f32 * 0.035 + s.id as f32 * 1.7;
    let mut want = s.angle + 0.45 * t.sin() * (0.6 * t).cos();
    let mut boost = false;

    // The best food in reach, favouring what is ahead.
    let mut best = 0.0;
    for f in w.food_near(hx, hy, 360.0) {
        let (dx, dy) = (f.x - hx, f.y - hy);
        let d = (dx * dx + dy * dy).sqrt().max(1.0);
        let ahead = (dx * fc + dy * fs) / d;
        let score = f.value as f32 / (d + 40.0) * (1.6 + ahead);
        if score > best {
            best = score;
            want = dy.atan2(dx);
            boost = f.value >= 6 && d < 240.0 && s.mass > 60.0 && rng.chance(1, 3);
        }
    }

    // Bold bots cut across the path of a smaller snake nearby.
    if nerve > 200 && s.mass > 80.0 {
        let prey = w
            .snakes
            .iter()
            .filter(|o| o.id != s.id && o.mass < s.mass * 0.8 && !o.ghost(w.tick))
            .map(|o| {
                let (ox, oy) = o.head();
                let ahead = (
                    ox + o.angle.cos() * (60.0 + o.radius() * 3.0),
                    oy + o.angle.sin() * (60.0 + o.radius() * 3.0),
                );
                let d = ((ahead.0 - hx).powi(2) + (ahead.1 - hy).powi(2)).sqrt();
                (d, ahead)
            })
            .filter(|(d, _)| *d < 320.0)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((d, (ax, ay))) = prey {
            want = (ay - hy).atan2(ax - hx);
            boost = d < 220.0 && s.mass > 60.0;
        }
    }

    // Keep off the edge.
    let from_centre = (hx * hx + hy * hy).sqrt();
    if from_centre > ARENA * 0.82 {
        let home = (-hy).atan2(-hx);
        let k = ((from_centre - ARENA * 0.82) / (ARENA * 0.12)).min(1.0);
        want += wrap(home - want) * k;
    }

    // Never steer into anyone: fan out from the wish until a way is clear.
    let look = 90.0 + r * 5.0 + if boost { 90.0 } else { 0.0 };
    let mut best_dir = (want, w.clearance(i, (hx, hy), want, look));
    if best_dir.1 >= look {
        return (want, boost);
    }
    for k in 1..=9 {
        for sign in [-1.0, 1.0] {
            let a = want + sign * k as f32 * 0.33;
            let c = w.clearance(i, (hx, hy), a, look);
            if c >= look {
                return (a, false);
            }
            if c > best_dir.1 {
                best_dir = (a, c);
            }
        }
    }
    (best_dir.0, false)
}
