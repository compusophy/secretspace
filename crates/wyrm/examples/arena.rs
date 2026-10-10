//! Runs bot arenas with no one watching and prints how they went: how big
//! snakes get, how often they die, how they boost. For tuning `laws.rs`.
//! cargo run -p secretspace-wyrm --release --example arena -- [minutes] [seeds]

use std::collections::HashMap;

use wyrm::laws::TICK_HZ;
use wyrm::world::World;

fn main() {
    let mut args = std::env::args().skip(1);
    let minutes: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(20);
    let seeds: u64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(4);
    let ticks = minutes * 60 * TICK_HZ;
    let (mut deaths, mut edge) = (0usize, 0usize);
    let mut biggest = Vec::new();
    let mut means = Vec::new();
    // Boost runs by length in ticks, and boosting ticks of big snakes.
    let mut runs: HashMap<u32, u32> = HashMap::new();
    let (mut big_ticks, mut big_boost) = (0u64, 0u64);
    let mut quiet_bursts = 0usize;
    let mut angle = 0.0f32;
    for seed in 1..=seeds {
        let mut w = World::new(seed);
        let mut run: HashMap<u16, u32> = HashMap::new();
        let (mut top, mut sum, mut n) = (0.0f32, 0.0f64, 0u64);
        for _ in 0..ticks {
            let dead = w.step();
            deaths += dead.len();
            edge += dead.iter().filter(|d| d.killer.is_none()).count();
            quiet_bursts += w.bursts.len().saturating_sub(dead.len());
            for s in &w.snakes {
                top = top.max(s.mass);
                sum += s.mass as f64;
                n += 1;
                angle = angle.max(s.angle.abs());
                if s.mass > 1000.0 {
                    big_ticks += 1;
                    big_boost += s.boosting as u64;
                }
                let r = run.entry(s.id).or_default();
                if s.boosting {
                    *r += 1;
                } else if *r > 0 {
                    *runs.entry((*r).min(10)).or_default() += 1;
                    *r = 0;
                }
            }
            run.retain(|id, _| w.find(*id).is_some());
        }
        biggest.push(top);
        means.push(sum / n.max(1) as f64);
    }
    let per_min = deaths as f64 / (minutes as f64 * seeds as f64);
    println!("{seeds} arenas of {minutes} min");
    println!("deaths a minute: {per_min:.1} ({edge} at the edge of {deaths})");
    println!("biggest each arena: {biggest:.0?}");
    println!("mean mass each arena: {means:.0?}");
    let total: u32 = runs.values().sum();
    let short = runs.get(&1).copied().unwrap_or(0) + runs.get(&2).copied().unwrap_or(0);
    println!("boost runs: {total}, of 1-2 ticks: {short}");
    let mut lens: Vec<_> = runs.into_iter().collect();
    lens.sort();
    println!("  by length (10 = 10 or more): {lens:?}");
    let share = big_boost as f64 / big_ticks.max(1) as f64 * 100.0;
    println!("snakes over 1000 boosting: {share:.1}% of their ticks");
    println!("bursts with no death (bots stepping out): {quiet_bursts}");
    println!("largest |heading|: {angle:.1} rad");
}
