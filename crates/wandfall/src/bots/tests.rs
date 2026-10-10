//! Bots' minds, a tick at a time, on the Spire's open plaza.

use super::sense::{self, Mark};
use super::*;
use crate::motion::Body;
use crate::world::{Phase, Slot};

/// A fight on the plaza: one bot at `at` facing east, and a person
/// standing at each of `foes` (x, z on the plaza's line z = -12).
fn scene(at: f32, foes: &[f32]) -> (World, usize, Vec<u16>) {
    let mut w = World::new(21);
    let ids: Vec<u16> = (0..foes.len())
        .map(|n| w.join(&format!("foe {n}"), 0))
        .collect();
    while w.phase != Phase::Fight {
        w.step();
    }
    let bot = w.players.iter().find(|p| p.bot).unwrap().id;
    w.players.retain(|p| !p.bot || p.id == bot);
    // Long into the fight: past the calm of the drop.
    w.tick += 200 * TICK_HZ;
    let z = -12.0;
    let stand = |x: f32, w: &World| Body {
        p: [x, w.map.height(x, z), z],
        ground: true,
        ..Body::default()
    };
    for (id, &x) in ids.iter().zip(foes) {
        let b = stand(x, &w);
        w.find_mut(*id).unwrap().body = b;
    }
    let b = stand(at, &w);
    let k = w.players.iter().position(|p| p.id == bot).unwrap();
    w.players[k].body = b;
    w.players[k].yaw = 0;
    (w, k, ids)
}

/// A storm closed to nothing at `c`.
fn closed(c: [f32; 2]) -> Now {
    Now {
        centre: c,
        r: 0.0,
        next: (c, 0.0),
        phase: STORM.len(),
        dps: STORM_END_DPS,
        ..Now::default()
    }
}

/// How many of `ticks` the bot at `k` fires on, thinking (not moving).
fn fires(w: &mut World, k: usize, storm: &Now, ticks: u32) -> u32 {
    let mut n = 0;
    for _ in 0..ticks {
        w.tick += 1;
        let (i, m) = think(w, k, storm, w.tick);
        let p = &mut w.players[k];
        (p.mind, p.yaw, p.pitch) = (m, i.yaw, i.pitch);
        n += (i.keys & keys::FIRE != 0) as u32;
    }
    n
}

#[test]
fn in_the_last_circle_a_bot_still_fights() {
    // At the middle, and well out of it (running in): it shoots back.
    for at in [0.0, -20.0] {
        let (mut w, k, _) = scene(at, &[at + 10.0]);
        let c = [at.max(0.0), -12.0];
        let shots = fires(&mut w, k, &closed(c), 90);
        assert!(shots > 30, "from {at}: fired on {shots} of 90 ticks");
    }
}

#[test]
fn it_keeps_to_the_one_it_fights() {
    let (mut w, k, ids) = scene(0.0, &[20.0, 22.0]);
    let mut m = w.players[k].mind;
    let first = sense::mark(&w, &w.players[k], &mut m, w.tick).unwrap();
    assert_eq!(first.id, ids[0], "the nearer");
    let ready = m.ready_at;
    // The other comes a little nearer: it stays on its first.
    let q = w.find_mut(ids[1]).unwrap();
    q.body.p[0] = 18.0;
    let again = sense::mark(&w, &w.players[k], &mut m, w.tick + 1).unwrap();
    assert_eq!(again.id, ids[0]);
    assert_eq!(m.ready_at, ready, "no fresh delay");
    // Much nearer: it turns to it.
    w.find_mut(ids[1]).unwrap().body.p[0] = 8.0;
    let near = sense::mark(&w, &w.players[k], &mut m, w.tick + 2).unwrap();
    assert_eq!(near.id, ids[1]);
}

#[test]
fn it_sees_ahead_hears_the_near_and_knows_who_hurt_it() {
    // Behind it, 14 m off: unseen; 8 m off: heard.
    let (w, k, _) = scene(0.0, &[-14.0]);
    let mut m = w.players[k].mind;
    assert!(sense::mark(&w, &w.players[k], &mut m, w.tick).is_none());
    let (w, k, _) = scene(0.0, &[-8.0]);
    let mut m = w.players[k].mind;
    assert!(sense::mark(&w, &w.players[k], &mut m, w.tick).is_some());
    // Hurt from behind: it knows by whom.
    let (mut w, k, ids) = scene(0.0, &[-14.0]);
    let tick = w.tick;
    let p = &mut w.players[k];
    (p.hurt_by, p.hurt_by_at) = (ids[0], tick);
    let mut m = w.players[k].mind;
    let mk = sense::mark(&w, &w.players[k], &mut m, tick).unwrap();
    assert!(mk.id == ids[0] && mk.seen);
}

