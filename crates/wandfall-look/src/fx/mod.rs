//! What spells, loot and levels look like. Every spell has one colour
//! and one shape, the same in its icon, its bolt, its landing and the
//! marks it leaves, so a glance says what is coming; each is drawn in
//! energy that flows, flames that lick, smoke, streaks, glints and an
//! arcane circle of its own colour where it is cast:
//!
//! - Fireball (orange): a roiling ball of fire trailing flame and smoke;
//!   a fireball bursting, a shockwave, debris, embers, a pall of smoke.
//! - Lance (gold): a beam of light, flowing, wound with motes.
//! - Frost (ice blue): cut crystals in a trail of mist and glints;
//!   crystals and mist about a chilled foot.
//! - Lightning (violet): a circle of runes on the ground, closing; then
//!   a forked bolt, a flash, a scorched blast.
//! - Blink (pink): a whirl of streaks, gone; a whirl, there.
//! - Ward (blue): a bubble of flowing light over a circle of runes; it
//!   shatters when it breaks.
//! - Mend (green): a circle of runes under the feet, motes winding up.
//! - Gust (pale): wind wheeling out in streaks, dust thrown up, a dome.
//! - Tether (lime): a rope of light thrown out slack, taut as it hauls,
//!   motes running up it; sparks where it bit.
//!
//! `bolts`: spells in flight; `casts`: what each cast and landing shows;
//! `blasts`: the big ones (a fireball's burst, lightning, blink, gust);
//! `marks`: on wizards, the fallen, dust, bursts where something struck;
//! `rope`: the Tether's; `meshes`: the shapes; `ground`: the island's
//! ground, for what lies on it; `gallery`: any one of them on its own, to
//! look at.

mod blasts;
mod bolts;
mod casts;
mod gallery;
mod ground;
mod marks;
pub mod meshes;
mod rain;
mod rope;

pub(crate) use blasts::forks;
pub use bolts::bolts;
pub use casts::shows;
pub use gallery::{gallery, NAMES};
pub use ground::Ground;
pub use marks::{burst, dust, falls, on_wizard, scar_life, scars};
pub use rain::rain;
pub use rope::ropes;

use render::geo::{self, hash, mix, rgb, unit, V3};
use render::{m4, Decal, Item, Light, Mark, Material, Mesh, Pass, Shape, Spark};
use wandfall::laws::{spell, LIGHTNING_RADIUS};
use wandfall::proto::{Ev, Loot, Seen};
use wandfall::world::STORM;

use crate::look::{Look, GOLD};

/// What one frame draws besides the island.
#[derive(Default)]
pub struct Draw {
    pub items: Vec<Item>,
    pub lights: Vec<Light>,
    pub sparks: Vec<Spark>,
    pub decals: Vec<render::Decal>,
}

pub(crate) const WHITE: V3 = [1.0; 3];
pub(crate) const UP: V3 = [0.0, 1.0, 0.0];
/// Everyone else's wand: its bolts, and the light where they strike.
pub const FOE: V3 = rgb(255, 96, 120);

/// A spell's colour (the wand's is gold; the storm's, violet).
pub fn colour(s: u8) -> V3 {
    match s {
        spell::FIREBALL => rgb(255, 120, 40),
        spell::LANCE => rgb(255, 215, 90),
        spell::FROST => rgb(120, 225, 255),
        spell::LIGHTNING => rgb(165, 115, 255),
        spell::BLINK => rgb(255, 100, 200),
        spell::WARD => rgb(80, 140, 255),
        spell::MEND => rgb(120, 235, 110),
        spell::GUST => rgb(215, 235, 245),
        spell::TETHER => rgb(200, 255, 80),
        STORM => rgb(175, 100, 255),
        _ => GOLD,
    }
}

/// The time effects are drawn at: `now` (ms), and every effect held at
/// an age (`hold`, the page's `?hold`: to look at them).
#[derive(Clone, Copy, Debug, Default)]
pub struct Clock {
    pub now: f64,
    pub hold: Option<f64>,
}

impl Clock {
    /// How long ago `when` was (ms), as held.
    pub fn age(&self, when: f64) -> f64 {
        let age = self.now - when;
        self.hold.map_or(age, |h| age.min(h))
    }
}

