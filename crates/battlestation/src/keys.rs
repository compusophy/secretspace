//! The keyboard: a tenkeyless ANSI layout, key by key (its code as the
//! browser names it, its legend, where it sits in key units, its width),
//! the finger that types it (as a touch typist does), and where it is on
//! the desk.

use std::sync::OnceLock;

use crate::laws::{
    CAP_H, DESK_TOP, KEYBOARD_RIM, KEYBOARD_SLOPE, KEYBOARD_X, KEYBOARD_Z, KEY_U, LAYOUT_D,
    LAYOUT_W, PLATE,
};
use crate::V3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
}

/// The thumb, index, middle, ring and little fingers.
pub const THUMB: usize = 0;
pub const INDEX: usize = 1;
pub const MIDDLE: usize = 2;
pub const RING: usize = 3;
pub const LITTLE: usize = 4;

/// A finger of a hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Finger {
    pub side: Side,
    pub digit: usize,
}

const fn f(side: Side, digit: usize) -> Finger {
    Finger { side, digit }
}

/// One key.
#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    /// `KeyboardEvent.code`: "KeyA", "Space".
    pub code: &'static str,
    /// What is printed on it.
    pub legend: &'static str,
    /// Its left edge and its row's top, in units from the layout's top
    /// left; its width in units.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub finger: Finger,
}

