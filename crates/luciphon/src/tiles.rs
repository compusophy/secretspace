//! The island as tiles: a 128x128 grid around (0, 0), in 32x32 chunks.
//! A tile is its ground (kind and level), the object on it, and the claim
//! that lit it. Tile (x, y) covers `[x, x+1) x [y, y+1)` in world tiles.

use engine::fixed::Fx;
use engine::wire::{Reader, Writer};

/// Ground kinds.
pub mod ground {
    pub const VOID: u8 = 0;
    pub const MARBLE: u8 = 1;
    pub const MEADOW: u8 = 2;
    pub const MOSS: u8 = 3;
    pub const DARK: u8 = 4;
    pub const ICE: u8 = 5;
    pub const MUD: u8 = 6;
    pub const WATER: u8 = 7;
    pub const GLASS: u8 = 8;
}

/// What stands on a tile.
pub mod obj {
    pub const NONE: u8 = 0;
    pub const BIRCH: u8 = 1;
    pub const OAK: u8 = 2;
    pub const ROCK: u8 = 3;
    pub const CRYSTAL: u8 = 4;
    pub const GLOWMOSS: u8 = 5;
    pub const BRAMBLE: u8 = 6;
    pub const PILLAR: u8 = 7;
    /// The bell-lantern at the centre (3x3).
    pub const LUCIPHON: u8 = 8;
    // Pieces people build (v0.2), on their own claim.
    pub const HEARTH: u8 = 9;
    pub const WALL: u8 = 10;
    pub const DOOR: u8 = 11;
    pub const THORNS: u8 = 12;
    pub const LANTERN: u8 = 13;
    pub const PLANTER: u8 = 14;
    /// A planter's Sunwheat: growing, ripe, and ripe too long.
    pub const SPROUT: u8 = 15;
    pub const RIPE: u8 = 16;
    pub const WILTED: u8 = 17;
    /// The high bit: a node struck bare, regrowing (a stump, rubble).
    pub const BARE: u8 = 0x80;

    /// Whether an object is a node that gives when struck.
    pub fn node(o: u8) -> bool {
        matches!(o, BIRCH | OAK | ROCK | GLOWMOSS | CRYSTAL)
    }

    /// Whether an object is a piece someone built.
    pub fn piece(o: u8) -> bool {
        matches!(
            o,
            HEARTH | WALL | DOOR | THORNS | LANTERN | PLANTER | SPROUT | RIPE | WILTED
        )
    }
}

/// The claim bits of a tile: whose it is, and the open-wick bit.
pub const WICK: u16 = 0x8000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Tile {
    /// Kind in the low 5 bits, level in the high 3.
    pub ground: u8,
    pub obj: u8,
    /// The claim that lit it (0: nobody's); the high bit is an open wick.
    pub claim: u16,
}

impl Tile {
    pub const fn new(kind: u8, obj: u8) -> Tile {
        Tile {
            ground: kind,
            obj,
            claim: 0,
        }
    }

    pub fn kind(self) -> u8 {
        self.ground & 31
    }

    pub fn to_u32(self) -> u32 {
        self.ground as u32 | (self.obj as u32) << 8 | (self.claim as u32) << 16
    }

    pub fn from_u32(v: u32) -> Tile {
        Tile {
            ground: v as u8,
            obj: (v >> 8) as u8,
            claim: (v >> 16) as u16,
        }
    }

    pub fn void(self) -> bool {
        self.kind() == ground::VOID
    }

    /// Bodies stop at it (a door stops everyone here; see `solid_for`).
    pub fn solid(self) -> bool {
        matches!(
            self.obj,
            obj::BIRCH
                | obj::OAK
                | obj::ROCK
                | obj::CRYSTAL
                | obj::PILLAR
                | obj::LUCIPHON
                | obj::HEARTH
                | obj::WALL
                | obj::DOOR
                | obj::LANTERN
                | obj::PLANTER
                | obj::SPROUT
                | obj::RIPE
                | obj::WILTED
        )
    }

    /// Whether it stops a body whose own land is `claim`: a door lets its
    /// owner through.
    pub fn solid_for(self, claim: u16) -> bool {
        if self.obj == obj::DOOR {
            return claim == 0 || self.owner() != claim;
        }
        self.solid()
    }

    /// The claim it belongs to (0: the commons), wick or not.
    pub fn owner(self) -> u16 {
        self.claim & !WICK
    }

    /// Whose open wick it is, if it is one.
    pub fn wick(self) -> Option<u16> {
        (self.claim & WICK != 0).then_some(self.claim & !WICK)
    }

    /// Kindled land (or a core) of this claim, not a wick.
    pub fn land_of(self, claim: u16) -> bool {
        claim != 0 && self.claim == claim
    }
}

