//! The rules the server holds a page to, whatever it sends: it moves no
//! faster than the clock.

use wandfall::laws::*;
use wandfall::motion::{keys, Input};
use wandfall::world::World;

/// A wizard in the lobby run forward for `ticks`, its page sending `n`
/// inputs a tick: the last input applied, and how far it went.
fn run(n: u16, ticks: u32) -> (u16, f32) {
    let mut w = World::new(5);
    let me = w.join("runner", 0);
    let from = w.find(me).unwrap().body.p;
    let mut seq = 0u16;
    for _ in 0..ticks {
        for _ in 0..n {
            seq += 1;
            let i = Input {
                seq,
                keys: keys::FWD,
                ..Input::default()
            };
            w.input(me, i);
        }
        w.step();
    }
    let p = w.find(me).unwrap();
    let to = p.body.p;
    (p.last.seq, (to[0] - from[0]).hypot(to[2] - from[2]))
}

#[test]
fn a_flood_of_inputs_moves_no_faster_than_the_clock() {
    let ticks = 2 * TICK_HZ;
    let (honest, walked) = run(1, ticks);
    assert_eq!(honest as u32, ticks, "one a tick, every one applied");
    for n in [2, 8, 24] {
        let (last, flooded) = run(n, ticks);
        assert!(
            last as u32 > ticks * (n as u32 - 1),
            "{n} a tick: kept up ({last})"
        );
        assert!(
            flooded <= walked + INPUT_BANK as f32 * SPRINT * DT,
            "{n} a tick: {flooded} m, against {walked} m"
        );
    }
}
