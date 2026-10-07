//! How Luciphon looks: a 3/4 top-down pixel world, north up, centred on
//! you. The ground is baked per chunk; things and Lumens stand in row
//! order with front faces; the Luciphon's light, the sight circle and the
//! Dark beyond it; the feel (`fx`) and the HUD in the world (`hud`). The
//! page and the hub's preview both draw through here.

pub mod fx;
pub mod ground;
pub mod hud;
pub mod palette;
pub mod sprites;

use luciphon::proto::kind;
use luciphon::tiles::{obj, Tiles};
use pixels::{Canvas, Grid, Rgba};

use ground::TILE;
use palette::{DARK, GOLD};
use sprites::*;

/// A body to draw, where the page decided it is now (interpolated, or
/// predicted for your own).
#[derive(Clone, Debug, Default)]
pub struct Body {
    pub id: u16,
    pub kind: u8,
    pub x: f32,
    pub y: f32,
    pub facing: u16,
    /// Movement in the low 3 bits, action in the next 3, ghost, down.
    pub state: u8,
    pub flame: u8,
    pub glim: u8,
    pub hue: u8,
    pub flow: u8,
    pub name: String,
    pub moving: bool,
    pub you: bool,
}

impl Body {
    pub fn mv(&self) -> u8 {
        self.state & 7
    }
    pub fn act(&self) -> u8 {
        (self.state >> 3) & 7
    }
    pub fn ghost(&self) -> bool {
        self.state & 64 != 0
    }
    pub fn down(&self) -> bool {
        self.state & 128 != 0
    }
}

/// Where the camera is and how much of the world shows.
#[derive(Clone, Copy, Debug)]
pub struct View {
    /// The world point at the screen's centre, in tiles.
    pub cx: f32,
    pub cy: f32,
    /// Sight, in tiles.
    pub sight: f32,
    /// Text scale.
    pub u: i32,
}

impl View {
    /// The screen pixel of a world point.
    pub fn to_screen(&self, c: &Canvas, x: f32, y: f32) -> (f32, f32) {
        let left = (self.cx * TILE as f32).round() - (c.w / 2) as f32;
        let top = (self.cy * TILE as f32).round() - (c.h / 2) as f32;
        (x * TILE as f32 - left, y * TILE as f32 - top)
    }
}

#[derive(Default)]
pub struct Look {
    pub ground: ground::Ground,
    pub fx: fx::Effects,
    pub under: hud::Underlight,
}

/// Which grid a facing shows, and whether it is mirrored.
fn facing_grid(h: u16) -> (Grid, bool) {
    match h.wrapping_add(8192) / 16384 {
        0 => (SIDE, false),
        1 => (FRONT, false),
        2 => (SIDE, true),
        _ => (BACK, false),
    }
}

/// Draw one Lumen with its feet at screen (sx, sy).
pub fn draw_lumen(c: &mut Canvas, b: &Body, sx: i32, sy: i32, now: f64) {
    let alpha = if b.ghost() { 120 } else { 255 };
    let flame = b.flame;
    let (g, flip) = facing_grid(b.facing);
    let paint = lumen_paint(b.hue, flame, alpha);
    let (mv, act) = (b.mv(), b.act());
    let down = b.down();
    let (x, y) = (sx - LUMEN_W / 2, sy - LUMEN_H + if down { 4 } else { 0 });
    // A shadow on the ground.
    c.fill_rect(sx - 4, sy - 1, 9, 2, Rgba(0, 0, 10, 80));
    // A dash leaves two after-images.
    if mv == 2 {
        let (ux, uy) = engine::fixed::unit(b.facing);
        for k in 1..=2 {
            let back = 5 * k;
            let ghost = lumen_paint(b.hue, flame, 70 / k as u8);
            let (gx, gy) = (
                x - (ux.to_f32() * back as f32) as i32,
                y - (uy.to_f32() * back as f32) as i32,
            );
            c.grid(&g, gx, gy, 1, flip, &ghost);
        }
    }
    // A hit's stun flashes.
    let stunned = mv == 3 && (now / 60.0) as i64 % 2 == 0;
    if stunned {
        let white = move |ch: u8| (ch != b'.').then_some(Rgba(255, 250, 240, alpha));
        c.grid(&g, x, y, 1, flip, &white);
    } else {
        c.grid(&g, x, y, 1, flip, &paint);
    }
    let stride = if b.moving && mv != 2 {
        1 + ((now / 110.0) as i64 % 2) as usize
    } else {
        0
    };
    c.grid(&FEET[stride], x, y + 14, 1, flip, &paint);
    // Winding up, striking: the fist gathers light toward the facing.
    if matches!(act, 1 | 2) {
        let (ux, uy) = engine::fixed::unit(b.facing);
        let (fx, fy) = (
            sx + (ux.to_f32() * 8.0) as i32,
            sy - 7 + (uy.to_f32() * 6.0) as i32,
        );
        c.glow_add(fx, fy, if act == 2 { 6 } else { 4 }, GOLD);
    }
    // Charging: a ring closes in.
    if act == 4 {
        c.glow_add(sx, sy - 8, 7, Rgba(255, 220, 150, 160));
    }
    // Flow: a halo, brighter each step.
    if b.flow > 0 {
        let a = 60 + 60 * b.flow.min(3) as u32;
        c.fill_rect(sx - 4, y - 3, 8, 1, Rgba(255, 220, 140, a as u8));
        c.fill_rect(sx - 3, y - 4, 6, 1, Rgba(255, 240, 190, (a / 2) as u8));
    }
    // Glim it carries orbits it as motes.
    let motes = [0, 1, 2, 3, 5, 8][b.glim.min(5) as usize];
    for k in 0..motes {
        let a = now / 600.0 + k as f64 * std::f64::consts::TAU / motes as f64;
        let (mx, my) = (sx as f64 + a.cos() * 8.0, (sy - 8) as f64 + a.sin() * 4.0);
        c.add(mx as i32, my as i32, GOLD, 255);
        c.add(mx as i32, my as i32 - 1, GOLD, 90);
    }
}

