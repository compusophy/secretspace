//! The practice range: a world a page runs for itself. No storm and no
//! match: you start by a ruin with training dummies about you (some
//! stand, some strafe, two spar when you want them to), you are hurt
//! only when sparring, the knocked out stand again, chests are set out
//! again now and then, and the spellbook's tools change your spells, your
//! level and the rules.

use crate::bots::Mind;
use crate::laws::*;
use crate::loot;
use crate::map::Kind as PropKind;
use crate::world::{Event, Phase, World};

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
    chests_at: u32,
}

/// Make `w` a practice range.
pub fn setup(w: &mut World) {
    // By the ruin nearest the middle of the island.
    let spawn = w
        .map
        .props
        .iter()
        .filter(|p| p.kind == PropKind::Pillar)
        .map(|p| [p.x, p.z])
        .min_by(|a, b| (a[0].hypot(a[1])).total_cmp(&b[0].hypot(b[1])))
        .unwrap_or([0.0, 0.0]);
    let spawn = clear(w, spawn);
    w.practice = Some(Practice {
        spawn,
        chests_at: w.tick + PRACTICE_CHESTS_EVERY,
        ..Practice::default()
    });
    w.phase = Phase::Fight;
    w.began = w.tick;
    for (k, &(d, deg, mode)) in DUMMIES.iter().enumerate() {
        let a = deg.to_radians();
        let at = clear(w, [spawn[0] + a.cos() * d, spawn[1] + a.sin() * d]);
        let mut b = w.player(&format!("dummy {}", k + 1), 0, true);
        b.body.p = [at[0], w.map.height(at[0], at[1]), at[1]];
        // Facing where you start.
        b.yaw = crate::trig::heading((spawn[1] - at[1]).atan2(spawn[0] - at[0]));
        b.mind = Mind::new(w.rng.next_u64());
        b.mind.dummy = mode;
        b.alive = true;
        b.entrant = true;
        w.players.push(b);
    }
    loot::scatter(w);
    // A few cubes just ahead of where you start, to learn by.
    let first = [spell::FROST, spell::WARD, spell::LIGHTNING, spell::BLINK];
    for (k, &sp) in first.iter().enumerate() {
        let a = (k as f32 - 1.5) * 0.35;
        let d = 4.5 + k as f32 * 1.2;
        let at = clear(w, [spawn[0] + a.cos() * d, spawn[1] + a.sin() * d]);
        loot::drop_scroll(w, sp, 1, [at[0], 0.0, at[1]], 0.0);
    }
    w.roster_dirty = true;
}

/// A free spot near `at`, on land.
fn clear(w: &World, at: [f32; 2]) -> [f32; 2] {
    for r in 0..40 {
        let a = r as f32 * 2.4;
        let d = r as f32 * 0.6;
        let p = [at[0] + a.cos() * d, at[1] + a.sin() * d];
        if w.map.land(p[0], p[1]) && w.map.near(p[0], p[1], 1.2).next().is_none() {
            return p;
        }
    }
    at
}

/// You arrive: at the start, alive, with a set of spells to try.
pub fn arrive(w: &mut World, name: &str) -> u16 {
    let spawn = w.practice.as_ref().map_or([0.0, 0.0], |p| p.spawn);
    let mut p = w.player(name, 0, false);
    p.body.p = [spawn[0], w.map.height(spawn[0], spawn[1]), spawn[1]];
    p.alive = true;
    p.entrant = true;
    crate::world::warmup(&mut p, &mut w.rng);
    // On the range every spell is known.
    p.book = [1; SPELLS.len()];
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
    if tick >= r.chests_at {
        r.chests_at = tick + PRACTICE_CHESTS_EVERY;
        if tick > 0 {
            loot::scatter(w);
        }
    }
    for p in w.players.iter_mut() {
        if due.contains(&p.id) {
            p.alive = true;
            p.hp = p.max_hp();
            p.shield = 0;
            p.body.chill = 0;
            if !p.bot {
                p.body.p = [spawn[0], w.map.height(spawn[0], spawn[1]), spawn[1]];
                p.body.v = [0.0; 3];
            }
            ev.push(Event::Lobby);
        }
        if p.bot && p.alive && tick.saturating_sub(p.hurt_at) >= DUMMY_WHOLE {
            p.hp = p.max_hp();
        }
        if !p.bot && no_cd {
            p.cds = [0; 4];
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
    if let Some(k) = w.find_mut(by) {
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
        let (d, at) = w
            .players
            .iter()
            .filter(|p| p.bot)
            .map(|p| (p.id, p.body.p))
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
