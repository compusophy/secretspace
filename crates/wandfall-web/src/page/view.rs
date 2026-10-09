//! A frame: where you see from, the match drawn, the HUD over it.

use super::*;

pub(super) fn frame(p: &mut Page, now: f64) {
    let dt = (now - p.last).clamp(0.0, 250.0);
    p.last = now;
    if dt > 0.0 {
        p.fps += (1000.0 / dt - p.fps) * 0.05;
    }
    pace(p, dt);
    net(p, now, dt);
    hands(p);
    inputs(p, dt);
    p.version.poll(now, false);
    if p.version.newer() && (!p.alive || p.st.frame.as_ref().is_some_and(|f| f.phase != 1)) {
        kit::version::reload();
    }
    let mut others = p.st.others(now);
    let (w, h) = p.g.css;
    let aspect = (w / h.max(1.0)) as f32;
    // Your feet, between the last tick predicted and this one.
    let k = (p.acc / MS_A_TICK) as f32;
    let (a, b) = (p.prev.p, p.pred.body.p);
    let feet = [
        a[0] + (b[0] - a[0]) * k,
        a[1] + (b[1] - a[1]) * k,
        a[2] + (b[2] - a[2]) * k,
    ];
    // Your own wizard as you predict it, not as the room last said (a
    // tenth of a second behind).
    if p.alive {
        if let Some(me) = others.iter_mut().find(|s| s.id == p.st.you) {
            let body = &p.pred.body;
            me.p = feet;
            me.yaw = trig::heading(p.aim.0);
            me.pitch = trig::pitch(p.aim.1);
            let set = |f: &mut u8, bit: u8, on: bool| {
                *f = if on { *f | bit } else { *f & !bit };
            };
            set(&mut me.flags, flag::GROUND, body.ground);
            set(&mut me.flags, flag::GLIDE, body.glide);
            set(&mut me.flags, flag::CROUCH, body.crouch);
            set(&mut me.flags, flag::SPRINT, body.sprint);
            set(&mut me.flags, flag::SLIDE, body.slide);
            set(&mut me.fx, proto::fx::TETHER, body.tether > 0);
        }
    }
    let (cam, watching, orbit) = camera(p, &others, feet, now, dt, aspect);
    let t = (now / 1000.0) as f32;
    let mut d = Draw::default();
    let mut in_storm = false;
    if let Some(i) = &p.island {
        let eyes = scene::Eyes {
            id: p.st.you,
            at: feet,
            alive: p.alive,
            first: false,
            tip: None,
        };
        let show = scene::Show {
            hold: p.hold,
            storm: !matches!(p.mode, Mode::Practice(_)),
        };
        in_storm = scene::draw(
            &i.look,
            &mut d,
            &mut p.st,
            &mut p.anims,
            &others,
            &eyes,
            &cam,
            (now, dt),
            show,
        );
    }
    if let (Some((name, age)), Some(i)) = (&p.gallery, &p.island) {
        let age = age.unwrap_or(now % 2500.0);
        let right = [-cam.yaw.sin(), 0.0, cam.yaw.cos()];
        fx::gallery(&i.look, &mut d, name, (age, now), feet, right);
    }
    p.ear = (cam.eye, cam.yaw);
    if let Some(i) = &p.island {
        let you = p.alive.then_some(p.st.you);
        p.sounds
            .steps
            .hear(&p.sounds.audio, &i.map, (&others, &p.anims), you, p.ear);
    }
    // The island's sound under everything, for where you are.
    if let Some(i) = &p.island {
        let at = |place: wandfall::places::Place, up: f32| {
            i.map
                .pois
                .iter()
                .find(|q| q.place == place)
                .map(|q| [q.x, q.level + up, q.z])
        };
        let b = &p.pred.body;
        let e = cam.eye;
        let storm = match (&p.st.frame, &p.mode) {
            (Some(f), Mode::Online(_)) if f.phase == 1 => Some(f.storm),
            _ => None,
        };
        let here = crate::ambience::Here {
            ear: p.ear,
            over: e[1] - i.map.height(e[0], e[2]),
            glide: p.alive && b.glide,
            speed: if p.alive { b.v[0].hypot(b.v[2]) } else { 0.0 },
            rift: at(wandfall::places::Place::Rift, -2.0),
            spire: at(wandfall::places::Place::Spire, 12.0),
            storm,
            // No crickets nor birds in the rain: the rain instead.
            night: p.sky.weigh(now, |h, w| match (h, w) {
                (_, Weather::Rain) => 0.0,
                (Hour::Night, _) => 1.0,
                (Hour::Dusk, _) => 0.35,
                _ => 0.0,
            }),
            birds: p.sky.weigh(now, |h, w| match (h, w) {
                (_, Weather::Rain) => 0.0,
                (Hour::Dawn, _) => 1.0,
                (Hour::Day, _) => 0.5,
                _ => 0.0,
            }),
            rain: p.sky.weigh(now, |_, w| (w == Weather::Rain) as i32 as f32),
        };
        p.sounds.ambience.tune(&p.sounds.audio, &here, false);
    }
    // Music under the title, the lobby and the result; none in a match.
    let music = match (&p.mode, p.st.frame.as_ref().map(|f| f.phase)) {
        (Mode::Title, _) => 0.32,
        (Mode::Online(_), Some(0) | Some(2)) => 0.2,
        _ => 0.0,
    };
    p.sounds.music.tune(&p.sounds.audio, music);
    // What the crosshair is on: you aim there from your own eyes (the
    // camera's place over your shoulder taken out); where Lightning would
    // strike, aiming with it ready.
    let mut on_target = false;
    if let (Some(i), true) = (&p.island, p.alive && orbit.is_none()) {
        let eye = [feet[0], feet[1] + p.pred.body.eye(), feet[2]];
        let (at, who) = camera::crosshair(&i.map, &others, p.st.you, &cam, eye, 400.0);
        p.aim = camera::toward(eye, at, &cam);
        let d2 = render::geo::sub(at, eye);
        on_target = who.is_some() && render::geo::dot(d2, d2) <= LANCE_RANGE * LANCE_RANGE;
        let own = p.st.frame.as_ref().and_then(|f| f.you.as_ref());
        let ready = own.is_some_and(|o| {
            (0..4).any(|k| o.slots[k].is_some_and(|s| s.0 == spell::LIGHTNING) && o.cds[k] == 0)
        });
        if p.aiming && ready {
            let look = trig::look(trig::heading(p.aim.0), trig::pitch(p.aim.1));
            let (at, _) = fx::sight(&i.map, &others, p.st.you, (eye, look), LIGHTNING_RANGE);
            let ground = [
                at[0],
                i.map.floor(at[0], at[2], at[1] + 0.3).max(SEA),
                at[2],
            ];
            fx::aim_ring(&i.look, &mut d, ground, t);
        }
    }
    // The island's hour, from the room (the range: dusk), or `?hour=`.
    let hour = p
        .hour
        .unwrap_or_else(|| Hour::from(p.st.frame.as_ref().map_or(RANGE_HOUR, |f| f.hour)));
    // And its weather, the same way (`?weather=`).
    let weather = p
        .weather
        .unwrap_or_else(|| Weather::from(p.st.frame.as_ref().map_or(0, |f| f.weather)));
    let look = p.sky.look((hour, weather), in_storm, now);
    let wet = p.sky.weigh(now, |_, w| (w == Weather::Rain) as i32 as f32);
    fx::rain(&mut d, cam.eye, t, wet, look.wind);
    // Lightning far off: thunder a while after (the farther, the later).
    if let Some(at) = sky::lightning(now, wet) {
        if p.thunder.0 != at {
            let far = 1500.0 + (at.fract() * 2000.0);
            p.thunder = (at, Some((now + far, (at * 7.3).sin() as f32)));
        }
    }
    if let Some((_, pan)) = p.thunder.1.filter(|t| now >= t.0) {
        p.sounds.ambience.thunder(&p.sounds.audio, pan);
        p.thunder.1 = None;
    }
    let scene = Frame {
        cam,
        look,
        time: t,
        items: &d.items,
        lights: &d.lights,
        sparks: &d.sparks,
        decals: &d.decals,
        view_fov: 0.9,
    };
    let perf = p.perf.then(|| {
        let s = p.r.stats;
        format!(
            "{:.0} fps  draws {} inst {} tris {}k shadow {:?}k lights {}  @{:.0},{:.0}{}",
            p.fps,
            s.draws,
            s.instances,
            s.triangles / 1000,
            s.shadow.map(|t| t / 1000),
            s.lights,
            cam.eye[0],
            cam.eye[2],
            if p.touch {
                format!(" {}", p.pad.debug())
            } else {
                String::new()
            }
        )
    });
    let Some(mut fr) = p.g.frame() else {
        return;
    };
    p.r.draw(&mut fr.encoder, &fr.view, p.g.size, &scene);
    let (vp, _, _) = cam.matrices(cam.fov);
    let ui = p.g.ui();
    let me = p.alive.then_some((feet, p.yaw));
    p.g.hud.wipe();
    p.spots = Spots::default();
    let practice = matches!(p.mode, Mode::Practice(_));
    match (&p.island, &p.mode) {
        (Some(_), Mode::Title) => {
            let note = "the island is drawn by the secretspace engine, on WebGPU";
            menu::title(&mut p.g.hud, &mut p.spots, ui, note);
        }
        (Some(i), _) => {
            let view = hud::View {
                st: &p.st,
                frame: p.st.frame.as_ref(),
                others: &others,
                vp,
                me,
                own: p.st.frame.as_ref().and_then(|f| f.you.as_ref()),
                watching,
                locked: kit::input::locked(),
                in_storm,
                now,
                ui,
                perf: perf.clone(),
                touch: p.touch,
                practice,
                on_target,
                book: p.book,
            };
            if p.orbit.is_none() {
                hud::draw(&mut p.g.hud, &i.mini, &view);
            }
            // The range's lessons, while you play.
            p.lesson_panel = None;
            if practice && p.alive && p.orbit.is_none() {
                let own = p.st.frame.as_ref().and_then(|f| f.you.as_ref());
                let you = p.st.you;
                let cast_at =
                    p.st.shows
                        .iter()
                        .filter_map(|&(when, e)| match e {
                            proto::Ev::Cast { by, stage: 0, .. } if by == you => Some(when),
                            _ => None,
                        })
                        .fold(0.0, f64::max);
                p.lessons.frame(&lessons::Watch {
                    now,
                    at: feet,
                    own,
                    hit_at: p.st.hit_at,
                    cast_at,
                    book: p.book,
                });
                let menu = p.book || p.paused || (!p.touch && !kit::input::locked());
                if !menu {
                    p.lesson_panel = p.lessons.draw(&mut p.g.hud, ui, p.touch, now);
                }
            }
            let own = p.st.frame.as_ref().and_then(|f| f.you.as_ref());
            if p.touch && p.alive && !p.book && !p.paused {
                p.pad.draw(&mut p.g.hud, own, p.g.css, p.g.scale);
            }
            // The online lobby: who is waiting.
            if !practice && p.st.frame.as_ref().is_some_and(|f| f.phase == 0) {
                let names: Vec<String> =
                    p.st.names
                        .values()
                        .filter(|n| !n.1)
                        .map(|n| n.0.clone())
                        .collect();
                menu::lobby(&mut p.g.hud, ui, &names, &p.st.hall);
            }
            if p.book {
                // The range's tools (ranks, levels, rules) only there.
                let rules = match &mut p.mode {
                    Mode::Practice(room) => room
                        .world()
                        .practice
                        .as_ref()
                        .map(|r| (r.no_cooldowns, r.sparring)),
                    _ => None,
                };
                if let Some(o) = own {
                    menu::book(&mut p.g.hud, &mut p.spots, ui, o, p.book_slot, rules);
                }
            } else if (p.paused || (!p.touch && !kit::input::locked())) && p.orbit.is_none() {
                menu::pause(&mut p.g.hud, &mut p.spots, ui, (practice, p.touch), &p.set);
            }
        }
        (None, _) => {
            let c = &mut p.g.hud;
            c.text_centred(
                c.w / 2,
                c.h / 2,
                "finding the island...",
                2 * ui,
                pixels::Rgba::rgb(250, 246, 236),
            );
        }
    }
    p.g.present(fr);
    if let Some(line) = perf {
        if (now as u64 / 500).is_multiple_of(2) {
            kit::document().set_title(&line);
        }
    }
}

