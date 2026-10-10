//! The engine's numbers (rule 3): its light grid and its limits.

/// The light grid: a square of cells about the eye, each listing the
/// lights that reach it (as many as the tier allows). Metres a cell;
/// cells a side.
pub const GRID_CELL: f32 = 4.0;
pub const GRID_CELLS: u32 = 64;
/// The most pixels the scene is drawn at (a 4K screen's scene is drawn
/// smaller and scaled up in the finish; the HUD stays sharp).
pub const SCENE_PIXELS: f32 = 2.5e6;
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

/// How much of the grass far off is left out (a share, at its reach; none
/// within half of it): the rest is drawn wider to cover as much.
pub const GRASS_THIN: f32 = 0.6;

/// Where each of the sun's shadow cascades ends (metres ahead of the
/// eye), for a tier that draws one, two or three; past the last, the
/// island's own layer.
pub const CASCADES: [&[f32]; 3] = [&[40.0], &[20.0, 85.0], &[14.0, 48.0, 150.0]];
/// How dark the sun's shadow is (1 fully).
pub const SHADOW_STRENGTH: f32 = 1.0;
/// The sun's shadow: how much nearer the sun a point must be than what
/// the map holds to be lit (metres, the same in every layer), and how
/// far out along its normal it is looked up (texels of its layer); how
/// wide its edge is softened (texels, a disc's radius); over how much of
/// its end a cascade blends into the next (a share); how far behind a
/// cascade's box what casts into it may stand (metres); the raster's own
/// bias (a constant, and by slope); and how far the sun turns (radians)
/// before the island's layer is drawn again.
pub const SHADOW_BIAS: f32 = 0.04;
pub const SHADOW_NORMAL: f32 = 1.5;
pub const SHADOW_SOFT: f32 = 1.25;
pub const SHADOW_BAND: f32 = 0.15;
pub const SHADOW_BACK: f32 = 300.0;
pub const SHADOW_RASTER: (i32, f32) = (2, 2.5);
pub const SHADOW_TURN: f32 = 0.0087;
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
/// Cloud cover (`Look::clouds`) up to which the shafts are whole, and at
/// which they are gone (the sun hidden).
pub const SHAFT_CLOUDS: (f32, f32) = (0.55, 0.95);
