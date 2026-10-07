use engine::rng::Rng;
use wyrm::laws::{body_len, CROWD, START_MASS, TICK_HZ};
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
    match Down::decode(&proto::died("noodle", 321)) {
        Some(Down::Died { by, score }) => assert_eq!((by.as_str(), score), ("noodle", 321)),
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
