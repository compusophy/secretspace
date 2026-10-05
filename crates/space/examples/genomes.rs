use space::founders::genesis;
use space::island::Island;
use std::collections::BTreeMap;
fn main() {
    let args: Vec<u64> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse().ok())
        .collect();
    let mut isl = Island::new(args.first().copied().unwrap_or(1));
    genesis(&mut isl);
    for _ in 0..args.get(1).copied().unwrap_or(6000) {
        isl.watched = true;
        isl.step();
    }
    let mut by: BTreeMap<String, (usize, String, u64)> = BTreeMap::new();
    for m in isl.motes() {
        let e = by
            .entry(m.genome.src.clone())
            .or_insert((0, m.name.to_string(), 0));
        e.0 += 1;
        e.2 += m.last_used as u64;
    }
    let mut v: Vec<_> = by.into_iter().collect();
    v.sort_by_key(|e| std::cmp::Reverse(e.1 .0));
    for (src, (n, name, used)) in v.iter().take(5) {
        println!("--- {n} x {name} (avg fuel {})\n{src}", used / *n as u64);
    }
}
