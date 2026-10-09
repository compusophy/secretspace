//! The page's practice range and its menus: starting the range (and the
//! address's hooks for looking at it), and what each menu button does.

use engine::room::{Outbox, Room, Who};
use wandfall::practice;
use wandfall::proto::{Up, PROTO};
use wandfall::room::Wandfall;

use super::{
    grab, leave, online, query, query_value, repaint, resume, send, Mode, Page, ME, PRACTICE_SEED,
};
use crate::menu::Act;

pub(super) fn practise(p: &mut Page) {
    leave(p);
    let mut room = Box::new(Wandfall::practice(PRACTICE_SEED));
    let name = Some(p.session.name())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "you".to_string());
    let who = Who {
        name,
        ..Who::default()
    };
    let mut out = Outbox::default();
    room.open(ME, &who, &mut out);
    p.inbox.extend(out.0.into_iter().map(|m| m.1));
    p.mode = Mode::Practice(room);
    send(p, &Up::Join { proto: PROTO });
    // `?spells=0,1,4,5` sets out the four slots; `?nocd` drops cooldowns;
    // `?spar` has the dummies fight back.
    if let Mode::Practice(room) = &mut p.mode {
        let w = room.world();
        let me = w.players.iter().find(|p| !p.bot).map_or(0, |p| p.id);
        if let Some(list) = query_value("spells") {
            if let Some(q) = w.find_mut(me) {
                q.slots = [None; 4];
            }
            for (slot, sp) in list.split(',').filter_map(|v| v.parse().ok()).enumerate() {
                practice::equip(w, me, slot, sp, 1);
            }
        }
        if let Some(r) = w.practice.as_mut() {
            r.no_cooldowns |= query("nocd");
            r.sparring |= query("spar");
        }
    }
    // `?look=yaw,pitch` (degrees) faces you a way to start.
    if let Some(v) = query_value("look") {
        let mut it = v
            .split(',')
            .filter_map(|x| x.parse::<f32>().ok())
            .filter(|x| x.is_finite());
        p.yaw = it.next().unwrap_or(0.0).to_radians();
        p.pitch = it.next().unwrap_or(0.0).to_radians();
    }
    if !p.touch {
        grab(p);
    }
}

/// `?cam=x,y,z,yaw,pitch` (metres, degrees): the camera held there, to
/// look at the island (with `?orbit`, which hides the HUD).
pub(super) fn fixed() -> Option<[f32; 5]> {
    thread_local! {
        static CAM: Option<[f32; 5]> = query_value("cam").and_then(|v| {
            let n: Vec<f32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
            (n.len() == 5 && n.iter().all(|x| x.is_finite()))
                .then(|| [n[0], n[1], n[2], n[3].to_radians(), n[4].to_radians()])
        });
    }
    CAM.with(|c| *c)
}

/// A menu button pressed.
pub(super) fn act(p: &mut Page, a: Act) {
    let you = p.st.you;
    let own = p.st.frame.as_ref().and_then(|f| f.you);
    match a {
        Act::Online => online(p),
        Act::Practice => practise(p),
        Act::Leave => leave(p),
        Act::Look(d) => {
            p.set.turn(d);
            p.set.save();
        }
        Act::Volume(d) => {
            p.set.loud(d);
            p.set.save();
            p.sounds.audio.set_volume(p.set.volume);
            p.sounds.audio.wake();
            p.sounds.cube([0.0; 3], ([0.0; 3], 0.0));
        }
        Act::Picture(k) => {
            if p.set.picture != k {
                p.set.picture = k;
                p.set.save();
                let q = p.set.quality("", p.g.caps.software, kit::touch());
                repaint(p, q);
            }
        }
        Act::Settings => {
            kit::input::unlock();
            p.meta.game_panel();
        }
        Act::CloseSettings => p.meta.back(),
        Act::Book => {
            p.book = true;
            p.meta.hide();
            kit::input::unlock();
        }
        Act::CloseBook => {
            p.book = false;
            if !p.touch {
                grab(p);
            }
        }
        Act::Slot(k) => p.book_slot = k,
        Act::Lessons => {
            p.lessons.again();
            resume(p);
        }
        Act::Stay => {}
        // Online or on the range, the room puts a spell you know in.
        Act::Spell(sp) => {
            let slot = p.book_slot as u8;
            send(p, &Up::Equip { slot, spell: sp });
        }
        _ => {
            let Mode::Practice(room) = &mut p.mode else {
                return;
            };
            let w = room.world();
            let slot = p.book_slot;
            let held = own.and_then(|o| o.slots[slot]);
            match a {
                Act::Rank(r) => {
                    if let Some((sp, _)) = held {
                        practice::equip(w, you, slot, sp, r);
                    }
                }
                Act::Level(d) => {
                    let level = own.map_or(1, |o| o.level) as i8;
                    let to = if d == 20 {
                        20
                    } else {
                        (level + d).clamp(1, 20)
                    };
                    practice::set_level(w, you, to as u8);
                }
                Act::NoCooldowns => {
                    if let Some(r) = w.practice.as_mut() {
                        r.no_cooldowns = !r.no_cooldowns;
                    }
                }
                Act::Sparring => {
                    if let Some(r) = w.practice.as_mut() {
                        r.sparring = !r.sparring;
                    }
                }
                _ => {}
            }
        }
    }
}
