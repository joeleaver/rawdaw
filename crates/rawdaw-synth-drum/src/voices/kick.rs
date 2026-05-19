//! Kick voice.
//!
//! Pitched sine with a dramatic pitch drop on each hit. The pitch
//! envelope falls from the patch's `start_hz` to `end_hz` over
//! `pitch_decay_s`; the amp envelope is sharp attack + decay-only.
//!
//! Patch values live in [`KickPatch`](crate::patch::KickPatch); the
//! synth node hands the patch to `prepare` at construction time.

use core::f32::consts::TAU;

use rawdaw_dsp::{Adsr, PitchEnvelope, Voice};

use crate::patch::KickPatch;

#[derive(Debug, Clone, Copy)]
pub struct KickVoice {
    note: u8,
    /// Sine phase accumulator in [0, 1).
    phase: f32,
    sample_rate: f32,
    velocity_amp: f32,
    pitch_env: PitchEnvelope,
    amp_env: Adsr,
}

impl KickVoice {
    pub fn new() -> Self {
        Self {
            note: 0,
            phase: 0.0,
            sample_rate: 0.0,
            velocity_amp: 0.0,
            pitch_env: PitchEnvelope::new(),
            amp_env: Adsr::new(),
        }
    }

    /// Configure sample rate + install the runtime kick patch.
    pub fn prepare(&mut self, sample_rate: u32, patch: &KickPatch) {
        self.sample_rate = sample_rate as f32;
        self.pitch_env.prepare(sample_rate);
        self.amp_env.prepare(sample_rate);
        self.set_patch(patch);
    }

    /// Install a runtime kick patch without touching sample-rate
    /// state. Used by [`DrumSynthNode`](crate::DrumSynthNode)'s
    /// `apply_param` to propagate parameter-event mutations.
    pub fn set_patch(&mut self, patch: &KickPatch) {
        self.pitch_env
            .set_shape(patch.start_hz, patch.end_hz, patch.pitch_decay_s);
        self.amp_env.set_params(patch.amp);
    }

    /// Tick one sample. Returns the kick's output for this sample.
    pub fn tick(&mut self) -> f32 {
        if self.amp_env.is_idle() {
            return 0.0;
        }
        let hz = self.pitch_env.tick();
        let phase_inc = if self.sample_rate > 0.0 {
            hz / self.sample_rate
        } else {
            0.0
        };
        let s = (self.phase * TAU).sin();
        self.phase += phase_inc;
        if self.phase >= 1.0 || self.phase < 0.0 {
            self.phase = self.phase.rem_euclid(1.0);
        }
        let amp = self.amp_env.tick();
        s * amp * self.velocity_amp
    }
}

impl Voice for KickVoice {
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
        self.pitch_env.note_on();
        // Patch has sustain_level=0 so the amp env walks Attack →
        // Decay → Idle automatically. No NoteOff required.
        self.amp_env.note_on();
    }

    fn note_off(&mut self) {
        // Drums are one-shot; ignore NoteOff so an early NoteOff
        // doesn't truncate a kick. The amp envelope's Release was
        // armed at note_on time, so the voice decays naturally.
    }
}

impl Default for KickVoice {
    fn default() -> Self {
        Self::new()
    }
}