/// One thing standing on tile (tx, ty): its sprite with its foot on the
/// tile's bottom edge, faded if a Lumen stands just behind it.
fn draw_thing(c: &mut Canvas, o: u8, sx: i32, sy: i32, faded: bool, now: f64) {
    let g = match o {
        obj::BIRCH => BIRCH,
        obj::OAK => OAK,
        obj::ROCK => ROCK,
        obj::CRYSTAL => CRYSTAL,
        obj::PILLAR => PILLAR,
        obj::BRAMBLE => BRAMBLE,
        _ => return,
    };
    let alpha = if faded { 90 } else { 255 };
    let paint = thing_paint(alpha);
    c.grid(&g, sx, sy - g.h() + 2, 1, false, &paint);
    if o == obj::CRYSTAL {
        let pulse = (128.0 + (now / 400.0 + sx as f64).sin() * 60.0) as u32;
        c.glow_add(sx + 7, sy - 6, 6, Rgba(127, 224, 255, pulse as u8));
    }
}

/// The Luciphon: a bell-lantern on a marble plinth, three tiles square,
/// its feet at screen (sx, sy) (the bottom of its middle row).
fn draw_luciphon(c: &mut Canvas, sx: i32, sy: i32, now: f64) {
    let plinth = palette::MARBLE;
    c.fill_rect(sx - 24, sy - 34, 48, 34, palette::shade(plinth, -20));
    c.fill_rect(sx - 24, sy - 40, 48, 8, plinth);
    c.fill_rect(sx - 24, sy - 2, 48, 4, palette::shade(plinth, -60));
    // The bell.
    let bell = Rgba::rgb(196, 160, 90);
    for k in 0..26 {
        let half = 6 + k * 10 / 26 + if k > 20 { 3 } else { 0 };
        c.fill_rect(
            sx - half,
            sy - 70 + k,
            half * 2,
            1,
            palette::shade(bell, -(k % 7) * 3),
        );
    }
    c.fill_rect(sx - 2, sy - 76, 4, 6, palette::shade(bell, -30));
    // The light inside, breathing.
    let breath = 0.75 + 0.25 * (now / 900.0).sin();
    c.glow_add(
        sx,
        sy - 52,
        (14.0 * breath) as i32,
        Rgba(255, 230, 160, 255),
    );
    c.glow_add(sx, sy - 52, 5, Rgba(255, 255, 240, 255));
}

/// Stars in the Dark, drifting a little with the camera.
fn stars(c: &mut Canvas, v: &View) {
    for k in 0..90u32 {
        let h = palette::hash(k as i32, 77);
        let (px, py) = ((h % 2048) as f32, ((h >> 11) % 2048) as f32);
        let (x, y) = (
            (px - v.cx * 3.0).rem_euclid(c.w.max(1) as f32),
            (py - v.cy * 3.0).rem_euclid(c.h.max(1) as f32),
        );
        let b = 40 + (h >> 22) % 90;
        c.pixel(x as i32, y as i32, Rgba(180, 190, 255, b as u8));
    }
}

