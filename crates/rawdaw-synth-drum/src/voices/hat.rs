//! Hi-hat voice (closed + open).
//!
//! High-passed white noise with a short amp envelope. The closed-hat
//! patch uses a fast decay (~30 ms); the open-hat patch uses a much
//! longer decay (~300 ms) to give the "tsssss" sustain. Both share
//! the same HP filter shape.

use rawdaw_dsp::{Adsr, AdsrParams, SvfHighpass, Voice};

const HAT_HP_HZ: f32 = 6000.0;
const HAT_HP_Q: f32 = 0.7;
const HAT_ATTACK_S: f32 = 0.0005;
const HAT_CLOSED_DECAY_S: f32 = 0.040;
const HAT_OPEN_DECAY_S: f32 = 0.300;
const HAT_SUSTAIN: f32 = 0.0;
const HAT_RELEASE_S: f32 = 0.020;

/// Distinguishes closed-vs-open at note_on time so the same voice
/// type covers both MIDI mappings (42 vs 46) — saves an extra pool
/// in `DrumSynthNode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HatStyle {
    Closed,
    Open,
}

#[derive(Debug, Clone, Copy)]
pub struct HatVoice {
    note: u8,
    sample_rate: f32,
    velocity_amp: f32,
    amp_env: Adsr,
    noise_hp: SvfHighpass,
    style: HatStyle,
}

impl HatVoice {
    pub fn new() -> Self {
        Self {
            note: 0,
            sample_rate: 0.0,
            velocity_amp: 0.0,
            amp_env: Adsr::new(),
            noise_hp: SvfHighpass::new(),
            style: HatStyle::Closed,
        }
    }

    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate as f32;
        self.amp_env.prepare(sample_rate);
        // Default to closed shape; `set_style` swaps it.
        self.set_style(HatStyle::Closed);
        self.noise_hp.prepare(sample_rate);
        self.noise_hp.set_cutoff(HAT_HP_HZ);
        self.noise_hp.set_resonance(HAT_HP_Q);
    }

    /// Choose closed-vs-open shape. Called by the parent node based
    /// on the MIDI note before triggering `note_on`.
    pub fn set_style(&mut self, style: HatStyle) {
        self.style = style;
        let decay_s = match style {
            HatStyle::Closed => HAT_CLOSED_DECAY_S,
            HatStyle::Open => HAT_OPEN_DECAY_S,
        };
        self.amp_env.set_params(AdsrParams {
            attack_s: HAT_ATTACK_S,
            decay_s,
            sustain_level: HAT_SUSTAIN,
            release_s: HAT_RELEASE_S,
        });
    }

    /// Tick one sample with the shared noise input.
    pub fn tick(&mut self, noise_sample: f32) -> f32 {
        if self.amp_env.is_idle() {
            return 0.0;
        }
        let filtered = self.noise_hp.tick(noise_sample);
        let amp = self.amp_env.tick();
        filtered * amp * self.velocity_amp
    }
}

impl Voice for HatVoice {
    fn note(&self) -> u8 {
        self.note
    }

    fn is_active(&self) -> bool {
        !self.amp_env.is_idle()
    }

    fn note_on(&mut self, note: u8, velocity: f32) {
        self.note = note;
        self.velocity_amp = velocity;
        self.noise_hp.reset_state();
        self.amp_env.note_on();
    }

    fn note_off(&mut self) {
        // One-shot.
    }
}

impl Default for HatVoice {
    fn default() -> Self {
        Self::new()
    }
}
