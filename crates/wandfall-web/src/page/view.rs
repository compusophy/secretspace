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
    let scene = Frame {
        cam,
        look: look::sky(in_storm),
        time: t,
        items: &d.items,
        lights: &d.lights,
        sparks: &d.sparks,
        view_fov: 0.9,
    };
    let perf = p.perf.then(|| {
        let s = p.r.stats;
        format!(
            "{:.0} fps  draws {} inst {} tris {}k lights {}  @{:.0},{:.0}{}",
            p.fps,
            s.draws,
            s.instances,
            s.triangles / 1000,
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
                menu::lobby(&mut p.g.hud, ui, &names);
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
                menu::pause(&mut p.g.hud, &mut p.spots, ui, practice, p.touch);
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
        return (cam, None, None);
    }
    // Out: over the shoulder of the winner, or someone still in it.
    let f = p.st.frame.as_ref();
    let pick = f
        .and_then(|f| others.iter().find(|s| s.id == f.winner && f.winner != 0))
        .or_else(|| others.iter().find(|s| s.flags & flag::ENTRANT != 0))
        .or(others.first());
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
            (cam, Some(p.st.name(s.id)), None)
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
