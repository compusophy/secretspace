//! Sine and cosine by arithmetic alone, so the server (x86) and the page
//! (wasm) compute the same bits: no library calls that differ by
//! platform. Angles are u16 for headings (65536 a turn; 0 east, 16384
//! south) and i16 for pitch (16384 straight up).

use std::f32::consts::TAU;

/// sin and cos of x in [0, pi/2] (Taylor, error under 4e-6).
fn quarter(x: f32) -> (f32, f32) {
    let x2 = x * x;
    let s = x * (1.0 - x2 / 6.0 * (1.0 - x2 / 20.0 * (1.0 - x2 / 42.0 * (1.0 - x2 / 72.0))));
    let c = 1.0
        - x2 / 2.0 * (1.0 - x2 / 12.0 * (1.0 - x2 / 30.0 * (1.0 - x2 / 56.0 * (1.0 - x2 / 90.0))));
    (s, c)
}

/// (sin, cos) of a heading.
pub fn sin_cos(a: u16) -> (f32, f32) {
    let x = (a & 0x3fff) as f32 * (TAU / 65536.0);
    let (s, c) = quarter(x);
    match a >> 14 {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

/// (sin, cos) of a pitch.
pub fn pitch_sin_cos(p: i16) -> (f32, f32) {
    sin_cos(p as u16)
}

/// The unit vector a heading and pitch look along: (cos yaw cos pitch,
/// sin pitch, sin yaw cos pitch).
pub fn look(yaw: u16, pitch: i16) -> [f32; 3] {
    let (sy, cy) = sin_cos(yaw);
    let (sp, cp) = pitch_sin_cos(pitch);
    [cy * cp, sp, sy * cp]
}

/// The angle of (x, y) from the x axis, -pi to pi, by arithmetic alone
/// (error under 3e-4): the same bits on both ends.
pub fn atan2(y: f32, x: f32) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    let (ax, ay) = (x.abs(), y.abs());
    let (lo, hi) = if ax < ay { (ax, ay) } else { (ay, ax) };
    if hi == 0.0 {
        return 0.0;
    }
    let a = lo / hi;
    let s = a * a;
    let mut r = ((-0.046_496_475 * s + 0.159_314_22) * s - 0.327_622_76) * s * a + a;
    if ay > ax {
        r = FRAC_PI_2 - r;
    }
    if x < 0.0 {
        r = PI - r;
    }
    if y < 0.0 {
        r = -r;
    }
    r
}

/// A heading from radians, and back (for the page and the bots, never
/// for what must match bit for bit).
pub fn heading(rad: f32) -> u16 {
    (rad / TAU * 65536.0).round() as i64 as u16
}

pub fn radians(a: u16) -> f32 {
    a as f32 / 65536.0 * TAU
}

pub fn pitch(rad: f32) -> i16 {
    (rad / TAU * 65536.0).round().clamp(-16000.0, 16000.0) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_to_the_library_everywhere() {
        for a in (0..=65535u16).step_by(97) {
            let (s, c) = sin_cos(a);
            let r = radians(a);
            assert!(
                (s - r.sin()).abs() < 1e-5 && (c - r.cos()).abs() < 1e-5,
                "{a}"
            );
        }
        for k in 0..720 {
            let a = k as f32 * 0.5f32.to_radians() - std::f32::consts::PI;
            let (y, x) = (a.sin() * 7.0, a.cos() * 7.0);
            assert!((atan2(y, x) - y.atan2(x)).abs() < 3e-4, "{k}");
        }
        let f = look(16384, 0);
        assert!(f[2] > 0.999, "16384 is south (+z)");
        assert!(look(0, 16384)[1] > 0.999, "up");
    }
}
