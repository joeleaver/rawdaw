//! Hi-hat voice (closed + open).
//!
//! High-passed white noise with a short amp envelope. The closed-hat
//! patch uses a fast decay (~30 ms); the open-hat patch uses a much
//! longer decay (~300 ms) to give the "tsssss" sustain. Both share
//! the same HP filter shape. The synth node installs the right sub-
//! patch via [`HatVoice::set_patch`] before triggering each note.

use rawdaw_dsp::{Adsr, SvfHighpass, Voice};

use crate::patch::HatPatch;

#[derive(Debug, Clone, Copy)]
pub struct HatVoice {
    note: u8,
    sample_rate: f32,
    velocity_amp: f32,
    amp_env: Adsr,
    noise_hp: SvfHighpass,
}

impl HatVoice {
    pub fn new() -> Self {
        Self {
            note: 0,
            sample_rate: 0.0,
            velocity_amp: 0.0,
            amp_env: Adsr::new(),
            noise_hp: SvfHighpass::new(),
        }
    }

    /// Configure sample rate + install a default patch. The drum
    /// node passes its `closed_hat` patch at prepare time; per-note
    /// `set_patch` swaps in `open_hat` when an open-hat MIDI note
    /// arrives.
    pub fn prepare(&mut self, sample_rate: u32, patch: &HatPatch) {
        self.sample_rate = sample_rate as f32;
        self.amp_env.prepare(sample_rate);
        self.noise_hp.prepare(sample_rate);
        self.set_patch(patch);
    }

    /// Install a hat sub-patch (closed or open). Called by the parent
    /// node based on the incoming MIDI note before triggering
    /// `note_on`. Setting params mid-tick is safe — `Adsr::set_params`
    /// installs the new ramps for subsequent samples without
    /// resetting envelope state.
    pub fn set_patch(&mut self, patch: &HatPatch) {
        self.amp_env.set_params(patch.amp);
        self.noise_hp.set_cutoff(patch.hp_hz);
        self.noise_hp.set_resonance(patch.hp_q);
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
