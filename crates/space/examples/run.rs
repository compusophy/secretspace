//! Headless: one island, printed as a census every so often.
//! cargo run -p secretspace-space --release --example run -- [ticks] [id] [watch-period]
use space::founders::genesis;
use space::island::Island;

fn main() {
    let args: Vec<u64> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse().ok())
        .collect();
    let ticks = args.first().copied().unwrap_or(5000);
    let id = args.get(1).copied().unwrap_or(1);
    let period = args.get(2).copied().unwrap_or(0);
    let mut isl = Island::new(id);
    genesis(&mut isl);
    for t in 0..ticks {
        isl.watched = period == 0 || (t / period) % 2 == 0;
        isl.step();
        if t % (ticks / 20).max(1) == 0 {
            let census: Vec<String> = isl
                .census()
                .iter()
                .take(6)
                .map(|l| format!("{}:{}", l.name, l.count))
                .collect();
            let maxgen = isl.motes().iter().map(|m| m.gen).max().unwrap_or(0);
            let light: u64 = isl.light().iter().map(|&l| l as u64).sum();
            println!(
                "t={t:6} sun={:3} pop={:4} light/cell={:3} gen={maxgen:3} births={} deaths={} miscarriages={}  {}",
                isl.sun,
                isl.motes().len(),
                light / space::laws::CELLS as u64,
                isl.counts.births,
                isl.counts.deaths,
                isl.counts.miscarriages,
                census.join(" ")
            );
        }
    }
    println!("hash {:016x} conserved={}", isl.hash(), isl.conserved());
}
