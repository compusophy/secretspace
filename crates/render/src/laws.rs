//! The engine's numbers (rule 3): its tiers, its light grid, its limits,
//! and how its shadows, bloom, occlusion, grass, sea and sparks are tuned.

use crate::Quality;

/// The tiers: desktops (High), phones and small GPUs (Medium), software
/// adapters and the weakest (Low). What each does is `Quality`'s.
pub const HIGH: Quality = Quality {
    scale: 1.0,
    msaa: 4,
    cascades: 3,
    shadow_size: 2048,
    grass_spacing: 0.24,
    grass_reach: 42.0,
    bloom_levels: 6,
    ao: 6,
    shafts: true,
    ssr: 16,
    decals: true,
    lights: 48,
};
pub const MEDIUM: Quality = Quality {
    scale: 0.8,
    msaa: 4,
    cascades: 2,
    shadow_size: 1536,
    grass_spacing: 0.34,
    grass_reach: 28.0,
    bloom_levels: 5,
    ao: 4,
    shafts: true,
    ssr: 10,
    decals: true,
    lights: 16,
};
pub const LOW: Quality = Quality {
    scale: 0.65,
    msaa: 1,
    cascades: 1,
    shadow_size: 1024,
    grass_spacing: 0.0,
    grass_reach: 0.0,
    bloom_levels: 4,
    ao: 0,
    shafts: false,
    ssr: 0,
    decals: false,
    lights: 8,
};

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
/// A spark smaller than this on screen (a radius, pixels) is drawn this
/// big and dimmed to keep its light, but never below this share of it.
pub const SPARK_MIN_PX: f32 = 1.0;
pub const SPARK_FAR_FLOOR: f32 = 0.15;
/// What glows or is see-through fades out over this many metres before
/// what stands behind it.
pub const SOFT_FADE: f32 = 0.45;
/// How deep the sea is (metres) where it shows nothing of its bed, and
/// is wholly its own deep colour.
pub const SEA_DEEP: f32 = 4.0;

/// How far about each point of the terrain (metres) its openness to the
/// sky is looked for: hills that far off shade a hollow's light.
pub const OPEN_REACH: f32 = 40.0;
/// How much of the grass far off is left out (a share, at its reach; none
/// within half of it): the rest is drawn wider to cover as much.
pub const GRASS_THIN: f32 = 0.6;
/// How much light reaches a blade's root (its tip all of it).
pub const GRASS_ROOT: f32 = 0.7;
/// Leaves: how fine (noise cells a metre) the gaps that break a crown's
/// edge are.
pub const LEAF_GRAIN: f32 = 7.0;

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
/// Culling what stands still: how far past its box a shadow cascade takes
/// what casts (a share of its half width: it is laid out again once its
/// box has moved this far); how much wider than the eye's the view's cone
/// is (radians); how far the eye may go before both are laid again (m).
pub const CAST_SLACK: f32 = 0.2;
pub const VIEW_SLACK: f32 = 0.35;
pub const EYE_SLACK: f32 = 6.0;
/// The most decals drawn a frame (the first so many given).
pub const DECALS: usize = 256;
/// The sun's shafts: steps marched from each pixel toward the sun, and
/// how much dimmer each step's light is than the last.
pub const SHAFT_STEPS: u32 = 40;
pub const SHAFT_DECAY: f32 = 0.955;
/// The tone map (the picture's light to the screen's): "aces" (film's
/// response, fitted) or "neutral" (Khronos's, hues kept).
pub const TONE_MAP: &str = "aces";
/// Bloom: each wider level is added at this share of the one finer, so
/// the narrow glow about a bright thing outweighs the wide veil (the
/// levels then averaged: `Look::bloom` is a share of the picture).
pub const BLOOM_FALLOFF: f32 = 0.75;
/// Ambient occlusion: how far about a point it looks (metres), how much
/// a surface must rise over another to shade it (a slope), how strongly
/// and how darkly (a power) it shades, and how far off it fades out
/// (metres).
pub const AO_REACH: f32 = 1.0;
pub const AO_SLACK: f32 = 0.15;
pub const AO_STRENGTH: f32 = 2.6;
pub const AO_POWER: f32 = 2.0;
pub const AO_FADE: f32 = 70.0;
/// How much ambient occlusion is eased on what faces the sun (1: none
/// left where it faces it squarely).
pub const AO_SUN: f32 = 0.5;
/// Shafts of sunlight: how strong, and how far about the sun on screen
/// the sky shines into them (a share of the screen's height).
pub const SHAFTS: f32 = 0.4;
pub const SHAFT_GLOW: f32 = 0.22;
/// Cloud cover (`Look::clouds`) up to which the shafts are whole, and at
/// which they are gone (the sun hidden).
pub const SHAFT_CLOUDS: (f32, f32) = (0.55, 0.95);
