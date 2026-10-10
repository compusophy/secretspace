//! Drawing between the server's frames: however they arrive (late, early,
//! two at once), no snake jumps when one lands, from head to tail tip.

use std::collections::HashMap;

use look::{along, drawn, Live};
use wyrm::proto::{self, Down};
use wyrm::view::Viewer;
use wyrm::world::World;

/// Where every bead of every snake is drawn at `now`: its head, each whole
/// point after it, and the tip of its tail.
fn drawing(live: &Live, now: f64) -> HashMap<u16, Vec<(f32, f32)>> {
    let alpha = live.alpha(now);
    live.mirror
        .snakes
        .values()
        .map(|s| {
            let (lag, len) = drawn(s, alpha);
            let mut at: Vec<(f32, f32)> = (0..)
                .map(|k| k as f32)
                .take_while(|&k| k < len - 1.0)
                .map(|k| along(s, lag + k))
                .collect();
            at.push(along(s, lag + len - 1.0));
            (s.id, at)
        })
        .collect()
}

#[test]
fn nothing_jumps_when_a_frame_lands() {
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut rand = |n: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % n
    };
    let mut w = World::new(21);
    let mut v = Viewer::default();
    v.you = w.spawn("me", None);
    let mut live = Live::default();
    live.receive(0.0, Down::decode(&proto::hello(2400, 20)).unwrap());
    let (mut now, mut together, mut checked) = (1000.0, false, 0usize);
    for t in 0..3000 {
        if t % 10 == 0 {
            let a = rand(628) as f32 / 100.0;
            w.steer(v.you, a, rand(3) == 0);
        }
        w.step();
        if w.find(v.you).is_none() {
            v.you = w.spawn("me", None);
        }
        let frame = Down::decode(&v.frame(&w)).unwrap();
        // Frames land late, early, or two in one screen refresh.
        let gap = match rand(4) {
            0 if !together => 0.0,
            1 => 35.0,
            2 => 50.0,
            _ => 70.0,
        };
        together = gap == 0.0;
        now += gap;
        let before = drawing(&live, now);
        live.receive(now, frame);
        for (id, after) in drawing(&live, now) {
            let Some(was) = before.get(&id) else {
                continue;
            };
            assert_eq!(was.len(), after.len(), "snake {id} at tick {t}");
            for (a, b) in was.iter().zip(&after) {
                let d = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
                assert!(d < 0.01, "snake {id} jumped {d} at tick {t}");
            }
            checked += 1;
        }
    }
    assert!(checked > 5_000, "{checked}");
}
