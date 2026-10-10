//! What the room says, taken in: the island, who is who, events, loot,
//! frames (your own wizard confirmed against its prediction); the link
//! lost and found again; a room on another protocol than this page.

use super::*;

pub(super) fn net(p: &mut Page, now: f64, dt: f64) {
    let mut got = std::mem::take(&mut p.inbox);
    let events = match &mut p.mode {
        Mode::Online(link) => link.poll(now),
        Mode::Practice(room) => {
            p.local = (p.local + dt).min(MS_A_TICK * 6.0);
            while p.local >= MS_A_TICK {
                p.local -= MS_A_TICK;
                let mut out = Outbox::default();
                room.tick(&mut out);
                got.extend(out.0.into_iter().map(|m| m.1));
            }
            Vec::new()
        }
        Mode::Title => Vec::new(),
    };
    for b in got {
        receive(p, &b, now);
    }
    // In order: a link found again before what it then says.
    for ev in events {
        match ev {
            Net::Up => {
                if p.lost.take().is_some() || p.st.joined {
                    forget(p);
                }
                send(p, &Up::Join { proto: PROTO });
            }
            // Lost (or a deploy holding still): the picture kept, your
            // wizard held where it was till the room is back.
            Net::Holding => {
                p.lost.get_or_insert(now);
                p.prev = p.pred.body;
            }
            Net::Message(b) => receive(p, &b, now),
        }
        // Left (a room on another protocol): the rest is not ours.
        if !matches!(p.mode, Mode::Online(_)) {
            break;
        }
    }
}

fn receive(p: &mut Page, b: &[u8], now: f64) {
    if let Some(seen) = Named::decode(b) {
        p.taken = seen.status == Status::Taken;
        if p.taken {
            // You play under the name the room had for you; the title says
            // to pick another.
            let say = "that name is someone else's: pick another on the title";
            p.st.feed.push_back((now, Line::Text(say.to_string())));
        } else if !seen.name.is_empty() {
            p.session.set_name(&seen.name);
            if !p.name.focused() {
                p.name.set_value(&seen.name);
            }
        }
        // From now on, the name the room knows (asked for again on every
        // reconnect, not renamed).
        if let Mode::Online(link) = &p.mode {
            link.set_hello(p.session.hello(&p.session.name(), false));
        }
        return;
    }
    if let Some((v, you, seed, _)) = proto::read_welcome(b) {
        if v != PROTO {
            mismatch(p, v, now);
            return;
        }
        p.st.you = you;
        p.st.joined = you != 0;
        island(p, seed);
        return;
    }
    if let Some(list) = proto::read_roster(b) {
        p.st.names = list
            .into_iter()
            .map(|(id, bot, n)| (id, (n, bot)))
            .collect();
        return;
    }
    if let Some(list) = proto::read_events(b) {
        p.sounds.events(&list, p.st.you, p.alive, p.ear);
        p.st.events(list, now);
        return;
    }
    if let Some(h) = proto::read_hall(b) {
        p.st.hall = h;
        return;
    }
    if let Some(l) = proto::Loot::decode(b) {
        p.sounds.loot(&p.st.loot, &l, p.ear);
        p.st.loot = l;
        return;
    }
    if let Some(f) = proto::Frame::decode(b) {
        p.sounds.frame(p.st.frame.as_ref(), &f, p.st.you, p.ear);
        match (&f.you, &p.island) {
            (Some(own), Some(i)) => {
                if !p.alive {
                    p.pred.reset(own.body);
                    p.prev = own.body;
                    p.alive = true;
                } else {
                    p.pred.confirm(own.body, own.seq, &i.map);
                }
            }
            // Out: every finger and toggle let go for the next life, and
            // the spellbook shut (there is no book of yours to show).
            _ => {
                if p.alive {
                    p.pad.reset();
                    p.book = false;
                }
                p.alive = false;
            }
        }
        p.st.take(f, now);
    }
}

/// The room speaks another protocol than this page. Newer: this page is
/// old, so the new one is loaded (once; if that brings this same page
/// back, the new pages are not out here yet, and the title says to try
/// again soon). Older: the room is behind the page (its deploy on the
/// way), so back to the title to say so; reloading would not help.
fn mismatch(p: &mut Page, v: u8, now: f64) {
    if v > PROTO && reload() {
        return;
    }
    leave(p);
    p.notice = Some(if v > PROTO {
        "a new Wandfall is on its way: try again in a minute"
    } else {
        "the island is being updated: try again in a minute"
    });
    // A newer page may be out already: it loads, at the title, if so.
    p.version.poll(now, true);
}