/// A wizard as drawn this frame: its feet, and its wand's tip (where
/// its spells leave, a flash, a rope).
#[derive(Clone, Copy, Debug)]
pub struct Drawn {
    pub id: u16,
    pub feet: V3,
    pub tip: V3,
}

/// What effects know of the wizards casting them: where each is drawn,
/// and how each stood as it cast (`State::stood`: when, who, its look,
/// its feet).
#[derive(Clone, Copy, Default)]
pub struct Casters<'a> {
    pub drawn: &'a [Drawn],
    pub stood: &'a [(f64, u16, V3, V3)],
}

impl Casters<'_> {
    pub fn drawn(&self, id: u16) -> Option<&Drawn> {
        self.drawn.iter().find(|w| w.id == id)
    }

    /// How `id` stood casting at `when`: its look, its feet.
    fn stood(&self, when: f64, id: u16) -> Option<(V3, V3)> {
        self.stood
            .iter()
            .find(|s| s.0 == when && s.1 == id)
            .map(|s| (s.2, s.3))
    }
}

/// An event's own seed: from who, what and where (and when it came), so
/// its sparks keep their ways as long as it shows, however the list
/// about it comes and goes.
fn seed_of(when: f64, e: &Ev) -> i32 {
    let at = |p: V3| (p[0] * 16.0) as i32 ^ (((p[2] * 16.0) as i32) << 11) ^ (p[1] * 16.0) as i32;
    let (who, what, p) = match *e {
        Ev::Cast {
            by,
            spell,
            stage,
            at,
        } => (by, (spell as i32) << 8 | stage as i32, at),
        Ev::Beam { by, spell, to, .. } => (by, (spell as i32) << 8 | 0xff, to),
        Ev::Level { who, level } => (who, level as i32, [0.0; 3]),
        Ev::Hit { to, what, .. } => (to, what as i32, [0.0; 3]),
        Ev::Out { who, .. } => (who, 0, [0.0; 3]),
        _ => (0, 0, [0.0; 3]),
    };
    hash(who as i32 ^ (when as i32) << 4, what, at(p) as u32) as i32
}

/// A glint's brightness through its life `u` (0 to 1): up and gone.
fn twinkle(u: f32) -> f32 {
    (1.0 - (u * 2.0 - 1.0).abs()).powi(2)
}

/// One of a stream of motes shed `rate` times a second, phased by
/// `phase`, `t` seconds in: how far through its life it is (0 to 1),
/// and which life it is (the same number all its life, so its path and
/// noise stay its own).
fn cycle(t: f32, rate: f32, phase: f32) -> (f32, i32) {
    let c = t * rate + phase;
    (c - c.floor(), c.floor() as i32)
}

/// A trail of `n` puffs streaming back, a new one shed `rate` times a
/// second, `t` seconds in: each one's way along it (0 just shed, 1 at
/// its end) and its life (the same as it streams back, so it flickers
/// rather than swaps).
fn trail(n: i32, rate: f32, t: f32) -> impl Iterator<Item = (f32, i32)> {
    let c = t * rate;
    let (f, k0) = (c - c.floor(), c.floor() as i32);
    (0..n).map(move |k| ((k as f32 + f) / n as f32, k0 - k))
}

/// A random direction, the same for the same seed and n (`up`: only
/// upward).
fn dir(seed: i32, n: i32, up: bool) -> V3 {
    let a = unit(hash(seed, n, 11)) * std::f32::consts::TAU;
    let y = unit(hash(seed, n, 12)) * 2.0 - 1.0;
    let y = if up { y.abs() } else { y };
    let r = (1.0 - y * y).max(0.0).sqrt();
    [r * a.cos(), y, r * a.sin()]
}

/// 0..1, the same for the same seed, n and k.
fn rnd(seed: i32, n: i32, k: u32) -> f32 {
    unit(hash(seed, n, k))
}

/// Two directions square to `n` and to each other.
fn across(n: V3) -> (V3, V3) {
    let side = geo::norm(geo::cross(
        n,
        if n[1].abs() < 0.9 {
            [0.0, 1.0, 0.0]
        } else {
            [1.0, 0.0, 0.0]
        },
    ));
    (side, geo::cross(n, side))
}

fn glow(d: &mut Draw, mesh: Mesh, m: render::M4, c: V3, a: f32) {
    d.items
        .push(Item::new(mesh, m).tint(c, a).glow(1.0).pass(Pass::Glow));
}

