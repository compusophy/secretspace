//! One snake: a dark rim, a striped body tapering to its tail and lit from
//! the upper left (a shaded side, a lit side, a glossy ridge), eyes, and
//! its name. The body is a run of overlapping circles from the tail up;
//! each circle's rim is drawn a few circles ahead of it and its light a
//! few behind, so the neighbours that overlap it never cut the rim or the
//! light into beads, and where a snake crosses itself its head still lies
//! over its tail.

use pixels::{Canvas, Rgba};
use wyrm::laws::{radius, STEP};
use wyrm::mirror::Seen;

use crate::{along, drawn, hue, phase, Scene, View, BG};

/// Stripes are this many points long.
const BAND: usize = 5;
/// The last share of a snake narrows to its tail, down to this share of
/// its width.
const TAPER: f32 = 0.2;
const TIP: f32 = 0.55;
/// The head is drawn this much wider than the body.
const HEAD: f32 = 1.1;

/// A circle of the body: where, how wide, and which stripe.
struct Bead {
    x: f32,
    y: f32,
    r: f32,
    band: bool,
}

pub(crate) fn draw(c: &mut Canvas, v: &View, s: &Seen, sc: &Scene, mine: bool, now: f64) {
    if s.body.is_empty() {
        return;
    }
    let (lag, len) = drawn(s, sc.alpha);
    let r = radius(s.mass as f32) * v.k;
    // Close points on a fat snake overlap anyway: draw fewer.
    let gap = STEP * v.k;
    let every = ((r * 0.6) / gap.max(0.1)).floor().max(1.0) as usize;
    let last = len - 1.0;
    let taper = (TAPER * len).max(1.0);
    let bead = |k: f32| {
        let (x, y) = v.at(along(s, lag + k));
        let narrow = TIP + (1.0 - TIP) * ((last - k) / taper).min(1.0);
        let wide = if k == 0.0 { HEAD } else { 1.0 };
        Bead {
            x,
            y,
            r: r * narrow * wide,
            band: (k as usize / BAND).is_multiple_of(2),
        }
    };
    // Head first; the tail's tip exactly where it is, between points.
    let mut beads: Vec<Bead> = (0..)
        .map(|k| k * every)
        .take_while(|&k| (k as f32) < last)
        .map(|k| bead(k as f32))
        .collect();
    beads.push(bead(last.max(0.0)));
    if !beads
        .iter()
        .step_by(4)
        .any(|b| v.sees((b.x, b.y), r + 40.0))
    {
        return;
    }

    // A ghost is drawn see-through by mixing it toward the ground, never
    // by stacking faded circles.
    let fog = if s.ghost {
        0.55 - 0.15 * phase(now, 120.0).sin()
    } else {
        0.0
    };
    let h = hue(s.hue);
    let ink = |sat: f32, l: f32| Rgba::hsl(h, sat, l).mix(BG, fog);
    let rim = ink(0.7, 0.14);
    // Each stripe in three tones: in shade, in the light, and its gloss.
    let shade = [ink(0.85, 0.42), ink(0.85, 0.34)];
    let lit = [ink(0.88, 0.58), ink(0.88, 0.48)];
    let gloss = [ink(0.95, 0.78), ink(0.95, 0.7)];

    if s.boosting {
        let flicker = 0.35 + 0.15 * phase(now, 60.0).sin();
        let glow = Rgba::hsl(h, 1.0, 0.65).fade(flicker * (1.0 - fog));
        for b in beads.iter().rev().step_by(3) {
            if v.sees((b.x, b.y), r * 2.6) {
                c.glow(b.x, b.y, r * 2.6, glow);
            }
        }
    }

    // How many beads apart two can still overlap.
    let reach = ((2.4 * r) / (every as f32 * gap)).ceil() as usize + 1;
    let see = |b: &Bead| v.sees((b.x, b.y), b.r + 2.0);
    let outline = |c: &mut Canvas, b: &Bead| {
        if see(b) {
            c.circle(b.x, b.y, b.r + 1.0 + 0.08 * b.r, rim);
        }
    };
    let fill = |c: &mut Canvas, b: &Bead| {
        if see(b) {
            c.circle(b.x, b.y, b.r, shade[!b.band as usize]);
        }
    };
    // The light on a bead, run on to the next bead headward, so the lit
    // side and the ridge are unbroken strokes.
    let stroke = |c: &mut Canvas, j: usize, off: (f32, f32), width: f32, ink: [Rgba; 2]| {
        let (b, to) = (&beads[j], &beads[j.saturating_sub(1)]);
        if see(b) && b.r * width > 1.0 {
            c.line(
                b.x + off.0 * b.r,
                b.y + off.1 * b.r,
                to.x + off.0 * to.r,
                to.y + off.1 * to.r,
                width * b.r,
                ink[!b.band as usize],
            );
        }
    };
    // All four layers in one pass from the tail up: the rim a few beads
    // ahead of the shade, the light a few behind it, the gloss behind that.
    let (n, w) = (beads.len() as isize, reach as isize);
    let at = |i: isize| (0..n).contains(&i).then_some(i as usize);
    for i in (-2 * w..n + w).rev() {
        if let Some(j) = at(i - w) {
            outline(c, &beads[j]);
        }
        if let Some(j) = at(i) {
            fill(c, &beads[j]);
        }
        if let Some(j) = at(i + w) {
            stroke(c, j, (-0.1, -0.12), 1.56, lit);
        }
        if let Some(j) = at(i + 2 * w) {
            stroke(c, j, (-0.3, -0.36), 0.56, gloss);
        }
    }

    // Eyes, looking where it heads (yours: where you point).
    let hd = &beads[0];
    let look = if mine {
        sc.steer.unwrap_or(s.angle)
    } else {
        s.angle
    };
    let white = Rgba::rgb(255, 255, 255).mix(BG, fog);
    let pupil = Rgba::rgb(11, 13, 20);
    for side in [-1.0f32, 1.0] {
        let e = s.angle + side * 0.75;
        let (ex, ey) = (hd.x + e.cos() * hd.r * 0.52, hd.y + e.sin() * hd.r * 0.52);
        c.circle(ex, ey, (hd.r * 0.36).max(1.2), white);
        c.circle(
            ex + look.cos() * hd.r * 0.14,
            ey + look.sin() * hd.r * 0.14,
            (hd.r * 0.19).max(0.7),
            pupil,
        );
    }

    // Names over other snakes.
    if let (Some(u), false) = (sc.names, mine) {
        let y = (hd.y - hd.r) as i32 - 10 * u;
        c.text_centred(hd.x as i32, y, &s.name, u, Rgba(255, 255, 255, 180));
    }
}
