//! Fixed-point numbers for anything a page predicts: positions, speeds,
//! headings and timers come out bit for bit the same in wasm and on the
//! server, which floats do not promise. `Fx` is Q16.16 in an `i32` with
//! saturating arithmetic only (no operator overloads, so nothing can wrap
//! by accident). Headings are `u16`, 65536 to a turn; sine comes from a
//! committed quarter-wave table, arctangent from CORDIC, lengths from an
//! integer square root.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fx(pub i32);

const ONE: i32 = 1 << 16;

/// An i64 squeezed into an i32, saturating.
const fn sat(v: i64) -> i32 {
    if v > i32::MAX as i64 {
        i32::MAX
    } else if v < i32::MIN as i64 {
        i32::MIN
    } else {
        v as i32
    }
}

impl Fx {
    pub const ZERO: Fx = Fx(0);
    pub const ONE: Fx = Fx(ONE);
    pub const HALF: Fx = Fx(ONE / 2);
    pub const MAX: Fx = Fx(i32::MAX);
    pub const MIN: Fx = Fx(i32::MIN);

    pub const fn int(n: i32) -> Fx {
        Fx(sat(n as i64 * ONE as i64))
    }
    /// Thousandths: `Fx::milli(2_500)` is 2.5.
    pub const fn milli(m: i32) -> Fx {
        Fx(sat(m as i64 * ONE as i64 / 1000))
    }
    /// `n / d`, rounded toward zero; 0 when `d` is 0.
    pub const fn ratio(n: i32, d: i32) -> Fx {
        if d == 0 {
            Fx(0)
        } else {
            Fx(sat(n as i64 * ONE as i64 / d as i64))
        }
    }
    pub const fn add(self, o: Fx) -> Fx {
        Fx(self.0.saturating_add(o.0))
    }
    pub const fn sub(self, o: Fx) -> Fx {
        Fx(self.0.saturating_sub(o.0))
    }
    pub const fn neg(self) -> Fx {
        Fx(self.0.saturating_neg())
    }
    pub const fn abs(self) -> Fx {
        Fx(self.0.saturating_abs())
    }
    pub const fn mul(self, o: Fx) -> Fx {
        Fx(sat((self.0 as i64 * o.0 as i64) >> 16))
    }
    pub const fn div(self, o: Fx) -> Fx {
        if o.0 == 0 {
            return if self.0 >= 0 { Fx::MAX } else { Fx::MIN };
        }
        Fx(sat(((self.0 as i64) << 16) / o.0 as i64))
    }
    pub const fn mul_int(self, n: i32) -> Fx {
        Fx(self.0.saturating_mul(n))
    }
    pub const fn div_int(self, n: i32) -> Fx {
        if n == 0 {
            self
        } else {
            // The one quotient that does not fit (the least by -1) saturates.
            Fx(self.0.saturating_div(n))
        }
    }
    /// The whole part, rounded toward minus infinity.
    pub const fn floor(self) -> i32 {
        self.0 >> 16
    }
    pub const fn is_neg(self) -> bool {
        self.0 < 0
    }
    /// For drawing only: never feed it back into anything predicted.
    pub fn to_f32(self) -> f32 {
        self.0 as f32 / ONE as f32
    }
}

