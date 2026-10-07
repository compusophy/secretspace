//! The feel in three dimensions: sparks that burst and fall, rings that
//! spread along the ground, a shake and a flash when you are hit. World
//! space, in tiles (x and y along the ground, h up); the renderer draws
//! the sparks as points.

use pixels::Rgba;

#[derive(Clone, Copy, Debug)]
struct Spark {
    p: [f32; 3],
    v: [f32; 3],
    c: [f32; 3],
    born: f64,
    life: f64,
    size: f32,
    fall: f32,
}

#[derive(Default)]
pub struct Fx {
    sparks: Vec<Spark>,
    seed: u32,
    shake_until: f64,
    shake: f32,
    /// A hit flash: its colour and when it ends.
    pub flash: Option<(Rgba, f64)>,
    last: f64,
}

fn col(c: Rgba) -> [f32; 3] {
    [c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0]
}

impl Fx {
    fn rand(&mut self) -> f32 {
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    fn add(&mut self, s: Spark) {
        if self.sparks.len() < 1500 {
            self.sparks.push(s);
        }
    }

    /// `n` sparks bursting from (x, y) at height h.
    pub fn burst(&mut self, at: [f32; 3], n: usize, speed: f32, c: Rgba, now: f64) {
        for _ in 0..n {
            let a = self.rand() * std::f32::consts::TAU;
            let up = self.rand() * 0.9 + 0.2;
            let k = speed * (0.4 + self.rand() * 0.6);
            let life = 350.0 + self.rand() as f64 * 450.0;
            self.add(Spark {
                p: at,
                v: [a.cos() * k, up * k, a.sin() * k],
                c: col(c),
                born: now,
                life,
                size: 0.07,
                fall: 9.0,
            });
        }
    }

    /// A ring spreading on the ground from (x, y) to radius `r`.
    pub fn ring(&mut self, at: [f32; 3], r: f32, c: Rgba, now: f64) {
        let n = (r * 18.0) as usize + 12;
        for k in 0..n {
            let a = k as f32 / n as f32 * std::f32::consts::TAU;
            self.add(Spark {
                p: [at[0], at[1] + 0.05, at[2]],
                v: [a.cos() * r * 2.2, 0.0, a.sin() * r * 2.2],
                c: col(c),
                born: now,
                life: 450.0,
                size: 0.06,
                fall: 0.0,
            });
        }
    }

    /// A hit landing: sparks; a big one shakes.
    pub fn hit(&mut self, at: [f32; 3], big: bool, now: f64) {
        let c = if big {
            Rgba::rgb(255, 210, 122)
        } else {
            Rgba::rgb(244, 238, 222)
        };
        self.burst(
            at,
            if big { 26 } else { 12 },
            if big { 5.0 } else { 3.0 },
            c,
            now,
        );
    }

    /// You were hit.
    pub fn hurt(&mut self, big: bool, now: f64) {
        self.shake_until = now + if big { 260.0 } else { 140.0 };
        self.shake = if big { 0.09 } else { 0.04 };
        self.flash = Some((Rgba(255, 90, 60, if big { 90 } else { 50 }), now + 160.0));
    }

    /// The camera's shake now (tiles), as (right, up).
    pub fn shake(&mut self, now: f64) -> (f32, f32) {
        if now >= self.shake_until {
            return (0.0, 0.0);
        }
        let k = ((self.shake_until - now) / 200.0).min(1.0) as f32 * self.shake;
        ((self.rand() - 0.5) * 2.0 * k, (self.rand() - 0.5) * 2.0 * k)
    }

    /// Move everything on to `now`; drop what has burnt out.
    pub fn age(&mut self, now: f64) {
        let dt = ((now - self.last) / 1000.0).clamp(0.0, 0.1) as f32;
        self.last = now;
        for s in &mut self.sparks {
            s.v[1] -= s.fall * dt;
            for k in 0..3 {
                s.p[k] += s.v[k] * dt;
            }
            if s.fall > 0.0 && s.p[1] < 0.02 && s.v[1] < 0.0 {
                s.p[1] = 0.02;
                s.v = [s.v[0] * 0.5, -s.v[1] * 0.3, s.v[2] * 0.5];
            }
        }
        self.sparks.retain(|s| now - s.born < s.life);
        if self.flash.is_some_and(|f| now >= f.1) {
            self.flash = None;
        }
    }

    /// Every spark as points: position (3), colour and alpha (4), size.
    pub fn points(&self, now: f64, out: &mut Vec<f32>) {
        for s in &self.sparks {
            let k = 1.0 - ((now - s.born) / s.life).clamp(0.0, 1.0) as f32;
            out.extend_from_slice(&[s.p[0], s.p[1], s.p[2], s.c[0], s.c[1], s.c[2], k, s.size]);
        }
    }

    pub fn len(&self) -> usize {
        self.sparks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sparks.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparks_burst_fall_and_burn_out() {
        let mut fx = Fx::default();
        fx.burst([0.0, 1.0, 0.0], 10, 3.0, Rgba::rgb(255, 0, 0), 0.0);
        fx.ring([0.0, 0.0, 0.0], 1.0, Rgba::rgb(0, 255, 0), 0.0);
        assert!(fx.len() > 20);
        for k in 1..10 {
            fx.age(k as f64 * 50.0);
        }
        let mut pts = Vec::new();
        fx.points(450.0, &mut pts);
        assert_eq!(pts.len() % 8, 0);
        assert!(pts.chunks(8).all(|p| p[1] >= 0.0));
        fx.age(2000.0);
        assert!(fx.is_empty());
        fx.hurt(true, 2000.0);
        assert_ne!(fx.shake(2010.0), (0.0, 0.0));
        assert_eq!(fx.shake(3000.0), (0.0, 0.0));
    }
}
