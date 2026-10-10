//! What the room says, taken in: the island, who is who, events, loot,
//! frames (your own wizard confirmed against its prediction).

use super::*;

pub(super) fn net(p: &mut Page, now: f64, dt: f64) {
    let mut got = std::mem::take(&mut p.inbox);
    let mut up = false;
    match &mut p.mode {
        Mode::Online(link) => {
            for ev in link.poll(now) {
                match ev {
                    Net::Up => up = true,
                    Net::Holding => {}
                    Net::Message(b) => got.push(b),
                }
            }
        }
        Mode::Practice(room) => {
            p.local = (p.local + dt).min(MS_A_TICK * 6.0);
            while p.local >= MS_A_TICK {
                p.local -= MS_A_TICK;
                let mut out = Outbox::default();
                room.tick(&mut out);
                got.extend(out.0.into_iter().map(|m| m.1));
            }
        }
        Mode::Title => {}
    }
    if up {
        send(p, &Up::Join { proto: PROTO });
    }
    for b in got {
        receive(p, &b, now);
    }
}

fn receive(p: &mut Page, b: &[u8], now: f64) {
    if let Some(seen) = Named::decode(b) {
        if seen.status != Status::Taken && !seen.name.is_empty() {
            p.session.set_name(&seen.name);
            if let Mode::Online(link) = &p.mode {
                link.set_hello(p.session.hello(&seen.name, false));
            }
        }
        return;
    }
    if let Some((v, you, seed, _)) = proto::read_welcome(b) {
        if v != PROTO {
            kit::version::reload();
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
                    // Corrected: drawn on from where it was (`offset`).
                    let was = p.pred.body.p;
                    p.pred.confirm(own.body, own.seq, &i.map);
                    let is = p.pred.body.p;
                    p.prev.p = [0, 1, 2].map(|k| p.prev.p[k] + is[k] - was[k]);
                }
            }
            _ => p.alive = false,
        }
        p.st.take(f, now);
    }
}
