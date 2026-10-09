//! Sound out, through Web Audio: one context (browsers keep it asleep
//! until a touch, a click or a key, so call `wake` on each), sounds handed
//! in as samples (`engine::synth` writes them), each played at a loudness,
//! a place left to right and a speed; and sounds played round and round
//! (`hum`), their loudness and place eased as things change (`tune`).
//! Without Web Audio it is silent and nothing else changes.

use web_sys::{AudioBuffer, AudioContext, AudioContextState, GainNode, StereoPannerNode};

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
        let v = if self.muted {
            0.0
        } else {
            volume.clamp(0.0, 2.0)
        };
        let _ = g.gain().set_target_at_time(v, now, 0.25);
        let _ = p.pan().set_target_at_time(pan.clamp(-1.0, 1.0), now, 0.25);
    }

    /// Wake the sound (a browser lets it play only after a person acts).
    pub fn wake(&self) {
        if let Some(c) = &self.ctx {
            if c.state() != AudioContextState::Running {
                let _ = c.resume();
            }
        }
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
        if self.muted || volume < 0.01 {
            return;
        }
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
            g.gain().set_value(volume.min(2.0));
            let p = c.create_stereo_panner().ok()?;
            p.pan().set_value(pan.clamp(-1.0, 1.0));
            src.connect_with_audio_node(&g).ok()?;
            g.connect_with_audio_node(&p).ok()?;
            p.connect_with_audio_node(m).ok()?;
            src.start().ok()
        };
        go();
    }
}