#[test]
fn the_lance_is_aimed_where_they_are_the_wand_ahead_of_them() {
    let (mut w, k, ids) = scene(-15.0, &[15.0]);
    // Running across its sight.
    w.find_mut(ids[0]).unwrap().body.v = [0.0, 0.0, RUN];
    let mut m = w.players[k].mind;
    let mk: Mark = sense::mark(&w, &w.players[k], &mut m, w.tick).unwrap();
    let me = &w.players[k];
    let straight = trig::heading((mk.chest[2] - me.eye()[2]).atan2(mk.chest[0] - me.eye()[0]));
    let off = |spell| {
        let n = 400;
        let sum: f32 = (0..n)
            .map(|t| {
                fight::aim(me, &mk, spell, t * 4, m.seed)
                    .0
                    .wrapping_sub(straight) as i16 as f32
            })
            .sum();
        sum / n as f32
    };
    let (lance, wand) = (off(Some(spell::LANCE)), off(None));
    assert!(lance.abs() * 4.0 < wand.abs(), "lance {lance}, wand {wand}");
}

#[test]
fn chilled_it_blinks_away_from_its_foe() {
    let (mut w, k, ids) = scene(0.0, &[9.0]);
    w.players[k].slots[2] = Some(Slot {
        spell: spell::BLINK,
        rank: 1,
    });
    let foe = |w: &World| w.find(ids[0]).unwrap().body.p;
    for _ in 0..600 {
        w.players[k].body.chill = 30;
        let before = w.players[k].body.p;
        w.step();
        if w.players[k].cds[2] > 0 {
            let after = w.players[k].body.p;
            let d = |p: [f32; 3]| (p[0] - foe(&w)[0]).hypot(p[2] - foe(&w)[2]);
            assert!(d(after) > d(before) + 3.0, "{before:?} -> {after:?}");
            return;
        }
    }
    panic!("it never blinked");
}

#[test]
fn a_cube_out_of_reach_is_not_gone_for() {
    let (mut w, k, _) = scene(0.0, &[]);
    let p = w.players[k].body.p;
    w.scrolls.clear();
    let up = crate::loot::Scroll {
        id: 1,
        spell: spell::LANCE,
        rank: 1,
        p: [p[0] + 6.0, p[1] + 12.0, p[2]],
    };
    w.scrolls.push(up);
    let storm = Now {
        r: STORM_START,
        next: ([0.0, 0.0], STORM_START),
        ..Now::default()
    };
    let mut m = w.players[k].mind;
    let plan = walk::plan(&w, &w.players[k], &mut m, &storm, None, w.tick);
    assert_eq!(plan.cube, None, "12 m up");
    w.scrolls[0].p[1] = p[1] + 0.5;
    let plan = walk::plan(&w, &w.players[k], &mut m, &storm, None, w.tick);
    assert_eq!(plan.cube.map(|c| c.0), Some(1), "on the ground");
}

#[test]
fn a_strafing_dummy_keeps_to_its_post() {
    let mut w = World::new(0x5eed_0007);
    crate::practice::setup(&mut w);
    w.join("me", 0);
    for _ in 0..10 * 60 * TICK_HZ {
        w.step();
    }
    for p in w.players.iter().filter(|p| p.mind.dummy >= 2) {
        let h = p.mind.home;
        let off = (p.body.p[0] - h[0]).hypot(p.body.p[2] - h[1]);
        assert!(off < DUMMY_LEASH * 2.0, "{} strayed {off} m", p.name);
    }
}

#[test]
fn keys_toward_walk_where_they_mean() {
    assert_eq!(keys_toward(0, 0), keys::FWD);
    assert_eq!(keys_toward(0, 16384), keys::RIGHT);
    assert_eq!(keys_toward(1000, 1000 + 32768), keys::BACK);
    assert_eq!(keys_toward(0, 49152 + 8192), keys::FWD | keys::LEFT);
}
