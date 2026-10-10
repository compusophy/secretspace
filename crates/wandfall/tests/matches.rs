//! Whole matches: a lobby, the drop, bots fighting, the storm closing, a
//! winner, and the next lobby; and a page's prediction matching the
//! server to the bit.

use wandfall::laws::*;
use wandfall::motion::{keys, Input};
use wandfall::predict::Predict;
use wandfall::view;
use wandfall::world::{Event, Phase, World};

/// A match of bots, watched (a person in it who is knocked out sends
/// everyone back to the lobby soon after).
#[test]
fn a_match_runs_to_a_winner_and_starts_again() {
    let mut w = World::new(42);
    assert_eq!(w.phase, Phase::Lobby);
    let hour = w.hour;
    let (mut began, mut winner, mut shot, mut storm, mut lobby) = (false, None, 0, 0, false);
    let (mut casts, mut levels, mut top, mut opened) = (0, 0, 1, 0);
    let (mut lying, mut fought): (Vec<u16>, bool) = (Vec::new(), false);
    let (mut hops, mut launches, mut tethers) = (0, 0, 0);
    for _ in 0..TICK_HZ * 60 * 6 {
        let before: Vec<(u16, wandfall::motion::Body)> =
            w.players.iter().map(|p| (p.id, p.body)).collect();
        let events = w.step();
        // The bots' moves: timed hops, and launch runes taken.
        for p in &w.players {
            if let Some((_, was)) = before.iter().find(|b| b.0 == p.id) {
                let is = &p.body;
                if was.ground && !is.ground && is.v[1] > 1.0 && was.landed <= HOP_WINDOW {
                    hops += 1;
                }
                if was.ground && !was.glide && is.glide {
                    launches += 1;
                }
            }
        }
        for e in events {
            match e {
                Event::Begin => {
                    began = true;
                    assert_eq!(w.entrants(), MATCH_SIZE, "bots fill the island");
                }
                Event::Out { by, .. } if by != 0 => shot += 1,
                Event::Out { .. } => storm += 1,
                Event::Win { who } => winner = Some(who),
                Event::Lobby => lobby = true,
                Event::Cast { stage: 0, .. } => casts += 1,
                Event::Cast {
                    spell: spell::TETHER,
                    stage: 1,
                    ..
                } => tethers += 1,
                Event::Level { level, .. } => {
                    levels += 1;
                    top = top.max(level);
                }
                _ => {}
            }
        }
        // Cubes gone from where they lay mid-fight: run over.
        let now: Vec<u16> = w.scrolls.iter().map(|s| s.id).collect();
        let fight = w.phase == Phase::Fight;
        if fight && fought {
            opened += lying.iter().filter(|id| !now.contains(id)).count();
        }
        (lying, fought) = (now, fight);
        if lobby {
            break;
        }
    }
    assert!(began, "the lobby ends");
    let winner = winner.expect("someone wins");
    assert!(
        shot >= 5,
        "wizards are shot down: {shot} (the storm took {storm})"
    );
    assert_eq!(
        shot + storm,
        MATCH_SIZE - 1,
        "everyone but the winner is out once"
    );
    assert!(
        lobby && w.players.iter().all(|p| !p.bot),
        "back to the lobby, bots gone"
    );
    assert_eq!(w.hour, (hour + 1) % HOURS, "the island's day turns an hour");
    assert!(opened >= 8, "cubes are picked up: {opened}");
    assert!(casts >= 10, "spells are cast: {casts}");
    assert!(
        levels >= 10 && top >= 4,
        "wizards level up: {levels} times, to {top}"
    );
    assert!(hops >= 20, "bots hop their way about: {hops}");
    println!(
        "knocked out {shot}, storm {storm}, cubes {opened}, casts {casts}, levels {levels} (top {top}), hops {hops}, launches {launches}, tethers {tethers}"
    );
    let _ = winner;
}

