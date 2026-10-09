//! What a player sets, kept between visits: how fast the view turns
//! (mouse and fingers), how loud it all is, and the picture (auto: as the
//! machine manages, stepping down when frames run slow; or a tier held).

use render::Quality;

const KEY: &str = "wandfall.settings";

/// The picture's choices, as the menu shows them.
pub const PICTURES: [&str; 4] = ["auto", "low", "mid", "high"];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Times the view's usual turning speed.
    pub look: f32,
    /// The sound's loudness, 0 to 1.
    pub volume: f32,
    /// Which of `PICTURES` (0: auto).
    pub picture: u8,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            look: 1.0,
            volume: 0.7,
            picture: 0,
        }
    }
}

impl Settings {
    /// As saved ("look volume picture"), or as they come.
    pub fn load() -> Settings {
        kit::load(KEY)
            .and_then(|s| Settings::read(&s))
            .unwrap_or_default()
    }

    fn read(s: &str) -> Option<Settings> {
        let mut it = s.split(' ');
        let look: f32 = it.next()?.parse().ok()?;
        let volume: f32 = it.next()?.parse().ok()?;
        let picture: u8 = it.next()?.parse().ok()?;
        let d = Settings::default();
        Some(Settings {
            look: if look.is_finite() {
                look.clamp(0.25, 4.0)
            } else {
                d.look
            },
            volume: if volume.is_finite() {
                volume.clamp(0.0, 1.0)
            } else {
                d.volume
            },
            picture: picture.min(PICTURES.len() as u8 - 1),
        })
    }

    pub fn save(&self) {
        kit::save(
            KEY,
            &format!("{:.3} {:.2} {}", self.look, self.volume, self.picture),
        );
    }

    /// A step faster (or slower, `d` below 0).
    pub fn turn(&mut self, d: i8) {
        self.look = (self.look * 1.15f32.powi(d as i32)).clamp(0.25, 4.0);
    }

    /// A step louder (or quieter).
    pub fn loud(&mut self, d: i8) {
        self.volume = ((self.volume * 10.0).round() + d as f32).clamp(0.0, 10.0) / 10.0;
    }

    /// The picture to start with: the address's `?q=` if it says, the
    /// tier held if one is, or what suits the machine.
    pub fn quality(&self, search: &str, software: bool, touch: bool) -> Quality {
        let held = PICTURES
            .get(self.picture as usize)
            .filter(|_| self.picture > 0);
        match held {
            Some(name) if !search.contains("q=") => {
                let tier = if *name == "mid" { "medium" } else { name };
                Quality::pick(&format!("q={tier}&{search}"), software, touch)
            }
            _ => Quality::pick(search, software, touch),
        }
    }

    /// Whether the picture steps down by itself when frames run slow.
    pub fn auto(&self) -> bool {
        self.picture == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_read_back_and_hold_their_bounds() {
        let s = Settings {
            look: 1.5,
            volume: 0.4,
            picture: 2,
        };
        let back = Settings::read(&format!("{:.3} {:.2} {}", s.look, s.volume, s.picture));
        assert_eq!(back, Some(s));
        let wild = Settings::read("99 -3 9").unwrap();
        assert_eq!((wild.look, wild.volume, wild.picture), (4.0, 0.0, 3));
        assert_eq!(Settings::read("NaN 0.5 1").unwrap().look, 1.0);
        assert!(Settings::read("nonsense").is_none());
        let mut t = Settings::default();
        for _ in 0..30 {
            t.loud(1);
            t.turn(-1);
        }
        assert_eq!(t.volume, 1.0);
        assert_eq!(t.look, 0.25);
        let held = Settings {
            picture: 1,
            ..Settings::default()
        };
        assert_eq!(held.quality("", false, false), Quality::LOW);
        assert_eq!(held.quality("?q=high", false, false), Quality::HIGH);
    }
}
