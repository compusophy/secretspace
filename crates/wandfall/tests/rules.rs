//! The rules the server holds a page to, whatever it sends: it moves no
//! faster than the clock, and one that stalled is not put back for it.

use wandfall::laws::*;
use wandfall::motion::{keys, Body, Input};
use wandfall::predict::Predict;
use wandfall::trig;
use wandfall::world::World;

/// A wizard in the lobby, set down at the start of a long run over open
/// land (no sea, nothing in the way, no launch rune, no steep hill), so
/// how far it gets is down to its inputs alone: the world, who, and the
/// way down the run (a yaw).
fn runner() -> (World, u16, u16) {
    let mut w = World::new(5);
    let me = w.join("runner", 0);
    let m = &w.map;
    // Whether the metre from (x, z) along (dx, dz) is open going.
    let open = |x: f32, z: f32, (dx, dz): (f32, f32)| {
        let y = m.height(x, z);
        let on = m.height(x + dx, z + dz);
        m.land(x, z)
            && m.near(x, z, 2.0).next().is_none()
            && m.floor(x, z, y) <= y + 0.05
            && (on - y).abs() < 0.2
            && m.strikes([x, y + 1.0, z], [x + dx, on + 1.0, z + dz])
                .is_none()
            && m.pads.iter().all(|q| (q[0] - x).hypot(q[2] - z) > 4.0)
    };
    let clear = |(x, z, yaw): (f32, f32, u16)| {
        let (s, c) = trig::sin_cos(yaw);
        (0..36).all(|k| open(x + c * k as f32, z + s * k as f32, (c, s)))
    };
    // Spots across the island, each tried eight ways round.
    let (x, z, yaw) = (-20..20)
        .flat_map(|j| (-20..20).map(move |i| (i as f32 * 8.0, j as f32 * 8.0)))
        .flat_map(|(x, z)| (0..8u16).map(move |d| (x, z, d * 8192)))
        .find(|&r| clear(r))
        .expect("a clear run somewhere");
    let p = [x, w.map.height(x, z), z];
    w.find_mut(me).unwrap().body = Body {
        p,
        ground: true,
        ..Body::default()
    };
    (w, me, yaw)
}

/// A wizard run forward for `ticks`, its page sending `n` inputs a tick:
/// the last input applied, and how far it went.
fn run(n: u16, ticks: u32) -> (u16, f32) {
    let (mut w, me, yaw) = runner();
    let from = w.find(me).unwrap().body.p;
    let mut seq = 0u16;
    for _ in 0..ticks {
        for _ in 0..n {
            seq += 1;
            let i = Input {
                seq,
                yaw,
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
    assert!(walked > RUN * 1.5, "nothing in its way: {walked} m");
    for n in [2, 8, 24] {
        let (last, flooded) = run(n, ticks);
        println!("{n} a tick: {flooded} m, against {walked} m");
        assert!(
            last as u32 > ticks * (n as u32 - 1),
            "{n} a tick: kept up ({last})"
        );
        assert!(
            flooded <= walked + SPRINT * DT,
            "{n} a tick: {flooded} m, against {walked} m"
        );
    }
}

/// A wizard sprinting forward for three seconds, its page's inputs held
/// back for `held` ticks a second in and then sent all at once (a stalled
/// connection, or a lag switch): the furthest its page's prediction was
/// put back once they came, and how far it went.
fn stalled(held: u32) -> (f32, f32) {
    let (mut w, me, yaw) = runner();
    let from = w.find(me).unwrap().body.p;
    let mut page = Predict::default();
    page.reset(w.find(me).unwrap().body);
    let mut late = Vec::new();
    let mut put_back = 0.0f32;
    for t in 0..3 * TICK_HZ {
        let i = Input {
            seq: t as u16 + 1,
            yaw,
            keys: keys::FWD | keys::SPRINT,
            ..Input::default()
        };
        page.push(i, &w.map);
        if (TICK_HZ..TICK_HZ + held).contains(&t) {
            late.push(i);
        } else {
            for i in late.drain(..) {
                w.input(me, i);
            }
            w.input(me, i);
        }
        w.step();
        let p = w.find(me).unwrap();
        let off = page.confirm(p.body, p.last.seq, &w.map);
        if t >= TICK_HZ + held {
            put_back = put_back.max(off);
        }
    }
    let to = w.find(me).unwrap().body.p;
    (put_back, (to[0] - from[0]).hypot(to[2] - from[2]))
}

#[test]
fn a_page_that_stalled_a_second_catches_up_where_it_thought_it_was() {
    let (_, honest) = stalled(0);
    assert!(honest > SPRINT * 2.5, "nothing in its way: {honest} m");
    let (put_back, went) = stalled(TICK_HZ);
    println!("put back {put_back} m; went {went} m, against {honest} m");
    assert!(put_back < 1.0, "put back {put_back} m once its inputs came");
    // Its wizard stood while it waited, so it is no further on for it.
    assert!(
        went <= honest,
        "a burst gets no further: {went} m, against {honest} m"
    );
    assert!(went > honest - 2.0, "nor much behind: {went} m, {honest} m");
}