/// A row: its top (units), then its keys (code, legend, width) from the
/// left; an empty code is a gap.
type Row = (f32, &'static [(&'static str, &'static str, f32)]);

const NAV: f32 = 0.25;

const ROWS: [Row; 6] = [
    (
        0.0,
        &[
            ("Escape", "esc", 1.0),
            ("", "", 1.0),
            ("F1", "f1", 1.0),
            ("F2", "f2", 1.0),
            ("F3", "f3", 1.0),
            ("F4", "f4", 1.0),
            ("", "", 0.5),
            ("F5", "f5", 1.0),
            ("F6", "f6", 1.0),
            ("F7", "f7", 1.0),
            ("F8", "f8", 1.0),
            ("", "", 0.5),
            ("F9", "f9", 1.0),
            ("F10", "f10", 1.0),
            ("F11", "f11", 1.0),
            ("F12", "f12", 1.0),
            ("", "", NAV),
            ("PrintScreen", "prt", 1.0),
            ("ScrollLock", "scr", 1.0),
            ("Pause", "brk", 1.0),
        ],
    ),
    (
        1.5,
        &[
            ("Backquote", "`", 1.0),
            ("Digit1", "1", 1.0),
            ("Digit2", "2", 1.0),
            ("Digit3", "3", 1.0),
            ("Digit4", "4", 1.0),
            ("Digit5", "5", 1.0),
            ("Digit6", "6", 1.0),
            ("Digit7", "7", 1.0),
            ("Digit8", "8", 1.0),
            ("Digit9", "9", 1.0),
            ("Digit0", "0", 1.0),
            ("Minus", "-", 1.0),
            ("Equal", "=", 1.0),
            ("Backspace", "bksp", 2.0),
            ("", "", NAV),
            ("Insert", "ins", 1.0),
            ("Home", "home", 1.0),
            ("PageUp", "pgup", 1.0),
        ],
    ),
    (
        2.5,
        &[
            ("Tab", "tab", 1.5),
            ("KeyQ", "Q", 1.0),
            ("KeyW", "W", 1.0),
            ("KeyE", "E", 1.0),
            ("KeyR", "R", 1.0),
            ("KeyT", "T", 1.0),
            ("KeyY", "Y", 1.0),
            ("KeyU", "U", 1.0),
            ("KeyI", "I", 1.0),
            ("KeyO", "O", 1.0),
            ("KeyP", "P", 1.0),
            ("BracketLeft", "[", 1.0),
            ("BracketRight", "]", 1.0),
            ("Backslash", "\\", 1.5),
            ("", "", NAV),
            ("Delete", "del", 1.0),
            ("End", "end", 1.0),
            ("PageDown", "pgdn", 1.0),
        ],
    ),
    (
        3.5,
        &[
            ("CapsLock", "caps", 1.75),
            ("KeyA", "A", 1.0),
            ("KeyS", "S", 1.0),
            ("KeyD", "D", 1.0),
            ("KeyF", "F", 1.0),
            ("KeyG", "G", 1.0),
            ("KeyH", "H", 1.0),
            ("KeyJ", "J", 1.0),
            ("KeyK", "K", 1.0),
            ("KeyL", "L", 1.0),
            ("Semicolon", ";", 1.0),
            ("Quote", "'", 1.0),
            ("Enter", "enter", 2.25),
        ],
    ),
    (
        4.5,
        &[
            ("ShiftLeft", "shift", 2.25),
            ("KeyZ", "Z", 1.0),
            ("KeyX", "X", 1.0),
            ("KeyC", "C", 1.0),
            ("KeyV", "V", 1.0),
            ("KeyB", "B", 1.0),
            ("KeyN", "N", 1.0),
            ("KeyM", "M", 1.0),
            ("Comma", ",", 1.0),
            ("Period", ".", 1.0),
            ("Slash", "/", 1.0),
            ("ShiftRight", "shift", 2.75),
            ("", "", NAV + 1.0),
            ("ArrowUp", "^", 1.0),
        ],
    ),
    (
        5.5,
        &[
            ("ControlLeft", "ctrl", 1.25),
            ("MetaLeft", "win", 1.25),
            ("AltLeft", "alt", 1.25),
            ("Space", "", 6.25),
            ("AltRight", "alt", 1.25),
            ("MetaRight", "win", 1.25),
            ("ContextMenu", "menu", 1.25),
            ("ControlRight", "ctrl", 1.25),
            ("", "", NAV),
            ("ArrowLeft", "<", 1.0),
            ("ArrowDown", "v", 1.0),
            ("ArrowRight", ">", 1.0),
        ],
    ),
];

/// The main block's fingers, as touch typing teaches them.
const FINGERS: [(Finger, &[&str]); 10] = [
    (
        f(Side::Left, LITTLE),
        &[
            "Backquote",
            "Digit1",
            "Tab",
            "KeyQ",
            "CapsLock",
            "KeyA",
            "ShiftLeft",
            "KeyZ",
            "ControlLeft",
        ],
    ),
    (f(Side::Left, RING), &["Digit2", "KeyW", "KeyS", "KeyX"]),
    (f(Side::Left, MIDDLE), &["Digit3", "KeyE", "KeyD", "KeyC"]),
    (
        f(Side::Left, INDEX),
        &[
            "Digit4", "Digit5", "KeyR", "KeyT", "KeyF", "KeyG", "KeyV", "KeyB",
        ],
    ),
    (f(Side::Left, THUMB), &["MetaLeft", "AltLeft"]),
    (f(Side::Right, THUMB), &["Space", "AltRight"]),
    (
        f(Side::Right, INDEX),
        &[
            "Digit6", "Digit7", "KeyY", "KeyU", "KeyH", "KeyJ", "KeyN", "KeyM",
        ],
    ),
    (f(Side::Right, MIDDLE), &["Digit8", "KeyI", "KeyK", "Comma"]),
    (
        f(Side::Right, RING),
        &["Digit9", "KeyO", "KeyL", "Period", "MetaRight"],
    ),
    (
        f(Side::Right, LITTLE),
        &[
            "Digit0",
            "Minus",
            "Equal",
            "Backspace",
            "KeyP",
            "BracketLeft",
            "BracketRight",
            "Backslash",
            "Semicolon",
            "Quote",
            "Enter",
            "Slash",
            "ShiftRight",
            "ContextMenu",
            "ControlRight",
        ],
    ),
];

/// Each finger's home key (the thumbs rest on the space bar).
pub const HOME: [(Finger, &str); 10] = [
    (f(Side::Left, THUMB), "Space"),
    (f(Side::Left, INDEX), "KeyF"),
    (f(Side::Left, MIDDLE), "KeyD"),
    (f(Side::Left, RING), "KeyS"),
    (f(Side::Left, LITTLE), "KeyA"),
    (f(Side::Right, THUMB), "Space"),
    (f(Side::Right, INDEX), "KeyJ"),
    (f(Side::Right, MIDDLE), "KeyK"),
    (f(Side::Right, RING), "KeyL"),
    (f(Side::Right, LITTLE), "Semicolon"),
];

/// Where on the space bar each thumb rests (of its width from the left).
pub const THUMB_ON_SPACE: [f32; 2] = [0.36, 0.52];

/// Every key, top row first, left to right.
pub fn layout() -> &'static [Key] {
    static KEYS: OnceLock<Vec<Key>> = OnceLock::new();
    KEYS.get_or_init(build)
}

