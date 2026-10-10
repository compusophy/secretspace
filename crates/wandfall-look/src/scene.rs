//! The match drawn from what a page has been told: the loot, the places,
//! every wizard (jointed, stepping as it moves), bolts, bursts, spells'
//! shows, the fallen, dust and the storm. Wandfall's page draws it over
//! your shoulder (your own wizard as you predict it); the hub's card
//! over a fighter's.

use std::collections::HashMap;

use render::{Camera, V3};
use wandfall::laws::{spell, GUST_RADIUS};
use wandfall::proto::{self, flag, Ev, Frame, Seen};
use wandfall::trig;
use wandfall::world::WAND;

use crate::fx::{self, Casters, Clock, Draw, Drawn, CAST_SHOWN};
use crate::look::Look;
use crate::rig;
use crate::state::State;

/// Who is looking: your wizard (its id, where its feet are, whether it
/// is alive).
#[derive(Clone, Copy, Debug, Default)]
pub struct Eyes {
    pub id: u16,
    pub at: V3,
    pub alive: bool,
}

/// How to draw it: every effect held at an age (`?hold`), and whether the
/// storm stands (not on the practice range).
#[derive(Clone, Copy, Debug, Default)]
pub struct Show {
    pub hold: Option<f64>,
    pub storm: bool,
}

/// How long after a wizard is knocked out it is no longer drawn standing
/// (ms): the frames it is drawn from run a tenth of a second behind, and
/// still have it on its feet.
const FALLING: f64 = 600.0;

/// Whether a Gust has just gone off near `s` (cast by another a moment
/// ago, within its reach and as far again as a throw carries it): it
/// throws whoever stands there, hurt or not.
fn gusted(shows: &[(f64, Ev)], s: &Seen, now: f64) -> bool {
    shows
        .iter()
        .rev()
        .take_while(|e| now - e.0 < 600.0)
        .any(|&(_, e)| {
            matches!(e, Ev::Cast { by, spell: spell::GUST, at, .. }
            if by != s.id && (at[0] - s.p[0]).hypot(at[2] - s.p[2]) < GUST_RADIUS + 6.0)
        })
}

/// How far `at` is from the eye (the rig draws coarser far off).
fn far(at: V3, eye: V3) -> f32 {
    let d = render::geo::sub(at, eye);
    render::geo::dot(d, d).sqrt()
}

