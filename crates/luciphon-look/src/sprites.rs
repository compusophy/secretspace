//! Every sprite, as a text grid. A Lumen is 12x16: a hood in its soul's
//! hue, two dots of light for eyes, and a core as bright as its Flame;
//! three facings (front, back, side, mirrored for the other side) and a
//! stride for the feet. Things on the ground stand on their tile's bottom
//! edge and rise above it, with front faces.

use pixels::{Grid, Rgba};

use crate::palette::{hood, OUTLINE};

pub const LUMEN_W: i32 = 12;
pub const LUMEN_H: i32 = 16;

pub const FRONT: Grid = Grid::new(&[
    "....oooo....",
    "...oHHHHo...",
    "..oHHhhHHo..",
    "..oHhffhHo..",
    ".oHhffffhHo.",
    ".ohfeffefho.",
    ".ohffffffho.",
    ".odhffffhdo.",
    "..odhhhhdo..",
    "..ohhcchho..",
    ".ohhcCCchho.",
    ".ohhhcchhho.",
    ".odhhhhhhdo.",
    ".odhhhhhhdo.",
]);

pub const BACK: Grid = Grid::new(&[
    "....oooo....",
    "...oHHHHo...",
    "..oHHHHHHo..",
    "..oHHhhHHo..",
    ".oHHhhhhHHo.",
    ".ohhhhhhhho.",
    ".ohhhhhhhho.",
    ".odhhhhhhdo.",
    "..odhhhhdo..",
    "..ohhhhhho..",
    ".ohhhhhhhho.",
    ".ohhhhhhhho.",
    ".odhhhhhhdo.",
    ".odhhhhhhdo.",
]);

pub const SIDE: Grid = Grid::new(&[
    "....oooo....",
    "...oHHHHo...",
    "..oHHHHhfo..",
    "..oHHhhffo..",
    ".oHHhhfffo..",
    ".ohhhhffefo.",
    ".ohhhhffffo.",
    ".odhhhhffo..",
    "..odhhhhdo..",
    "..ohhhcchho.",
    ".ohhhhcCcho.",
    ".ohhhhhcho..",
    ".odhhhhhhdo.",
    ".odhhhhhhdo.",
]);

/// Feet: standing, and the two strides of a run.
pub const FEET: [Grid; 3] = [
    Grid::new(&["..oddddddo..", "...oo..oo..."]),
    Grid::new(&["..odddddo...", "..oo....oo.."]),
    Grid::new(&["...oddddddo.", "....oo.oo..."]),
];

/// How a Lumen's grid is coloured: its hue, its Flame (the core), and how
/// see-through it is (a ghost).
pub fn lumen_paint(hue: u8, flame: u8, alpha: u8) -> impl Fn(u8) -> Option<Rgba> {
    let (light, mid, shadow) = hood(hue);
    let f = flame.min(100) as f32 / 100.0;
    let core = Rgba::rgb(255, 210, 122).mix(Rgba::rgb(70, 60, 70), 1.0 - f);
    let core_hot = Rgba::rgb(255, 245, 210).mix(Rgba::rgb(90, 80, 90), 1.0 - f);
    move |ch| {
        let c = match ch {
            b'o' => OUTLINE,
            b'H' => light,
            b'h' => mid,
            b'd' => shadow,
            b'f' => Rgba::rgb(10, 9, 16),
            b'e' => Rgba::rgb(255, 236, 170),
            b'c' => core,
            b'C' => core_hot,
            _ => return None,
        };
        Some(Rgba(c.0, c.1, c.2, alpha))
    }
}

pub const BIRCH: Grid = Grid::new(&[
    ".....gGGGg......",
    "...gGGgGGGGg....",
    "..gGGGGgGGGGg...",
    ".gGGgGGGGGgGGg..",
    ".gGGGGGgGGGGGg..",
    "gGGgGGGGGGGgGGg.",
    "gGGGGGGgGGGGGGg.",
    "gGgGGGGGGGgGGGg.",
    ".gGGGgGGGGGGGg..",
    ".gGGGGGGgGGGgg..",
    "..ggGGGGGGGgg...",
    "...gggGwGggg....",
    ".......wW.......",
    ".......wW.......",
    ".......wk.......",
    ".......wW.......",
    ".......kW.......",
    ".......wW.......",
    "......swWs......",
    "......ssss......",
]);

