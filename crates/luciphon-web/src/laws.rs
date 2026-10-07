//! How the page reads a pointer and shows the world: every threshold of
//! the gestures (CSS pixels and milliseconds), the camera, the effects.
//! The world's own laws come from the server in the Welcome.

pub struct Feel {
    /// A press that moves this far before `hold_ms` is a drag.
    pub drag_px: f64,
    /// Released sooner, unmoved: a tap. Still pressed by then: a hold.
    pub hold_ms: f64,
    /// The stick: a walk from `walk_px`, a run from `run_px`; past
    /// `trail_px` the origin trails the pointer.
    pub walk_px: f64,
    pub run_px: f64,
    pub trail_px: f64,
    /// A flick: at least `flick_speed` px/ms over the last `flick_window`
    /// ms, and `flick_px` travelled in the last `flick_span` ms; never
    /// within `swing_ms` of passing within `swing_px` of the origin.
    pub flick_speed: f64,
    pub flick_window: f64,
    pub flick_px: f64,
    pub flick_span: f64,
    pub swing_px: f64,
    pub swing_ms: f64,
    /// A hold let go sooner than this after the press does nothing.
    pub release_ms: f64,
    /// An aim this long or more throws; up to `throw_far_px`.
    pub throw_px: f64,
    pub throw_far_px: f64,
    /// Leave this ring, come back inside `cancel_px`: a free cancel.
    pub ring_px: f64,
    pub cancel_px: f64,
    /// The Heart: its size, how far above the bottom, its hit radius.
    pub heart_px: f64,
    pub heart_above: f64,
    pub heart_hit: f64,
    /// Others are drawn this far behind the newest frame.
    pub behind_ms: f64,
    /// Prediction errors under this many tiles are blended over
    /// `blend_ms`; larger ones snap.
    pub blend_tiles: f32,
    pub blend_ms: f64,
    /// Frames are drawn at most this often.
    pub frame_ms: f64,
}

pub const FEEL: Feel = Feel {
    drag_px: 10.0,
    hold_ms: 300.0,
    walk_px: 10.0,
    run_px: 28.0,
    trail_px: 60.0,
    flick_speed: 1.0,
    flick_window: 50.0,
    flick_px: 24.0,
    flick_span: 80.0,
    swing_px: 12.0,
    swing_ms: 100.0,
    release_ms: 400.0,
    throw_px: 40.0,
    throw_far_px: 160.0,
    ring_px: 24.0,
    cancel_px: 16.0,
    heart_px: 56.0,
    heart_above: 72.0,
    heart_hit: 32.0,
    behind_ms: 66.0,
    blend_tiles: 0.25,
    blend_ms: 100.0,
    frame_ms: 1000.0 / 61.0,
};
