//! The wire's promises: hostile bytes never panic; the page rebuilds every
//! tile and body exactly; prediction matches the server bit for bit;
//! residents act only through what a thumb can.

use engine::fixed::Fx;
use engine::rng::Rng;
use engine::room::{Outbox, Room, Who};
use luciphon::laws::LAWS;
use luciphon::mirror::Mirror;
use luciphon::motion::{Intent, Verb};
use luciphon::predict::Predictor;
use luciphon::proto::{Down, Up, PROTO};
use luciphon::room::Luciphon;
use luciphon::thumb::Thumb;
use luciphon::view::{ents, Viewer};
use luciphon::world::World;

#[test]
fn hostile_bytes_never_panic() {
    let mut room = Luciphon::new(1);
    let mut out = Outbox::default();
    room.open(1, &Who::guest(false), &mut out);
    room.message(1, &Up::Join { proto: PROTO }.encode(), &mut out);
    let mut rng = Rng::new(9);
    let good: Vec<Vec<u8>> = vec![
        Up::Join { proto: PROTO }.encode(),
        Up::Input {
            seq: 1,
            it: Intent {
                heading: 5,
                throttle: 255,
                verb: Verb::None,
                aim: 0,
                jump: true,
            },
        }
        .encode(),
        Up::Ping { t: 1, rtt: 9 }.encode(),
        engine::who::Hello {
            proto: 1,
            key: [3; 16],
            name: "x".into(),
            rename: false,
            build: 0,
        }
        .encode(),
    ];
    for k in 0..20_000 {
        let mut b = good[k % good.len()].clone();
        for _ in 0..rng.below(4) {
            let i = rng.below(b.len() as u64) as usize;
            b[i] = rng.below(256) as u8;
        }
        b.truncate(rng.below(b.len() as u64 + 2) as usize);
        let _ = Up::decode(&b);
        room.message(1, &b, &mut out);
        if k % 100 == 0 {
            room.tick(&mut out);
        }
        let _ = Down::decode(&b);
    }
    // Downs, mangled, too.
    out.0.clear();
    room.tick(&mut out);
    for (_, msg) in out.0.iter().take(20) {
        for _ in 0..200 {
            let mut b = msg.clone();
            let i = rng.below(b.len() as u64) as usize;
            b[i] ^= 1 << rng.below(8);
            b.truncate(rng.below(b.len() as u64 + 1) as usize);
            let _ = Down::decode(&b);
        }
    }
}

#[test]
fn the_browser_rebuilds_every_tile_and_body_exactly() {
    let mut room = Luciphon::new(4);
    let mut out = Outbox::default();
    let who = Who {
        soul: 7,
        name: "me".into(),
        watch: false,
        build: 0,
    };
    room.open(1, &who, &mut out);
    room.message(1, &Up::Join { proto: PROTO }.encode(), &mut out);
    let mut m = Mirror::default();
    let mut rng = Rng::new(2);
    let mut seq = 0u16;
    let mut thumb = Thumb::default();
    let mut checked = 0;
    for t in 0..30 * 120 {
        // Run around, now and then strike or dash.
        let raw = Intent {
            heading: ((t / 40) as u16).wrapping_mul(9000),
            throttle: if t % 50 < 44 { 255 } else { 0 },
            verb: match t % 50 {
                44 => Verb::Flick,
                47 => Verb::Tap,
                _ => Verb::None,
            },
            aim: rng.below(65536) as u16,
            jump: t % 50 == 20,
        };
        let (it, _) = thumb.fit(raw);
        seq = seq.wrapping_add(1);
        room.message(1, &Up::Input { seq, it }.encode(), &mut out);
        out.0.clear();
        room.tick(&mut out);
        for (_, msg) in &out.0 {
            if let Some(d) = Down::decode(msg) {
                m.apply(&d);
            }
        }
        // Every known chunk is the world's, tile for tile.
        let w = room.world();
        for &(cx, cy) in &m.chunks {
            assert_eq!(m.tiles.chunk(cx, cy), w.tiles.chunk(cx, cy));
        }
        // Every entity the page knows is exactly as the server has it now.
        let now = ents(w);
        for e in m.ents.values() {
            let real = now
                .iter()
                .find(|r| r.0.id == e.id)
                .expect("only live things are known");
            assert_eq!(&real.0, e);
            checked += 1;
        }
        if let (Some(own), Some(me)) = (&m.own, w.lumens.iter().find(|l| l.soul == 7)) {
            assert_eq!(own.me, me.me);
        }
    }
    assert!(checked > 1000, "{checked}");
    assert!(m.chunks.len() >= 2);
}

