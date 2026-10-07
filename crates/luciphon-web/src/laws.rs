//! How the page reads hands and shows the world: looking, the phone's
//! stick and buttons, the camera, the fog, smoothing. The world's own laws
//! come from the server in the Welcome.

pub struct Feel {
    /// Radians a CSS pixel of mouse movement turns; of a look drag.
    pub mouse: f32,
    pub drag: f32,
    /// How far up or down you can look, radians.
    pub pitch_max: f32,
    /// Vertical field of view, radians (wider on a tall screen).
    pub fov: f32,
    pub fov_tall: f32,
    /// Eye height above the ground, tiles; and how far the picture goes.
    pub eye: f32,
    pub far: f32,
    /// Fog from here to there, tiles.
    pub fog_near: f32,
    pub fog_far: f32,
    /// A mouse press held this long charges a heavy.
    pub click_hold_ms: f64,
    /// The phone's stick: nothing inside `walk_px`, a run from `stick_px`.
    pub walk_px: f64,
    pub stick_px: f64,
    /// Pushed this far, a sprint.
    pub sprint_px: f64,
    /// A look touch lifted sooner and stiller than this is a strike; held
    /// still this long, a charge.
    pub tap_ms: f64,
    pub tap_px: f64,
    pub hold_ms: f64,
    /// The phone's buttons: size, and the gap from the screen's edge.
    pub button_px: f64,
    pub edge_px: f64,
    /// A throw's range follows the look: from `throw_low` (radians,
    /// looking down) for the nearest to `throw_high` for the farthest.
    pub throw_low: f32,
    pub throw_high: f32,
    /// Others are drawn this far behind the newest frame.
    pub behind_ms: f64,
    /// Prediction errors under this many tiles are blended over
    /// `blend_ms`; larger ones snap.
    pub blend_tiles: f32,
    pub blend_ms: f64,
    /// Device pixels a CSS pixel, at most (phones draw fewer).
    pub max_dpr: f64,
    pub max_dpr_touch: f64,
}

pub const FEEL: Feel = Feel {
    mouse: 0.0024,
    drag: 0.0065,
    pitch_max: 1.35,
    fov: 1.2,
    fov_tall: 1.45,
    eye: 1.05,
    far: 60.0,
    fog_near: 9.0,
    fog_far: 27.0,
    click_hold_ms: 220.0,
    walk_px: 8.0,
    stick_px: 44.0,
    sprint_px: 60.0,
    tap_ms: 260.0,
    tap_px: 14.0,
    hold_ms: 320.0,
    button_px: 62.0,
    edge_px: 22.0,
    throw_low: -0.35,
    throw_high: 0.45,
    behind_ms: 66.0,
    blend_tiles: 0.25,
    blend_ms: 90.0,
    max_dpr: 2.0,
    max_dpr_touch: 1.5,
};
