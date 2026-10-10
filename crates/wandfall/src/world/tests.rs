//! The match's rules, a wizard at a time.

use super::*;
use crate::motion::cast;
use crate::trig;

/// A fight with two people in it (`a` and `b`) and bots.
fn fight() -> (World, u16, u16) {
    let mut w = World::new(21);
    let a = w.join("a", 0);
    let b = w.join("b", 0);
    while w.phase != Phase::Fight {
        w.step();
    }
    (w, a, b)
}

fn at(w: &World, id: u16) -> usize {
    w.players.iter().position(|p| p.id == id).unwrap()
}

#[test]
fn a_quitter_or_the_storm_knocks_out_to_the_credit_of_who_hurt_them() {
    let (mut w, a, b) = fight();
    let mut ev = Vec::new();
    w.hurt(a, b, 10, WAND, &mut ev);
    ev.clear();
    w.leave(b, &mut ev);
    assert!(
        ev.contains(&Event::Out {
            who: b,
            by: a,
            place: MATCH_SIZE as u16
        }),
        "{ev:?}"
    );
    assert_eq!(w.find(a).unwrap().kills, 1);
    // Hurt long ago, then finished by the storm: the storm's.
    let bot = w.players.iter().find(|p| p.bot).unwrap().id;
    w.hurt(a, bot, 10, WAND, &mut ev);
    w.tick += KILL_CREDIT_SECS * TICK_HZ;
    ev.clear();
    let hp = w.find(bot).unwrap().hp;
    w.hurt(0, bot, hp, STORM, &mut ev);
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::Out { who, by: 0, .. } if *who == bot)),
        "{ev:?}"
    );
    // Hurt just now, then finished by the storm: theirs.
    let other = w.players.iter().find(|p| p.bot && p.alive).unwrap().id;
    w.hurt(a, other, 10, WAND, &mut ev);
    ev.clear();
    let hp = w.find(other).unwrap().hp;
    w.hurt(0, other, hp, STORM, &mut ev);
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::Out { who, by, .. } if *who == other && *by == a)),
        "{ev:?}"
    );
}

#[test]
fn a_quitter_no_one_hurt_is_gone_without_a_word() {
    let (mut w, _, b) = fight();
    let mut ev = Vec::new();
    w.leave(b, &mut ev);
    assert!(ev.is_empty(), "not 'to the storm': {ev:?}");
    assert!(w.find(b).is_none());
}

#[test]
fn the_last_two_falling_together_the_last_to_fall_wins() {
    let (mut w, a, b) = fight();
    w.players.retain(|p| !p.bot);
    let mut ev = Vec::new();
    w.hurt(0, a, 10_000, STORM, &mut ev);
    w.hurt(0, b, 10_000, STORM, &mut ev);
    let ev = w.step();
    assert!(ev.contains(&Event::Win { who: b }), "{ev:?}");
    assert_eq!(w.find(b).unwrap().place, 1);
    assert_eq!(w.find(a).unwrap().place, 2);
}

#[test]
fn the_storm_burns_for_every_second_outside_however_it_is_timed() {
    let (mut w, a, _) = fight();
    w.players.retain(|p| p.id == a);
    let k = at(&w, a);
    w.players[k].body.glide = false;
    // Far outside a storm long closed, but in it on every beat.
    w.tick = w.began + 400 * TICK_HZ - 1;
    let hp = w.players[k].hp;
    let mut ev = Vec::new();
    for _ in 0..3 * TICK_HZ {
        let beat = (w.tick + 1).is_multiple_of(TICK_HZ);
        w.players[k].body.p = if beat {
            let c = w.storm_now().centre;
            [c[0], 0.0, c[1]]
        } else {
            [150.0, 0.0, 150.0]
        };
        w.tick += 1;
        w.storm_and_regen(&mut ev);
    }
    assert!(w.players[k].hp < hp, "burned: {} of {hp}", w.players[k].hp);
}

#[test]
fn the_island_holds_match_size_and_the_rest_go_first_next_time() {
    let mut w = World::new(3);
    let ids: Vec<u16> = (0..MATCH_SIZE + 4)
        .map(|n| w.join(&format!("p{n}"), 0))
        .collect();
    while w.phase != Phase::Fight {
        w.step();
    }
    assert_eq!(w.entrants(), MATCH_SIZE);
    assert!(w.players.iter().all(|p| !p.bot), "no bots needed");
    let late = &ids[MATCH_SIZE..];
    assert!(late.iter().all(|&id| !w.find(id).unwrap().entrant));
    let mut ev = Vec::new();
    w.begin(&mut ev);
    assert!(
        late.iter().all(|&id| w.find(id).unwrap().entrant),
        "next time, they go"
    );
}

#[test]
fn everyone_here_out_a_new_match_gathers_soon() {
    let (mut w, a, b) = fight();
    let mut ev = Vec::new();
    w.hurt(0, a, 10_000, STORM, &mut ev);
    w.hurt(0, b, 10_000, STORM, &mut ev);
    assert!(w.alive() > 1, "bots fight on");
    for _ in 0..(OUT_LINGER_SECS + 1) * TICK_HZ {
        w.step();
        if w.phase != Phase::Fight {
            break;
        }
    }
    assert_eq!(w.phase, Phase::Lobby, "not made to wait the bots out");
}

#[test]
fn a_spell_is_aimed_as_the_input_that_cast_it() {
    let (mut w, a, _) = fight();
    w.players.retain(|p| p.id == a);
    let k = at(&w, a);
    let (x, z) = (-15.0, -12.0);
    let p = &mut w.players[k];
    p.body = Body {
        p: [x, w.map.height(x, z), z],
        ground: true,
        ..Body::default()
    };
    p.slots[0] = Some(Slot {
        spell: spell::LANCE,
        rank: 1,
    });
    p.cds = [0; 4];
    p.credit = INPUT_BANK;
    // Inputs held back, then caught up on in one tick: the first casts,
    // looking east; the rest look north.
    let north = Input {
        yaw: trig::heading(std::f32::consts::FRAC_PI_2),
        ..Input::default()
    };
    w.input(
        a,
        Input {
            cast: cast::SLOT[0],
            ..Input::default()
        },
    );
    for _ in 0..INPUT_JITTER + 1 {
        w.input(a, north);
    }
    let ev = w.step();
    assert_eq!(w.players[k].yaw, north.yaw, "it caught up");
    let beam = ev.iter().find_map(|e| match e {
        Event::Beam { from, to, .. } => Some((*from, *to)),
        _ => None,
    });
    let (from, to) = beam.expect("it cast");
    assert!(
        to[0] - from[0] > (to[2] - from[2]).abs(),
        "east, as it was cast: {from:?} {to:?}"
    );
}