/// Everything in the match into `d`, at `now` (`dt` since the last
/// frame); whether the eyes are out in the storm.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    look: &Look,
    d: &mut Draw,
    st: &mut State,
    anims: &mut HashMap<u16, rig::Anim>,
    others: &[Seen],
    eyes: &Eyes,
    cam: &Camera,
    (now, dt): (f64, f64),
    show: Show,
) -> bool {
    let t = (now / 1000.0) as f32;
    let clock = Clock {
        now,
        hold: show.hold,
    };
    // What is over let go each frame; held, only when events come (to
    // look at them as long as one likes).
    if show.hold.is_none() {
        st.prune(now);
    }
    // A wizard just knocked out falls where it was drawn (not where the
    // newest frame has it, a tick on), and is no longer drawn standing.
    for f in st.falls.iter_mut().filter(|f| !f.4) {
        if let Some(s) = others.iter().find(|s| s.id == f.2) {
            (f.1, f.3) = (s.p, s.yaw);
        }
        f.4 = true;
    }
    let falling: Vec<u16> = st
        .falls
        .iter()
        .filter(|f| now - f.0 < FALLING)
        .map(|f| f.2)
        .collect();
    fx::loot(look, d, &st.loot, t, cam.eye);
    let wind = st.frame.as_ref().map_or([0.0; 2], |f| {
        crate::sky::wind(
            crate::sky::Hour::from(f.hour),
            crate::sky::Weather::from(f.weather),
        )
    });
    look.places(d, t, (cam.eye, wind));
    let mut drawn = Vec::with_capacity(others.len());
    for s in others {
        if s.flags & flag::ALIVE == 0 || falling.contains(&s.id) {
            continue;
        }
        // Casting (or firing) raises its arm; a hit flinches it.
        let cast = fx::casting(&st.shows, s.id, now);
        let tip = fx::tip(cast);
        let fired = st.fired.get(&s.id).map_or(1e9, |&f| now - f);
        let hit = st
            .bursts
            .iter()
            .filter(|b| b.1 == s.id)
            .map(|b| now - b.0)
            .fold(1e9, f64::min);
        let aimed = (
            trig::radians(s.yaw),
            s.pitch as f32 / 65536.0 * std::f32::consts::TAU,
        );
        let fresh = !anims.contains_key(&s.id);
        let a = anims.entry(s.id).or_default();
        let glide = s.flags & flag::GLIDE != 0;
        if fresh {
            a.seat.x = glide as i32 as f32;
        }
        let slide = s.flags & flag::SLIDE != 0;
        let stance = (
            s.flags & flag::GROUND != 0,
            s.flags & flag::CROUCH != 0,
            slide,
        );
        // Hauled by a Tether or thrown by a Gust: not its own leap.
        if s.fx & proto::fx::TETHER != 0 || gusted(&st.shows, s, now) {
            a.shove();
        }
        if let Some(hard) = a.step(s.p, aimed, stance, dt as f32 / 1000.0) {
            st.dust.push((now, s.p, hard));
        }
        a.seat.step(glide as i32 as f32, 0.06, dt as f32 / 1000.0);
        // Drawn facing and looking as eased, so an aim that jumps
        // does not jerk the body.
        let (yaw, aim) = (a.face.x, a.pitch.x);
        let pose = rig::Pose {
            spell: cast,
            flash: (1.0 - hit / 200.0).max(0.0) as f32,
            arm: (tip.1 * 2.0)
                .max((1.0 - fired / CAST_SHOWN) as f32 * 1.5)
                .min(1.0),
            aim,
            tip,
            t,
        };
        // A slide kicks up dust behind it.
        if slide && (now / 90.0).floor() != ((now - dt) / 90.0).floor() {
            st.dust.push((now, s.p, 0.25));
        }
        // Far off, the coarser wizard.
        let frames = look
            .rig
            .wizard(d, s.id, s.p, yaw, a, &pose, far(s.p, cam.eye));
        drawn.push(Drawn {
            id: s.id,
            feet: s.p,
            tip: frames.tip(),
        });
        fx::on_wizard(look, d, s, t);
    }
    fx::bolts(look, d, &st.bolts(now), eyes.id, t);
    // Light where something struck someone: gold for your wand's hits,
    // everyone else's wand in its own colour; sparks falling to the
    // ground under one standing.
    for &(at, who, what, by) in &st.bursts {
        if let Some(s) = others.iter().find(|s| s.id == who) {
            let c = if what == WAND && by != eyes.id {
                fx::FOE
            } else {
                fx::colour(what)
            };
            let floor = (s.flags & flag::GROUND != 0).then_some(s.p[1] + 0.05);
            let seed = at as i32 ^ (who as i32).wrapping_mul(7919);
            let pos = [s.p[0], s.p[1] + 1.2, s.p[2]];
            fx::burst(d, pos, ((now - at) as f32, seed), c, floor);
        }
    }
    let who = Casters {
        drawn: &drawn,
        stood: &st.stood,
    };
    fx::shows(look, d, &st.shows, clock, &who);
    fx::ropes(look, d, others, &st.shows, clock, &who);
    // The knocked out fall, and burst into sparks.
    for &(when, at, who, yaw, _) in &st.falls {
        let age = (clock.age(when) / 1000.0) as f32;
        look.rig
            .fallen(d, who, (at, far(at, cam.eye)), trig::radians(yaw), age);
    }
    let falls: Vec<_> = st.falls.iter().map(|f| (f.0, f.1, f.2)).collect();
    fx::falls(look, d, &falls, clock);
    fx::dust(look, d, &st.dust, now);
    fx::scars(d, &st.scars, now);
    anims.retain(|id, _| others.iter().any(|s| s.id == *id));
    match &st.frame {
        Some(f) if f.phase == 1 && show.storm => {
            look.storm(d, f.storm.0, f.storm.1, cam.eye, t);
            in_storm(eyes, f)
        }
        _ => false,
    }
}

/// Whether the eyes' wizard stands out in the storm: where its feet are,
/// not where the camera is (behind it, over its shoulder).
fn in_storm(eyes: &Eyes, f: &Frame) -> bool {
    let (c, r) = f.storm;
    let e = eyes.at;
    eyes.alive && (e[0] - c[0]).powi(2) + (e[2] - c[1]).powi(2) > r * r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_storm_is_where_your_feet_are_not_where_the_camera_is() {
        let f = Frame {
            phase: 1,
            storm: ([0.0, 0.0], 50.0),
            ..Frame::default()
        };
        // Just inside the wall, the camera a few metres behind you, out
        // past it: you are not in the storm.
        let inside = Eyes {
            id: 1,
            at: [48.0, 0.0, 0.0],
            alive: true,
        };
        assert!(!in_storm(&inside, &f));
        // Just outside, the camera behind you, inside: you are.
        let outside = Eyes {
            at: [52.0, 0.0, 0.0],
            ..inside
        };
        assert!(in_storm(&outside, &f));
        assert!(!in_storm(
            &Eyes {
                alive: false,
                ..outside
            },
            &f
        ));
    }
}
