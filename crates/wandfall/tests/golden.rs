//! The same motion on every build. A scripted minute of running,
//! sprinting, hopping, sliding, crouching, air jumps, wall kicks off the
//! tower, a climb onto a rock, a launch rune and Tethers, every body
//! hashed tick by tick. The page predicts its wizard with this code and
//! the server moves it with the same, so what it does must not change
//! unnoticed: when motion changes on purpose, the hash changes with it,
//! and so must PROTO (an old page and a new server would not move
//! alike). Run on wasm as well (`cargo test --target wasm32-wasip1`, with
//! a WASI runner) to know the page moves as the server does, bit for bit.

use engine::rng::Rng;
use wandfall::laws::*;
use wandfall::map::{Kind, Map};
use wandfall::motion::{keys, step, Body, Input};
use wandfall::proto::PROTO;
use wandfall::trig;

/// The protocol the hash below was taken at, and the hash.
const GOLDEN: (u8, &str) = (17, "cfd194e6430102c11435c176b4a623592e97b0ea");

/// A body's every field, as bits.
fn bits(b: &Body, out: &mut Vec<u8>) {
    for x in b.p.iter().chain(&b.v).chain(&b.anchor).chain(&b.wall_n) {
        out.extend(x.to_bits().to_le_bytes());
    }
    let flags = [
        b.ground,
        b.glide,
        b.crouch,
        b.sprint,
        b.slide,
        b.winded,
        b.held,
        b.air_jumped,
    ];
    out.extend(flags.map(|f| f as u8));
    out.extend(b.chill.to_le_bytes());
    out.extend(b.spent.to_le_bytes());
    out.extend([
        b.coyote, b.buffer, b.slide_cd, b.breath, b.landed, b.mantle, b.tether, b.walls, b.wall,
    ]);
}

/// The keys of a drill, `t` ticks in: runs, sprints, hops timed and not,
/// slides, strafes, back-pedals with air jumps, aims; and now and then
/// anything at all.
fn drill(t: u32, rng: &mut Rng) -> u16 {
    let press = |every: u32| if t % every < 2 { keys::JUMP } else { 0 };
    match (t / 24) % 8 {
        0 => keys::FWD | keys::SPRINT,
        1 => keys::FWD | keys::SPRINT | press(12),
        2 => keys::FWD | keys::SPRINT | if t % 24 > 6 { keys::CROUCH } else { 0 },
        3 => keys::FWD | keys::RIGHT | press(8),
        4 => keys::BACK | keys::LEFT | press(6),
        5 => keys::FWD | keys::JUMP,
        6 => keys::AIM | keys::LEFT,
        _ => rng.next_u64() as u16 & 0x1df,
    }
}

fn hex(h: [u8; 20]) -> String {
    h.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn motion_is_the_same_on_every_build() {
    let map = Map::new(0x601d);
    let mut rng = Rng::new(7);
    let mut out = Vec::new();
    let ground = |x: f32, z: f32| Body {
        p: [x, map.height(x, z), z],
        ground: true,
        ..Body::default()
    };
    // Where each part runs, and toward what: across the plaza; into the
    // tower's wall (kicking off it); onto a rock (climbing it); onto a
    // launch rune (and the broom); anywhere.
    let pad = map.pads[0];
    let rock = map
        .props
        .iter()
        .find(|q| q.kind == Kind::Rock && q.h > 1.2 && q.h < 2.0 && map.land(q.x, q.z))
        .copied()
        .unwrap();
    let off = rock.r + 3.0;
    let spot = map.spot(&mut rng);
    let parts = [
        (ground(-12.0, -12.0), [12.0, -12.0]),
        (ground(-TOWER_RADIUS - 4.0, 0.0), [0.0, 0.0]),
        (ground(rock.x - off, rock.z), [rock.x, rock.z]),
        (ground(pad[0] - 6.0, pad[2]), [pad[0], pad[2]]),
        (ground(spot[0], spot[1]), [0.0, 0.0]),
    ];
    for (mut b, to) in parts {
        for t in 0..12 * TICK_HZ {
            // Facing what it runs at, the look wandering a little.
            let want = trig::heading(trig::atan2(to[1] - b.p[2], to[0] - b.p[0]));
            let yaw = want
                .wrapping_add((rng.next_u64() % 1200) as u16)
                .wrapping_sub(600);
            // Now and then a Tether caught ahead and above.
            if t % 90 == 60 {
                let (s, c) = trig::sin_cos(yaw);
                b.anchor = [b.p[0] + c * 25.0, b.p[1] + 8.0, b.p[2] + s * 25.0];
                b.tether = TETHER_TICKS;
                b.mantle = 0;
            }
            let i = Input {
                seq: t as u16,
                yaw,
                pitch: 0,
                keys: drill(t, &mut rng),
                cast: 0,
                view: 0,
            };
            step(&mut b, &i, &map);
            bits(&b, &mut out);
        }
    }
    let h = hex(engine::sha1::sha1(&out));
    assert!(
        GOLDEN == (PROTO, h.as_str()),
        "motion moves otherwise (hash {h}): if that is meant, bump PROTO past {} and set GOLDEN to (PROTO, \"{h}\")",
        GOLDEN.0
    );
}
