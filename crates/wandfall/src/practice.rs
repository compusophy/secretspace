//! The practice range: a world a page runs for itself. No storm and no
//! match: you start in the open by a ruin, cubes to learn by ahead of
//! you and training dummies about you, each in plain sight (some stand,
//! some strafe about their posts, two spar when you want them to); you
//! are hurt only when sparring, dummies neither loot nor level, the
//! knocked out stand again, cubes are set out again now and then, and
//! the spellbook's tools change your spells, your level and the rules.

use crate::bots::Mind;
use crate::laws::*;
use crate::loot;
use crate::map::Kind as PropKind;
use crate::motion::{Body, Input};
use crate::trig;
use crate::world::{warmup, Event, Phase, World};

#[derive(Clone, Debug, Default)]
pub struct Practice {
    /// Spells are always ready.
    pub no_cooldowns: bool,
    /// The sparring dummies fight back, and you can be hurt.
    pub sparring: bool,
    /// Where you start.
    pub spawn: [f32; 2],
    /// Who stands again, and when.
    respawns: Vec<(u16, u32)>,
    loot_at: u32,
}

/// Make `w` a practice range.
pub fn setup(w: &mut World) {
    let spawn = start(w);
    w.hour = RANGE_HOUR;
    w.practice = Some(Practice {
        spawn,
        loot_at: w.tick + PRACTICE_LOOT_EVERY,
        ..Practice::default()
    });
    w.phase = Phase::Fight;
    w.began = w.tick;
    for (k, &(d, deg, mode)) in DUMMIES.iter().enumerate() {
        let at = post(w, spawn, d, deg);
        let mut b = w.player(&format!("dummy {}", k + 1), 0, true);
        b.body.p = [at[0], w.map.height(at[0], at[1]), at[1]];
        // Facing where you start.
        b.yaw = trig::heading((spawn[1] - at[1]).atan2(spawn[0] - at[0]));
        b.mind = Mind::new(w.rng.next_u64());
        b.mind.dummy = mode;
        b.mind.home = at;
        b.alive = true;
        b.entrant = true;
        // A sparring dummy fights with spells, as you do.
        if mode == 3 {
            warmup(&mut b, &mut w.rng);
        }
        w.players.push(b);
    }
    loot::scatter(w);
    lesson_cubes(w, spawn);
    w.roster_dirty = true;
}

/// A free spot near `at`, on land, nothing standing within `r`.
fn clear(w: &World, at: [f32; 2], r: f32) -> Option<[f32; 2]> {
    (0..40).find_map(|k| {
        let (a, d) = (k as f32 * 2.4, k as f32 * 0.6);
        let p = [at[0] + a.cos() * d, at[1] + a.sin() * d];
        (w.map.land(p[0], p[1]) && w.map.near(p[0], p[1], r).next().is_none()).then_some(p)
    })
}

/// Whether one standing at `a` sees the chest of one at `b`.
fn sees(w: &World, a: [f32; 2], b: [f32; 2]) -> bool {
    let eye = [a[0], w.map.height(a[0], a[1]) + EYE, a[1]];
    let to = [b[0], w.map.height(b[0], b[1]) + 1.1, b[1]];
    w.map.strikes(eye, to).is_none()
}

/// Where you start: in the open by the ruin nearest the middle of the
/// island (no cap overhead), the way east (where you first look, and the
/// lesson cubes lie) clear.
fn start(w: &World) -> [f32; 2] {
    let ruin = w
        .map
        .props
        .iter()
        .filter(|p| p.kind == PropKind::Pillar)
        .map(|p| [p.x, p.z])
        .min_by(|a, b| (a[0].hypot(a[1])).total_cmp(&b[0].hypot(b[1])))
        .unwrap_or([0.0, 0.0]);
    // Nothing standing near, no tree's or mushroom's cap overhead.
    let open = |p: [f32; 2]| {
        let capped = |q: &crate::map::Prop| matches!(q.kind, PropKind::Tree | PropKind::Shroom);
        w.map.land(p[0], p[1])
            && w.map.near(p[0], p[1], RANGE_CLEAR).next().is_none()
            && !w.map.near(p[0], p[1], RANGE_CLEAR * 2.0).any(capped)
            && sees(w, p, [p[0] + RANGE_VIEW, p[1]])
    };
    // Ring after ring about the ruin, its own pillars and all.
    (0..12)
        .flat_map(|ring| (0..12).map(move |k| (ring, k)))
        .map(|(ring, k)| {
            let (a, d) = ((k as f32 * 30.0).to_radians(), 5.0 + ring as f32 * 1.5);
            [ruin[0] + a.cos() * d, ruin[1] + a.sin() * d]
        })
        .find(|&p| open(p))
        .or_else(|| clear(w, ruin, 1.2))
        .unwrap_or(ruin)
}