#[test]
fn prediction_matches_the_server() {
    // One Lumen, inputs through a 3-tick pipe each way, some lost.
    let mut w = World::new(LAWS, 5);
    let id = w.spawn("me", 1, None);
    let mut p = Predictor::default();
    let mut rng = Rng::new(77);
    let mut thumb = Thumb::default();
    let mut up: std::collections::VecDeque<(u32, u16, Intent)> = Default::default();
    let mut down: std::collections::VecDeque<(u32, u16, luciphon::proto::Own)> = Default::default();
    let mut predicted: std::collections::HashMap<u16, luciphon::combat::Me> = Default::default();
    let mut lost_at = 0u32;
    let (mut same, mut compared) = (0, 0);
    let mut heading = 0u16;
    let mut regen = (1000, 0, 0);
    for t in 0..10_000u32 {
        let seq = t as u16;
        // A player: runs, turns, jumps, dashes, strikes, holds.
        if rng.chance(1, 20) {
            heading = heading.wrapping_add(rng.below(40_000) as u16);
        }
        let r = rng.below(100);
        let raw = Intent {
            heading,
            throttle: if r < 80 {
                255
            } else if r < 88 {
                120
            } else {
                0
            },
            verb: match rng.below(40) {
                0 => Verb::Tap,
                1 => Verb::Flick,
                2 => Verb::Hold { held_for: 9 },
                3 => Verb::Release {
                    range: rng.below(3) as u8 * 100,
                },
                4 => Verb::Cancel,
                _ => Verb::None,
            },
            aim: rng.below(65536) as u16,
            jump: rng.chance(1, 30),
        };
        let (it, _) = thumb.fit(raw);
        p.push(seq, it, &w.tiles, &w.laws);
        predicted.insert(seq, p.me);
        if rng.chance(1, 300) {
            lost_at = t;
        } else {
            up.push_back((t + 3, seq, it));
        }
        // The server, three ticks on.
        while up.front().is_some_and(|u| u.0 <= t) {
            let (_, s, i) = up.pop_front().unwrap();
            w.intend(id, s, i);
        }
        w.step();
        // Thorns throw a body about, and a lance that misses is slow to
        // recover: that is the server's, not predicted.
        use luciphon::world::Event;
        if w.events
            .iter()
            .any(|e| matches!(e, Event::Thorns { .. } | Event::Whiff { .. }))
        {
            lost_at = t;
        }
        let l = w.find(id).unwrap();
        // Rekindled running out is the server's clock, not predicted.
        // So are a gathered bag's weight and a hearth's land.
        if (l.me.body.regen, l.me.body.load, l.me.body.claim) != regen {
            regen = (l.me.body.regen, l.me.body.load, l.me.body.claim);
            lost_at = t;
        }
        if l.ack == seq.wrapping_sub(3) && t > lost_at + 12 && l.alive() && l.ghost == 0 {
            if let Some(mine) = predicted.get(&l.ack) {
                compared += 1;
                if *mine == l.me {
                    same += 1;
                }
            }
        }
        down.push_back((t + 3, l.ack, luciphon::view::own(l)));
        while down.front().is_some_and(|d| d.0 <= t) {
            let (_, ack, own) = down.pop_front().unwrap();
            p.reconcile(&own, ack, &w.tiles, &w.laws);
        }
        if !w.find(id).unwrap().alive() {
            lost_at = t;
        }
    }
    eprintln!("{same} of {compared} predicted ticks were exact");
    assert!(compared > 5_000, "{compared}");
    assert_eq!(same, compared);
}

#[test]
fn bots_use_only_what_a_thumb_can() {
    let mut w = World::new(LAWS, 3);
    for k in 0..8 {
        let b = luciphon::bots::Brain::new(&mut w.rng);
        w.spawn(luciphon::bots::NAMES[k], 0, Some(b));
    }
    let mut thumbs = [Thumb::default(); 8];
    let mut acks = [0u16; 8];
    let mut bent = 0;
    for _ in 0..30 * 120 {
        w.step();
        for (k, l) in w.lumens.iter().enumerate() {
            if !l.alive() {
                // Back from the Underlight with a fresh thumb.
                thumbs[k] = Thumb::default();
            }
            // At most one Intent a tick, and only what a thumb can make.
            assert!(l.queue.is_empty());
            if l.ack != acks[k] {
                assert_eq!(l.ack, acks[k].wrapping_add(1), "one a tick");
                acks[k] = l.ack;
                if thumbs[k].fit(l.last).1 {
                    bent += 1;
                }
            }
        }
    }
    assert_eq!(bent, 0, "residents made {bent} Intents no thumb could");
    let _ = Fx::ZERO;
    let _ = Viewer::default();
}

#[test]
fn a_page_too_old_is_told_and_not_let_in() {
    let mut room = Luciphon::new(6);
    let mut out = Outbox::default();
    let who = Who {
        soul: 11,
        name: "old".into(),
        watch: false,
        build: 0,
    };
    room.open(1, &who, &mut out);
    out.0.clear();
    room.message(1, &Up::Join { proto: PROTO - 1 }.encode(), &mut out);
    let told = out.0.iter().any(|(_, m)| {
        matches!(Down::decode(m), Some(Down::Welcome { you: 0, oldest, .. }) if oldest > PROTO - 1)
    });
    assert!(told, "a Welcome saying how old is too old");
    assert!(room.world().lumens.iter().all(|l| l.soul != 11));
    room.message(1, &Up::Join { proto: PROTO }.encode(), &mut out);
    assert!(room.world().lumens.iter().any(|l| l.soul == 11));
}
