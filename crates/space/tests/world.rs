//! The physics: determinism, conservation, the sun, heredity, and motes
//! crossing between islands over an unreliable wire.

use space::founders::genesis;
use space::island::{Island, Traveler};
use space::laws::*;
use space::net::{Net, NetEvent, Portal};
use space::rng::Rng;
use space::wire::{decode, encode, CensusEntry, Envelope, Hello, Msg};

fn world(id: u64) -> Island {
    let mut isl = Island::new(id);
    genesis(&mut isl);
    isl
}

#[test]
fn same_inputs_same_island() {
    let (mut a, mut b) = (world(7), world(7));
    for t in 0..3000 {
        let watched = (t / 400) % 2 == 0;
        a.watched = watched;
        b.watched = watched;
        a.step();
        b.step();
    }
    assert_eq!(a.hash(), b.hash());
    assert!(
        !a.motes().is_empty(),
        "the founders' world should still be alive"
    );
}

#[test]
fn adjacent_ids_diverge() {
    let (mut a, mut b) = (world(42), world(43));
    for _ in 0..300 {
        a.watched = true;
        b.watched = true;
        a.step();
        b.step();
    }
    assert_ne!(a.hash(), b.hash());
}

#[test]
fn every_erg_is_accounted_for_every_tick() {
    let mut isl = world(3);
    isl.portals = [0, 60, 0, 30];
    let mut rng = Rng::new(9);
    let mut wire: Vec<(u8, Traveler)> = Vec::new();
    for t in 0..4000u64 {
        isl.watched = (t / 300) % 3 != 2;
        // Send a few departures back in on another side, as if a neighbor
        // returned them; drop the rest.
        for (side, tr) in wire.drain(..) {
            if rng.chance(1, 2) {
                isl.arrive(opposite(side), tr);
            }
        }
        isl.step();
        assert!(
            isl.conserved(),
            "tick {t}: held {} expected {}",
            isl.held(),
            isl.ledger.expected()
        );
        wire.extend(isl.take_departures());
    }
    assert!(isl.counts.departures > 0, "something should have crossed");
    assert!(isl.counts.arrivals > 0);
    assert!(isl.counts.births > 0);
    assert!(isl.counts.deaths > 0);
}

#[test]
fn the_sun_follows_attention() {
    let mut isl = world(1);
    isl.watched = true;
    for _ in 0..(SUN_MAX / SUN_RAMP) {
        isl.step();
    }
    assert_eq!(isl.sun, SUN_MAX);
    isl.watched = false;
    for _ in 0..(SUN_MAX / SUN_RAMP / 2) {
        isl.step();
    }
    assert_eq!(isl.sun, SUN_MAX / 2);
    for _ in 0..SUN_MAX {
        isl.step();
    }
    assert_eq!(isl.sun, 0);
    let before = isl.ledger.minted;
    isl.step();
    assert_eq!(isl.ledger.minted, before, "no light in the dark");
}

#[test]
fn darkness_starves() {
    let mut isl = world(5);
    isl.watched = true;
    for _ in 0..500 {
        isl.step();
    }
    let lit = isl.motes().len();
    assert!(lit > 0);
    isl.watched = false;
    for _ in 0..6000 {
        isl.step();
    }
    assert!(
        isl.motes().len() < lit / 4,
        "an unwatched island should mostly die: {} of {lit}",
        isl.motes().len()
    );
}

#[test]
fn nomads_flee_a_setting_sun_through_a_bright_portal() {
    let mut isl = Island::new(11);
    let nomad = space::founders::FOUNDERS
        .iter()
        .find(|f| f.0 == "nomad")
        .unwrap();
    isl.seed(nomad.0, nomad.1, 30, 600);
    isl.watched = true;
    for _ in 0..100 {
        isl.step();
    }
    isl.portals = [0, 101, 0, 0];
    isl.watched = false;
    let mut left = 0;
    for _ in 0..400 {
        isl.step();
        left += isl
            .take_departures()
            .iter()
            .filter(|(side, _)| *side == 1)
            .count();
    }
    assert!(left >= 10, "only {left} nomads ran east");
}

#[test]
fn releasing_costs_the_land_and_starts_a_lineage() {
    let mut isl = world(2);
    let before = isl.held();
    let lineage = isl
        .release(space::founders::TEMPLATE, "mine", "tester", 20, 15)
        .expect("release");
    assert_eq!(
        isl.held(),
        before,
        "release moves light into a mote, it mints nothing"
    );
    assert!(isl
        .motes()
        .iter()
        .any(|m| m.lineage == lineage && &*m.author == "tester"));
    assert!(isl
        .release("fly()", "bad", "t", 1, 1)
        .unwrap_err()
        .contains("E0105"));
}

#[test]
fn children_inherit_and_sometimes_mutate() {
    let mut isl = world(8);
    isl.watched = true;
    for _ in 0..3000 {
        isl.step();
    }
    let top = isl.census()[0].id;
    let genomes: std::collections::BTreeSet<u64> = isl
        .motes()
        .iter()
        .filter(|m| m.lineage == top)
        .map(|m| m.genome.hash)
        .collect();
    assert!(isl.motes().iter().any(|m| m.gen > 2));
    assert!(genomes.len() > 1, "mutation should have produced variants");
    assert!(isl.counts.miscarriages + isl.counts.births > 0);
}

