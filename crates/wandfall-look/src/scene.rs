//! The match drawn from what a page has been told: the loot, the places,
//! every wizard (jointed, stepping as it moves), bolts, bursts, spells'
//! shows, the fallen, dust and the storm. Wandfall's page draws it from
//! your eyes (your own wizard left out; your wand and broom are its own);
//! the hub's card from over a fighter's shoulder.

use std::collections::HashMap;

use render::{Camera, V3};
use wandfall::proto::{flag, Seen};
use wandfall::trig;

use crate::fx::{self, Draw};
use crate::look::Look;
use crate::rig;
use crate::state::State;

/// Who is looking: your wizard (its id, where it stands, whether it is
/// alive); whether from its own eyes (then it is not drawn); where your
/// wand's tip is (your spells leave it).
#[derive(Clone, Copy, Debug, Default)]
pub struct Eyes {
    pub id: u16,
    pub at: V3,
    pub alive: bool,
    pub first: bool,
    pub tip: Option<V3>,
}

/// How to draw it: every effect held at an age (`?hold`), and whether the
/// storm stands (not on the practice range).
#[derive(Clone, Copy, Debug, Default)]
pub struct Show {
    pub hold: Option<f64>,
    pub storm: bool,
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
    let own = |id: u16| eyes.first && id == eyes.id;
    fx::loot(look, d, &st.loot, t, cam.eye);
    look.places(d, t);
    for s in others {
        if s.flags & flag::ALIVE == 0 {
            continue;
        }
        if !own(s.id) {
            // Casting (or firing) raises its arm; a hit flinches it.
            let tip = fx::tip(&st.shows, s.id, now);
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
            if let Some(hard) = a.step(s.p, aimed, stance, dt as f32 / 1000.0) {
                st.dust.push((now, s.p, hard));
            }
            a.seat.step(glide as i32 as f32, 0.06, dt as f32 / 1000.0);
            // Drawn facing and looking as eased, so an aim that jumps
            // does not jerk the body.
            let (yaw, aim) = (a.face.x, a.pitch.x);
            let pose = rig::Pose {
                flash: (1.0 - hit / 200.0).max(0.0) as f32,
                arm: (tip.1 * 2.0)
                    .max((1.0 - fired as f32 / 450.0) * 1.5)
                    .min(1.0),
                aim,
                tip,
                glide,
                t,
            };
            // A slide kicks up dust behind it.
            if slide && (now / 90.0).floor() != ((now - dt) / 90.0).floor() {
                st.dust.push((now, s.p, 0.25));
            }
            // Far off, the coarser wizard.
            let far = render::geo::dot(
                render::geo::sub(s.p, cam.eye),
                render::geo::sub(s.p, cam.eye),
            ) > 16.0 * 16.0;
            look.rig.wizard(d, s.id, s.p, yaw, a, &pose, far);
        }
        fx::on_wizard(look, d, s, t, own(s.id));
    }
    for b in st.bolts(now) {
        fx::bolt(look, d, &b, b.by == eyes.id, t);
    }
    for &(at, who, what) in &st.bursts {
        if let Some(s) = others.iter().find(|s| s.id == who) {
            let pos = [s.p[0], s.p[1] + 1.2, s.p[2]];
            look.burst(
                &mut d.lights,
                &mut d.sparks,
                pos,
                ((now - at) as f32, at as u32),
                fx::colour(what),
            );
        }
    }
    let at_of = |id: u16| {
        if eyes.alive && id == eyes.id {
            return Some(eyes.at);
        }
        others.iter().find(|s| s.id == id).map(|s| s.p)
    };
    // `hold` holds every effect at that age (to look at them).
    let hold = show.hold;
    let held: Vec<_>;
    let shows = match hold {
        Some(ms) => {
            held = st
                .shows
                .iter()
                .map(|&(w, e)| (w.max(now - ms), e))
                .collect();
            &held
        }
        None => &st.shows,
    };
    fx::shows(look, d, shows, now, (eyes.id, eyes.tip), at_of);
    // The knocked out fall, and burst into sparks.
    let falls: Vec<_> = st
        .falls
        .iter()
        .map(|&(w, at, who, _)| (hold.map_or(w, |ms| w.max(now - ms)), at, who))
        .collect();
    for &(when, at, who, yaw) in &st.falls {
        if !(eyes.alive && own(who)) {
            let when = hold.map_or(when, |ms| when.max(now - ms));
            let age = ((now - when) / 1000.0) as f32;
            look.rig.fallen(d, who, at, trig::radians(yaw), age);
        }
    }
    fx::falls(look, d, &falls, now);
    fx::dust(look, d, &st.dust, now);
    anims.retain(|id, _| others.iter().any(|s| s.id == *id));
    let mut in_storm = false;
    if let Some(f) = &st.frame {
        if f.phase == 1 && show.storm {
            look.storm(d, f.storm.0, f.storm.1, cam.eye, t);
            let e = cam.eye;
            in_storm = eyes.alive
                && (e[0] - f.storm.0[0]).powi(2) + (e[2] - f.storm.0[1]).powi(2)
                    > f.storm.1 * f.storm.1;
        }
    }
    in_storm
}