/// sin of a quarter turn in 256 steps, Q16.16; entry 256 is exactly 1.
const QUARTER: [i32; 257] = [
    0, 402, 804, 1206, 1608, 2010, 2412, 2814, 3216, 3617, 4019, 4420, 4821, 5222, 5623, 6023,
    6424, 6824, 7224, 7623, 8022, 8421, 8820, 9218, 9616, 10014, 10411, 10808, 11204, 11600, 11996,
    12391, 12785, 13180, 13573, 13966, 14359, 14751, 15143, 15534, 15924, 16314, 16703, 17091,
    17479, 17867, 18253, 18639, 19024, 19409, 19792, 20175, 20557, 20939, 21320, 21699, 22078,
    22457, 22834, 23210, 23586, 23961, 24335, 24708, 25080, 25451, 25821, 26190, 26558, 26925,
    27291, 27656, 28020, 28383, 28745, 29106, 29466, 29824, 30182, 30538, 30893, 31248, 31600,
    31952, 32303, 32652, 33000, 33347, 33692, 34037, 34380, 34721, 35062, 35401, 35738, 36075,
    36410, 36744, 37076, 37407, 37736, 38064, 38391, 38716, 39040, 39362, 39683, 40002, 40320,
    40636, 40951, 41264, 41576, 41886, 42194, 42501, 42806, 43110, 43412, 43713, 44011, 44308,
    44604, 44898, 45190, 45480, 45769, 46056, 46341, 46624, 46906, 47186, 47464, 47741, 48015,
    48288, 48559, 48828, 49095, 49361, 49624, 49886, 50146, 50404, 50660, 50914, 51166, 51417,
    51665, 51911, 52156, 52398, 52639, 52878, 53114, 53349, 53581, 53812, 54040, 54267, 54491,
    54714, 54934, 55152, 55368, 55582, 55794, 56004, 56212, 56418, 56621, 56823, 57022, 57219,
    57414, 57607, 57798, 57986, 58172, 58356, 58538, 58718, 58896, 59071, 59244, 59415, 59583,
    59750, 59914, 60075, 60235, 60392, 60547, 60700, 60851, 60999, 61145, 61288, 61429, 61568,
    61705, 61839, 61971, 62101, 62228, 62353, 62476, 62596, 62714, 62830, 62943, 63054, 63162,
    63268, 63372, 63473, 63572, 63668, 63763, 63854, 63944, 64031, 64115, 64197, 64277, 64354,
    64429, 64501, 64571, 64639, 64704, 64766, 64827, 64884, 64940, 64993, 65043, 65091, 65137,
    65180, 65220, 65259, 65294, 65328, 65358, 65387, 65413, 65436, 65457, 65476, 65492, 65505,
    65516, 65525, 65531, 65535, 65536,
];

/// atan(2^-i) in 1/256ths of a heading unit, for CORDIC.
const ATAN: [i64; 20] = [
    2097152, 1238021, 654136, 332050, 166669, 83416, 41718, 20860, 10430, 5215, 2608, 1304, 652,
    326, 163, 81, 41, 20, 10, 5,
];

fn quarter(r: u32) -> i64 {
    // r in 0..=16384: 64 heading units a step, interpolated in between.
    let i = (r >> 6) as usize;
    let f = (r & 63) as i64;
    if i >= 256 {
        return QUARTER[256] as i64;
    }
    let (a, b) = (QUARTER[i] as i64, QUARTER[i + 1] as i64);
    a + (b - a) * f / 64
}

pub fn sin(h: u16) -> Fx {
    let r = (h & 0x3fff) as u32;
    let v = match h >> 14 {
        0 => quarter(r),
        1 => quarter(16384 - r),
        2 => -quarter(r),
        _ => -quarter(16384 - r),
    };
    Fx(v as i32)
}

pub fn cos(h: u16) -> Fx {
    sin(h.wrapping_add(16384))
}

/// The heading of the vector `(x, y)`: 0 along +x, 16384 along +y. 0 for
/// the zero vector.
pub fn atan2(y: Fx, x: Fx) -> u16 {
    let (mut x, mut y) = (x.0 as i64, y.0 as i64);
    if x == 0 && y == 0 {
        return 0;
    }
    let mut z: i64 = 0;
    if x < 0 {
        x = -x;
        y = -y;
        z = 32768 << 8;
    }
    // More bits for the shifts to work with.
    x <<= 20;
    y <<= 20;
    for (i, a) in ATAN.iter().enumerate() {
        let (dx, dy) = (y >> i, x >> i);
        if y > 0 {
            x += dx;
            y -= dy;
            z += a;
        } else {
            x -= dx;
            y += dy;
            z -= a;
        }
    }
    ((z + 128) >> 8).rem_euclid(65536) as u16
}