/// An energy shell about `at` (bright at its edge, clear face on).
fn shell(look: &Look, d: &mut Draw, at: V3, r: V3, c: V3, a: f32) {
    d.items.push(
        Item::new(look.orb, m4::place(at, 0.0, r))
            .tint(c, a)
            .glow(0.4)
            .material(Material::Rim)
            .pass(Pass::Glow),
    );
}

/// A ball of fire about `at`: `grain` noise cells a metre, `heat` how
/// much brighter.
fn energy(look: &Look, d: &mut Draw, at: V3, r: V3, (c, a): (V3, f32), grain: f32, heat: f32) {
    plasma(look, d, at, r, (c, a), (grain, heat), 0.0);
}

/// A ball of flowing energy about `at`, its rim glowing by `rim` (0 fire,
/// ragged at its edges; 1 a bubble of plasma).
fn plasma(
    look: &Look,
    d: &mut Draw,
    at: V3,
    r: V3,
    (c, a): (V3, f32),
    (grain, heat): (f32, f32),
    rim: f32,
) {
    if a <= 0.0 {
        return;
    }
    d.items.push(
        Item::new(look.orb, m4::place(at, 0.0, r))
            .tint(c, a)
            .glow(heat)
            .detail(grain)
            .rough(rim)
            .material(Material::Energy)
            .pass(Pass::Glow),
    );
}

/// How far over the ground a ring or circle lying on it is drawn, so the
/// grass does not hide it.
const OVER_GRASS: f32 = 0.22;

/// A soft ring lying flat at `at` (over the grass).
fn ring(look: &Look, d: &mut Draw, at: V3, r: f32, c: V3, a: f32, turn: f32) {
    if r > 0.01 && a > 0.0 {
        let at = [at[0], at[1] + OVER_GRASS, at[2]];
        glow(d, look.ring, m4::place(at, turn, [r, 1.0, r]), c, a);
    }
}

/// An arcane circle at `at` facing `up`, `r` across, turned by `turn`;
/// lying flat (facing up), over the grass.
fn sigil(look: &Look, d: &mut Draw, at: V3, up: V3, r: f32, (c, a): (V3, f32), turn: f32) {
    if r <= 0.01 || a <= 0.0 {
        return;
    }
    let at = if up[1] > 0.99 {
        [at[0], at[1] + OVER_GRASS, at[2]]
    } else {
        at
    };
    let (s, o) = across(up);
    let (sn, cs) = turn.sin_cos();
    let x = geo::add(geo::scale(s, cs * r), geo::scale(o, sn * r));
    let z = geo::add(geo::scale(s, -sn * r), geo::scale(o, cs * r));
    let m = m4::basis(at, x, geo::scale(up, 0.01), z);
    d.items.push(
        Item::new(look.sigil, m)
            .tint(c, a)
            .glow(1.2)
            .pass(Pass::Glow),
    );
}

/// How a beam is drawn: a hot core (it blooms), a halo bright at its
/// edges, or energy flowing along it (`grain` cells a metre).
#[derive(Clone, Copy)]
pub(crate) enum Ray {
    Core,
    Halo,
    Flow(f32),
}

/// A beam from `a` to `b`, `r` thick (one length of a rope or a fork:
/// its ends meet the next).
pub(crate) fn beam(look: &Look, d: &mut Draw, ab: (V3, V3), r: f32, c: (V3, f32), ray: Ray) {
    beam_of(look.beam, d, ab, r, c, ray);
}

/// A lone beam from `a` to `b`, `r` thick, drawn to a point at each end.
fn lone(look: &Look, d: &mut Draw, ab: (V3, V3), r: f32, c: (V3, f32), ray: Ray) {
    beam_of(look.ray, d, ab, r, c, ray);
}

/// A shaft of light up from `a` toward `b`, `r` thick, fading as it goes.
fn shaft(look: &Look, d: &mut Draw, ab: (V3, V3), r: f32, c: (V3, f32), ray: Ray) {
    beam_of(look.shaft, d, ab, r, c, ray);
}

/// A line of light up from `a` to `b`, `r` thick: bright most of the
/// way, fading out at its top (a cube's pillar's core).
fn pillar(look: &Look, d: &mut Draw, ab: (V3, V3), r: f32, c: (V3, f32), ray: Ray) {
    beam_of(look.pillar, d, ab, r, c, ray);
}