pub const OAK: Grid = Grid::new(&[
    "....gggGGgg.....",
    "..gggGGGGGggg...",
    ".ggGGGgGGGGGgg..",
    "gggGGGGGGgGGGgg.",
    "ggGGgGGGGGGGGgg.",
    "gGGGGGGgGGGGGGg.",
    "ggGGGGGGGGgGGgg.",
    "gggGGgGGGGGGggg.",
    ".gggGGGGGGgggg..",
    "..ggggGbGgggg...",
    "......bBb.......",
    "......bBb.......",
    "......bBk.......",
    ".....bbBbb......",
    ".....sssss......",
]);

pub const ROCK: Grid = Grid::new(&[
    "....rrrrrr......",
    "..rrRrrrrrRr....",
    ".rRRrrrrrRRRr...",
    ".rRRRRrrRRRRRr..",
    "rRRRRRRRRRRRRRr.",
    "qRRRRRRRRRRRRRq.",
    "qqqRRRRRRRRRqqq.",
    "qqqqqqqqqqqqqqq.",
    ".qqqqqqqqqqqqq..",
    "..sssssssssss...",
]);

pub const CRYSTAL: Grid = Grid::new(&[
    "......x.........",
    ".....xXx........",
    ".....xXx...x....",
    "....xXXx..xXx...",
    "....xXXXx.xXx...",
    "...xXXXXx.xXXx..",
    "...xXXXXxxXXXx..",
    "..xXXXXXXxXXXx..",
    "..xXXXXXXxXXx...",
    "..xxXXXXxxxx....",
    "...ssssssss.....",
]);

pub const PILLAR: Grid = Grid::new(&[
    "..mmmmmmmmmm....",
    "..MMMMMMMMMM....",
    "...mMMMMMMm.....",
    "...mMmMMmMm.....",
    "...mMmMMmMm.....",
    "...mMmMMmMm.....",
    "...mMmMMmMm.....",
    "...mMmMMmMm.....",
    "...mMmMMmMm.....",
    "...mMmMMmMm.....",
    "...mMmMMmMm.....",
    "...mMmMMmMm.....",
    "...mMmMMmMm.....",
    "...mMmMMmMm.....",
    "...mMmMMmMm.....",
    "...mMmMMmMm.....",
    "...mMmMMmMm.....",
    "..mmMMMMMMmm....",
    "..MMMMMMMMMM....",
    "..pppppppppp....",
    "..ssssssssss....",
]);

pub const BRAMBLE: Grid = Grid::new(&[
    "...t.....t......",
    ".t.vt..t.vt..t..",
    "..vVv.tvVvt.vVt.",
    ".tvVVvvVVVvvVVv.",
    "vVVvVVVvVVVVVvVt",
    "tvVVVvVVVVvVVVv.",
    ".vVVVVVVvVVVVVvt",
    "tvvVVvVVVVVvVvv.",
    "..vvvvvvvvvvvv..",
    "..ssssssssssss..",
]);

pub const STUMP: Grid = Grid::new(&["......kwWk......", ".....swwWWs.....", ".....ssssss....."]);

pub const RUBBLE: Grid = Grid::new(&["...rr....rR.....", "..qRRq..qRRq.r..", "...ssss..sss.s.."]);

pub const SHARD: Grid = Grid::new(&[
    ".......x........",
    "......xXx.......",
    "......xXx.......",
    ".....ssss.......",
]);

pub const WALL: Grid = Grid::new(&[
    "bBbBbBbBbBbBbBbB",
    "BbbbBbbbBbbbBbbb",
    "bBbBbBbBbBbBbBbB",
    "bbbbbbbbbbbbbbbb",
    "BbbbBbbbBbbbBbbb",
    "bbbbbbbbbbbbbbbb",
    "qqqqqqqqqqqqqqqq",
    "qbqqqbqqqbqqqbqq",
    "qqqqqqqqqqqqqqqq",
    "qqbqqqbqqqbqqqbq",
    "qqqqqqqqqqqqqqqq",
    "ssssssssssssssss",
]);

pub const DOOR: Grid = Grid::new(&[
    "bBbBbBbBbBbBbBbB",
    "Bbbbbkkkkkkbbbbb",
    "bBbbkhhhhhhkbBbB",
    "bbbbkhhhhhhkbbbb",
    "Bbbbkhhhhhhkbbbb",
    "bbbbkhhhhhhkbbbb",
    "qqqqkhhhhhhkqqqq",
    "qbqqkhhhhGhkqbqq",
    "qqqqkhhhhhhkqqqq",
    "qqbqkhhhhhhkqqbq",
    "qqqqkhhhhhhkqqqq",
    "ssssssssssssssss",
]);

