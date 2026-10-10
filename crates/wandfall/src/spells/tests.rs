//! Each spell, cast in a duel on the Spire's plaza.

use super::*;
use crate::motion::{keys, Body, Input};
use crate::world::{Phase, Slot};

fn duel(gap: f32) -> (World, u16, u16) {
    let mut w = World::new(21);
    let a = w.join("a", 0);
    let b = w.join("b", 0);
    // Into the fight, then side by side on the Spire's open plaza.
    while w.phase != Phase::Fight {
        w.step();
    }
    w.players.retain(|p| !p.bot);
    let [x, z] = [-gap / 2.0, -12.0];
    for (k, dx) in [(0usize, 0.0f32), (1, gap)] {
        let p = &mut w.players[k];
        p.body.glide = false;
        p.body.ground = true;
        p.body.p = [x + dx, w.map.height(x + dx, z), z];
        p.yaw = 0;
        p.pitch = 0;
    }
    // Looking at the other's chest.
    let (e, c) = (w.players[0].eye(), w.players[1].body.p);
    let dy = c[1] + 1.0 - e[1];
    w.players[0].pitch = trig::pitch(dy.atan2(gap));
    (w, a, b)
}

/// `k` casts from `slot` as it stands and looks now.
fn cast_now(w: &mut World, k: usize, slot: usize, ev: &mut Vec<Event>) {
    let aim = Aim::of(&w.players[k]);
    cast(w, k, slot, aim, ev);
}

fn give(w: &mut World, k: usize, slot: usize, spell: u8) {
    w.players[k].slots[slot] = Some(Slot { spell, rank: 1 });
    w.players[k].cds[slot] = 0;
}

fn fly(w: &mut World, ticks: u32, ev: &mut Vec<Event>) {
    for _ in 0..ticks {
        w.tick += 1;
        let mut more = Vec::new();
        for b in std::mem::take(&mut w.bolts) {
            let e = ahead(b.p, b.v, DT);
            match through(b.p, e, w.players[1].body.p, HEIGHT) {
                Some(_) => impact(w, &b, e, w.players[1].id, &mut more),
                None if b.life > 1 => w.bolts.push(Bolt {
                    p: e,
                    life: b.life - 1,
                    ..b
                }),
                None => impact(w, &b, e, 0, &mut more),
            }
        }
        tick(w, &mut more);
        ev.append(&mut more);
    }
}

#[test]
fn a_ward_takes_the_hurt_first_breaks_and_mend_heals() {
    let (mut w, a, b) = duel(4.0);
    let mut ev = Vec::new();
    give(&mut w, 1, 2, spell::WARD);
    cast_now(&mut w, 1, 2, &mut ev);
    assert!(w.find(b).unwrap().shield >= 40);
    w.hurt(a, b, 20, WAND, &mut ev);
    assert_eq!(w.find(b).unwrap().hp, HEALTH, "the ward took it");
    ev.clear();
    w.hurt(a, b, 40, WAND, &mut ev);
    let hp = w.find(b).unwrap().hp;
    assert!(hp < HEALTH);
    assert!(
        ev.iter().any(|e| matches!(
            e,
            Event::Cast {
                spell: spell::WARD,
                stage: 2,
                ..
            }
        )),
        "and it broke"
    );
    give(&mut w, 1, 3, spell::MEND);
    cast_now(&mut w, 1, 3, &mut ev);
    fly(&mut w, MEND_TICKS, &mut ev);
    assert!(w.find(b).unwrap().hp > hp, "mended");
}

#[test]
fn the_lance_strikes_at_once_and_far() {
    let (mut w, _, b) = duel(30.0);
    let mut ev = Vec::new();
    give(&mut w, 0, 0, spell::LANCE);
    cast_now(&mut w, 0, 0, &mut ev);
    assert!(w.find(b).unwrap().hp < HEALTH, "struck, no flight");
    assert!(ev.iter().any(|e| matches!(e, Event::Beam { .. })));
}

#[test]
fn frost_is_deadly_close_and_chills() {
    let mut dealt = Vec::new();
    for gap in [3.0, 18.0] {
        let (mut w, _, b) = duel(gap);
        let mut ev = Vec::new();
        give(&mut w, 0, 0, spell::FROST);
        cast_now(&mut w, 0, 0, &mut ev);
        assert_eq!(w.bolts.len(), FROST_SHARDS);
        fly(&mut w, FROST_LIFE + 1, &mut ev);
        let q = w.find(b).unwrap();
        dealt.push(HEALTH - q.hp);
        if gap < 5.0 {
            assert!(q.body.chill > 0, "chilled");
        }
    }
    assert!(dealt[0] >= 30, "close, most shards: {dealt:?}");
    assert!(dealt[0] > dealt[1] * 2, "far, few: {dealt:?}");
}