fn beam_of(mesh: Mesh, d: &mut Draw, (a, b): (V3, V3), r: f32, (c, alpha): (V3, f32), ray: Ray) {
    let along = geo::sub(b, a);
    let len = geo::dot(along, along).sqrt();
    if len < 0.01 || alpha <= 0.0 {
        return;
    }
    let (side, other) = across(geo::scale(along, 1.0 / len));
    let m = m4::basis(a, geo::scale(side, r), along, geo::scale(other, r));
    let it = Item::new(mesh, m).tint(c, alpha).pass(Pass::Glow);
    d.items.push(match ray {
        Ray::Core => it.glow(3.0),
        Ray::Halo => it.glow(1.0).material(Material::Rim),
        Ray::Flow(grain) => it
            .glow(0.6)
            .detail(grain)
            .rough(1.0)
            .material(Material::Energy),
    });
}

/// An ice crystal at `at` pointing along `dir`, `len` long and `w` wide.
fn crystal(look: &Look, d: &mut Draw, at: V3, dir: V3, (len, w): (f32, f32), c: V3, a: f32) {
    let (side, other) = across(dir);
    let m = m4::basis(
        at,
        geo::scale(dir, len),
        geo::scale(side, w),
        geo::scale(other, w),
    );
    d.items.push(
        Item::new(look.crystal, m)
            .tint(c, a)
            .glow(0.7)
            .material(Material::Rim)
            .pass(Pass::Glow),
    );
}

fn light(d: &mut Draw, p: V3, r: f32, c: V3, k: f32) {
    if k > 0.01 {
        d.lights.push(Light {
            p,
            r,
            c: geo::scale(c, k),
        });
    }
}

/// Sparks thrown out from a point: how many, how fast (m/s), how long
/// they last (s), how fast they fall (m/s²; 0 none, below 0 they rise),
/// hot then cooling to a colour, how big (m), their shape, and how much
/// of their motion shows as a streak (s; 0 none); `up`: thrown upward
/// only; `floor`: the ground they come down on, if they are thrown
/// from it (they settle there, cooling, rather than sink into it).
#[derive(Clone, Copy)]
struct Spray {
    n: i32,
    speed: f32,
    life: f32,
    fall: f32,
    hot: V3,
    cold: V3,
    size: f32,
    shape: Shape,
    streak: f32,
    up: bool,
    floor: Option<f32>,
}

impl Spray {
    fn new(n: i32, speed: f32, life: f32, (hot, cold): (V3, V3), size: f32) -> Spray {
        Spray {
            n,
            speed,
            life,
            fall: 0.0,
            hot,
            cold,
            size,
            shape: Shape::Glow,
            streak: 0.0,
            up: false,
            floor: None,
        }
    }
}

/// `s` thrown from `at`, `t` seconds ago.
fn spray(d: &mut Draw, at: V3, t: f32, seed: i32, s: Spray) {
    let f = 1.0 - t / s.life;
    if f <= 0.0 || t < 0.0 {
        return;
    }
    for k in 0..s.n {
        let v0 = geo::scale(
            dir(seed, k, s.up || s.fall > 0.0),
            s.speed * (0.35 + 0.65 * rnd(seed, k, 13)),
        );
        // A little drag, so they slow as they spread.
        let go = (1.0 - (-2.2 * t).exp()) / 2.2;
        let mut p = [
            at[0] + v0[0] * go,
            at[1] + v0[1] * go - 0.5 * s.fall * t * t,
            at[2] + v0[2] * go,
        ];
        let slow = (-2.2 * t).exp();
        let mut v = [v0[0] * slow, v0[1] * slow - s.fall * t, v0[2] * slow];
        if let Some(floor) = s.floor.filter(|&y| p[1] < y) {
            p[1] = floor;
            v[1] = 0.0;
        }
        let col = mix(s.hot, s.cold, (1.0 - f).min(1.0));
        d.sparks.push(Spark {
            p,
            size: s.size * (0.4 + 0.6 * f),
            c: [col[0], col[1], col[2], f],
            v: geo::scale(v, s.streak),
            shape: s.shape,
            seed: rnd(seed, k, 14),
        });
    }
}

