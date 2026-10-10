use engine::rng::Rng;
use wyrm::laws::{body_len, CROWD, DEATH_DROP, START_MASS, TICK_HZ};
use wyrm::mirror::Mirror;
use wyrm::proto::{self, unq, Down, Up};
use wyrm::view::Viewer;
use wyrm::world::World;

#[test]
fn a_new_arena_is_busy_and_bots_live_a_while() {
    let mut w = World::new(7);
    assert_eq!(w.snakes.len(), CROWD);
    let (mut deaths, mut crowd) = (0, 0);
    let ticks = TICK_HZ as usize * 60;
    for _ in 0..ticks {
        deaths += w.step().len();
        crowd += w.snakes.len();
    }
    // A minute of bots: some die, most do not, and the dead are replaced.
    eprintln!("{deaths} deaths, {} snakes on average", crowd / ticks);
    assert!(deaths < CROWD * 3, "{deaths} deaths in a minute");
    assert!(crowd / ticks >= CROWD - 4, "bots come back");
    let biggest = w.snakes.iter().map(|s| s.mass).fold(0.0, f32::max);
    assert!(biggest > START_MASS * 3.0, "bots eat and grow: {biggest}");
}

#[test]
fn a_person_joins_steers_and_bots_make_room() {
    let mut w = World::new(3);
    let me = w.spawn("me", None);
    assert_eq!(w.humans(), 1);
    let s = w.find(me).unwrap();
    assert_eq!(s.body.len(), body_len(START_MASS));
    w.steer(me, 1.0, false);
    for _ in 0..40 {
        w.step();
        if w.find(me).is_none() {
            break;
        }
    }
    // The crowd stays the same size: a bot stepped out for the person.
    assert!(w.snakes.len() <= CROWD + 1);
}

#[test]
fn a_dead_snake_becomes_food_where_it_lay() {
    let mut w = World::new(5);
    let me = w.spawn("me", None);
    let before = w.food.len();
    let d = w.remove(me).expect("it was there");
    assert!(d.human);
    assert!(w.food.len() > before, "it left food behind");
    assert!(w.find(me).is_none());
}

#[test]
fn a_dead_snake_leaves_its_share_whatever_its_size() {
    for mass in [12.0, 37.0, 100.0, 1000.0, 3000.0, 40_000.0] {
        let mut w = World::new(5);
        let me = w.spawn("me", None);
        let s = w.snakes.iter_mut().find(|s| s.id == me).unwrap();
        s.mass = mass;
        // A body as long as that mass makes, behind its head.
        let head = s.head();
        s.body = (0..body_len(mass))
            .map(|k| (head.0 - k as f32 * 9.0, head.1))
            .collect();
        let worth = |w: &World| w.food.values().map(|f| f.value as u64).sum::<u64>();
        let before = worth(&w);
        w.remove(me);
        let dropped = worth(&w) - before;
        assert_eq!(dropped, (mass * DEATH_DROP).round() as u64, "mass {mass}");
    }
}

#[test]
fn a_bot_makes_room_without_a_burst() {
    let mut w = World::new(4);
    for _ in 0..40 {
        w.step();
    }
    assert_eq!(w.snakes.len(), CROWD);
    let food = w.food.len();
    w.spawn("me", None);
    w.step();
    assert_eq!(w.snakes.len(), CROWD, "a bot stepped out");
    assert!(w.bursts.is_empty(), "quietly: no ring");
    assert!(w.food.len() <= food + 20, "and no feast left behind");
}

#[test]
fn a_bot_never_steps_out_where_someone_sees_it() {
    let bots = |w: &World| w.snakes.iter().filter(|s| s.bot.is_some()).count();
    let inside = |b: (f32, f32, f32, f32), e: (f32, f32, f32, f32)| {
        b.2 >= e.0 && b.0 <= e.2 && b.3 >= e.1 && b.1 <= e.3
    };
    for (eye, everything) in [
        ((-900.0, -600.0, 900.0, 600.0), false),
        ((-3e3, -3e3, 3e3, 3e3), true),
    ] {
        let mut w = World::new(4);
        for _ in 0..40 {
            w.step();
        }
        w.eyes = vec![eye];
        // People arrive, and bots make room for them.
        for k in 0..6 {
            w.spawn(&format!("person {k}"), None);
        }
        let (start, mut left) = (bots(&w), 0);
        for _ in 0..TICK_HZ * 2 {
            let was: Vec<(u16, (f32, f32, f32, f32))> =
                w.snakes.iter().map(|s| (s.id, s.bbox)).collect();
            let died: Vec<u16> = w.step().iter().map(|d| d.id).collect();
            for (id, bbox) in was {
                if w.find(id).is_none() && !died.contains(&id) {
                    assert!(!inside(bbox, eye), "{id} left in plain view");
                    left += 1;
                }
            }
        }
        // With all the arena in view, they wait; else they go.
        assert_eq!(left == 0, everything, "{left} of {start} left");
    }
}