/// Where you see from: held (`?cam`), turning about a wizard (`?orbit`),
/// over your own shoulder, or over the shoulder of someone still in the
/// match when you are out (and their name). Also where `?orbit` is.
fn camera(
    p: &mut Page,
    others: &[proto::Seen],
    feet: [f32; 3],
    now: f64,
    dt: f64,
    aspect: f32,
) -> (Camera, Option<String>, Option<[f32; 3]>) {
    // `?orbit=ID`: turn slowly about a wizard (0: yourself), to look at it.
    let orbit = p.orbit.and_then(|id| {
        let id = if id == 0 { p.st.you } else { id };
        let at = others.iter().find(|s| s.id == id).map(|s| s.p);
        // Still where it was when it falls.
        p.orbit_at = at.or(p.orbit_at);
        p.orbit_at
    });
    let secs = dt as f32 / 1000.0;
    if let Some(c) = range::fixed() {
        let (eye, yaw, pitch) = ([c[0], c[1], c[2]], c[3], c[4]);
        let fov = camera::FOV;
        return (
            Camera {
                eye,
                yaw,
                pitch,
                fov,
                aspect,
            },
            None,
            orbit,
        );
    }
    if let Some(o) = orbit {
        // `&close`: near, over the head (`&close=degrees`: held there,
        // 0 in front of it).
        let held = query_value("close").and_then(|v| v.parse::<f32>().ok());
        let (mut r, mut up, mut pitch) = if query("close") || held.is_some() {
            (1.7, 1.9, -0.22)
        } else {
            (3.6, 1.2, -0.08)
        };
        // `&lift=metres&dist=metres`: from there, looking level.
        if let Some(l) = query_value("lift").and_then(|v| v.parse::<f32>().ok()) {
            (up, pitch) = (l, 0.0);
        }
        if let Some(d) = query_value("dist").and_then(|v| v.parse::<f32>().ok()) {
            r = d;
        }
        let a = held.map_or((now / 5000.0) as f32, f32::to_radians);
        let cam = Camera {
            eye: [o[0] + a.cos() * r, o[1] + up, o[2] + a.sin() * r],
            yaw: a + std::f32::consts::PI,
            pitch,
            fov: camera::FOV,
            aspect,
        };
        return (cam, None, orbit);
    }
    let Some(i) = &p.island else {
        return (Camera::default(), None, None);
    };
    if p.alive {
        let b = &p.pred.body;
        let stance = camera::Stance {
            aiming: p.aiming,
            crouch: b.crouch,
            glide: b.glide,
            fast: b.sprint || b.slide,
        };
        let mut cam = p
            .chase
            .view(&i.map, feet, (p.yaw, p.pitch), stance, aspect, secs);
        // A shake when you are hurt.
        let hurt = (1.0 - (now - p.st.hurt_at) / 220.0).max(0.0) as f32;
        cam.yaw += (now as f32 * 0.09).sin() * 0.012 * hurt;
        cam.pitch += (now as f32 * 0.13).sin() * 0.012 * hurt;
        // And when something bursts near you.
        camera::shake(&mut cam, camera::rumble(&p.st.shows, now, feet), now);
        return (cam, None, None);
    }
    // Out: over the shoulder of whoever took you, then of whoever is
    // still in it (space or the arrows, or a tap: the next one).
    let mut list: Vec<&proto::Seen> = others
        .iter()
        .filter(|s| s.flags & flag::ENTRANT != 0)
        .collect();
    list.sort_by_key(|s| s.id);
    let step = std::mem::take(&mut p.cycle);
    let n = list.len() as i32;
    let on = p.watch.and_then(|id| list.iter().position(|s| s.id == id));
    let winner = p.st.frame.as_ref().map_or(0, |f| f.winner);
    let first = || {
        let by = p.st.out.map_or(0, |o| o.0);
        [by, winner]
            .into_iter()
            .filter(|&id| id != 0)
            .find_map(|id| list.iter().position(|s| s.id == id))
            .or((n > 0).then_some(0))
    };
    let at = match on {
        Some(k) => Some((k as i32 + step).rem_euclid(n.max(1)) as usize),
        None => first(),
    };
    let pick = at.map(|k| list[k]).or(others.first());
    p.watch = pick.map(|s| s.id);
    match pick {
        Some(s) => {
            let yaw = trig::radians(s.yaw) + 0.4 * ((now / 9000.0) as f32).sin();
            let stance = camera::Stance {
                glide: s.flags & flag::GLIDE != 0,
                ..camera::Stance::default()
            };
            let cam = p
                .chase
                .view(&i.map, s.p, (yaw, -0.25), stance, aspect, secs);
            let name = p.st.name(s.id);
            let label = match (n > 1, p.touch) {
                (false, _) => name,
                (true, false) => format!("{name} - space: the next one"),
                (true, true) => format!("{name} - tap: the next one"),
            };
            (cam, Some(label), None)
        }
        None => {
            let a = (now / 20000.0) as f32;
            let cam = Camera {
                eye: [a.cos() * 120.0, 45.0, a.sin() * 120.0],
                yaw: a + std::f32::consts::PI,
                pitch: -0.25,
                fov: camera::FOV,
                aspect,
            };
            (cam, None, None)
        }
    }
}
