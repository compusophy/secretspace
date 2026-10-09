//! Rain: drops streaking down about the eye (faint: a drop does not
//! shine), slanting with the wind,
//! each column fixed to the ground as you move through it (it wraps
//! round you, so it never runs out), as thick as `k` says.

use render::geo::{hash, unit, V3};
use render::Spark;

use super::Draw;

/// How wide the box of rain about the eye is, how tall, how many drops
/// fill it, and how fast they fall (m/s).
const WIDE: f32 = 36.0;
const TALL: f32 = 22.0;
const DROPS: i32 = 900;
const FALL: f32 = 17.0;

/// Rain about `eye`, `t` seconds in, `k` of a downpour (0 none), blown
/// by `wind` (x, z).
pub fn rain(d: &mut Draw, eye: V3, t: f32, k: f32, wind: [f32; 2]) {
    let n = (DROPS as f32 * k.clamp(0.0, 1.0)) as i32;
    let wrap = |at: f32, e: f32| e + (at - e + WIDE * 0.5).rem_euclid(WIDE) - WIDE * 0.5;
    for i in 0..n {
        let u = |c: u32| unit(hash(i, 7, c));
        let speed = FALL * (0.85 + 0.3 * u(3));
        let x = wrap(u(1) * WIDE * 8.0 + wind[0] * 1.2 * t, eye[0]);
        let z = wrap(u(2) * WIDE * 8.0 + wind[1] * 1.2 * t, eye[2]);
        let y = eye[1] + TALL * 0.55 - (t * speed + u(4) * TALL).rem_euclid(TALL);
        // Faint up close (they would blot the view), and toward the top.
        let near = ((x - eye[0]).powi(2) + (z - eye[2]).powi(2)).sqrt();
        let a = 0.1 * (near / 3.0).min(1.0);
        d.sparks.push(Spark {
            p: [x, y, z],
            size: 0.02,
            c: [0.7, 0.75, 0.85, a],
            // Its streak: how far it falls in a thirtieth of a second.
            v: [wind[0] * 0.04, -speed * 0.033, wind[1] * 0.04],
            ..Default::default()
        });
    }
}
