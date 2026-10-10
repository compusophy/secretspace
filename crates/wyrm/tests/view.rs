//! What a browser is sent when its view jumps: a snake it still shows is
//! not sent as gone while its camera eases away, and a new snake of its
//! own is sent only what is round it, where its camera goes at once.

use wyrm::laws::{view, TICK_HZ};
use wyrm::proto::{Down, Frame};
use wyrm::view::Viewer;
use wyrm::world::{Snake, World};

fn frame(v: &mut Viewer, w: &World) -> Frame {
    match Down::decode(&v.frame(w)) {
        Some(Down::Frame(f)) => f,
        _ => panic!("a frame decodes"),
    }
}

/// The box a 1280 x 800 screen shows round `at` while it is not playing,
/// `pad` units bigger each way.
fn shown(at: (f32, f32), pad: f32) -> (f32, f32, f32, f32) {
    let (_, hw, hh) = view(18.0, 1280.0, 800.0);
    let (hw, hh) = (hw + pad, hh + pad);
    (at.0 - hw, at.1 - hh, at.0 + hw, at.1 + hh)
}

fn overlaps(s: &Snake, b: (f32, f32, f32, f32)) -> bool {
    let (x0, y0, x1, y1) = s.bbox;
    x1 >= b.0 && x0 <= b.2 && y1 >= b.1 && y0 <= b.3
}

#[test]
fn a_snake_the_page_still_shows_is_not_sent_as_gone() {
    let mut w = World::new(17);
    for _ in 0..60 {
        w.step();
    }
    // A watcher's view moves from one snake to another far off, as when
    // a dead player goes on to watch their killer.
    let (a, b) = w
        .snakes
        .iter()
        .flat_map(|a| w.snakes.iter().map(move |b| (a, b)))
        .find(|(a, b)| !overlaps(a, shown(b.head(), 400.0)))
        .map(|(a, b)| (a.id, b.id))
        .expect("two snakes far apart");
    let mut v = Viewer::default();
    v.watching = a;
    assert!(frame(&mut v, &w).snakes.iter().any(|s| s.id == a));
    v.watching = b;
    w.step();
    assert!(w.find(a).is_some() && w.find(b).is_some());
    // The page eases its camera across, still showing the first a moment.
    let f = frame(&mut v, &w);
    assert!(!f.gone.contains(&a) && f.snakes.iter().any(|s| s.id == a));
    // Once it is over there, the first is gone.
    let mut gone = false;
    for _ in 0..TICK_HZ * 2 {
        w.step();
        gone |= frame(&mut v, &w).gone.contains(&a);
    }
    assert!(gone, "{a} is let go");
}

#[test]
fn a_new_snake_of_yours_is_sent_only_what_is_round_it() {
    let mut w = World::new(23);
    for _ in 0..60 {
        w.step();
    }
    let me = w.spawn("me", None);
    // A dead player watching the snake furthest from where they will
    // play again.
    let at = w.find(me).expect("spawned").head();
    let far = w
        .snakes
        .iter()
        .filter(|s| s.id != me)
        .max_by(|a, b| {
            let d = |s: &Snake| (s.head().0 - at.0).hypot(s.head().1 - at.1);
            d(a).total_cmp(&d(b))
        })
        .map(|s| s.id)
        .expect("bots");
    let mut v = Viewer::default();
    v.watching = far;
    for _ in 0..TICK_HZ * 2 {
        w.step();
        frame(&mut v, &w);
    }
    // They play: their page puts its camera straight on the new snake,
    // still zoomed as it was, and so does the box sent.
    v.you = me;
    w.step();
    let f = frame(&mut v, &w);
    let around = shown(w.find(me).expect("a ghost lives").head(), 150.0);
    assert!(!overlaps(w.find(far).expect("it lives"), around));
    assert!(f.gone.contains(&far), "what was round {far} goes at once");
    for s in &f.snakes {
        let real = w.find(s.id).expect("only live snakes are sent");
        assert!(overlaps(real, around), "{} is out of view", s.id);
    }
    assert!(f.snakes.iter().any(|s| s.id == me));
}
