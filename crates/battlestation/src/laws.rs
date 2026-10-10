//! Every number that tunes the battlestation: where things stand on the
//! desk and how big they are, the hands, how fast they move, the screen.

use crate::V3;

// The room (metres).

/// The desk's top, its thickness, its half width (it spans x from minus
/// this to this) and its depth (z from minus this to its front edge, 0).
pub const DESK_TOP: f32 = 0.74;
pub const DESK_THICK: f32 = 0.035;
pub const DESK_HALF: f32 = 0.85;
pub const DESK_DEPTH: f32 = 0.75;
/// The wall behind the desk, the side walls, the ceiling, the wall at
/// your back.
pub const WALL_N: f32 = -0.80;
pub const WALL_SIDE: f32 = 1.70;
pub const CEILING: f32 = 2.60;
pub const WALL_S: f32 = 2.20;

// You, seated.

/// Your eyes; how far down you look; the field of view (vertical,
/// radians).
pub const EYE: V3 = [0.0, 1.17, 0.33];
pub const PITCH: f32 = -0.36;
pub const FOV: f32 = 1.12;
/// Leaning in (the wheel): how far the eye moves toward the screen at
/// most (all the way in, it looks at the screen's middle), how far a
/// wheel notch takes it (of the whole), and how quickly.
pub const LEAN: f32 = 0.34;
pub const LEAN_NOTCH: f32 = 0.12;
pub const LEAN_OMEGA: f32 = 7.0;
/// The widest the view gets for a narrow screen (radians, vertical), so
/// a phone held upright still sees the desk.
pub const FOV_MOST: f32 = 1.75;
/// The head follows the cursor across the screen: radians at its edge
/// (sideways, up and down), and how quickly.
pub const FOLLOW_YAW: f32 = 0.07;
pub const FOLLOW_PITCH: f32 = 0.04;
pub const HEAD_OMEGA: f32 = 5.0;
/// Breathing: how far the head rises and falls, and how often (Hz).
pub const BREATH: f32 = 0.0035;
pub const BREATH_HZ: f32 = 0.22;

// The monitor.

/// The picture's middle, its size, how far its top leans back (radians).
pub const SCREEN_AT: V3 = [0.0, 1.07, -0.48];
pub const SCREEN_W: f32 = 0.597;
pub const SCREEN_H: f32 = 0.336;
pub const SCREEN_TILT: f32 = 0.07;
/// The bezel's width around the picture, and the panel's depth.
pub const BEZEL: f32 = 0.009;
pub const PANEL_DEPTH: f32 = 0.025;
/// The picture in pixels (the terminal draws into this).
pub const SCREEN_PX: (i32, i32) = (480, 270);

// The keyboard (tenkeyless, ANSI).

/// One key unit; a keycap's width at its foot and at its top (of a
/// unit), its height; how far a key travels down.
pub const KEY_U: f32 = 0.019;
pub const CAP_FOOT: f32 = 0.0178;
pub const CAP_TOP: f32 = 0.0128;
pub const CAP_H: f32 = 0.0085;
pub const KEY_TRAVEL: f32 = 0.0038;
/// The keyboard's middle on the desk (x, z), its border beyond the keys,
/// the case's height at the front, how much it rises toward the back
/// (metres a metre), and the plate under the caps above the case's foot.
pub const KEYBOARD_X: f32 = -0.07;
pub const KEYBOARD_Z: f32 = -0.17;
pub const KEYBOARD_RIM: f32 = 0.012;
pub const CASE_H: f32 = 0.020;
pub const KEYBOARD_SLOPE: f32 = 0.09;
pub const PLATE: f32 = 0.017;
/// The layout's size in units (wide, deep).
pub const LAYOUT_W: f32 = 18.25;
pub const LAYOUT_D: f32 = 6.5;

// The mouse.