/// Puffs of smoke (or mist, or dust) rising from `at`, `t` seconds ago:
/// `n` of them drifting out by `spread` m and up by `rise` m/s, growing
/// from `size` to `size * grow`, thinning by `life` s.
#[allow(clippy::too_many_arguments)]
fn puffs(
    d: &mut Draw,
    at: V3,
    (t, seed): (f32, i32),
    (n, life): (i32, f32),
    (c, a): (V3, f32),
    (size, grow): (f32, f32),
    (spread, rise): (f32, f32),
) {
    let f = 1.0 - t / life;
    if f <= 0.0 || t < 0.0 {
        return;
    }
    let ease = 1.0 - (-3.0 * t / life).exp();
    for k in 0..n {
        let w = dir(seed, k, true);
        let out = spread * (0.4 + 0.6 * rnd(seed, k, 15)) * ease;
        let p = [
            at[0] + w[0] * out,
            at[1] + w[1] * out * 0.5 + rise * t * (0.6 + 0.4 * rnd(seed, k, 16)),
            at[2] + w[2] * out,
        ];
        // Thickest a moment after it puffs out, then thinning.
        let thick = (t / (life * 0.12)).min(1.0) * f * f;
        d.sparks.push(Spark {
            p,
            size: size * (1.0 + (grow - 1.0) * ease) * (0.7 + 0.6 * rnd(seed, k, 17)),
            c: [c[0], c[1], c[2], a * thick],
            shape: Shape::Smoke,
            seed: rnd(seed, k, 18),
            ..Default::default()
        });
    }
}

/// A glint at `at`: a star `size` across.
fn glint(d: &mut Draw, at: V3, size: f32, c: V3, a: f32) {
    if a > 0.01 {
        d.sparks.push(Spark {
            p: at,
            size,
            c: [c[0], c[1], c[2], a],
            shape: Shape::Star,
            ..Default::default()
        });
    }
}

/// The spell a wizard has just cast, and how fresh (1 just now, 0 after
/// `CAST_SHOWN` ms), if one.
pub fn casting(list: &[(f64, Ev)], who: u16, now: f64) -> Option<(u8, f32)> {
    list.iter().rev().find_map(|&(when, e)| match e {
        Ev::Cast {
            by,
            spell,
            stage: 0,
            ..
        } if by == who && now - when < CAST_SHOWN => {
            Some((spell, (1.0 - (now - when) / CAST_SHOWN) as f32))
        }
        _ => None,
    })
}

/// How long a cast shows at the wand and in the caster's gesture, and
/// a bolt loosed in its arm (ms).
pub const CAST_SHOWN: f64 = 450.0;

/// The colour and brightness of a wizard's wand tip, from what it is
/// casting (`casting`): the spell, flaring and fading; gold at rest.
pub fn tip(cast: Option<(u8, f32)>) -> (V3, f32) {
    cast.map_or((GOLD, 0.0), |(s, f)| (colour(s), f))
}

/// How far off a cube's pillar shows whole, faint, and not at all (m).
const LOOT_CLEAR: f32 = 30.0;
const LOOT_FAINT: f32 = 90.0;
const LOOT_SEEN: f32 = 140.0;

