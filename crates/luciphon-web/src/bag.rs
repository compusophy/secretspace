//! Your gear, in a panel over the world (Tab or I; the phone's bag
//! button): what you wear and what it adds up to, what you carry (tap a
//! piece to wear or drop it), your materials, what you can craft (by the
//! Luciphon or your hearth; tap to make it) and your skills.

use lucilook::palette::{DIM, GOLD, INK};
use luciphon::gear::{get, worn};
use luciphon::laws::GEAR;
use luciphon::proto::Own;
use pixels::{fit_scale, text_width, Canvas, Rect, Rgba};

pub const SKILLS: [&str; 7] = [
    "hewing",
    "delving",
    "kindling",
    "tending",
    "valor",
    "wayfaring",
    "voice",
];
const SLOTS: [&str; 3] = ["wand", "robe", "charm"];

/// What a press on the panel means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// A piece in the bag (its place).
    Item(usize),
    Wear,
    Drop,
    /// Craft this gear.
    Craft(u8),
    Close,
}

/// A rarity's colour.
pub fn rarity(r: u8) -> Rgba {
    match r {
        1 => Rgba::rgb(127, 224, 255),
        2 => Rgba::rgb(200, 150, 255),
        3 => GOLD,
        _ => INK,
    }
}

/// What a piece (or everything worn) adds, in a few words.
pub fn stats(w: &luciphon::gear::Worn) -> String {
    let mut out = Vec::new();
    if w.bolt != 0 {
        out.push(format!("+{} damage", w.bolt / 1000));
    }
    if w.reach != 0 {
        out.push(format!("+{} reach", w.reach / 1000));
    }
    if w.haste != 0 {
        out.push(format!("{} faster", w.haste));
    }
    if w.armor != 0 {
        out.push(format!("{}% armor", w.armor / 10));
    }
    if w.flame != 0 {
        out.push(format!("+{} flame", w.flame / 1000));
    }
    if w.breath != 0 {
        out.push(format!("+{}% breath", w.breath / 10));
    }
    if out.is_empty() {
        "plain".into()
    } else {
        out.join(" ")
    }
}

fn piece(id: u8) -> luciphon::gear::Worn {
    let mut g = [0u8; 3];
    if let Some(p) = get(id) {
        g[p.slot as usize % 3] = id;
    }
    worn(g)
}

