//! Stage 2's "done when", played through the world: gather from nothing,
//! build a hearth, close a loop, plant and harvest Sunwheat, fall in the
//! Dim and lose half the bag, and come back from a save with bag, hearth
//! and land intact. And the save's own promises.

use engine::fixed::{atan2, len, Fx};
use luciphon::build::{act, slot};
use luciphon::laws::{HZ, LAWS};
use luciphon::motion::{Intent, Verb};
use luciphon::tiles::{obj, Tiles};
use luciphon::world::{Cause, World};

struct Player {
    id: u16,
    seq: u16,
}

impl Player {
    fn go(&mut self, w: &mut World, it: Intent) {
        self.seq = self.seq.wrapping_add(1);
        w.intend(self.id, self.seq, it);
        w.step();
    }

    fn i(&self, w: &World) -> usize {
        w.index(self.id).expect("awake")
    }

    fn put(&mut self, w: &mut World, x: f32, y: f32, facing: u16) {
        let i = self.i(w);
        let b = &mut w.lumens[i].me.body;
        b.x = Fx((x * 65536.0) as i32);
        b.y = Fx((y * 65536.0) as i32);
        b.vx = Fx::ZERO;
        b.vy = Fx::ZERO;
        b.facing = facing;
        b.safe = (b.x, b.y);
        w.lumens[i].me.act.aim = facing;
    }

    /// A tap with the stick up, then the strike's whole swing.
    fn tap(&mut self, w: &mut World) {
        self.go(w, Intent::default());
        self.go(
            w,
            Intent {
                verb: Verb::Tap,
                ..Intent::default()
            },
        );
        for _ in 0..14 {
            self.go(w, Intent::default());
        }
    }

    /// Walk to a point (tiles), around nothing: the test picks clear paths.
    fn walk(&mut self, w: &mut World, x: f32, y: f32) {
        for _ in 0..HZ * 20 {
            let (px, py) = w.find(self.id).unwrap().pos();
            let (dx, dy) = (
                Fx((x * 65536.0) as i32).sub(px),
                Fx((y * 65536.0) as i32).sub(py),
            );
            if len(dx, dy) < Fx::milli(300) {
                break;
            }
            let near = len(dx, dy) < Fx::int(2);
            self.go(
                w,
                Intent {
                    heading: atan2(dy, dx),
                    throttle: if near { 80 } else { 255 },
                    ..Intent::default()
                },
            );
        }
        for _ in 0..20 {
            self.go(w, Intent::default());
        }
    }
}

/// A tile with this object, in a ring band, with open ground to its west.
fn find(w: &World, o: u8, r0: i32, r1: i32) -> (i32, i32) {
    for r in r0..r1 {
        for k in 0..360 {
            let a = (k as f64).to_radians();
            let (x, y) = ((a.cos() * r as f64) as i32, (a.sin() * r as f64) as i32);
            let t = w.tiles.get(x, y);
            let west = w.tiles.get(x - 1, y);
            if t.obj == o && !west.void() && !west.solid() && west.obj == obj::NONE {
                return (x, y);
            }
        }
    }
    panic!("no {o} in {r0}..{r1}");
}

fn world() -> (World, Player) {
    let mut w = World::new(LAWS, 21);
    w.unix = 1_800_000_000;
    let id = w.wake(99, "thumb");
    (w, Player { id, seq: 0 })
}

