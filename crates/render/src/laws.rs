//! The engine's numbers (rule 3): its light grid and its limits.

/// The light grid: a square of cells about the eye, each listing the
/// lights that reach it. Metres a cell; cells a side; lights a cell.
pub const GRID_CELL: f32 = 4.0;
pub const GRID_CELLS: u32 = 64;
pub const MAX_PER_CELL: u32 = 48;
/// The nearest the camera sees (metres); there is no far plane.
pub const NEAR: f32 = 0.05;
/// A spark's size on screen, in pixels, at most.
pub const SPARK_MAX_PX: f32 = 48.0;