#[test]
fn wire_round_trips_every_message() {
    let traveler = Traveler {
        id: 1,
        lineage: 2,
        name: "nomad".into(),
        author: "ü".into(),
        gen: 3,
        hops: 4,
        age: 5,
        balance: 600,
        mem: [1, -2, 3, i64::MIN, 0, 0, 0, i64::MAX],
        genome: "harvest()".into(),
        offset: 9,
    };
    let msgs = vec![
        Msg::Hello(Hello {
            place: "Denver".into(),
            tz_min: -360,
            sun: 77,
            asleep: true,
            free: 2,
            pop: 120,
        }),
        Msg::LinkReq { side: 3 },
        Msg::LinkAck { side: 1, yours: 3 },
        Msg::LinkNo,
        Msg::Unlink { side: 2 },
        Msg::Mote {
            seq: 99,
            side: 1,
            traveler,
        },
        Msg::MoteAck { seq: 99 },
        Msg::Census(vec![CensusEntry {
            lineage: 5,
            count: 6,
            name: "sprout".into(),
            author: "genesis".into(),
        }]),
        Msg::Bye,
    ];
    for msg in msgs {
        let env = Envelope {
            from: 10,
            to: 20,
            msg,
        };
        assert_eq!(decode(&encode(&env)).unwrap(), env);
    }
}

#[test]
fn hostile_bytes_never_panic_the_decoder() {
    let mut rng = Rng::new(4);
    let good = encode(&Envelope {
        from: 1,
        to: 0,
        msg: Msg::Census(vec![CensusEntry {
            lineage: 1,
            count: 1,
            name: "x".into(),
            author: "y".into(),
        }]),
    });
    for _ in 0..20_000 {
        let mut b = good.clone();
        match rng.below(3) {
            0 => b.truncate(rng.below(b.len() as u64) as usize),
            1 => {
                let i = rng.below(b.len() as u64) as usize;
                b[i] = rng.next_u64() as u8;
            }
            _ => b = (0..rng.below(80)).map(|_| rng.next_u64() as u8).collect(),
        }
        let _ = decode(&b);
    }
}

// ---- many islands over a bad wire --------------------------------------------

struct Tab {
    island: Island,
    net: Net,
    alive: bool,
}

/// The shared clock, in ms, carried across `simulate` calls.
struct Clock(u64);

fn hello(isl: &Island) -> Hello {
    Hello {
        place: format!("island-{}", isl.id % 1000),
        tz_min: 0,
        sun: isl.sun as u8,
        asleep: false,
        free: 0,
        pop: isl.motes().len() as u16,
    }
}

/// Run `tabs` for `ticks`, 100 ms apart, every envelope heard by every
/// tab (one browser's broadcast channel), with loss and duplication.
fn simulate(
    tabs: &mut [Tab],
    clock: &mut Clock,
    ticks: u64,
    rng: &mut Rng,
    loss: u64,
    dup: u64,
) -> (u64, u64) {
    let (mut lost, mut delivered) = (0, 0);
    let mut bus: Vec<Vec<u8>> = Vec::new();
    for _ in 0..ticks {
        clock.0 += 100;
        let now = clock.0;
        for b in std::mem::take(&mut bus) {
            let Ok(env) = decode(&b) else {
                panic!("our own bytes decode")
            };
            for tab in tabs.iter_mut().filter(|x| x.alive) {
                tab.net.receive(now, env.clone());
            }
        }
        for tab in tabs.iter_mut().filter(|x| x.alive) {
            let census: Vec<CensusEntry> = tab
                .island
                .census()
                .iter()
                .map(|l| CensusEntry {
                    lineage: l.id,
                    count: l.count,
                    name: l.name.to_string(),
                    author: l.author.to_string(),
                })
                .collect();
            tab.net.tick(now, hello(&tab.island), &census);
            let (out, events) = tab.net.drain();
            for e in events {
                match e {
                    NetEvent::Arrive { side, traveler } => tab.island.arrive(side, traveler),
                    NetEvent::Lost { .. } => lost += 1,
                    NetEvent::Delivered { .. } => delivered += 1,
                    _ => {}
                }
            }
            tab.island.portals = tab.net.portal_view();
            tab.island.step();
            assert!(tab.island.conserved());
            for (side, tr) in tab.island.take_departures() {
                if let Err(tr) = tab.net.send_mote(now, side, tr) {
                    tab.island.arrive(side, tr);
                }
            }
            let (out2, _) = tab.net.drain();
            for env in out.into_iter().chain(out2) {
                if rng.chance(loss, 100) {
                    continue;
                }
                let b = encode(&env);
                if rng.chance(dup, 100) {
                    bus.push(b.clone());
                }
                bus.push(b);
            }
        }
    }
    (lost, delivered)
}

fn tabs(n: u64) -> Vec<Tab> {
    (1..=n)
        .map(|id| {
            let mut island = world(id * 1000 + 7);
            island.watched = true;
            Tab {
                net: Net::new(island.id),
                island,
                alive: true,
            }
        })
        .collect()
}

