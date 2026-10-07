//! Lantern noir: the Dark, gold light, marble, glass, and one hue a soul.
//! Day light (0.85, a grey-gold haze) is baked into these colours.

use pixels::Rgba;

pub const DARK: Rgba = Rgba::rgb(11, 13, 26);
pub const DEEP: Rgba = Rgba::rgb(6, 7, 15);
pub const GOLD: Rgba = Rgba::rgb(255, 210, 122);
pub const EMBER: Rgba = Rgba::rgb(255, 122, 61);
pub const TEAL: Rgba = Rgba::rgb(95, 211, 200);
pub const HUSH: Rgba = Rgba::rgb(125, 111, 147);
pub const MARBLE: Rgba = Rgba::rgb(206, 200, 186);
pub const RIM: Rgba = Rgba::rgb(127, 224, 255);
pub const INK: Rgba = Rgba::rgb(244, 238, 222);
pub const DIM: Rgba = Rgba(244, 238, 222, 150);
pub const OUTLINE: Rgba = Rgba::rgb(14, 12, 22);

/// A ground kind's colour, and how much it varies tile to tile.
pub fn ground(kind: u8) -> (Rgba, i32) {
    use luciphon::tiles::ground as g;
    match kind {
        g::MARBLE => (MARBLE, 6),
        g::MEADOW => (Rgba::rgb(64, 96, 66), 10),
        g::MOSS => (Rgba::rgb(46, 82, 62), 8),
        g::DARK => (Rgba::rgb(36, 54, 54), 8),
        g::ICE => (Rgba::rgb(150, 196, 214), 6),
        g::MUD => (Rgba::rgb(78, 60, 46), 8),
        g::WATER => (Rgba::rgb(42, 82, 118), 6),
        g::GLASS => (Rgba::rgb(72, 112, 138), 10),
        _ => (DARK, 0),
    }
}

/// A soul's hue as a hood: light, mid and shadow.
pub fn hood(hue: u8) -> (Rgba, Rgba, Rgba) {
    let h = hue as f32 / 256.0 * 360.0;
    (
        Rgba::hsl(h, 0.55, 0.62),
        Rgba::hsl(h, 0.5, 0.45),
        Rgba::hsl(h, 0.45, 0.28),
    )
}

/// A cheap, fixed hash of a tile, for its variation.
pub fn hash(x: i32, y: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9e37_79b1) ^ (y as u32).wrapping_mul(0x85eb_ca77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^ (h >> 12)
}

pub fn shade(c: Rgba, d: i32) -> Rgba {
    let f = |v: u8| (v as i32 + d).clamp(0, 255) as u8;
    Rgba(f(c.0), f(c.1), f(c.2), c.3)
}
