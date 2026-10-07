//! The grammar every Intent must fit, a person's or a resident's alike: a
//! charge begins only when none is under way, and only a charge can be
//! released or cancelled; strikes and dashes may come while moving. An
//! Intent that does not fit is trimmed until it does, so a script gains
//! nothing a person could not do.

use crate::motion::{Intent, Verb};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Thumb {
    /// A hold is under way.
    holding: bool,
}

impl Thumb {
    /// The Intent as a person could have made it; whether it was changed.
    pub fn fit(&mut self, it: Intent) -> (Intent, bool) {
        let mut out = it;
        let ok = match it.verb {
            Verb::None => true,
            Verb::Tap | Verb::Flick | Verb::Hold { .. } => !self.holding,
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
    fn only_what_a_person_can() {
        let mut t = Thumb::default();
        // Running and striking at once, and dashing.
        assert!(!t.fit(i(255, Verb::Tap)).1);
        assert!(!t.fit(i(255, Verb::Flick)).1);
        // A release with no charge is nothing.
        assert_eq!(t.fit(i(0, Verb::Release { range: 0 })).0.verb, Verb::None);
        // A charge, walking: no strike or second charge until it ends.
        assert!(!t.fit(i(100, Verb::Hold { held_for: 0 })).1);
        assert_eq!(t.fit(i(100, Verb::Tap)).0.verb, Verb::None);
        assert_eq!(t.fit(i(0, Verb::Hold { held_for: 3 })).0.verb, Verb::None);
        assert!(!t.fit(i(0, Verb::Release { range: 0 })).1);
        assert_eq!(t.fit(i(0, Verb::Cancel)).0.verb, Verb::None);
    }
}
