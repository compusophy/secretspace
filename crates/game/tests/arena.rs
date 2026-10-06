use game::laws::{body_len, CROWD, START_MASS, TICK_HZ};
use game::mirror::Mirror;
use game::proto::{self, unq, Down, Up};
use game::rng::Rng;
use game::view::Viewer;
use game::world::World;

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