/// Draw the panel; where its presses go.
pub fn draw(
    c: &mut Canvas,
    o: &Own,
    picked: Option<usize>,
    near: bool,
    u: i32,
) -> Vec<(Rect, Pick)> {
    let uf = u as f32;
    let w = (c.w as f32 - 16.0 * uf).min(300.0 * uf);
    let x = (c.w as f32 - w) / 2.0;
    let row = 10.0 * uf;
    let mut spots = Vec::new();
    c.fill_rect(0, 0, c.w, c.h, Rgba(4, 5, 12, 140));
    let top = 8.0 * uf;
    let panel = Rect::new(x - 6.0 * uf, top, w + 12.0 * uf, c.h as f32 - 2.0 * top);
    c.round_rect(panel, 6.0, Rgba(14, 16, 32, 235));
    c.round_rect_line(panel, 6.0, 1.0, Rgba(255, 210, 122, 90));
    let close = Rect::new(x + w - 14.0 * uf, top + 4.0 * uf, 14.0 * uf, 12.0 * uf);
    c.text(close.x as i32 + 3 * u, close.y as i32 + 2 * u, "x", u, DIM);
    spots.push((close, Pick::Close));
    let mut y = top + 6.0 * uf;
    let head = |c: &mut Canvas, y: &mut f32, t: &str| {
        *y += 4.0 * uf;
        c.text(x as i32, *y as i32, t, u, GOLD);
        *y += row + 2.0 * uf;
    };
    let line = |c: &mut Canvas, y: &mut f32, t: &str, col: Rgba| {
        let s = fit_scale(t, w as i32, u);
        c.text(x as i32, *y as i32, t, s, col);
        *y += row;
    };

    head(c, &mut y, "worn");
    for (k, &id) in o.gear.iter().enumerate() {
        let (name, col) = get(id).map_or(("-", DIM), |g| (g.name, rarity(g.rarity)));
        c.text(x as i32, y as i32, SLOTS[k], u, DIM);
        c.text(x as i32 + 40 * u, y as i32, name, u, col);
        y += row;
    }
    line(c, &mut y, &stats(&worn(o.gear)), Rgba(244, 238, 222, 170));

    head(c, &mut y, "carried");
    let half = (w - 4.0 * uf) / 2.0;
    let n = o.bag.iter().filter(|&&g| g != 0).count();
    if n == 0 {
        line(
            c,
            &mut y,
            "nothing yet: craft, or take it from the fallen",
            DIM,
        );
    }
    for (k, &id) in o.bag.iter().enumerate().filter(|(_, &g)| g != 0) {
        let Some(g) = get(id) else { continue };
        let at = k % 2;
        let r = Rect::new(x + at as f32 * (half + 4.0 * uf), y, half, row + 2.0 * uf);
        let on = picked == Some(k);
        c.round_rect(r, 3.0, Rgba(255, 255, 255, if on { 50 } else { 14 }));
        let s = fit_scale(g.name, (half - 4.0 * uf) as i32, u);
        c.text(
            r.x as i32 + 2 * u,
            r.y as i32 + u,
            g.name,
            s,
            rarity(g.rarity),
        );
        spots.push((r, Pick::Item(k)));
        if at == 1 || k + 1 == o.bag.len() || o.bag[k + 1..].iter().all(|&g| g == 0) {
            y += row + 4.0 * uf;
        }
    }
    if let Some(g) = picked.and_then(|k| o.bag.get(k)).and_then(|&id| get(id)) {
        line(
            c,
            &mut y,
            &stats(&piece(o.bag[picked.unwrap_or(0)])),
            Rgba(244, 238, 222, 190),
        );
        let wear = Rect::new(x, y, half, row + 4.0 * uf);
        let drop = Rect::new(x + half + 4.0 * uf, y, half, row + 4.0 * uf);
        for (r, t, p) in [(wear, "wear", Pick::Wear), (drop, "drop", Pick::Drop)] {
            c.round_rect(
                r,
                3.0,
                if p == Pick::Wear {
                    GOLD
                } else {
                    Rgba(255, 255, 255, 30)
                },
            );
            let ink = if p == Pick::Wear {
                Rgba::rgb(40, 26, 8)
            } else {
                INK
            };
            let tw = text_width(t, u);
            c.text(
                (r.x + r.w / 2.0) as i32 - tw / 2,
                r.y as i32 + 2 * u,
                t,
                u,
                ink,
            );
            spots.push((r, p));
        }
        let _ = g;
        y += row + 6.0 * uf;
    }
    let mats = format!(
        "glim {}   wood {}   stone {}   wheat {}",
        o.glim, o.wood, o.stone, o.wheat
    );
    line(c, &mut y, &mats, Rgba(244, 238, 222, 200));

    head(
        c,
        &mut y,
        if near {
            "craft"
        } else {
            "craft (by the luciphon or your hearth)"
        },
    );
    // Every recipe on one line, all at the size the longest fits.
    let mut recipes = Vec::new();
    for (k, g) in GEAR.iter().enumerate() {
        let Some((wood, stone, glim, skill, level)) = g.craft else {
            continue;
        };
        let have = o.levels[skill as usize % 7];
        let can = near && o.wood >= wood && o.stone >= stone && o.glim >= glim && have >= level;
        let mut cost = Vec::new();
        for (v, n) in [(wood, "w"), (stone, "s"), (glim, "g")] {
            if v > 0 {
                cost.push(format!("{v}{n}"));
            }
        }
        let t = format!(
            "{}  {}  {} {}",
            g.name,
            cost.join(" "),
            &SKILLS[skill as usize % 7][..3],
            level
        );
        recipes.push((k as u8 + 1, t, can));
    }
    let s = recipes
        .iter()
        .map(|r| fit_scale(&r.1, w as i32 - 4 * u, u))
        .min()
        .unwrap_or(u);
    let tall = (10 * s) as f32 + 2.0 * uf;
    for (id, t, can) in recipes {
        let r = Rect::new(x, y, w, tall);
        if can {
            c.round_rect(r, 3.0, Rgba(255, 210, 122, 40));
            spots.push((r, Pick::Craft(id)));
        }
        let col = if can { GOLD } else { Rgba(244, 238, 222, 110) };
        c.text(x as i32 + 2 * u, y as i32 + u, &t, s, col);
        y += tall;
    }

    head(c, &mut y, "skills");
    let mut skills = String::new();
    for (k, name) in SKILLS.iter().enumerate() {
        skills.push_str(&format!("{} {}  ", &name[..3], o.levels[k]));
    }
    line(c, &mut y, skills.trim_end(), Rgba(244, 238, 222, 200));
    spots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_fits_a_phone_and_offers_what_it_can() {
        let mut c = Canvas::new(390, 844);
        let mut o = Own {
            gear: luciphon::laws::STARTING_GEAR,
            wood: 100,
            glim: 50,
            ..Own::default()
        };
        o.levels = [5; 7];
        o.bag[0] = 2;
        let spots = draw(&mut c, &o, Some(0), true, 2);
        assert!(spots.iter().any(|s| s.1 == Pick::Craft(2)), "an oak wand");
        assert!(spots.iter().any(|s| s.1 == Pick::Wear));
        assert!(spots.iter().any(|s| s.1 == Pick::Item(0)));
        assert!(
            spots.iter().all(|s| s.0.y + s.0.h <= c.h as f32),
            "on screen"
        );
        // Far from a light, nothing to craft.
        let spots = draw(&mut c, &o, None, false, 2);
        assert!(!spots.iter().any(|s| matches!(s.1, Pick::Craft(_))));
        assert_eq!(stats(&piece(2)), "+2 damage");
    }
}