#[test]
fn fireball_bursts_and_lightning_strikes_late() {
    let (mut w, _, b) = duel(12.0);
    let mut ev = Vec::new();
    give(&mut w, 0, 0, spell::FIREBALL);
    cast_now(&mut w, 0, 0, &mut ev);
    fly(&mut w, 20, &mut ev);
    let hp = w.find(b).unwrap().hp;
    assert!(hp < HEALTH - 20, "burst: {hp}");
    give(&mut w, 0, 1, spell::LIGHTNING);
    cast_now(&mut w, 0, 1, &mut ev);
    assert_eq!(w.find(b).unwrap().hp, hp, "not yet");
    fly(&mut w, LIGHTNING_DELAY + 1, &mut ev);
    assert!(w.find(b).unwrap().hp < hp - 20, "struck");
}

#[test]
fn gust_throws_back_and_blows_bolts_away_and_blink_moves_you() {
    let (mut w, _, b) = duel(4.0);
    let mut ev = Vec::new();
    give(&mut w, 1, 0, spell::FIREBALL);
    w.players[1].yaw = 32768;
    cast_now(&mut w, 1, 0, &mut ev);
    assert_eq!(w.bolts.len(), 1);
    give(&mut w, 0, 2, spell::GUST);
    cast_now(&mut w, 0, 2, &mut ev);
    assert!(w.bolts.is_empty(), "blown away");
    let v = w.find(b).unwrap().body.v;
    assert!(v[0] > 5.0 && v[1] > 0.0, "thrown away and up: {v:?}");
    let before = w.players[0].body.p;
    w.players[0].body.chill = 30;
    give(&mut w, 0, 3, spell::BLINK);
    cast_now(&mut w, 0, 3, &mut ev);
    let after = w.players[0].body.p;
    assert!(after != before, "blinked");
    assert_eq!(w.players[0].body.chill, 0, "and shook off the chill");
    assert!(w.players[0].cds[3] > 0, "and now it cools down");
    cast_now(&mut w, 0, 3, &mut ev);
    assert_eq!(w.players[0].body.p, after, "not twice");
}

#[test]
fn the_lance_strikes_where_your_page_saw_them() {
    let (mut w, _, b) = duel(30.0);
    give(&mut w, 0, 0, spell::LANCE);
    w.step();
    w.step();
    let seen = w.tick;
    // Then they step out of the line.
    let k = w.players.iter().position(|p| p.id == b).unwrap();
    w.players[k].body.p[2] += 3.0;
    w.step();
    let hp = w.players[k].hp;
    let mut ev = Vec::new();
    cast_now(&mut w, 0, 0, &mut ev);
    assert_eq!(w.players[k].hp, hp, "missed, where they are now");
    // As the caster's page saw them a tick ago: struck.
    w.players[0].cds[0] = 0;
    w.players[0].behind = w.tick - seen;
    cast_now(&mut w, 0, 0, &mut ev);
    assert!(w.players[k].hp < hp, "struck where it saw them");
    // A page cannot ask for further back than REWIND.
    let hp = w.players[k].hp;
    for _ in 0..REWIND + 2 {
        w.step();
    }
    w.players[0].cds[0] = 0;
    w.players[0].behind = w.tick - seen;
    cast_now(&mut w, 0, 0, &mut ev);
    assert_eq!(w.players[k].hp, hp, "too far back: as now");
}

#[test]
fn a_tether_catches_where_you_look() {
    let (mut w, _, _) = duel(30.0);
    let mut ev = Vec::new();
    give(&mut w, 0, 2, spell::TETHER);
    // Looking down at the ground ahead: it catches there.
    w.players[0].pitch = -6000;
    cast_now(&mut w, 0, 2, &mut ev);
    let b = w.players[0].body;
    assert_eq!(b.tether, TETHER_TICKS);
    let ahead = b.anchor[0] - b.p[0];
    assert!(ahead > 1.0 && ahead < 40.0, "{b:?}");
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::Cast {
            spell: spell::TETHER,
            stage: 1,
            ..
        }
    )));
}

#[test]
fn lightning_and_the_tether_aim_where_your_page_saw_them() {
    let (mut w, _, b) = duel(16.0);
    let k = w.players.iter().position(|p| p.id == b).unwrap();
    give(&mut w, 0, 0, spell::LIGHTNING);
    give(&mut w, 0, 2, spell::TETHER);
    w.step();
    w.step();
    let seen = w.tick;
    let c = w.players[k].body.p;
    // Then they step out of the line.
    w.players[k].body.p[2] += 3.0;
    w.step();
    w.players[0].behind = w.tick - seen;
    let mut ev = Vec::new();
    cast_now(&mut w, 0, 0, &mut ev);
    let zone = w.zones.last().unwrap().at;
    assert!(
        (zone[0] - c[0]).abs() < 1.5,
        "struck where it saw them: {zone:?} {c:?}"
    );
    cast_now(&mut w, 0, 2, &mut ev);
    let anchor = w.players[0].body.anchor;
    assert!(
        (anchor[0] - c[0]).abs() < 1.5,
        "caught where it saw them: {anchor:?} {c:?}"
    );
}

