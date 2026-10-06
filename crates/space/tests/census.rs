//! The census nobody runs: sketches estimate well, merge safely, and gossip
//! between neighbours converges on the whole world's numbers.

use space::census::{Census, Sketch};
use space::rng::{splitmix, Rng};
use space::wire::{decode, encode, Envelope, Msg};

fn within(est: u32, truth: u32, pct: u32) -> bool {
    let (e, t) = (est as i64, truth as i64);
    (e - t).abs() * 100 <= t * pct as i64
}

#[test]
fn sketches_estimate_distinct_counts() {
    for (n, pct) in [
        (1, 0),
        (5, 0),
        (20, 20),
        (100, 20),
        (1_000, 30),
        (20_000, 30),
    ] {
        let mut s = Sketch::default();
        for i in 0..n {
            let h = splitmix(i as u64 + 77);
            s.insert(h);
            s.insert(h); // the same item twice counts once
        }
        assert!(
            within(s.estimate(), n, pct),
            "{n} items estimated as {}",
            s.estimate()
        );
    }
    assert_eq!(Sketch::default().estimate(), 0);
}

#[test]
fn merging_is_idempotent_and_commutative() {
    let mut rng = Rng::new(5);
    let (mut a, mut b) = (Sketch::default(), Sketch::default());
    for _ in 0..300 {
        a.insert(rng.next_u64());
        b.insert(rng.next_u64());
    }
    let mut ab = a;
    ab.merge(&b);
    let mut ba = b;
    ba.merge(&a);
    assert_eq!(ab, ba);
    let mut again = ab;
    again.merge(&a);
    again.merge(&ab);
    assert_eq!(again, ab, "hearing the same gossip twice changes nothing");
    assert!(within(ab.estimate(), 600, 30));
}

/// A ring of islands, each also linked across: gossip only between
/// neighbours, and every island learns the whole world.
#[test]
fn gossip_between_neighbours_converges_on_the_world() {
    const N: usize = 40;
    const EPOCH: u32 = 1000;
    let lineage_all = 11u64;
    let lineage_half = 22u64;
    let mut views: Vec<Census> = (0..N).map(|_| Census::default()).collect();
    let neighbours = |i: usize| [(i + 1) % N, (i + N - 1) % N, (i + 7) % N];
    for _round in 0..25 {
        let mut said = Vec::new();
        for (i, v) in views.iter_mut().enumerate() {
            let mut here = vec![(lineage_all, "drifter", "genesis", 10)];
            if i % 2 == 0 {
                here.push((lineage_half, "wayfarer", "you", 3));
            }
            v.note(EPOCH, 100 + i as u64, &here);
            said.push(v.gossip(&[lineage_half]));
        }
        for (i, v) in views.iter_mut().enumerate() {
            for j in neighbours(i) {
                // Through the wire, as it really travels.
                let env = Envelope {
                    from: j as u64,
                    to: 0,
                    msg: Msg::Gossip(said[j].clone()),
                };
                let Msg::Gossip(g) = decode(&encode(&env)).unwrap().msg else {
                    panic!()
                };
                v.merge(EPOCH, &g);
            }
        }
    }
    for v in &views {
        assert!(within(v.islands(), N as u32, 20), "islands {}", v.islands());
        let view = v.view();
        let all = view.iter().find(|k| k.lineage == lineage_all).unwrap();
        let half = view.iter().find(|k| k.lineage == lineage_half).unwrap();
        assert!(
            within(all.islands, 40, 20),
            "drifter on {} islands",
            all.islands
        );
        assert!(
            within(half.islands, 20, 20),
            "wayfarer on {} islands",
            half.islands
        );
        assert!(within(all.motes, 400, 30), "drifter motes {}", all.motes);
        assert!(within(half.motes, 60, 20), "wayfarer motes {}", half.motes);
        assert_eq!(view[0].lineage, lineage_all, "most widespread first");
        assert!(!all.exact, "40 islands is past exact counting");
    }
}

#[test]
fn small_worlds_are_counted_exactly() {
    const EPOCH: u32 = 7;
    let mut views: Vec<Census> = (0..5).map(|_| Census::default()).collect();
    for _ in 0..6 {
        let said: Vec<_> = views
            .iter_mut()
            .enumerate()
            .map(|(i, v)| {
                v.note(
                    EPOCH,
                    500 + i as u64,
                    &[
                        (1, "drifter", "genesis", 100 + i as u32),
                        (2, "wayfarer", "you", if i < 3 { 7 } else { 0 }),
                    ],
                );
                v.gossip(&[])
            })
            .collect();
        // A line: each island hears only the next one along.
        for i in 0..5 {
            if i + 1 < 5 {
                let g = said[i + 1].clone();
                views[i].merge(EPOCH, &g);
                let g = said[i].clone();
                views[i + 1].merge(EPOCH, &g);
            }
        }
    }
    for v in &views {
        assert_eq!(v.islands(), 5);
        let view = v.view();
        let d = view.iter().find(|k| k.lineage == 1).unwrap();
        let w = view.iter().find(|k| k.lineage == 2).unwrap();
        assert!(d.exact && w.exact);
        assert_eq!((d.islands, d.motes), (5, 100 + 101 + 102 + 103 + 104));
        assert_eq!((w.islands, w.motes), (3, 21));
    }
}

#[test]
fn a_closed_island_ages_out_within_two_epochs() {
    let mut c = Census::default();
    c.note(50, 1, &[(9, "x", "y", 4)]);
    c.note(50, 2, &[(9, "x", "y", 4)]);
    assert_eq!(c.islands(), 2);
    // Island 2 is gone; island 1 keeps noting itself.
    c.note(51, 1, &[(9, "x", "y", 4)]);
    assert_eq!(c.islands(), 2, "still remembered for one epoch");
    c.note(52, 1, &[(9, "x", "y", 4)]);
    assert_eq!(c.islands(), 1);
    assert_eq!(c.view()[0].islands, 1);
    // Gossip from a stale epoch is ignored; a clock jump forgets everything.
    let mut old = Census::default();
    old.note(10, 3, &[(9, "x", "y", 4)]);
    c.merge(52, &old.gossip(&[]));
    assert_eq!(c.islands(), 1);
    c.note(90, 1, &[]);
    assert_eq!(c.view().len(), 0);
}