pub const THORNS: Grid = Grid::new(&[
    "...y.....y......",
    ".y.vy..y.vy..y..",
    "..vVv.yvVvy.vVy.",
    ".yvVVvvVVVvvVVv.",
    "vVVvVVVvVVVVVvVy",
    "yvVVVvVVVVvVVVv.",
    ".vVVVVVVvVVVVVvy",
    "..vvvvvvvvvvvv..",
    "..ssssssssssss..",
]);

pub const LANTERN: Grid = Grid::new(&[
    "......kkkk......",
    ".....kyYYyk.....",
    ".....kYYYYk.....",
    ".....kyYYyk.....",
    "......kkkk......",
    ".......bb.......",
    ".......bB.......",
    ".......bb.......",
    ".......bB.......",
    ".......bb.......",
    "......bbBb......",
    "......ssss......",
]);

pub const PLANTER: Grid = Grid::new(&[
    "..bBbBbBbBbBb...",
    "..bmmmmmmmmmb...",
    "..qqqqqqqqqqq...",
    "..sssssssssss...",
]);

pub const SPROUT: Grid = Grid::new(&[
    "....g..g..g.....",
    "....G..G..G.....",
    "..bBbBbBbBbBb...",
    "..bmmmmmmmmmb...",
    "..qqqqqqqqqqq...",
    "..sssssssssss...",
]);

pub const RIPE: Grid = Grid::new(&[
    "...y.y.y.y.y....",
    "...Y.Y.Y.Y.Y....",
    "...y.y.y.y.y....",
    "...g.g.g.g.g....",
    "..bBbBbBbBbBb...",
    "..bmmmmmmmmmb...",
    "..qqqqqqqqqqq...",
    "..sssssssssss...",
]);

pub const WILTED: Grid = Grid::new(&[
    "....w.w..w.w....",
    "...wb.bw.b.bw...",
    "..bBbBbBbBbBb...",
    "..bmmmmmmmmmb...",
    "..qqqqqqqqqqq...",
    "..sssssssssss...",
]);

/// Things' palette.
pub fn thing_paint(alpha: u8) -> impl Fn(u8) -> Option<Rgba> {
    move |ch| {
        let c = match ch {
            b'g' => Rgba::rgb(78, 112, 70),
            b'G' => Rgba::rgb(150, 178, 104),
            b'w' => Rgba::rgb(222, 218, 204),
            b'W' => Rgba::rgb(244, 240, 228),
            b'k' => Rgba::rgb(40, 38, 44),
            b'b' => Rgba::rgb(82, 62, 48),
            b'B' => Rgba::rgb(112, 86, 64),
            b'r' => Rgba::rgb(150, 150, 160),
            b'R' => Rgba::rgb(116, 116, 128),
            b'q' => Rgba::rgb(70, 70, 84),
            b'x' => Rgba::rgb(90, 170, 210),
            b'X' => Rgba::rgb(170, 236, 255),
            b'm' => Rgba::rgb(170, 164, 150),
            b'M' => Rgba::rgb(226, 220, 206),
            b'p' => Rgba::rgb(120, 114, 104),
            b't' => Rgba::rgb(190, 120, 150),
            b'v' => Rgba::rgb(70, 30, 52),
            b'V' => Rgba::rgb(110, 50, 80),
            b'y' => Rgba::rgb(230, 180, 90),
            b'Y' => Rgba::rgb(255, 228, 150),
            b'h' => Rgba::rgb(52, 38, 32),
            b's' => return Some(Rgba(0, 0, 10, 70)),
            _ => return None,
        };
        Some(Rgba(c.0, c.1, c.2, alpha))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_grid_is_square_edged() {
        for g in [FRONT, BACK, SIDE] {
            assert!(g.rows.iter().all(|r| r.len() == LUMEN_W as usize));
        }
        for g in FEET {
            assert!(g.rows.iter().all(|r| r.len() == LUMEN_W as usize));
        }
        for g in [
            BIRCH, OAK, ROCK, CRYSTAL, PILLAR, BRAMBLE, STUMP, RUBBLE, SHARD, WALL, DOOR, THORNS,
            LANTERN, PLANTER, SPROUT, RIPE, WILTED,
        ] {
            assert!(g.rows.iter().all(|r| r.len() == 16), "{:?}", g.rows[0]);
        }
    }
}