/// A dummy's post: `d` metres from where you start, about `deg` round,
/// turned a little either way till you can see it.
fn post(w: &World, spawn: [f32; 2], d: f32, deg: f32) -> [f32; 2] {
    let at = |turn: f32| {
        let a = (deg + turn).to_radians();
        clear(w, [spawn[0] + a.cos() * d, spawn[1] + a.sin() * d], 1.2)
    };
    [0.0, 10.0, -10.0, 20.0, -20.0, 30.0, -30.0, 40.0, -40.0]
        .into_iter()
        .filter_map(at)
        .find(|&p| sees(w, spawn, p))
        .or_else(|| at(0.0))
        .unwrap_or(spawn)
}

/// A few cubes just ahead of where you start, to learn by (laid again
/// whenever the range's cubes are).
fn lesson_cubes(w: &mut World, spawn: [f32; 2]) {
    let first = [
        spell::FROST,
        spell::WARD,
        spell::LIGHTNING,
        spell::BLINK,
        spell::TETHER,
    ];
    for (k, &sp) in first.iter().enumerate() {
        let a = (k as f32 - 2.0) * 0.35;
        let d = 4.5 + k as f32 * 1.2;
        let near = [spawn[0] + a.cos() * d, spawn[1] + a.sin() * d];
        let at = clear(w, near, 1.2).unwrap_or(near);
        loot::drop_scroll(w, sp, 1, [at[0], 0.0, at[1]], 0.0);
    }
}

/// You arrive: at the start, alive, with a set of spells to try.
pub fn arrive(w: &mut World, name: &str) -> u16 {
    let spawn = w.practice.as_ref().map_or([0.0, 0.0], |p| p.spawn);
    let mut p = w.player(name, 0, false);
    p.body.p = [spawn[0], w.map.height(spawn[0], spawn[1]), spawn[1]];
    p.alive = true;
    p.entrant = true;
    warmup(&mut p, &mut w.rng);
    // On the range every spell is known, and the Tether is always to
    // hand (F), for its lesson.
    p.book = [1; SPELLS.len()];
    if p.slots[2].is_some_and(|s| s.spell == spell::TETHER) {
        p.slots[2] = p.slots[3];
    }
    p.slots[3] = Some(crate::world::Slot {
        spell: spell::TETHER,
        rank: 1,
    });
    let id = p.id;
    w.players.push(p);
    w.roster_dirty = true;
    id
}

/// One tick of the range's own rules.
pub fn step(w: &mut World, ev: &mut Vec<Event>) {
    let tick = w.tick;
    let Some(r) = w.practice.as_mut() else {
        return;
    };
    let due: Vec<u16> = r
        .respawns
        .iter()
        .filter(|x| tick >= x.1)
        .map(|x| x.0)
        .collect();
    r.respawns.retain(|x| tick < x.1);
    let (no_cd, spawn) = (r.no_cooldowns, r.spawn);
    if tick >= r.loot_at {
        r.loot_at = tick + PRACTICE_LOOT_EVERY;
        if tick > 0 {
            loot::scatter(w);
            lesson_cubes(w, spawn);
        }
    }
    for p in w.players.iter_mut() {
        if due.contains(&p.id) {
            // Up again, afresh: a person where they started, a dummy at
            // its post (nothing of how they fell held over).
            let at = if p.bot { p.mind.home } else { spawn };
            p.body = Body {
                p: [at[0], w.map.height(at[0], at[1]), at[1]],
                ground: true,
                ..Body::default()
            };
            p.alive = true;
            p.hp = p.max_hp();
            (p.shield, p.shield_until, p.mend, p.mend_until) = (0, 0, 0, 0);
            p.queue.clear();
            p.last = Input {
                seq: p.last.seq,
                ..Input::default()
            };
            // Your knocked-out card goes (not at a dummy's standing up).
            if !p.bot {
                ev.push(Event::Lobby);
            }
        }
        if p.bot && p.alive && tick.saturating_sub(p.hurt_at) >= DUMMY_WHOLE {
            p.hp = p.max_hp();
        }
        if !p.bot && no_cd {
            p.cds = [0; 4];
            p.spell_cds = [0; SPELLS.len()];
            p.cool = 0;
        }
    }
}