fn build() -> Vec<Key> {
    let mut keys: Vec<Key> = Vec::new();
    for (y, row) in ROWS {
        let mut x = 0.0;
        for &(code, legend, w) in row {
            if !code.is_empty() {
                keys.push(Key {
                    code,
                    legend,
                    x,
                    y,
                    w,
                    finger: f(Side::Left, LITTLE),
                });
            }
            x += w;
        }
    }
    // The main block by the table; the function row by the number row
    // under it; the arrows and the block above them by column.
    let numbers: Vec<(f32, f32, Finger)> = keys
        .iter()
        .filter(|k| k.y == 1.5)
        .filter_map(|k| main_finger(k.code).map(|f| (k.x, k.x + k.w, f)))
        .collect();
    for k in &mut keys {
        let mid = k.x + k.w / 2.0;
        k.finger = if let Some(f) = main_finger(k.code) {
            f
        } else if k.x >= 15.0 {
            let digit = match (mid - 15.25) as usize {
                0 => INDEX,
                1 => MIDDLE,
                _ => RING,
            };
            f(Side::Right, digit)
        } else {
            numbers
                .iter()
                .find(|(a, b, _)| mid >= *a && mid < *b)
                .map_or(f(Side::Right, LITTLE), |n| n.2)
        };
    }
    keys
}

fn main_finger(code: &str) -> Option<Finger> {
    FINGERS
        .iter()
        .find(|(_, codes)| codes.contains(&code))
        .map(|(f, _)| *f)
}

/// The key with this code, if this keyboard has it.
pub fn find(code: &str) -> Option<&'static Key> {
    layout().iter().find(|k| k.code == code)
}

/// The home key of a finger.
pub fn home_key(finger: Finger) -> &'static Key {
    let code = HOME
        .iter()
        .find(|(f, _)| *f == finger)
        .map_or("Space", |h| h.1);
    find(code).unwrap_or(&layout()[0])
}

/// Where the keyboard's front edge is (z).
pub fn front() -> f32 {
    KEYBOARD_Z + LAYOUT_D * KEY_U / 2.0 + KEYBOARD_RIM
}

/// How high the keyboard has risen this far back from its front.
pub fn rise(z: f32) -> f32 {
    (front() - z).max(0.0) * KEYBOARD_SLOPE
}

/// A point of the layout (units from its top left) on the desk, at the
/// height of the keytops there (at rest).
pub fn at(x: f32, y: f32) -> V3 {
    let wx = KEYBOARD_X - LAYOUT_W * KEY_U / 2.0 + x * KEY_U;
    let wz = KEYBOARD_Z - LAYOUT_D * KEY_U / 2.0 + y * KEY_U;
    [wx, cap_foot(wz) + CAP_H, wz]
}

/// The foot of a keycap at rest, this far back (y).
pub fn cap_foot(z: f32) -> f32 {
    DESK_TOP + PLATE + 0.0045 + rise(z)
}

impl Key {
    /// The middle of its top, at rest.
    pub fn top(&self) -> V3 {
        at(self.x + self.w / 2.0, self.y + 0.5)
    }

    /// Where a finger presses it: the middle, or for the space bar, where
    /// that thumb rests (toward its front edge).
    pub fn spot(&self, finger: Finger) -> V3 {
        if self.code == "Space" && finger.digit == THUMB {
            let k = THUMB_ON_SPACE[(finger.side == Side::Right) as usize];
            at(self.x + self.w * k, self.y + 0.75)
        } else {
            self.top()
        }
    }
}