/// The whole picture: Dark, ground, things and bodies in row order, the
/// Luciphon's light, the feel, and the sight circle.
pub fn world(c: &mut Canvas, look: &mut Look, v: &View, tiles: &Tiles, bodies: &[Body], now: f64) {
    c.clear(DARK);
    stars(c, v);
    let (shx, shy) = look.fx.shake(now);
    let left = (v.cx * TILE as f32).round() as i32 - c.w / 2 - shx;
    let top = (v.cy * TILE as f32).round() as i32 - c.h / 2 - shy;
    look.ground.draw(c, left, top);
    let to = move |x: f32, y: f32| (x * TILE as f32 - left as f32, y * TILE as f32 - top as f32);
    look.fx.draw_under(c, &to, now);

    // Things and bodies, back to front.
    enum Item<'a> {
        Thing(u8, i32, i32),
        Lucy,
        Body(&'a Body),
    }
    let mut items: Vec<(i32, Item)> = Vec::new();
    let (tx0, tx1) = (left.div_euclid(TILE) - 1, (left + c.w).div_euclid(TILE) + 1);
    let (ty0, ty1) = (top.div_euclid(TILE) - 1, (top + c.h).div_euclid(TILE) + 3);
    for ty in ty0..=ty1 {
        for tx in tx0..=tx1 {
            let o = tiles.get(tx, ty).obj;
            if o != obj::NONE && o != obj::GLOWMOSS && o != obj::LUCIPHON {
                items.push(((ty + 1) * TILE, Item::Thing(o, tx, ty)));
            }
        }
    }
    if (ty0..=ty1).contains(&0) && (tx0..=tx1).contains(&0) {
        items.push((2 * TILE, Item::Lucy));
    }
    for b in bodies {
        items.push(((b.y * TILE as f32) as i32, Item::Body(b)));
    }
    items.sort_by_key(|i| i.0);
    // Lumens on screen, for fading what stands in front of them.
    let lumens: Vec<(i32, i32)> = bodies
        .iter()
        .filter(|b| b.kind == kind::LUMEN)
        .map(|b| ((b.x * TILE as f32) as i32, (b.y * TILE as f32) as i32))
        .collect();
    for (_, it) in &items {
        match it {
            Item::Thing(o, tx, ty) => {
                let (sx, sy) = (tx * TILE - left, (ty + 1) * TILE - top);
                let (wx, wy) = (tx * TILE + 8, (ty + 1) * TILE);
                let faded = lumens
                    .iter()
                    .any(|&(lx, ly)| (lx - wx).abs() < 12 && ly < wy && wy - ly < 26);
                draw_thing(c, *o, sx, sy, faded, now);
            }
            Item::Lucy => draw_luciphon(c, 8 - left, 2 * TILE - top, now),
            Item::Body(b) => {
                let (sx, sy) = to(b.x, b.y);
                match b.kind {
                    kind::LUMEN => draw_lumen(c, b, sx as i32, sy as i32, now),
                    kind::MOTE => {
                        c.glow_add(sx as i32, sy as i32 - 6, 5, GOLD);
                        c.glow_add(sx as i32, sy as i32 - 6, 2, Rgba(255, 255, 230, 255));
                    }
                    _ => {
                        let pulse = (180.0 + (now / 250.0 + b.id as f64).sin() * 70.0) as u32;
                        c.glow_add(
                            sx as i32,
                            sy as i32 - 2,
                            4,
                            Rgba(255, 210, 122, pulse.min(255) as u8),
                        );
                        c.pixel(sx as i32, sy as i32 - 2, Rgba(255, 250, 220, 255));
                    }
                }
            }
        }
    }
    // Names over others.
    for b in bodies
        .iter()
        .filter(|b| b.kind == kind::LUMEN && !b.you && !b.name.is_empty())
    {
        let (sx, sy) = to(b.x, b.y);
        let w = pixels::text_width(&b.name, 1);
        c.text_shadowed(
            sx as i32 - w / 2,
            sy as i32 - LUMEN_H - 12,
            &b.name,
            1,
            Rgba(244, 238, 222, 200),
        );
    }
    look.fx.draw_over(c, &to, now);

    // The Luciphon's light, always lit.
    let (lx, ly) = to(0.5, 0.5);
    if lx > -200.0 && ly > -200.0 && lx < c.w as f32 + 200.0 && ly < c.h as f32 + 200.0 {
        c.glow_add(lx as i32, ly as i32 - 30, 70, Rgba(255, 210, 122, 50));
    }
    // Beyond sight, the Dark.
    let (sx, sy) = (c.w as f32 / 2.0 + shx as f32, c.h as f32 / 2.0 + shy as f32);
    let r = v.sight * TILE as f32;
    let far = ((c.w * c.w + c.h * c.h) as f32).sqrt() / 2.0;
    if r < far {
        c.outside_circle(sx, sy, r, Rgba(11, 13, 26, 225));
        c.ring(sx, sy, r - 6.0, 14.0, Rgba(11, 13, 26, 90));
    }
}
