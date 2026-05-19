//! Snare voice.
//!
//! A tonal body (sine with fast pitch drop) mixed with a noise burst
//! (filtered to emphasize the snare-wire register). The body gives
//! the "thwack" of the drumhead; the noise gives the "tssss" of the
//! wires. Patch values live in
//! [`SnarePatch`](crate::patch::SnarePatch).
//!
//! The noise sample is supplied by the parent `DrumSynthNode`'s
//! shared `NoiseSource` — passed in to `tick` so simultaneous snare
//! hits don't each pull from their own private streams.

use core::f32::consts::TAU;

use rawdaw_dsp::{Adsr, PitchEnvelope, SvfHighpass, Voice};

use crate::patch::SnarePatch;

#[derive(Debug, Clone, Copy)]
pub struct SnareVoice {
    note: u8,
    phase: f32,
    sample_rate: f32,
    velocity_amp: f32,
    pitch_env: PitchEnvelope,
    amp_env: Adsr,
    noise_hp: SvfHighpass,
    /// Crossfade between body sine (0.0) and filtered noise (1.0).
    /// Copied from the patch at `prepare`.
    noise_mix: f32,
}

impl SnareVoice {
    pub fn new() -> Self {
        Self {
            note: 0,
            phase: 0.0,
            sample_rate: 0.0,
            velocity_amp: 0.0,
            pitch_env: PitchEnvelope::new(),
            amp_env: Adsr::new(),
            noise_hp: SvfHighpass::new(),
            noise_mix: 0.0,
        }
    }

    pub fn prepare(&mut self, sample_rate: u32, patch: &SnarePatch) {
        self.sample_rate = sample_rate as f32;
        self.pitch_env.prepare(sample_rate);
        self.amp_env.prepare(sample_rate);
        self.noise_hp.prepare(sample_rate);
        self.set_patch(patch);
    }

    /// Install a runtime snare patch without touching sample-rate
    /// state.
    pub fn set_patch(&mut self, patch: &SnarePatch) {
        self.pitch_env.set_shape(
            patch.body_start_hz,
            patch.body_end_hz,
            patch.body_pitch_decay_s,
        );
        self.amp_env.set_params(patch.amp);
        self.noise_hp.set_cutoff(patch.noise_hp_hz);
        self.noise_hp.set_resonance(patch.noise_hp_q);
        self.noise_mix = patch.noise_mix;
    }

    /// Tick one sample. `noise_sample` is the next sample from the
    /// node's shared `NoiseSource` — pre-mixed white noise that the
    /// voice's own HP filter will shape.
    pub fn tick(&mut self, noise_sample: f32) -> f32 {
        if self.amp_env.is_idle() {
            return 0.0;
        }
        // Body: pitch-swept sine.
        let hz = self.pitch_env.tick();
        let phase_inc = if self.sample_rate > 0.0 {
            hz / self.sample_rate
        } else {
            0.0
        };
        let body = (self.phase * TAU).sin();
        self.phase += phase_inc;
        if self.phase >= 1.0 || self.phase < 0.0 {
            self.phase = self.phase.rem_euclid(1.0);
        }
        // Noise: high-passed.
        let noise = self.noise_hp.tick(noise_sample);

        let mixed = body * (1.0 - self.noise_mix) + noise * self.noise_mix;
        let amp = self.amp_env.tick();
        mixed * amp * self.velocity_amp
    }
}

impl Voice for SnareVoice {
    fn note(&self) -> u8 {
        self.note
    }

    fn is_active(&self) -> bool {
        !self.amp_env.is_idle()
    }

    fn note_on(&mut self, note: u8, velocity: f32) {
        self.note = note;
        self.velocity_amp = velocity;
        self.phase = 0.0;
        self.noise_hp.reset_state();
        self.pitch_env.note_on();
        self.amp_env.note_on();
    }

    fn note_off(&mut self) {
        // One-shot drum; NoteOff is intentionally ignored.
    }
}

impl Default for SnareVoice {
    fn default() -> Self {
        Self::new()
    }
}