/// Someone on the range is knocked out: they stand again soon.
pub fn fallen(w: &mut World, who: u16, by: u16, ev: &mut Vec<Event>) {
    let tick = w.tick;
    let mut level = 1;
    if let Some(p) = w.find_mut(who) {
        p.alive = false;
        p.hp = 0;
        level = p.level;
    }
    if let Some(r) = w.practice.as_mut() {
        r.respawns.push((who, tick + DUMMY_RESPAWN));
    }
    // Your knockouts count; a sparring dummy's do not (nor does it level).
    if let Some(k) = w.find_mut(by).filter(|k| !k.bot) {
        k.kills += 1;
    }
    loot::gain(w, by, XP_KNOCKOUT + level as u32 * XP_KNOCKOUT_LEVEL, ev);
    ev.push(Event::Out { who, by, place: 0 });
}

/// The range's spellbook: every spell is known, at any rank; put
/// `spell` at `rank` in `slot` (if it is of that slot's kind), at once.
pub fn equip(w: &mut World, id: u16, slot: usize, spell: u8, rank: u8) -> bool {
    let i = spell as usize;
    let Some(p) = w.find_mut(id).filter(|_| i < SPELLS.len()) else {
        return false;
    };
    p.book[i] = rank.clamp(1, MAX_RANK);
    for s in p.slots.iter_mut().flatten().filter(|s| s.spell == spell) {
        s.rank = p.book[i];
    }
    let ok = loot::equip(p, slot, spell);
    if ok {
        p.cds[slot] = 0;
        p.spell_cds[i] = 0;
    }
    ok
}

