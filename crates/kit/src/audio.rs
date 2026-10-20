//! Sound out, through Web Audio: one context (browsers keep it asleep
//! until a touch, a click or a key; it wakes itself on them, inside the
//! very event, which is the only time an iPhone allows it), sounds handed
//! in as samples (`engine::synth` writes them), each played at a loudness,
//! a place left to right and a speed; and sounds played round and round
//! (`hum`), their loudness and place eased as things change (`tune`).
//! Without Web Audio it is silent and nothing else changes; a loudness,
//! place or speed that is not a number is not played. Hosted (a cartridge),
//! it listens on nothing (the host's events wake it, `wake`), and is
//! silent while the host is not looking (`host::drawing`).

use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use web_sys::{AudioBuffer, AudioContext, AudioContextState, GainNode, StereoPannerNode};

/// A sound's loudness (at most 2), place (-1 to 1) and speed as played,
/// or None if it is not to be (too quiet, or not numbers).
fn heard(volume: f32, pan: f32, speed: f32) -> Option<(f32, f32, f32)> {
    let numbers = volume.is_finite() && pan.is_finite() && speed.is_finite();
    (numbers && volume >= 0.01 && speed > 0.0)
        .then(|| (volume.min(2.0), pan.clamp(-1.0, 1.0), speed))
}

/// Wake `ctx` whenever a person acts, inside the event (capture: before
/// anything on the page can stop it).
fn wake_on_touch(ctx: &AudioContext) {
    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_capture(true);
    for event in ["pointerup", "touchend", "click", "keydown"] {
        let ctx = ctx.clone();
        let wake = Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
            if ctx.state() != AudioContextState::Running {
                let _ = ctx.resume();
            }
        });
        let _ = crate::window().add_event_listener_with_callback_and_add_event_listener_options(
            event,
            wake.as_ref().unchecked_ref(),
            &opts,
        );
        wake.forget();
    }
}

/// Hosted and not being looked at (hidden, or not polled): no sound.
fn quiet() -> bool {
    crate::host::hosted() && !crate::host::drawing(crate::now())
}

pub struct Audio {
    ctx: Option<AudioContext>,
    master: Option<GainNode>,
    sounds: Vec<Option<AudioBuffer>>,
    /// Sounds going round and round: each one's loudness and place.
    hums: Vec<Option<(GainNode, StereoPannerNode)>>,
    pub muted: bool,
}

impl Default for Audio {
    fn default() -> Audio {
        Audio::new()
    }
}

impl Audio {
    pub fn new() -> Audio {
        let ctx = AudioContext::new().ok();
        if let Some(c) = ctx.as_ref().filter(|_| !crate::host::hosted()) {
            wake_on_touch(c);
        }
        let master = ctx.as_ref().and_then(|c| {
            let g = c.create_gain().ok()?;
            g.gain().set_value(0.7);
            g.connect_with_audio_node(&c.destination()).ok()?;
            Some(g)
        });
        Audio {
            ctx,
            master,
            sounds: Vec::new(),
            hums: Vec::new(),
            muted: false,
        }
    }

