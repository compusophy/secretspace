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

/// Where each of the sun's shadow cascades ends (metres ahead of the eye).
pub const CASCADES: [f32; 3] = [14.0, 48.0, 150.0];
/// How dark the sun's shadow is (1 fully).
pub const SHADOW_STRENGTH: f32 = 1.0;
/// The picture's own format before tone mapping.
pub const HDR: &str = "rgba16float";