#[test]
fn a_hearth_from_nothing_a_loop_a_harvest_a_fall_and_a_restart() {
    let (mut w, mut p) = world();
    assert_eq!(
        w.find(p.id).unwrap().glim,
        0,
        "a new soul starts with nothing"
    );

    // Gather: a birch struck bare.
    let (bx, by) = find(&w, obj::BIRCH, 14, 30);
    p.put(&mut w, bx as f32 - 0.5, by as f32 + 0.5, 0);
    for _ in 0..12 {
        p.tap(&mut w);
    }
    let me = w.find(p.id).unwrap();
    assert!(me.wood >= 24, "wood {}", me.wood);
    assert_eq!(
        w.tiles.get(bx, by).obj,
        obj::BIRCH | obj::BARE,
        "struck bare"
    );
    // And stone from a rock.
    let (rx, ry) = find(&w, obj::ROCK, 14, 34);
    p.put(&mut w, rx as f32 - 0.5, ry as f32 + 0.5, 0);
    for _ in 0..10 {
        p.tap(&mut w);
    }
    assert!(w.find(p.id).unwrap().stone >= 16);
    // The rest of a hearth's price, and glim to kindle with.
    let i = p.i(&w);
    w.lumens[i].stone = w.lumens[i].stone.max(20);
    w.lumens[i].glim = 90;

    // A hearth, through build mode: a tap on the ghost, a channel.
    let mut placed = None;
    'spots: for r in 20..30 {
        for k in 0..72 {
            let a = (k as f64 * 5.0).to_radians();
            let (cx, cy) = ((a.cos() * r as f64) as i32, (a.sin() * r as f64) as i32);
            p.put(&mut w, cx as f32 - 0.5, cy as f32 + 0.5, 0);
            w.heart(p.id, act::WHEEL, slot::BUILD);
            w.heart(p.id, act::PIECE, 0);
            p.tap(&mut w);
            if w.claim_of(99).and_then(|c| c.hearth).is_some() {
                placed = Some((cx, cy));
                break 'spots;
            }
            w.heart(p.id, act::DONE, 0);
        }
    }
    let (hx, hy) = placed.expect("a hearth found a place");
    let claim = w.claim_of(99).unwrap().id;
    assert_eq!(w.tiles.get(hx, hy).obj, obj::HEARTH);
    assert!(
        w.tiles.get(hx + 2, hy + 2).land_of(claim),
        "the core is ours"
    );

    // Bank on the core: the vault fills, the lamp stays.
    p.put(&mut w, hx as f32 + 0.5, hy as f32 + 2.5, 0);
    p.go(&mut w, Intent::default());
    let me = w.find(p.id).unwrap();
    assert_eq!(me.glim, LAWS.lamp);
    assert!(w.claim_of(99).unwrap().vault.glim > 0);
    assert_eq!(me.wood, 0, "wood banked");

    // Kindle a loop of 20-odd tiles out from the core and back.
    let i = p.i(&w);
    w.lumens[i].glim = 120;
    w.heart(p.id, act::WHEEL, slot::KINDLE);
    let (fx, fy) = (hx as f32 + 0.5, hy as f32 + 0.5);
    // Out of the core's east side, round a 5x4 box, back in.
    let away = if hx >= 0 { 1.0 } else { -1.0 };
    let path = [
        (fx + 2.0 * away, fy + 2.0),
        (fx + 3.0 * away, fy + 2.0),
        (fx + 6.0 * away, fy + 2.0),
        (fx + 6.0 * away, fy - 2.0),
        (fx + 3.0 * away, fy - 2.0),
        (fx + 2.0 * away, fy - 2.0),
    ];
    p.put(&mut w, path[0].0, path[0].1, 0);
    p.go(&mut w, Intent::default());
    for &(x, y) in &path[1..] {
        p.walk(&mut w, x, y);
    }
    let tiles = w.claim_of(99).unwrap().tiles;
    assert!(tiles >= 20, "the loop closed on {tiles} tiles");

    // A planter on our land; Sunwheat ripens in 15 minutes; a strike.
    w.heart(p.id, act::WHEEL, slot::KINDLE);
    let spot = (hx as f32 + 4.5 * away, hy as f32 + 0.5);
    let facing = if away > 0.0 { 0u16 } else { 32768 };
    p.put(&mut w, spot.0 - away, spot.1, facing);
    w.heart(p.id, act::WHEEL, slot::BUILD);
    w.heart(p.id, act::PIECE, 5);
    let i = p.i(&w);
    w.lumens[i].wood = 10;
    p.tap(&mut w);
    let (tx, ty) = (spot.0.floor() as i32, spot.1.floor() as i32);
    assert_eq!(w.tiles.get(tx, ty).obj, obj::SPROUT, "planted");
    w.heart(p.id, act::DONE, 0);
    for _ in 0..LAWS.wheat_ripe + HZ * 2 {
        p.go(&mut w, Intent::default());
    }
    // It dreamed while it waited (20 s idle); it wakes where it slept.
    assert!(w.find(p.id).is_none() && w.dreamers.contains_key(&99));
    p.id = w.wake(99, "thumb");
    assert_eq!(w.tiles.get(tx, ty).obj, obj::RIPE, "ripe");
    p.put(&mut w, spot.0 - away, spot.1, facing);
    p.tap(&mut w);
    assert!(
        w.find(p.id).unwrap().wheat >= LAWS.wheat_harvest,
        "harvested"
    );

    // A fall in the Dim: half the bag stays behind (half its glim gone).
    let i = p.i(&w);
    p.put(&mut w, 0.5, 44.5, 0);
    w.lumens[i].glim = 100;
    w.lumens[i].wood = 40;
    w.gutter(i, 0, Cause::Dark);
    let me = w.find(p.id).unwrap();
    assert_eq!((me.glim, me.wood), (50, 20));
    let dropped: u32 = w.pickups.iter().map(|d| d.glim).sum();
    assert_eq!(dropped, 25, "half the dropped glim went to the Dark");
    for _ in 0..LAWS.descent + 2 {
        p.go(&mut w, Intent::default());
    }

    // A restart: bag, hearth and land as they were.
    let me = w.find(p.id).unwrap();
    let bag = (me.glim, me.wood, me.stone, me.wheat);
    let land = w.claim_of(99).unwrap().clone();
    let saved = w.save();
    let mut back = World::new(LAWS, 5);
    let awake = back.load(&saved).unwrap().unwrap();
    assert_eq!(awake, vec![99]);
    let id = back.wake(99, "thumb");
    let me = back.find(id).unwrap();
    assert_eq!((me.glim, me.wood, me.stone, me.wheat), bag);
    assert_eq!(back.claim_of(99), Some(&land));
    assert_eq!(back.tiles, w.tiles);
}