    /// How loud everything is, 0 to 1.
    pub fn set_volume(&self, v: f32) {
        if let Some(m) = &self.master {
            m.gain().set_value(if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            });
        }
    }

    /// Sound `id` played round and round, silent till `tune`d: its number.
    pub fn hum(&mut self, id: usize) -> usize {
        let h = (|| {
            let (c, m) = (self.ctx.as_ref()?, self.master.as_ref()?);
            let b = self.sounds.get(id)?.as_ref()?;
            let src = c.create_buffer_source().ok()?;
            src.set_buffer(Some(b));
            src.set_loop(true);
            let g = c.create_gain().ok()?;
            g.gain().set_value(0.0);
            let p = c.create_stereo_panner().ok()?;
            src.connect_with_audio_node(&g).ok()?;
            g.connect_with_audio_node(&p).ok()?;
            p.connect_with_audio_node(m).ok()?;
            src.start().ok()?;
            Some((g, p))
        })();
        self.hums.push(h);
        self.hums.len() - 1
    }

    /// A hum's loudness and place (-1 left to 1 right), eased toward them
    /// over a moment (silent when muted).
    pub fn tune(&self, hum: usize, volume: f32, pan: f32) {
        let (Some(c), Some(Some((g, p)))) = (&self.ctx, self.hums.get(hum)) else {
            return;
        };
        let now = c.current_time();
        let (v, pan) = match heard(volume, pan, 1.0) {
            Some((v, pan, _)) if !self.muted && !quiet() => (v, pan),
            _ => (
                0.0,
                if pan.is_finite() {
                    pan.clamp(-1.0, 1.0)
                } else {
                    0.0
                },
            ),
        };
        let _ = g.gain().set_target_at_time(v, now, 0.25);
        let _ = p.pan().set_target_at_time(pan, now, 0.25);
    }

    /// Wake the sound now (after an interruption: a call, say); it wakes
    /// itself whenever a person acts.
    pub fn wake(&self) {
        if let Some(c) = &self.ctx {
            if c.state() != AudioContextState::Running {
                let _ = c.resume();
            }
        }
    }

    /// Done with sound: the context closed (a browser allows only so many),
    /// every sound let go; silent from now on.
    pub fn close(&mut self) {
        if let Some(c) = self.ctx.take() {
            let _ = c.close();
        }
        self.master = None;
        self.sounds.clear();
        self.hums.clear();
    }

    /// A sound to play later, `rate` samples a second: its number.
    pub fn add(&mut self, samples: &[f32], rate: u32) -> usize {
        let b = self.ctx.as_ref().and_then(|c| {
            let b = c
                .create_buffer(1, samples.len().max(1) as u32, rate as f32)
                .ok()?;
            b.copy_to_channel(samples, 0).ok()?;
            Some(b)
        });
        self.sounds.push(b);
        self.sounds.len() - 1
    }

    /// Play sound `id` at `volume` (1 as written), `pan` (-1 left to 1
    /// right) and `speed` (1 as written; faster is higher).
    pub fn play(&self, id: usize, volume: f32, pan: f32, speed: f32) {
        let Some((volume, pan, speed)) =
            heard(volume, pan, speed).filter(|_| !self.muted && !quiet())
        else {
            return;
        };
        let (Some(c), Some(m), Some(Some(b))) = (&self.ctx, &self.master, self.sounds.get(id))
        else {
            return;
        };
        if c.state() != AudioContextState::Running {
            return;
        }
        let go = || -> Option<()> {
            let src = c.create_buffer_source().ok()?;
            src.set_buffer(Some(b));
            src.playback_rate().set_value(speed);
            let g = c.create_gain().ok()?;
            g.gain().set_value(volume);
            let p = c.create_stereo_panner().ok()?;
            p.pan().set_value(pan);
            src.connect_with_audio_node(&g).ok()?;
            g.connect_with_audio_node(&p).ok()?;
            p.connect_with_audio_node(m).ok()?;
            src.start().ok()
        };
        go();
    }
}

#[cfg(test)]
mod tests {
    use super::heard;

    #[test]
    fn what_is_not_a_number_is_not_played() {
        assert_eq!(heard(0.5, -3.0, 1.2), Some((0.5, -1.0, 1.2)));
        assert_eq!(heard(5.0, 0.0, 1.0), Some((2.0, 0.0, 1.0)));
        assert_eq!(heard(0.001, 0.0, 1.0), None);
        // AudioParam throws on these, and the frame dies with it.
        assert_eq!(heard(f32::NAN, 0.0, 1.0), None);
        assert_eq!(heard(0.5, f32::NAN, 1.0), None);
        assert_eq!(heard(0.5, 0.0, f32::INFINITY), None);
        assert_eq!(heard(f32::INFINITY, 0.0, 1.0), None);
        assert_eq!(heard(0.5, 0.0, 0.0), None);
    }
}