/// Grid size in tiles, chunk size, and chunks a side.
pub const SIZE: i32 = 128;
pub const CHUNK: i32 = 32;
pub const CHUNKS: i32 = SIZE / CHUNK;
const HALF: i32 = SIZE / 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tiles {
    pub t: Vec<Tile>,
}

impl Default for Tiles {
    fn default() -> Tiles {
        Tiles {
            t: vec![Tile::default(); (SIZE * SIZE) as usize],
        }
    }
}

impl Tiles {
    /// The index of tile (x, y), if it is on the grid.
    pub fn index(x: i32, y: i32) -> Option<usize> {
        let (gx, gy) = (x + HALF, y + HALF);
        ((0..SIZE).contains(&gx) && (0..SIZE).contains(&gy)).then_some((gy * SIZE + gx) as usize)
    }

    /// The tile at an index.
    pub fn at_index(i: usize) -> (i32, i32) {
        (i as i32 % SIZE - HALF, i as i32 / SIZE - HALF)
    }

    /// The tile (x, y); void off the grid.
    pub fn get(&self, x: i32, y: i32) -> Tile {
        Tiles::index(x, y).map_or(Tile::default(), |i| self.t[i])
    }

    pub fn set(&mut self, x: i32, y: i32, tile: Tile) {
        if let Some(i) = Tiles::index(x, y) {
            self.t[i] = tile;
        }
    }

    /// The tile under a point.
    pub fn under(&self, x: Fx, y: Fx) -> Tile {
        self.get(x.floor(), y.floor())
    }

    pub fn solid(&self, x: i32, y: i32) -> bool {
        self.get(x, y).solid()
    }

    pub fn void(&self, x: i32, y: i32) -> bool {
        self.get(x, y).void()
    }

    /// Chunk (cx, cy)'s tiles, row by row.
    pub fn chunk(&self, cx: i32, cy: i32) -> Vec<Tile> {
        let mut out = Vec::with_capacity((CHUNK * CHUNK) as usize);
        for y in 0..CHUNK {
            for x in 0..CHUNK {
                out.push(self.get(cx * CHUNK + x - HALF, cy * CHUNK + y - HALF));
            }
        }
        out
    }

    /// The chunk a tile is in.
    pub fn chunk_of(x: i32, y: i32) -> (i32, i32) {
        ((x + HALF).div_euclid(CHUNK), (y + HALF).div_euclid(CHUNK))
    }

    pub fn set_chunk(&mut self, cx: i32, cy: i32, tiles: &[Tile]) {
        for (k, &tile) in tiles.iter().enumerate().take((CHUNK * CHUNK) as usize) {
            let (x, y) = (k as i32 % CHUNK, k as i32 / CHUNK);
            self.set(cx * CHUNK + x - HALF, cy * CHUNK + y - HALF, tile);
        }
    }
}

/// A chunk as runs: `[count u8, tile u32]...` until 1024 tiles.
pub fn rle(tiles: &[Tile], w: &mut Writer) {
    let mut i = 0;
    while i < tiles.len() {
        let t = tiles[i];
        let mut n = 1;
        while i + n < tiles.len() && n < 255 && tiles[i + n] == t {
            n += 1;
        }
        w.u8(n as u8).u32(t.to_u32());
        i += n;
    }
}

/// A chunk back from its runs; None unless it is exactly one chunk.
pub fn unrle(r: &mut Reader) -> Option<Vec<Tile>> {
    let want = (CHUNK * CHUNK) as usize;
    let mut out = Vec::with_capacity(want);
    while out.len() < want {
        let n = r.u8()? as usize;
        let t = Tile::from_u32(r.u32()?);
        if n == 0 || out.len() + n > want {
            return None;
        }
        out.extend(std::iter::repeat_n(t, n));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_round_trip_through_runs() {
        let mut t = Tiles::default();
        t.set(-64, -64, Tile::new(ground::MEADOW, obj::BIRCH));
        t.set(-60, -63, Tile::new(ground::ICE, 0));
        for x in -40..-36 {
            t.set(x, -50, Tile::new(ground::GLASS, obj::CRYSTAL));
        }
        for cy in 0..CHUNKS {
            for cx in 0..CHUNKS {
                let tiles = t.chunk(cx, cy);
                let mut w = Writer::default();
                rle(&tiles, &mut w);
                assert!(w.0.len() <= 6 * 1024);
                let back = unrle(&mut Reader::new(&w.0)).unwrap();
                assert_eq!(back, tiles);
            }
        }
        assert_eq!(Tiles::chunk_of(-64, -64), (0, 0));
        assert_eq!(Tiles::chunk_of(63, 63), (3, 3));
        assert_eq!(Tiles::at_index(Tiles::index(-3, 7).unwrap()), (-3, 7));
        assert!(t.void(500, 0));
        assert!(unrle(&mut Reader::new(&[0, 1, 2, 3, 4])).is_none());
    }
}
