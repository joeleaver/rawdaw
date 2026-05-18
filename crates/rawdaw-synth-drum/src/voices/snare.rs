//! Snare voice.
//!
//! A tonal body (sine with fast pitch drop) mixed with a noise burst
//! (filtered to emphasize the snare-wire register). The body gives
//! the "thwack" of the drumhead; the noise gives the "tssss" of the
//! wires.
//!
//! The noise sample is supplied by the parent `DrumSynthNode`'s
//! shared `NoiseSource` — passed in to `tick` so simultaneous snare
//! hits don't each pull from their own private streams (the v0
//! perceptual difference is nil, and shared noise costs nothing).

use core::f32::consts::TAU;

use rawdaw_dsp::{Adsr, AdsrParams, PitchEnvelope, SvfHighpass, Voice};

const SNARE_BODY_START_HZ: f32 = 240.0;
const SNARE_BODY_END_HZ: f32 = 130.0;
const SNARE_BODY_PITCH_DECAY_S: f32 = 0.030;
const SNARE_ATTACK_S: f32 = 0.001;
const SNARE_DECAY_S: f32 = 0.140;
const SNARE_SUSTAIN: f32 = 0.0;
const SNARE_RELEASE_S: f32 = 0.020;

/// Mix between body sine and noise. 0 = pure body, 1 = pure noise.
/// 0.7 gives a snare-like sound with the noise dominant.
const SNARE_NOISE_MIX: f32 = 0.7;
/// Noise is high-passed so the bottom end doesn't muddy the body.
const SNARE_NOISE_HP_HZ: f32 = 1500.0;

#[derive(Debug, Clone, Copy)]
pub struct SnareVoice {
    note: u8,
    phase: f32,
    sample_rate: f32,
    velocity_amp: f32,
    pitch_env: PitchEnvelope,
    amp_env: Adsr,
    noise_hp: SvfHighpass,
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
        }
    }

    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate as f32;
        self.pitch_env.prepare(sample_rate);
        self.pitch_env.set_shape(
            SNARE_BODY_START_HZ,
            SNARE_BODY_END_HZ,
            SNARE_BODY_PITCH_DECAY_S,
        );
        self.amp_env.prepare(sample_rate);
        self.amp_env.set_params(AdsrParams {
            attack_s: SNARE_ATTACK_S,
            decay_s: SNARE_DECAY_S,
            sustain_level: SNARE_SUSTAIN,
            release_s: SNARE_RELEASE_S,
        });
        self.noise_hp.prepare(sample_rate);
        self.noise_hp.set_cutoff(SNARE_NOISE_HP_HZ);
        self.noise_hp.set_resonance(0.7);
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

        let mixed = body * (1.0 - SNARE_NOISE_MIX) + noise * SNARE_NOISE_MIX;
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
