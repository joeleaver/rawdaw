//! Kick voice.
//!
//! Pitched sine with a dramatic pitch drop on each hit. The pitch
//! envelope falls from `KICK_START_HZ` to `KICK_END_HZ` over
//! `KICK_PITCH_DECAY_S`; the amp envelope is a sharp attack into a
//! medium-length release that decays to silence.
//!
//! Patch values are hardcoded for v0; the round-3 drum-kit format
//! will expose them as per-kit / per-voice parameters.

use core::f32::consts::TAU;

use rawdaw_dsp::{Adsr, AdsrParams, PitchEnvelope, Voice};

const KICK_START_HZ: f32 = 110.0;
const KICK_END_HZ: f32 = 45.0;
const KICK_PITCH_DECAY_S: f32 = 0.060;
/// Sharp attack so the hit is immediate; the amp curve below is
/// effectively decay-only.
const KICK_ATTACK_S: f32 = 0.001;
/// Drum amp envelopes don't sustain; the "sustain level" is 0 so
/// after decay the voice idles into the release. Decay = the
/// audible body length.
const KICK_DECAY_S: f32 = 0.250;
const KICK_SUSTAIN: f32 = 0.0;
const KICK_RELEASE_S: f32 = 0.020;

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

    /// Configure sample rate and load the hardcoded patch. Called
    /// once per voice when the node is `prepare`d.
    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate as f32;
        self.pitch_env.prepare(sample_rate);
        self.pitch_env
            .set_shape(KICK_START_HZ, KICK_END_HZ, KICK_PITCH_DECAY_S);
        self.amp_env.prepare(sample_rate);
        self.amp_env.set_params(AdsrParams {
            attack_s: KICK_ATTACK_S,
            decay_s: KICK_DECAY_S,
            sustain_level: KICK_SUSTAIN,
            release_s: KICK_RELEASE_S,
        });
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