/// Every erg that left an island is either on another island, burned on
/// the way in, still in flight, or was lost with a vanished island.
fn global_balance(tabs: &[Tab]) -> (u64, u64) {
    let exported: u64 = tabs.iter().map(|t| t.island.ledger.exported).sum();
    let imported: u64 = tabs.iter().map(|t| t.island.ledger.imported).sum();
    (exported, imported)
}

#[test]
fn islands_link_and_motes_cross() {
    let mut rng = Rng::new(1);
    let mut clock = Clock(0);
    let mut world = tabs(4);
    let (_, delivered) = simulate(&mut world, &mut clock, 1500, &mut rng, 0, 0);
    for tab in &world {
        let open = tab
            .net
            .portals
            .iter()
            .filter(|p| matches!(p, Portal::Open { .. }))
            .count();
        assert!(open >= 1, "island {} has no open portal", tab.island.id);
        assert_eq!(tab.net.peers.len(), 3);
    }
    // Links are symmetric: if my side s is open to you on your side t, your
    // side t is open to me on my side s.
    for a in &world {
        for (s, p) in a.net.portals.iter().enumerate() {
            if let Portal::Open { peer, theirs } = *p {
                let b = world.iter().find(|t| t.island.id == peer).unwrap();
                assert_eq!(
                    b.net.portals[theirs as usize],
                    Portal::Open {
                        peer: a.island.id,
                        theirs: s as u8
                    }
                );
            }
        }
    }
    assert!(delivered > 20, "only {delivered} motes crossed");
    let (exported, imported) = global_balance(&world);
    let flying: usize = world.iter().map(|t| t.net.in_flight()).sum();
    assert!(imported <= exported);
    assert!(flying > 0 || imported == exported);
}

#[test]
fn a_bad_wire_never_duplicates_a_mote() {
    let mut rng = Rng::new(2);
    let mut clock = Clock(0);
    let mut world = tabs(3);
    simulate(&mut world, &mut clock, 2500, &mut rng, 20, 20);
    let (exported, imported) = global_balance(&world);
    assert!(
        imported <= exported,
        "imported {imported} > exported {exported}: a mote was duplicated"
    );
    let mut ids = std::collections::BTreeSet::new();
    for tab in &world {
        for m in tab.island.motes() {
            assert!(ids.insert(m.id), "mote {} lives on two islands", m.id);
        }
    }
}

#[test]
fn a_vanished_island_takes_its_wire_with_it() {
    let mut rng = Rng::new(3);
    let mut clock = Clock(0);
    let mut world = tabs(3);
    simulate(&mut world, &mut clock, 800, &mut rng, 0, 0);
    world[2].alive = false;
    let (lost, _) = simulate(&mut world, &mut clock, 200, &mut rng, 0, 0);
    for tab in &world[..2] {
        assert!(!tab.net.peers.contains_key(&world[2].island.id));
        assert!(tab
            .net
            .portals
            .iter()
            .all(|p| !matches!(p, Portal::Open { peer, .. } if *peer == world[2].island.id)));
    }
    let _ = lost;
}

#[test]
fn a_sleeping_island_shuts_its_portals() {
    let mut rng = Rng::new(5);
    let mut clock = Clock(0);
    let mut world = tabs(2);
    simulate(&mut world, &mut clock, 300, &mut rng, 0, 0);
    assert!(world[0]
        .net
        .portals
        .iter()
        .any(|p| matches!(p, Portal::Open { .. })));
    world[1].net.sleep();
    simulate(&mut world, &mut clock, 50, &mut rng, 0, 0);
    for tab in &world {
        assert!(tab
            .net
            .portals
            .iter()
            .all(|p| !matches!(p, Portal::Open { .. })));
    }
    world[1].net.wake();
    simulate(&mut world, &mut clock, 100, &mut rng, 0, 0);
    assert!(world[0]
        .net
        .portals
        .iter()
        .any(|p| matches!(p, Portal::Open { .. })));
}

#[test]
fn the_census_counts_islands() {
    let mut rng = Rng::new(6);
    let mut clock = Clock(0);
    let mut world = tabs(3);
    simulate(&mut world, &mut clock, 400, &mut rng, 0, 0);
    let entries = |isl: &Island| -> Vec<CensusEntry> {
        isl.census()
            .iter()
            .map(|l| CensusEntry {
                lineage: l.id,
                count: l.count,
                name: l.name.to_string(),
                author: l.author.to_string(),
            })
            .collect()
    };
    // Each island's last census reached the others within one census period.
    let w = world[0].net.world(clock.0, &entries(&world[0].island));
    assert!(!w.is_empty());
    for l in &w {
        assert!(l.tabs >= 1 && l.tabs <= 3);
    }
    let top = &w[0];
    let holders = world
        .iter()
        .filter(|t| t.island.census().iter().any(|c| c.id == top.lineage))
        .count() as u32;
    assert!(
        top.tabs.abs_diff(holders) <= 1,
        "census says {} tabs, islands say {holders}",
        top.tabs
    );
}