/// Spell cubes (their icon on every face, under a pillar of their light,
/// taller by rank, a bright core in it, a circle of runes turning under
/// them). Near ones stand out whole; the pillars fade with distance, so
/// far ones do not stripe the skyline; a rank III cube's stands out,
/// wider, its core gold, pulsing.
pub fn loot(look: &Look, d: &mut Draw, l: &Loot, t: f32, eye: V3) {
    for &(id, s, rank, p) in &l.scrolls {
        let far = ((p[0] - eye[0]).powi(2) + (p[2] - eye[2]).powi(2)).sqrt();
        if far > LOOT_SEEN {
            continue;
        }
        // A spell cube, turning and bobbing, tipped to show its top.
        let c = colour(s);
        let bob = 0.85 + 0.12 * (t * 2.2 + id as f32).sin();
        let at = [p[0], p[1] + bob, p[2]];
        let a = t * 1.1 + id as f32;
        let k = 0.5 + 0.06 * rank as f32;
        let (st, ct) = (0.42f32.sin(), 0.42f32.cos());
        let tip = m4::basis([0.0; 3], [1.0, 0.0, 0.0], [0.0, ct, st], [0.0, -st, ct]);
        let m = m4::mul(&m4::place(at, a, [k; 3]), &tip);
        if let Some(&cube) = look.cubes.get(s as usize) {
            d.items.push(Item::new(cube, m).rough(0.45));
        }
        // Its light rises from above it (higher, the higher its rank),
        // fading far off.
        let top = 3.0 + 1.5 * rank as f32;
        let fade = if far < LOOT_CLEAR {
            1.0
        } else if far < LOOT_FAINT {
            1.0 - 0.7 * (far - LOOT_CLEAR) / (LOOT_FAINT - LOOT_CLEAR)
        } else {
            0.3 * (LOOT_SEEN - far) / (LOOT_SEEN - LOOT_FAINT)
        };
        let best = rank >= 3;
        let pulse = if best {
            0.8 + 0.2 * (t * 2.0).sin()
        } else {
            1.0
        };
        let up = ([at[0], at[1] + k * 0.9, at[2]], [p[0], p[1] + top, p[2]]);
        let wide = (0.05 + 0.02 * rank as f32) * if best { 2.0 } else { 1.0 };
        beam(look, d, up, wide, (c, 0.35 * fade * pulse), Ray::Halo);
        let core = if best {
            (GOLD, fade * pulse)
        } else {
            (mix(c, WHITE, 0.35), 0.6 * fade)
        };
        pillar(look, d, up, 0.02, core, Ray::Core);
        let floor = [p[0], p[1] + 0.06, p[2]];
        sigil(look, d, floor, UP, 0.62, (c, 0.55), t * 0.4);
        if rank > 1 {
            ring(look, d, [p[0], p[1] + 0.07, p[2]], 0.85, GOLD, 0.6, -t);
        }
        // Now and then a glint on it.
        let wink = ((t * 0.7 + id as f32 * 0.37).fract() - 0.9).max(0.0) * 10.0;
        if wink > 0.0 {
            let side = [at[0] + 0.3 * k, at[1] + 0.35 * k, at[2]];
            glint(d, side, 0.5 * (1.0 - (wink * 2.0 - 1.0).abs()), WHITE, 1.0);
        }
        light(d, at, 3.0 + rank as f32, c, 1.2);
    }
}

/// What a look from `eye` along `dir` meets within `range`, as the page
/// sees it: the point, and the wizard there (not `me`), if one.
pub fn sight(
    map: &wandfall::map::Map,
    others: &[Seen],
    me: u16,
    (eye, dir): (V3, V3),
    range: f32,
) -> (V3, Option<u16>) {
    let end = geo::add(eye, geo::scale(dir, range));
    let mut first = (map.strikes(eye, end).unwrap_or(1.0), None);
    for s in others
        .iter()
        .filter(|s| s.id != me && s.flags & wandfall::proto::flag::ALIVE != 0)
    {
        let tall = if s.flags & wandfall::proto::flag::CROUCH != 0 {
            wandfall::laws::CROUCH_HEIGHT
        } else {
            wandfall::laws::HEIGHT
        };
        if let Some(t) = wandfall::world::through(eye, end, s.p, tall) {
            if t < first.0 {
                first = (t, Some(s.id));
            }
        }
    }
    (geo::add(eye, geo::scale(dir, range * first.0)), first.1)
}

/// Where Lightning would strike: a faint circle of runes, while you aim
/// with it ready.
pub fn aim_ring(look: &Look, d: &mut Draw, at: V3, t: f32) {
    let c = colour(spell::LIGHTNING);
    let pulse = 0.35 + 0.15 * (t * 6.0).sin();
    runes(look, d, at, LIGHTNING_RADIUS, (c, pulse), t * 0.5);
}

/// How much of a warning circle is laid on the ground under the one
/// drawn over the grass (where the renderer lays decals).
const LAID: f32 = 0.6;

/// A circle of runes on the ground about `at`, `r` across, turned by
/// `turn`, telling where something will strike: over the grass, so it
/// reads from a wizard's own eyes (laid on the ground alone, the grass
/// hides it), and laid on the ground under it too where the renderer
/// lays decals, so it takes to a slope, a step, a deck.
fn runes(look: &Look, d: &mut Draw, at: V3, r: f32, (c, a): (V3, f32), turn: f32) {
    if r <= 0.01 || a <= 0.0 {
        return;
    }
    sigil(look, d, [at[0], at[1] + 0.08, at[2]], UP, r, (c, a), turn);
    if look.marks {
        d.decals.push(Decal {
            p: at,
            r,
            depth: lay_depth(r),
            yaw: turn,
            c: [c[0], c[1], c[2], a * LAID],
            mark: Mark::Runes,
            seed: 0.37,
        });
    }
}

