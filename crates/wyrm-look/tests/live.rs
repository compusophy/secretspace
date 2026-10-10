//! What a page keeps when its link drops and comes back: the old picture
//! until the new connection's first frame, which then starts afresh, with
//! nothing of the old one left over.

use look::Live;
use wyrm::proto::{self, Down};
use wyrm::view::Viewer;
use wyrm::world::World;

#[test]
fn a_reconnect_keeps_the_picture_until_the_new_one_is_in() {
    let mut w = World::new(31);
    let mut v = Viewer::default();
    v.you = w.spawn("me", None);
    let mut live = Live::default();
    let hello = || Down::decode(&proto::hello(2400, 20)).unwrap();
    live.receive(0.0, hello());
    let mut now = 1000.0;
    for _ in 0..40 {
        w.step();
        now += 50.0;
        live.receive(now, Down::decode(&v.frame(&w)).unwrap());
    }
    // The link drops; the new connection's Hello comes first.
    let was: Vec<u16> = live.mirror.snakes.keys().copied().collect();
    let food = live.mirror.food.len();
    now += 2000.0;
    assert!(live.receive(now, hello()).is_none());
    assert_eq!(live.mirror.snakes.keys().copied().collect::<Vec<_>>(), was);
    assert_eq!(live.mirror.food.len(), food, "the old picture stays");

    // Its first frame, from a viewer that knows nothing yet and sees less
    // (a phone, say): only what it says is drawn, and nothing jumps in
    // from the old picture's timing.
    let mut fresh = Viewer::default();
    fresh.you = v.you;
    fresh.screen = (390.0, 844.0);
    w.step();
    now += 10.0;
    let Some(Down::Frame(f)) = Down::decode(&fresh.frame(&w)) else {
        panic!("a frame decodes");
    };
    assert!(was.iter().any(|id| f.snakes.iter().all(|s| s.id != *id)));
    live.receive(now, Down::Frame(f.clone()));
    let mut ids: Vec<u16> = f.snakes.iter().map(|s| s.id).collect();
    ids.sort_unstable();
    assert_eq!(live.mirror.snakes.keys().copied().collect::<Vec<_>>(), ids);
    assert_eq!(live.mirror.food.len(), f.food.len());
    assert!(live.gulps.is_empty());
    for s in live.mirror.snakes.values() {
        assert_eq!((s.lag, s.short), (0.0, 0.0), "snake {}", s.id);
    }
    assert_eq!(live.alpha(now), 0.0);
}