/// Its home (x, z), its length, width and height; how far it moves for
/// a pixel of the cursor; the screen pixels the cursor moves for a pixel
/// of your own mouse.
pub const MOUSE_X: f32 = 0.31;
pub const MOUSE_Z: f32 = -0.15;
pub const MOUSE_L: f32 = 0.118;
pub const MOUSE_W: f32 = 0.064;
pub const MOUSE_H: f32 = 0.038;
/// How the hand on it turns (radians), and the mat under it all.
pub const MOUSE_YAW: f32 = -0.05;
pub const PAD: f32 = 0.003;
pub const MOUSE_PER_PX: f32 = 0.00028;
pub const CURSOR_SPEED: f32 = 0.65;
/// How much your mouse must move, in pixels within `MOUSE_WINDOW`
/// seconds, before the right hand leaves the keys for it.
pub const MOUSE_WAKE: f32 = 6.0;
pub const MOUSE_WINDOW: f32 = 0.25;

// The hands.

/// Each finger's segments (from the knuckle out) and radius: thumb,
/// index, middle, ring, little. The thumb's first is its metacarpal.
pub const SEGMENTS: [[f32; 3]; 5] = [
    [0.036, 0.033, 0.028],
    [0.041, 0.024, 0.020],
    [0.046, 0.028, 0.021],
    [0.043, 0.026, 0.021],
    [0.033, 0.019, 0.018],
];
pub const RADII: [f32; 5] = [0.0098, 0.0084, 0.0087, 0.0082, 0.0074];
/// Where each finger's root sits, from the wrist, on a right hand (out
/// to the little finger's side, up, forward); a left hand mirrors it.
pub const ROOTS: [V3; 5] = [
    [-0.022, -0.012, 0.030],
    [-0.027, 0.004, 0.088],
    [-0.007, 0.006, 0.092],
    [0.012, 0.003, 0.086],
    [0.029, -0.004, 0.074],
];
/// The thumb's metacarpal leaves its root this way (as `ROOTS`).
pub const THUMB_OUT: V3 = [-0.5, -0.42, 0.76];
/// The palm: its width, length (wrist to knuckles) and thickness.
pub const PALM: V3 = [0.074, 0.092, 0.026];
/// How each hand turns toward the middle (radians), and how its back
/// slopes down toward the fingers.
pub const HAND_YAW: f32 = 0.16;
pub const HAND_PITCH: f32 = 0.10;
/// The wrist above the home row's keytops, and behind the fingertips.
pub const WRIST_UP: f32 = 0.036;
pub const WRIST_BACK: f32 = 0.148;
/// A finger at rest floats this far over its keytop.
pub const HOVER: f32 = 0.0012;
/// How far the middle joint bends for each radian the first does not
/// (the last joint follows the middle one by `COUPLE`); the most a
/// finger bends; how far it may splay from the hand's line (radians).
pub const COUPLE: f32 = 0.72;
pub const MOST_BEND: f32 = 1.9;
pub const SPLAY: f32 = 0.38;
/// Springs (see `Spring`): a fingertip, a wrist, the hand going to the
/// mouse and back, a key going down and up.
pub const TIP_OMEGA: f32 = 62.0;
pub const WRIST_OMEGA: f32 = 15.0;
pub const SWAP_OMEGA: f32 = 9.0;
pub const KEY_OMEGA: f32 = 95.0;
/// How long a finger stays over a key it let go before going home (s).
pub const LINGER: f32 = 0.14;
/// How much of a far reach the wrist makes (the finger the rest):
/// near keys (within `REACH_NEAR` units of home) this much, far ones
/// (`REACH_FAR` units) that.
pub const REACH_NEAR: f32 = 1.2;
pub const REACH_FAR: f32 = 3.5;
pub const WRIST_NEAR: f32 = 0.40;
pub const WRIST_FAR: f32 = 0.92;
/// The hand rises this high as it crosses to the mouse and back.
pub const SWAP_ARC: f32 = 0.045;
/// Where the forearm points from (a shoulder, about), each side.
pub const SHOULDER: V3 = [0.20, 0.95, 0.55];
pub const FOREARM: f32 = 0.27;

// The terminal.

/// Lines the terminal keeps, commands it remembers.
pub const SCROLLBACK: usize = 400;
pub const HISTORY: usize = 64;
/// The longest command line.
pub const LINE_MAX: usize = 240;