/// The integer square root, rounded down.
pub fn isqrt(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// The length of `(x, y)`.
pub fn len(x: Fx, y: Fx) -> Fx {
    let (a, b) = (x.0 as i128, y.0 as i128);
    let l = isqrt((a * a + b * b) as u128);
    Fx(l.min(i32::MAX as u128) as i32)
}

/// The signed turn from heading `a` to heading `b`, the short way.
pub fn turn(a: u16, b: u16) -> i32 {
    b.wrapping_sub(a) as i16 as i32
}

/// A heading as a unit vector.
pub fn unit(h: u16) -> (Fx, Fx) {
    (cos(h), sin(h))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    #[test]
    fn arithmetic_saturates_and_never_wraps() {
        assert_eq!(Fx::MAX.add(Fx::ONE), Fx::MAX);
        assert_eq!(Fx::MIN.sub(Fx::ONE), Fx::MIN);
        assert_eq!(Fx::int(3).mul(Fx::HALF), Fx::milli(1_500));
        assert_eq!(Fx::int(3).div(Fx::int(2)), Fx::milli(1_500));
        assert_eq!(Fx::ONE.div(Fx::ZERO), Fx::MAX);
        assert_eq!(Fx::int(40_000).mul(Fx::int(40_000)), Fx::MAX);
        assert_eq!(Fx::milli(-1).floor(), -1);
        assert_eq!(Fx::MIN.div_int(-1), Fx::MAX);
        assert_eq!(Fx::int(9).div_int(3), Fx::int(3));
        assert_eq!(Fx::int(-7).div_int(2), Fx(-7 * 65536 / 2));
        assert_eq!(Fx::int(5).div_int(0), Fx::int(5));
    }

    #[test]
    fn golden_values_never_move() {
        assert_eq!(sin(0), Fx(0));
        assert_eq!(sin(16384), Fx::ONE);
        assert_eq!(sin(32768), Fx(0));
        assert_eq!(sin(49152), Fx::ONE.neg());
        assert_eq!(sin(8192).0, 46341);
        assert_eq!(cos(0), Fx::ONE);
        assert_eq!(atan2(Fx::ONE, Fx::ONE), 8192);
        assert_eq!(atan2(Fx::ZERO, Fx::ONE.neg()), 32768);
        assert_eq!(len(Fx::int(3), Fx::int(4)), Fx::int(5));
        assert_eq!(isqrt(1 << 64), 1 << 32);
    }

    #[test]
    fn it_agrees_with_floats_closely() {
        let mut rng = Rng::new(5);
        let tau = std::f64::consts::TAU;
        for _ in 0..100_000 {
            let h = rng.below(65536) as u16;
            let a = h as f64 / 65536.0 * tau;
            assert!(
                (sin(h).to_f32() as f64 - a.sin()).abs() < 1.0 / 4096.0,
                "sin {h}"
            );
            assert!(
                (cos(h).to_f32() as f64 - a.cos()).abs() < 1.0 / 4096.0,
                "cos {h}"
            );
            let x = Fx(rng.below(1 << 22) as i32 - (1 << 21));
            let y = Fx(rng.below(1 << 22) as i32 - (1 << 21));
            if x.0 == 0 && y.0 == 0 {
                continue;
            }
            let want = (y.0 as f64).atan2(x.0 as f64).rem_euclid(tau) / tau * 65536.0;
            let got = atan2(y, x) as f64;
            let d = (got - want).abs().min(65536.0 - (got - want).abs());
            assert!(d < 65536.0 / 4096.0, "atan2 {x:?} {y:?}: {got} vs {want}");
            let l = (x.to_f32() as f64).hypot(y.to_f32() as f64);
            assert!((len(x, y).to_f32() as f64 - l).abs() < 1.0 / 4096.0 + l * 1e-6);
        }
    }

    #[test]
    fn turns_go_the_short_way() {
        assert_eq!(turn(0, 1000), 1000);
        assert_eq!(turn(1000, 0), -1000);
        assert_eq!(turn(65000, 500), 1036);
        assert_eq!(turn(0, 32768), -32768);
    }
}