/// The key that types a character on a US layout, and whether Shift is
/// held for it.
pub fn for_char(c: char) -> Option<(&'static str, bool)> {
    const SHIFTED: [(char, &str); 21] = [
        ('~', "Backquote"),
        ('!', "Digit1"),
        ('@', "Digit2"),
        ('#', "Digit3"),
        ('$', "Digit4"),
        ('%', "Digit5"),
        ('^', "Digit6"),
        ('&', "Digit7"),
        ('*', "Digit8"),
        ('(', "Digit9"),
        (')', "Digit0"),
        ('_', "Minus"),
        ('+', "Equal"),
        ('{', "BracketLeft"),
        ('}', "BracketRight"),
        ('|', "Backslash"),
        (':', "Semicolon"),
        ('"', "Quote"),
        ('<', "Comma"),
        ('>', "Period"),
        ('?', "Slash"),
    ];
    if let Some((_, code)) = SHIFTED.iter().find(|(k, _)| *k == c) {
        return Some((code, true));
    }
    let plain = layout().iter().find(|k| {
        k.legend.len() == 1
            && k.legend.starts_with(c.to_ascii_uppercase())
            && !k.code.starts_with("Arrow")
    });
    match c {
        ' ' => Some(("Space", false)),
        '\n' => Some(("Enter", false)),
        '\t' => Some(("Tab", false)),
        'A'..='Z' => plain.map(|k| (k.code, true)),
        _ => plain.map(|k| (k.code, false)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tenkeyless_board_has_87_keys_each_once() {
        let keys = layout();
        assert_eq!(keys.len(), 87);
        for (i, a) in keys.iter().enumerate() {
            assert!(
                keys[i + 1..].iter().all(|b| b.code != a.code),
                "{} twice",
                a.code
            );
        }
    }

    #[test]
    fn rows_fit_the_layout_and_keys_never_overlap() {
        let keys = layout();
        for a in keys {
            assert!(a.x + a.w <= LAYOUT_W + 1e-4, "{} past the edge", a.code);
            assert!(a.y + 1.0 <= LAYOUT_D + 1e-4, "{} past the back", a.code);
            for b in keys {
                if a.code != b.code && a.y == b.y {
                    let apart = a.x + a.w <= b.x + 1e-4 || b.x + b.w <= a.x + 1e-4;
                    assert!(apart, "{} and {} overlap", a.code, b.code);
                }
            }
        }
        // The main block is 15 units wide in every row.
        for row in [1.5, 2.5, 3.5, 4.5, 5.5] {
            let end = keys
                .iter()
                .filter(|k| k.y == row && k.x < 15.0)
                .map(|k| k.x + k.w)
                .fold(0.0, f32::max);
            assert!((end - 15.0).abs() < 1e-4, "row {row} ends at {end}");
        }
    }

    #[test]
    fn fingers_type_from_their_own_side() {
        for k in layout() {
            let mid = k.x + k.w / 2.0;
            match k.finger.side {
                Side::Left => assert!(mid < 7.5, "{} by the left hand at {mid}", k.code),
                Side::Right => assert!(mid > 5.5, "{} by the right hand at {mid}", k.code),
            }
        }
        assert_eq!(find("F12").unwrap().finger, f(Side::Right, LITTLE));
        assert_eq!(find("Escape").unwrap().finger, f(Side::Left, LITTLE));
        assert_eq!(find("ArrowLeft").unwrap().finger, f(Side::Right, INDEX));
        assert_eq!(find("ArrowUp").unwrap().finger, f(Side::Right, MIDDLE));
        for (finger, code) in HOME {
            let k = find(code).unwrap();
            if finger.digit != THUMB {
                assert_eq!(k.finger, finger, "{code} is its own finger's home");
            }
        }
    }

    #[test]
    fn every_printable_character_has_its_key() {
        for c in (32u8..127).map(char::from) {
            let (code, _) = for_char(c).unwrap_or_else(|| panic!("no key for {c:?}"));
            assert!(find(code).is_some(), "{c:?} -> {code}");
        }
        assert_eq!(for_char('a'), Some(("KeyA", false)));
        assert_eq!(for_char('A'), Some(("KeyA", true)));
        assert_eq!(for_char('?'), Some(("Slash", true)));
        assert_eq!(for_char('/'), Some(("Slash", false)));
        assert_eq!(for_char('7'), Some(("Digit7", false)));
    }

    #[test]
    fn the_board_rises_toward_the_back() {
        let near = find("Space").unwrap().top();
        let far = find("F5").unwrap().top();
        assert!(far[1] > near[1] && far[2] < near[2]);
        assert!(near[1] > DESK_TOP + 0.02 && far[1] < DESK_TOP + 0.05);
    }
}
