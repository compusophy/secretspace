//! Any one effect on its own, to look at (the page's `?fx=name&age=ms`):
//! made from the same events and bolts a match sends; `sparks`, one spark
//! of each shape; `scars`, what spells leave on the ground; `gesture`
//! and a spell's number, a wizard casting it.

use render::geo::{self, V3};
use wandfall::laws::spell;
use wandfall::proto::{fx, BoltSeen, Ev, Seen};
use wandfall::world::WAND;

use super::{bolt, falls, on_wizard, shows, Draw};
use crate::look::Look;

/// The effects there are, by name.
pub const NAMES: [&str; 25] = [
    "wand",
    "fireball",
    "burst",
    "lance",
    "frost",
    "shatter",
    "mark",
    "strike",
    "blink",
    "arrive",
    "ward",
    "break",
    "mend",
    "gust",
    "level",
    "shield",
    "chill",
    "mending",
    "fall",
    "sparks",
    "storm",
    "tether",
    "catch",
    "scars",
    "storm-far",
];

/// Effect `name` at `age` ms (`now` ms, `t` s), cast from `at` (feet)
/// toward `fwd`.
pub fn gallery(look: &Look, d: &mut Draw, name: &str, (age, now): (f64, f64), at: V3, fwd: V3) {
    let t = (now / 1000.0) as f32;
    let secs = (age / 1000.0) as f32;
    let chest = [at[0], at[1] + 1.3, at[2]];
    let ahead = |m: f32| geo::add(chest, geo::scale(fwd, m));
    let on = |m: f32| {
        let p = geo::add(at, geo::scale(fwd, m));
        [p[0], p[1] + 1.0, p[2]]
    };
    let flying = |kind: u8, speed: f32| BoltSeen {
        id: 7,
        by: 1,
        kind,
        p: ahead(1.0 + speed * secs),
        v: geo::scale(fwd, speed),
    };
    let cast = |s: u8, stage: u8, p: V3| Ev::Cast {
        by: 1,
        spell: s,
        stage,
        at: p,
    };
    // `gesture` and a spell's number: a wizard casting it, facing you.
    if let Some(sp) = name
        .strip_prefix("gesture")
        .and_then(|n| n.parse::<u8>().ok())
    {
        let fresh = (1.0 - age / 450.0).clamp(0.0, 1.0) as f32;
        let me = geo::add(at, geo::scale(fwd, 2.5));
        let yaw = fwd[2].atan2(fwd[0]) + std::f32::consts::FRAC_PI_2;
        let pose = crate::rig::Pose {
            flash: 0.0,
            arm: 1.0,
            aim: 0.1,
            tip: (super::colour(sp), fresh),
            spell: Some((sp, fresh)),
            t,
        };
        let a = crate::rig::Anim::default();
        look.rig.wizard(d, 3, me, yaw, &a, &pose, 0.0);
        return;
    }
    let ev = match name {
        "wand" => {
            bolt(look, d, &flying(WAND, 30.0), false, t);
            None
        }
        "fireball" => {
            bolt(look, d, &flying(spell::FIREBALL, 8.0), false, t);
            None
        }
        "frost" => {
            bolt(look, d, &flying(spell::FROST, 14.0), false, t);
            Some(cast(spell::FROST, 0, ahead(0.6)))
        }
        "burst" => Some(cast(spell::FIREBALL, 1, on(4.0))),
        "lance" => Some(Ev::Beam {
            by: 1,
            spell: spell::LANCE,
            from: ahead(0.6),
            to: ahead(14.0),
        }),
        "shatter" => Some(cast(spell::FROST, 1, on(4.0))),
        "mark" => Some(cast(
            spell::LIGHTNING,
            2,
            geo::add(at, geo::scale(fwd, 5.0)),
        )),
        "strike" => Some(cast(
            spell::LIGHTNING,
            1,
            geo::add(at, geo::scale(fwd, 5.0)),
        )),
        "blink" => Some(cast(spell::BLINK, 0, [at[0], at[1] + 1.6, at[2]])),
        "arrive" => Some(cast(spell::BLINK, 1, [at[0], at[1] + 1.0, at[2]])),
        "ward" => Some(cast(spell::WARD, 0, [at[0], at[1] + 1.5, at[2]])),
        "break" => Some(cast(spell::WARD, 2, [at[0], at[1] + 1.0, at[2]])),
        "mend" => Some(cast(spell::MEND, 0, [at[0], at[1] + 1.55, at[2]])),
        "gust" => Some(cast(spell::GUST, 0, [at[0], at[1] + 1.5, at[2]])),
        "level" => Some(Ev::Level { who: 1, level: 2 }),
        "shield" | "chill" | "mending" => {
            let mark = match name {
                "shield" => fx::SHIELD,
                "chill" => fx::CHILLED,
                _ => fx::MENDING,
            };
            let s = Seen {
                id: 1,
                p: at,
                fx: mark,
                ..Seen::default()
            };
            on_wizard(look, d, &s, t, false);
            None
        }
        "sparks" => {
            for (k, shape) in [
                render::Shape::Smoke,
                render::Shape::Flame,
                render::Shape::Star,
                render::Shape::Glow,
            ]
            .into_iter()
            .enumerate()
            {
                d.sparks.push(render::Spark {
                    p: geo::add(ahead(2.0 + k as f32 * 1.2), [0.0, 0.5, 0.0]),
                    size: 1.0,
                    c: [1.0, 0.6, 0.3, 1.0],
                    shape,
                    seed: 0.3,
                    ..Default::default()
                });
            }
            None
        }
        "storm" => {
            let c = geo::sub(at, geo::scale(fwd, -20.0));
            look.storm(d, [c[0], c[2]], 26.0, at, t);
            None
        }
        // The wall 30 m ahead of the eye (`fwd` is to its right).
        "storm-far" => {
            let c = geo::sub(at, geo::scale([fwd[2], 0.0, -fwd[0]], 80.0));
            look.storm(d, [c[0], c[2]], 110.0, at, t);
            None
        }
        "fall" => {
            falls(look, d, &[(now - age, at, 1)], now);
            None
        }
        // A burn to one side, lightning's ahead, frost's to the other
        // side, the Lance's near.
        "scars" => {
            let side = [-fwd[2], 0.0, fwd[0]];
            let by =
                |m: f32, s: f32| geo::add(at, geo::add(geo::scale(fwd, m), geo::scale(side, s)));
            let list = [
                (now - age, by(2.0, 3.5), spell::FIREBALL),
                (now - age, by(5.0, 0.0), spell::LIGHTNING),
                (now - age, by(2.5, -3.5), spell::FROST),
                (now - age, by(1.5, 0.8), spell::LANCE),
            ];
            super::scars(d, &list, now);
            None
        }
        // A rope to a point up ahead (`age` since it was thrown), and the
        // catch there.
        "tether" | "catch" => {
            let to = geo::add(ahead(9.0), [0.0, 4.0, 0.0]);
            let e = cast(spell::TETHER, 1, to);
            if name == "tether" {
                let s = Seen {
                    id: 1,
                    p: at,
                    fx: fx::TETHER,
                    ..Seen::default()
                };
                super::ropes(look, d, &[s], &[(now - age, e)], now);
            }
            Some(e)
        }
        _ => None,
    };
    if let Some(e) = ev {
        shows(look, d, &[(now - age, e)], now, (0, None), |_| Some(at));
    }
}
