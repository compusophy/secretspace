//! Can a freshly released mind establish a lineage on a mature island?
//! cargo run -p secretspace-space --release --example release
use space::founders::{genesis, FOUNDERS, TEMPLATE};
use space::island::Island;

fn trial(src: &str, seed: u64, x: i64, y: i64, clutch: usize) -> (u64, usize) {
    let mut isl = Island::new(seed);
    genesis(&mut isl);
    for _ in 0..400 {
        isl.watched = true;
        isl.step();
    }
    let endow: u64 = std::env::args()
        .nth(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(space::laws::RELEASE_ENDOW);
    let clearing: i64 = std::env::args()
        .nth(3)
        .and_then(|a| a.parse().ok())
        .unwrap_or(space::laws::CLEARING);
    let lin = isl
        .release_clutch(src, "trial", "me", x, y, clutch, endow, clearing)
        .unwrap();
    let mut t = 0;
    while t < 3000 {
        isl.watched = true;
        isl.step();
        t += 1;
        if !isl.motes().iter().any(|m| m.lineage == lin) {
            break;
        }
    }
    (t, isl.motes().iter().filter(|m| m.lineage == lin).count())
}

fn main() {
    let mut minds: Vec<(&str, &str)> = FOUNDERS.iter().map(|f| (f.0, f.1)).collect();
    minds.push(("template", TEMPLATE));

    let clutch: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(space::laws::CLUTCH);
    for (name, src) in minds {
        let mut lived = Vec::new();
        let mut alive = 0;
        for seed in 1..=6 {
            for (x, y) in [(5, 25), (24, 15), (44, 3)] {
                let (t, n) = trial(src, seed, x, y, clutch);
                lived.push(t);
                if n > 0 {
                    alive += 1;
                }
            }
        }
        lived.sort();
        println!(
            "{name:9} established {alive:2}/18 · median life {} ticks",
            lived[lived.len() / 2]
        );
    }
}
