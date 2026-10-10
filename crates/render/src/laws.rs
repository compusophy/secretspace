//! The engine's numbers (rule 3): its light grid and its limits.

/// The light grid: a square of cells about the eye, each listing the
/// lights that reach it. Metres a cell; cells a side; lights a cell.
pub const GRID_CELL: f32 = 4.0;
pub const GRID_CELLS: u32 = 64;
pub const MAX_PER_CELL: u32 = 48;
/// The nearest the camera sees (metres); there is no far plane.
pub const NEAR: f32 = 0.05;
/// The shaders' clock wraps at this many seconds (about four and a half
/// hours: one jump then), so it stays precise to the millisecond and
/// what it is fed to (sines, noise) never grows huge.
pub const TIME_WRAP: f32 = 16384.0;
/// A spark of light's size on screen, in pixels, at most; a flame's or a
/// puff's, as a share of the screen's height; and how near the eye a
/// spark fades out (metres), so none blinds it.
pub const SPARK_MAX_PX: f32 = 64.0;
pub const PUFF_MAX: f32 = 0.45;
pub const SPARK_NEAR: f32 = 0.6;

/// Where each of the sun's shadow cascades ends (metres ahead of the eye).
pub const CASCADES: [f32; 3] = [14.0, 48.0, 150.0];
/// How dark the sun's shadow is (1 fully).
pub const SHADOW_STRENGTH: f32 = 1.0;
/// The picture's own format before tone mapping.
pub const HDR: &str = "rgba16float";
/// Ambient occlusion: how far about a point it looks (metres), how much
/// a surface must rise over another to shade it (a slope), how strongly
/// and how darkly (a power) it shades, and how far off it fades out
/// (metres).
pub const AO_REACH: f32 = 1.0;
pub const AO_SLACK: f32 = 0.15;
pub const AO_STRENGTH: f32 = 2.6;
pub const AO_POWER: f32 = 2.0;
pub const AO_FADE: f32 = 70.0;
/// Shafts of sunlight: how strong, and how far about the sun on screen
/// the sky shines into them (a share of the screen's height).
pub const SHAFTS: f32 = 0.4;
pub const SHAFT_GLOW: f32 = 0.22;