/// A ring of light on the ground about `at`, `r` across, telling the
/// edge of where something will strike: as `runes` are, over the grass
/// and laid under it.
fn rim(look: &Look, d: &mut Draw, at: V3, r: f32, c: V3, a: f32) {
    ring(look, d, [at[0], at[1] + 0.08, at[2]], r, c, a, 0.0);
    if look.marks {
        lay_ring(d, at, r, c, a * LAID);
    }
}

/// A ring of light rushing out along the ground about `at`, `r` across
/// (a shockwave, dust, a fall: nothing to dodge): laid on it as a decal
/// where the renderer lays them (nothing in the air, all of it on a
/// slope), else a soft ring lying flat.
fn wave(look: &Look, d: &mut Draw, at: V3, r: f32, c: V3, a: f32) {
    if look.marks {
        lay_ring(d, at, r, c, a);
    } else {
        ring(look, d, at, r, c, a, 0.0);
    }
}

/// A soft ring of light laid on the ground about `at`, `r` across.
fn lay_ring(d: &mut Draw, at: V3, r: f32, c: V3, a: f32) {
    if r > 0.01 && a > 0.0 {
        d.decals.push(Decal {
            p: at,
            // The decal's ring is brightest a little in from its edge.
            r: r / 0.88,
            depth: lay_depth(r),
            yaw: 0.0,
            c: [c[0], c[1], c[2], a],
            mark: Mark::Ring,
            seed: 0.0,
        });
    }
}

/// How far up and down a decal `r` across reaches (m): enough to lie
/// whole on a slope of some 25 degrees.
fn lay_depth(r: f32) -> f32 {
    (r * 0.95).max(1.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_effect_keeps_its_seed_while_the_list_about_it_changes() {
        let shard = |x: f32| Ev::Cast {
            by: 4,
            spell: spell::FROST,
            stage: 1,
            at: [x, 1.0, 2.0],
        };
        // Its seed is its own, wherever in the list it is: an older show
        // let go does not change it.
        let list = [(100.0, shard(0.0)), (900.0, shard(1.0))];
        let seeds = |l: &[(f64, Ev)]| l.iter().map(|(w, e)| seed_of(*w, e)).collect::<Vec<_>>();
        assert_eq!(seeds(&list)[1], seeds(&list[1..])[0]);
        // Frost's shards, landing together, each their own.
        let batch: Vec<i32> = (0..7)
            .map(|k| seed_of(900.0, &shard(k as f32 * 0.5)))
            .collect();
        for (k, s) in batch.iter().enumerate() {
            assert!(!batch[..k].contains(s), "{batch:?}");
        }
    }

    #[test]
    fn a_trail_s_puffs_keep_their_own_life_as_they_stream_back() {
        // Over a few sheddings, the puff with each life only moves back.
        let rate = 14.0;
        let mut was: Vec<(i32, f32)> = Vec::new();
        for f in 0..40 {
            let t = 3.0 + f as f32 / 120.0;
            let now: Vec<(i32, f32)> = trail(16, rate, t).map(|(u, life)| (life, u)).collect();
            for &(life, u) in &now {
                if let Some(&(_, u0)) = was.iter().find(|p| p.0 == life) {
                    assert!(u >= u0 && u - u0 < 0.02, "life {life}: {u0} to {u}");
                }
            }
            was = now;
        }
        // A mote's life stays the same through it.
        let (u0, l0) = cycle(2.0, 9.0, 0.3);
        let (u1, l1) = cycle(2.0 + 0.5 * (1.0 - u0) / 9.0, 9.0, 0.3);
        assert!(l0 == l1 && u1 > u0);
    }

    #[test]
    fn sparks_thrown_on_the_ground_come_down_on_it() {
        let mut d = Draw::default();
        let s = Spray {
            fall: 12.0,
            floor: Some(0.0),
            ..Spray::new(30, 10.0, 1.0, (WHITE, WHITE), 0.1)
        };
        spray(&mut d, [0.0, 0.1, 0.0], 0.8, 7, s);
        assert!(d.sparks.iter().all(|p| p.p[1] >= 0.0));
        assert!(d.sparks.iter().any(|p| p.p[1] == 0.0));
    }
}
