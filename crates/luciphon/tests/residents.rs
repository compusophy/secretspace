//! The proxy check (§15, Stage 1): ten minutes of eight residents, headless.
//! Every technique but the feint should happen, and Lumens should fall in
//! at least three different ways.

use luciphon::bots::{Brain, NAMES, RESIDENTS};
use luciphon::laws::{HZ, LAWS};
use luciphon::world::World;

#[test]
fn ten_minutes_of_residents_use_every_technique() {
    let mut w = World::new(LAWS, 11);
    for name in &NAMES[..RESIDENTS] {
        let brain = Brain::new(&mut w.rng);
        w.spawn(name, 0, Some(brain));
    }
    let start = std::time::Instant::now();
    for _ in 0..HZ * 600 {
        w.step();
    }
    let mut tally: Vec<_> = w.tally.iter().collect();
    tally.sort();
    for (k, v) in &tally {
        eprintln!("{k:>20} {v}");
    }
    eprintln!("{:?} for ten minutes", start.elapsed());
    let has = |k: &str| w.tally.get(k).copied().unwrap_or(0) > 0;
    for t in [
        "dash",
        "jump",
        "running strike",
        "landed strike",
        "heavy",
        "throw",
        "dodge",
        "lance",
    ] {
        assert!(has(t), "no {t} in ten minutes");
    }
    let causes = [
        "fell to the Dark",
        "fell to thorns",
        "fell to a wall",
        "fell to a mote",
        "fell to a strike",
        "knocked down",
    ]
    .iter()
    .filter(|k| has(k))
    .count();
    assert!(causes >= 3, "only {causes} ways to fall");
}