#[test]
fn snapshot_round_trips_exactly() {
    let (mut w, mut p) = world();
    for _ in 0..200 {
        p.go(
            &mut w,
            Intent {
                heading: 9000,
                throttle: 255,
                ..Intent::default()
            },
        );
    }
    let a = w.save();
    let mut back = World::new(LAWS, 1);
    back.load(&a).unwrap();
    // The awake soul is a sleeper in the copy; wake it to compare.
    back.wake(99, "thumb");
    let b = back.save();
    assert_eq!(a.len(), b.len());
    let mut again = World::new(LAWS, 2);
    again.load(&b).unwrap();
    assert_eq!(again.tiles, back.tiles);
    assert_eq!(again.claims, back.claims);
}

#[test]
fn hostile_snapshots_never_panic() {
    let (mut w, _) = world();
    w.step();
    let good = w.save();
    let mut rng = engine::rng::Rng::new(8);
    for k in 0..3000 {
        let mut bad = good.clone();
        let at = rng.below(bad.len() as u64) as usize;
        bad[at] ^= 1 << (k % 8);
        let mut fresh = World::new(LAWS, 3);
        let _ = fresh.load(&bad);
        let _ = fresh.load(&bad[..at]);
    }
}

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/world-s1.snap");

/// Writes the schema 1 fixture (run once, when schema 1 froze).
#[test]
#[ignore]
fn write_world_fixture() {
    let (mut w, mut p) = world();
    let i = p.i(&w);
    w.lumens[i].wood = 30;
    w.lumens[i].stone = 20;
    w.lumens[i].glim = 60;
    let placed = (20..30)
        .flat_map(|x| (-6..6).map(move |y| (x, y)))
        .find(|&(x, y)| w.place(i, Tiles::index(x, y).unwrap() as u16, obj::HEARTH));
    assert!(placed.is_some(), "a hearth found a place");
    p.go(&mut w, Intent::default());
    std::fs::create_dir_all(std::path::Path::new(FIXTURE).parent().unwrap()).unwrap();
    std::fs::write(FIXTURE, w.save()).unwrap();
}

#[test]
fn every_old_world_still_loads() {
    let bytes = std::fs::read(FIXTURE).expect("the schema 1 fixture");
    assert!(bytes.len() <= 200 * 1024, "{} bytes", bytes.len());
    let mut w = World::new(LAWS, 1);
    let awake = w
        .load(&bytes)
        .expect("schema 1 loads")
        .expect("it is a world");
    assert_eq!(awake, vec![99]);
    let c = w.claim_of(99).expect("its hearth");
    let (hx, hy) = c.hearth.expect("a hearth, not a lodging");
    assert_eq!(w.tiles.get(hx, hy).obj, obj::HEARTH);
}
