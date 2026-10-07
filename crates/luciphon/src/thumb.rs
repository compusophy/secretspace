//! What one thumb can do, and in what order: the grammar every Intent must
//! fit, a person's or a resident's alike. A tap is a press and a lift with
//! the stick up; a flick ends a drag; a hold starts with the stick up and
//! roots you (the drag then aims) until it is released or cancelled. An
//! Intent that does not fit is trimmed until it does, so a script gains
//! nothing a thumb could not do.

use crate::motion::{Intent, Verb};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Thumb {
    /// The stick was down last tick.
    stick: bool,
    /// A hold is under way.
    holding: bool,
}

impl Thumb {
    /// The Intent as a thumb could have made it; whether it was changed.
    pub fn fit(&mut self, it: Intent) -> (Intent, bool) {
        let mut out = it;
        if self.holding {
            // The drag aims while charging; it does not run.
            out.throttle = 0;
        }
        let ok = match it.verb {
            Verb::None => true,
            Verb::Tap => !self.stick && !self.holding && it.throttle == 0,
            Verb::Flick => self.stick && !self.holding && it.throttle == 0,
            Verb::Hold { .. } => !self.stick && !self.holding && it.throttle == 0,
            Verb::Release { .. } | Verb::Cancel => self.holding,
        };
        if !ok {
            out.verb = Verb::None;
        }
        match out.verb {
            Verb::Hold { .. } => self.holding = true,
            Verb::Release { .. } | Verb::Cancel => self.holding = false,
            _ => {}
        }
        self.stick = out.throttle > 0;
        (out, out != it)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn i(throttle: u8, verb: Verb) -> Intent {
        Intent {
            throttle,
            verb,
            ..Intent::default()
        }
    }

    #[test]
    fn only_what_a_thumb_can() {
        let mut t = Thumb::default();
        // Running and striking at once: the tap is dropped.
        assert!(!t.fit(i(255, Verb::None)).1);
        assert_eq!(t.fit(i(255, Verb::Tap)).0.verb, Verb::None);
        // A flick ends the drag.
        assert_eq!(t.fit(i(0, Verb::Flick)).0.verb, Verb::Flick);
        // A flick with no drag before it is nothing.
        assert_eq!(t.fit(i(0, Verb::Flick)).0.verb, Verb::None);
        // A tap with the stick up is a strike.
        assert_eq!(t.fit(i(0, Verb::Tap)).0.verb, Verb::Tap);
        // A hold roots: running while charging is not allowed.
        assert_eq!(
            t.fit(i(0, Verb::Hold { held_for: 9 })).0.verb,
            Verb::Hold { held_for: 9 }
        );
        assert_eq!(t.fit(i(255, Verb::None)).0.throttle, 0);
        assert_eq!(t.fit(i(0, Verb::Tap)).0.verb, Verb::None);
        assert_eq!(
            t.fit(i(0, Verb::Release { range: 0 })).0.verb,
            Verb::Release { range: 0 }
        );
        assert_eq!(t.fit(i(0, Verb::Cancel)).0.verb, Verb::None);
    }
}