#[test]
fn headings_stay_within_a_turn() {
    let mut w = World::new(8);
    let me = w.spawn("me", None);
    for t in 0..TICK_HZ * 120 {
        // Coil one way for good.
        w.steer(me, t as f32 * 0.3, false);
        w.step();
        for s in &w.snakes {
            assert!(s.angle.abs() <= std::f32::consts::PI, "{}", s.angle);
        }
    }
}

#[test]
fn the_browser_rebuilds_every_body_exactly() {
    let mut w = World::new(11);
    let me = w.spawn("me", None);
    let mut v = Viewer::default();
    v.you = me;
    let mut m = Mirror::default();
    let mut rng = Rng::new(1);
    for t in 0..TICK_HZ * 40 {
        if t % 10 == 0 {
            let a = (rng.below(628) as f32) / 100.0;
            w.steer(me, a, rng.chance(1, 3));
        }
        w.step();
        let bytes = v.frame(&w);
        let Some(Down::Frame(f)) = Down::decode(&bytes) else {
            panic!("a frame decodes");
        };
        m.apply(&f);
        for s in m.snakes.values() {
            let real = w.find(s.id).expect("only live snakes are shown");
            assert_eq!(s.body.len(), real.body.len(), "length of {}", s.name);
            for (a, b) in s.body.iter().zip(real.body.iter()) {
                assert_eq!(*a, (unq(proto::q(b.0)), unq(proto::q(b.1))));
            }
        }
        if w.find(me).is_none() {
            v.you = w.spawn("me", None);
        }
    }
    assert!(!m.snakes.is_empty());
    assert!(!m.food.is_empty());
}

#[test]
fn messages_round_trip() {
    for up in [
        Up::Join {
            name: "zoë 🐍".into(),
        },
        Up::Steer {
            angle: 12345,
            boost: true,
        },
        Up::Screen { w: 1280, h: 800 },
    ] {
        assert_eq!(Up::decode(&up.encode()), Some(up));
    }
    match Down::decode(&proto::died("noodle", 321, 4, 95)) {
        Some(Down::Died {
            by,
            score,
            kills,
            secs,
        }) => assert_eq!((by.as_str(), score, kills, secs), ("noodle", 321, 4, 95)),
        other => panic!("{other:?}"),
    }
    match Down::decode(&proto::feed(7, "noodle", "bean", 50)) {
        Some(Down::Feed {
            killer_id,
            killer,
            victim,
            score,
        }) => assert_eq!(
            (killer_id, killer.as_str(), victim.as_str(), score),
            (7, "noodle", "bean", 50)
        ),
        other => panic!("{other:?}"),
    }
}

#[test]
fn hostile_bytes_never_panic() {
    let mut rng = Rng::new(99);
    let mut w = World::new(2);
    let mut v = Viewer::default();
    w.step();
    let good = v.frame(&w);
    for _ in 0..20_000 {
        let n = rng.below(64) as usize;
        let mut b: Vec<u8> = (0..n).map(|_| rng.below(256) as u8).collect();
        let _ = Up::decode(&b);
        let _ = Down::decode(&b);
        // Corrupt a real frame too: counts that lie must not allocate or panic.
        b = good.clone();
        let i = rng.below(b.len() as u64) as usize;
        b[i] = rng.below(256) as u8;
        b.truncate(rng.below(b.len() as u64 + 1) as usize);
        let _ = Down::decode(&b);
    }
}

#[test]
fn a_watcher_sees_the_game_but_cannot_play_and_is_not_counted() {
    use engine::room::{Outbox, Room, Who};
    let mut room = wyrm::room::Wyrm::new(9);
    let mut out = Outbox::default();
    room.open(1, &Who::guest(true), &mut out);
    room.open(2, &Who::guest(false), &mut out);
    let join = Up::Join { name: "me".into() }.encode();
    room.message(1, &join, &mut out);
    room.message(2, &join, &mut out);
    out.0.clear();
    room.tick(&mut out);
    assert_eq!(
        room.people(),
        1,
        "only the player is one of the people here"
    );
    assert!(room.playing() > 1, "the bots are playing too");
    let frames: Vec<Down> = out
        .0
        .iter()
        .filter(|(conn, _)| *conn == 1)
        .filter_map(|(_, b)| Down::decode(b))
        .collect();
    let Some(Down::Frame(f)) = frames.first() else {
        panic!("the watcher is sent frames");
    };
    assert_eq!(f.you, 0, "the watcher has no snake");
    assert!(!f.snakes.is_empty(), "and sees the game");
    room.close(1);
    room.close(2);
    assert_eq!(room.people(), 0);
}