#[test]
fn bots_fight_while_no_one_is_here_and_make_way_for_someone() {
    let mut w = World::new(1);
    for _ in 0..TICK_HZ * 30 {
        w.step();
    }
    assert_eq!(w.phase, Phase::Fight, "a match of bots, to watch");
    assert_eq!(w.players.len(), MATCH_SIZE);
    assert!(w.players.iter().all(|p| p.bot));
    // Someone arrives: the bots' match gives way to a lobby for them.
    let me = w.join("late", 0);
    w.step();
    assert_eq!(w.phase, Phase::Lobby);
    assert!(w.find(me).is_some_and(|p| p.alive));
    for _ in 0..(LOBBY_SECS + 1) * TICK_HZ {
        w.step();
    }
    assert_eq!(w.phase, Phase::Fight);
    assert!(w.find(me).is_some_and(|p| p.entrant), "and they are in it");
}

#[test]
fn the_page_predicts_its_wizard_exactly() {
    let mut w = World::new(5);
    let me = w.join("runner", 0);
    let mut page = Predict::default();
    page.reset(w.find(me).unwrap().body);
    let mut rng = engine::rng::Rng::new(9);
    let mut seq = 0u16;
    let mut worst = 0.0f32;
    // Through the lobby, the drop and the fight.
    for t in 0..TICK_HZ * 40 {
        // Inputs come in bursts and gaps, as a real page's do.
        let n = [1, 1, 1, 2, 0, 1][t as usize % 6];
        for _ in 0..n {
            seq = seq.wrapping_add(1);
            let i = Input {
                seq,
                yaw: (t * 300) as u16,
                pitch: 0,
                keys: (rng.next_u64() as u16)
                    & (keys::FWD
                        | keys::LEFT
                        | keys::JUMP
                        | keys::AIM
                        | keys::SPRINT
                        | keys::CROUCH),
                cast: 0,
                view: 0,
            };
            page.push(i, &w.map);
            w.input(me, i);
        }
        w.step();
        let f = view::frame(&w, me);
        if let Some(own) = f.you {
            let off = page.confirm(own.body, own.seq, &w.map);
            // The drop lifts everyone into the sky: that one frame moves.
            if !(w.phase == Phase::Fight && w.tick == w.began) {
                worst = worst.max(off);
            }
        }
    }
    assert_eq!(worst, 0.0, "the prediction never moved");
}

/// Balance, as bots play it: over a few matches every spell lands, spells
/// are the big moments (a fair share of the hurt), and none of them, nor
/// the wand, does all the work.
#[test]
fn every_spell_has_its_place() {
    let mut by_what = [0i64; 256];
    for seed in [7u64, 8, 9] {
        let mut w = World::new(seed);
        let mut lobby = false;
        for _ in 0..TICK_HZ * 60 * 6 {
            for e in w.step() {
                match e {
                    Event::Hit { amount, what, .. } => by_what[what as usize] += amount as i64,
                    Event::Lobby => lobby = true,
                    _ => {}
                }
            }
            if lobby {
                break;
            }
        }
    }
    let all: i64 = by_what.iter().sum();
    let share = |k: usize| by_what[k] * 100 / all.max(1);
    let wand = share(wandfall::world::WAND as usize);
    let spells: i64 = (0..SPELLS.len()).map(|k| by_what[k]).sum::<i64>() * 100 / all.max(1);
    for (k, s) in SPELLS.iter().enumerate() {
        println!("{:>10} {:>3}%", s.name, share(k));
    }
    println!("{:>10} {wand:>3}%", "wand");
    for &k in &spell::OFFENSE {
        assert!(by_what[k as usize] > 0, "{} lands", SPELLS[k as usize].name);
        assert!(
            share(k as usize) < 35,
            "{} does not rule",
            SPELLS[k as usize].name
        );
    }
    assert!(spells >= 25, "spells are the moments: {spells}%");
    assert!(wand < 70, "the wand is not everything: {wand}%");
}