#[test]
fn lightning_shatters_a_ward() {
    let (mut w, _, b) = duel(12.0);
    let mut ev = Vec::new();
    give(&mut w, 1, 2, spell::WARD);
    cast_now(&mut w, 1, 2, &mut ev);
    give(&mut w, 0, 1, spell::LIGHTNING);
    cast_now(&mut w, 0, 1, &mut ev);
    fly(&mut w, LIGHTNING_DELAY + 1, &mut ev);
    let q = w.find(b).unwrap();
    assert_eq!(q.shield, 0, "the ward is gone");
    assert!(q.hp < HEALTH - 20, "and the strike went through: {}", q.hp);
}

#[test]
fn a_fireball_that_strikes_deals_all_of_it() {
    let (mut w, _, b) = duel(10.0);
    let mut ev = Vec::new();
    give(&mut w, 0, 0, spell::FIREBALL);
    cast_now(&mut w, 0, 0, &mut ev);
    fly(&mut w, 20, &mut ev);
    let taken: i32 = ev
        .iter()
        .filter_map(|e| match e {
            Event::Hit { to, amount, .. } if *to == b => Some(*amount as i32),
            _ => None,
        })
        .sum();
    assert_eq!(taken, power(spell::FIREBALL, 1), "struck once, in full");
}

#[test]
fn gust_throws_a_tethered_wizard_off_its_rope() {
    let (mut w, _, b) = duel(4.0);
    let mut ev = Vec::new();
    let k = w.players.iter().position(|p| p.id == b).unwrap();
    // Hauling in on its rope, past the caster.
    let p = w.players[0].body.p;
    w.players[k].body.anchor = [p[0] - 10.0, p[1] + 1.0, p[2]];
    w.players[k].body.tether = TETHER_TICKS;
    give(&mut w, 0, 2, spell::GUST);
    cast_now(&mut w, 0, 2, &mut ev);
    assert_eq!(w.players[k].body.tether, 0, "let go");
    let before = w.players[k].body.p[0];
    for _ in 0..5 {
        crate::motion::step(&mut w.players[k].body, &Input::default(), &w.map);
    }
    assert!(w.players[k].body.p[0] > before, "and away it goes");
}

#[test]
fn every_rank_of_tether_pulls_long_enough_to_get_there() {
    let map = crate::map::Map::new(11);
    for rank in 1..=MAX_RANK {
        let reach = power(spell::TETHER, rank);
        // High over the island, the far end of its reach.
        let mut b = Body {
            p: [-100.0, 120.0, 0.0],
            ..Body::default()
        };
        b.anchor = [b.p[0] + reach as f32, b.p[1], 0.0];
        b.tether = tether_ticks(reach);
        let mut closest = f32::MAX;
        while b.tether > 0 {
            crate::motion::step(&mut b, &Input::default(), &map);
            closest = closest.min((b.p[0] - b.anchor[0]).abs());
        }
        assert!(
            closest < TETHER_ARRIVE + 0.5,
            "rank {rank}: {closest} m short"
        );
    }
}

#[test]
fn blink_goes_the_way_you_steer_and_is_not_spent_against_a_wall() {
    let (mut w, _, _) = duel(30.0);
    let mut ev = Vec::new();
    give(&mut w, 0, 3, spell::BLINK);
    let before = w.players[0].body.p;
    // Facing east, steering left (north, toward -z).
    w.players[0].last.keys = keys::LEFT;
    cast_now(&mut w, 0, 3, &mut ev);
    let after = w.players[0].body.p;
    assert!(
        before[2] - after[2] > 5.0 && (after[0] - before[0]).abs() < 1.0,
        "sideways: {before:?} -> {after:?}"
    );
    // Facing the tower's wall: nowhere to go, nothing spent.
    let (x, z) = (-TOWER_RADIUS - RADIUS - 0.1, 0.0);
    w.players[0].body.p = [x, w.map.height(x, z), z];
    w.players[0].last.keys = 0;
    w.players[0].cds[3] = 0;
    w.players[0].body.chill = 30;
    ev.clear();
    cast_now(&mut w, 0, 3, &mut ev);
    assert_eq!(w.players[0].cds[3], 0, "not spent");
    assert_eq!(w.players[0].body.chill, 30, "nor the chill shaken");
    assert!(ev.is_empty(), "{ev:?}");
}