#[test]
fn a_restart_brings_a_soul_back_to_its_snake() {
    use engine::room::{Outbox, Room, Who};
    let who = Who {
        soul: 42,
        name: "zoë".into(),
        watch: false,
        build: 0,
    };
    let mut room = wyrm::room::Wyrm::new(9);
    let mut out = Outbox::default();
    room.open(1, &who, &mut out);
    // A soul goes by its own name, whatever the page asks for.
    room.message(1, &Up::Join { name: "x".into() }.encode(), &mut out);
    for _ in 0..30 {
        room.tick(&mut out);
    }
    let you = |out: &Outbox, conn: u32| {
        out.0
            .iter()
            .rev()
            .filter(|(c, _)| *c == conn)
            .find_map(|(_, b)| match Down::decode(b) {
                Some(Down::Frame(f)) => Some(f.you),
                _ => None,
            })
    };
    assert_ne!(you(&out, 1), Some(0));
    let saved = room.save().expect("an arena is worth keeping");

    let mut back = wyrm::room::Wyrm::new(10);
    back.load(&saved).unwrap();
    let mut out = Outbox::default();
    back.open(7, &who, &mut out);
    back.message(7, &Up::Join { name: "x".into() }.encode(), &mut out);
    back.tick(&mut out);
    let Some(id) = you(&out, 7).filter(|&id| id != 0) else {
        panic!("it is playing again");
    };
    let names: Vec<String> = out
        .0
        .iter()
        .filter_map(|(_, b)| match Down::decode(b) {
            Some(Down::Frame(f)) => Some(f),
            _ => None,
        })
        .flat_map(|f| f.snakes.into_iter())
        .filter(|s| s.id == id)
        .filter_map(|s| s.new.map(|n| n.0))
        .collect();
    assert_eq!(names.first().map(String::as_str), Some("zoë"));
}

#[test]
fn the_browser_rebuilds_every_body_exactly_whatever_it_watches() {
    let mut w = World::new(13);
    let mut rng = Rng::new(5);
    // Four browsers: three play (and, dead, watch their killer), one only
    // watches; their screens change shape now and then.
    let mut views: Vec<(Viewer, Mirror)> = (0..4)
        .map(|_| (Viewer::default(), Mirror::default()))
        .collect();
    for (v, _) in views.iter_mut().take(3) {
        v.you = w.spawn("me", None);
    }
    let (mut checked, mut watched) = (0usize, 0usize);
    for t in 0..TICK_HZ * 300 {
        for (k, (v, _)) in views.iter_mut().enumerate() {
            if t % 13 == k as u32 {
                let sw = 150 + rng.below(5000) as u16;
                let sh = 150 + rng.below(3000) as u16;
                v.screen = (sw as f32, sh as f32);
            }
            if v.you != 0 && t % 10 == 0 {
                let a = (rng.below(628) as f32) / 100.0;
                w.steer(v.you, a, rng.chance(1, 3));
            }
            if k < 3 && v.you == 0 && t % 97 == 0 {
                v.you = w.spawn("me", None);
            }
        }
        for d in w.step() {
            for (v, _) in views.iter_mut().filter(|(v, _)| v.you == d.id) {
                v.you = 0;
                v.watching = d.killer.as_ref().map_or(0, |k| k.0);
            }
        }
        for (v, m) in views.iter_mut() {
            let Some(Down::Frame(f)) = Down::decode(&v.frame(&w)) else {
                panic!("a frame decodes");
            };
            m.apply(&f);
            watched += (v.you == 0) as usize;
            for s in m.snakes.values() {
                let real = w.find(s.id).expect("only live snakes are shown");
                assert_eq!(s.body.len(), real.body.len(), "length of {}", s.name);
                for (a, b) in s.body.iter().zip(real.body.iter()) {
                    assert_eq!(*a, (unq(proto::q(b.0)), unq(proto::q(b.1))));
                }
                checked += 1;
            }
            for (id, p) in &m.food {
                let real = w.food.get(id).expect("only food still there is shown");
                let at = (unq(proto::q(real.x)), unq(proto::q(real.y)));
                assert_eq!((p.x, p.y, p.value), (at.0, at.1, real.value));
            }
        }
    }
    assert!(checked > 10_000 && watched > 1000, "{checked} {watched}");
}

#[test]
fn a_guest_goes_by_the_name_the_server_gave_it() {
    use engine::room::{Outbox, Room, Who};
    let mut room = wyrm::room::Wyrm::new(9);
    let mut out = Outbox::default();
    // One that never said Hello, asking for a bot's name.
    room.open(1, &Who::guest(false), &mut out);
    room.message(
        1,
        &Up::Join {
            name: "noodle".into(),
        }
        .encode(),
        &mut out,
    );
    // One the server named, though it was not given a soul.
    let named = Who {
        name: "zed".into(),
        ..Who::guest(false)
    };
    room.open(2, &named, &mut out);
    room.message(2, &Up::Join { name: "x".into() }.encode(), &mut out);
    out.0.clear();
    room.tick(&mut out);
    let names: Vec<String> = out
        .0
        .iter()
        .filter_map(|(_, b)| match Down::decode(b) {
            Some(Down::Frame(f)) => Some(f),
            _ => None,
        })
        .flat_map(|f| {
            let you = f.you;
            f.snakes
                .into_iter()
                .filter(move |s| s.id == you)
                .filter_map(|s| s.new.map(|n| n.0))
        })
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(names.iter().any(|n| n == "zed"), "{names:?}");
    assert!(names.iter().any(|n| n.starts_with("snake ")), "{names:?}");
}
