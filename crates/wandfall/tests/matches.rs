//! Whole matches: a lobby, the drop, bots fighting, the storm closing, a
//! winner, and the next lobby; and a page's prediction matching the
//! server to the bit.

use wandfall::laws::*;
use wandfall::motion::{keys, Input};
use wandfall::predict::Predict;
use wandfall::view;
use wandfall::world::{Event, Phase, World};

#[test]
fn a_match_runs_to_a_winner_and_starts_again() {
    let mut w = World::new(42);
    let me = w.join("tester", 0);
    assert_eq!(w.phase, Phase::Lobby);
    let (mut began, mut winner, mut shot, mut storm, mut lobby) = (false, None, 0, 0, false);
    for _ in 0..TICK_HZ * 60 * 6 {
        for e in w.step() {
            match e {
                Event::Begin => {
                    began = true;
                    assert_eq!(w.entrants(), MATCH_SIZE, "bots fill the island");
                }
                Event::Out { by, .. } if by != 0 => shot += 1,
                Event::Out { .. } => storm += 1,
                Event::Win { who } => winner = Some(who),
                Event::Lobby => lobby = true,
                Event::Hit { .. } => {}
            }
        }
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
    assert!(w.find(me).is_some());
    let _ = winner;
}

#[test]
fn no_one_here_no_match() {
    let mut w = World::new(1);
    for _ in 0..TICK_HZ * 30 {
        w.step();
    }
    assert_eq!(w.phase, Phase::Lobby);
    assert!(w.players.is_empty());
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
                keys: (rng.next_u64() as u8) & (keys::FWD | keys::LEFT | keys::JUMP),
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