/// The spellbook: your level.
pub fn set_level(w: &mut World, id: u16, level: u8) {
    if let Some(p) = w.find_mut(id) {
        p.level = level.clamp(1, MAX_LEVEL);
        p.xp = 0;
        p.hp = p.max_hp();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::{keys, Input};
    use crate::world::{Slot, WAND};

    #[test]
    fn dummies_fall_and_stand_again_and_you_are_safe() {
        let mut w = World::new(5);
        setup(&mut w);
        let me = w.join("me", 0);
        assert_eq!(w.players.iter().filter(|p| p.bot).count(), DUMMIES.len());
        let d = w.players.iter().find(|p| p.bot).unwrap().id;
        let mut ev = Vec::new();
        w.hurt(me, d, 10_000, WAND, &mut ev);
        assert!(!w.find(d).unwrap().alive, "knocked out");
        assert_eq!(w.find(me).unwrap().kills, 1);
        for _ in 0..DUMMY_RESPAWN + 2 {
            w.step();
        }
        assert!(w.find(d).unwrap().alive, "and up again");
        let full = w.find(me).unwrap().hp;
        assert!(
            w.find(me).unwrap().level < 5,
            "XP only for what was really taken"
        );
        w.hurt(d, me, 50, WAND, &mut ev);
        assert_eq!(w.find(me).unwrap().hp, full, "not hurt when not sparring");
        w.practice.as_mut().unwrap().sparring = true;
        w.hurt(d, me, 50, WAND, &mut ev);
        assert!(w.find(me).unwrap().hp < full, "hurt when sparring");
        // No storm, ever.
        for _ in 0..TICK_HZ * 300 {
            w.input(
                me,
                Input {
                    keys: keys::FWD,
                    ..Input::default()
                },
            );
            w.step();
        }
        assert!(w.find(me).unwrap().alive);
    }

    #[test]
    fn a_lance_on_the_range_strikes_a_dummy() {
        let mut w = World::new(0x5eed_0007);
        setup(&mut w);
        let me = w.join("me", 0);
        assert!(equip(&mut w, me, 0, spell::LANCE, 1));
        // Face the nearest dummy, level.
        let k = w.players.iter().position(|p| p.id == me).unwrap();
        let eye = w.players[k].eye();
        // The nearest in plain sight.
        let (d, at) = w
            .players
            .iter()
            .filter(|p| p.bot)
            .map(|p| (p.id, p.body.p))
            .filter(|(_, at)| {
                let to = [at[0], at[1] + 1.0, at[2]];
                w.map.strikes(eye, to).is_none()
            })
            .min_by(|a, b| {
                let da = (a.1[0] - eye[0]).powi(2) + (a.1[2] - eye[2]).powi(2);
                let db = (b.1[0] - eye[0]).powi(2) + (b.1[2] - eye[2]).powi(2);
                da.total_cmp(&db)
            })
            .unwrap();
        let yaw = crate::trig::heading((at[2] - eye[2]).atan2(at[0] - eye[0]));
        let dy = at[1] + 1.0 - eye[1];
        let flat = ((at[0] - eye[0]).powi(2) + (at[2] - eye[2]).powi(2)).sqrt();
        let pitch = crate::trig::pitch(dy.atan2(flat));
        let mut hits = 0;
        for _ in 0..3 {
            w.input(
                me,
                Input {
                    yaw,
                    pitch,
                    cast: crate::motion::cast::SLOT[0],
                    ..Input::default()
                },
            );
            for e in w.step() {
                if let Event::Hit { to, .. } = e {
                    if to == d {
                        hits += 1;
                    }
                }
            }
            w.find_mut(me).unwrap().cds = [0; 4];
        }
        assert_eq!(hits, 3, "every lance strikes");
    }

    #[test]
    fn you_start_in_the_open_with_every_dummy_in_sight() {
        let mut w = World::new(0x5eed_0007);
        setup(&mut w);
        let spawn = w.practice.as_ref().unwrap().spawn;
        assert!(w.map.near(spawn[0], spawn[1], RANGE_CLEAR).next().is_none());
        for p in w.players.iter().filter(|p| p.bot) {
            let at = [p.body.p[0], p.body.p[2]];
            assert!(sees(&w, spawn, at), "{} hidden at {at:?}", p.name);
        }
    }

    #[test]
    fn the_lesson_cubes_come_back_and_dummies_leave_the_cubes() {
        let mut w = World::new(5);
        setup(&mut w);
        w.practice.as_mut().unwrap().sparring = true;
        let me = w.join("me", 0);
        let spawn = w.practice.as_ref().unwrap().spawn;
        let lessons = |w: &World| {
            w.scrolls
                .iter()
                .filter(|s| (s.p[0] - spawn[0]).hypot(s.p[2] - spawn[1]) < 11.0)
                .count()
        };
        let laid = lessons(&w);
        assert!(laid >= 5);
        // Out of the way, unhurt, as the range sets its cubes out again.
        let k = w.players.iter().position(|p| p.id == me).unwrap();
        w.players[k].body.p[0] += 100.0;
        let cubes = w.scrolls.len();
        for _ in 0..PRACTICE_LOOT_EVERY + 1 {
            w.players[k].hp = w.players[k].max_hp();
            w.step();
        }
        assert!(lessons(&w) >= 5, "laid again");
        assert!(w.scrolls.len() >= cubes, "none taken by a dummy");
        assert!(
            w.players.iter().filter(|p| p.bot).all(|p| p.level == 1),
            "nor did one level"
        );
    }

    #[test]
    fn standing_again_starts_afresh() {
        let mut w = World::new(5);
        setup(&mut w);
        w.practice.as_mut().unwrap().sparring = true;
        let me = w.join("me", 0);
        let d = w.players.iter().find(|p| p.bot).unwrap().id;
        let k = w.players.iter().position(|p| p.id == me).unwrap();
        // Knocked out mid-Tether.
        w.players[k].body.tether = 40;
        w.players[k].body.anchor = [0.0, 50.0, 0.0];
        let mut ev = Vec::new();
        w.hurt(d, me, 10_000, WAND, &mut ev);
        assert_eq!(
            w.find(d).unwrap().kills,
            0,
            "a dummy's knockout is not counted"
        );
        let mut lobbies = 0;
        for _ in 0..DUMMY_RESPAWN + 2 {
            lobbies += w.step().iter().filter(|e| **e == Event::Lobby).count();
        }
        let p = w.find(me).unwrap();
        assert!(p.alive && p.body.tether == 0, "{:?}", p.body);
        assert_eq!(lobbies, 1, "your card goes, once");
        // A dummy standing up says nothing.
        w.hurt(me, d, 10_000, WAND, &mut ev);
        let mut lobbies = 0;
        for _ in 0..DUMMY_RESPAWN + 2 {
            lobbies += w.step().iter().filter(|e| **e == Event::Lobby).count();
        }
        assert!(w.find(d).unwrap().alive);
        assert_eq!(lobbies, 0);
    }

    #[test]
    fn the_spellbook_equips_and_levels() {
        let mut w = World::new(5);
        setup(&mut w);
        let me = w.join("me", 0);
        assert!(equip(&mut w, me, 0, spell::FIREBALL, 3));
        assert!(
            !equip(&mut w, me, 0, spell::WARD, 1),
            "a utility spell in an offensive slot"
        );
        assert!(equip(&mut w, me, 1, spell::FIREBALL, 2));
        let p = w.find(me).unwrap();
        assert_eq!(
            p.slots[1],
            Some(Slot {
                spell: spell::FIREBALL,
                rank: 2
            })
        );
        assert!(
            p.slots[0].is_none_or(|s| s.spell != spell::FIREBALL),
            "the same spell moved"
        );
        set_level(&mut w, me, 20);
        assert_eq!(w.find(me).unwrap().hp, loot::max_hp(20));
    }
}
