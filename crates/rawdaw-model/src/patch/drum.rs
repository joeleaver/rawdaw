//! Serialized drum-synth patch types.
//!
//! Mirror of the per-voice constants in `rawdaw-synth-drum`. As with
//! the wavetable patch, model-layer mirrors use only model primitives
//! so `rawdaw-model` stays free of any `rawdaw-dsp` dependency. The
//! `From` impls that convert these into the runtime drum-voice types
//! land in the synth crate during U3.
//!
//! ## Default
//!
//! [`DrumPatchData::default()`] returns the v0 drum patch — values are
//! bit-equal to the hardcoded constants in
//! `crates/rawdaw-synth-drum/src/voices/{kick,snare,hat}.rs`. U3 will
//! lift those constants into reads from this default and verify audio
//! byte-identity.

use serde::{Deserialize, Serialize};

use super::wavetable::AdsrParamsData;

/// Current drum-patch format version.
pub const DRUM_PATCH_FORMAT_VERSION: u32 = 1;

/// Kick-drum patch: pitch-swept sine + amp envelope.
///
/// Mirror of the constants in
/// `crates/rawdaw-synth-drum/src/voices/kick.rs`. `start_hz` / `end_hz`
/// bound the pitch envelope's exponential decay; `pitch_decay_s` is
/// the time constant. `amp.sustain_level = 0` so the AD shape runs
/// once per hit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct KickPatchData {
    pub start_hz: f32,
    pub end_hz: f32,
    pub pitch_decay_s: f32,
    pub amp: AdsrParamsData,
}

impl Default for KickPatchData {
    fn default() -> Self {
        Self {
            start_hz: 110.0,
            end_hz: 45.0,
            pitch_decay_s: 0.060,
            amp: AdsrParamsData {
                attack_s: 0.001,
                decay_s: 0.250,
                sustain_level: 0.0,
                release_s: 0.020,
            },
        }
    }
}

/// Snare patch: pitched body + high-passed noise.
///
/// Mirror of the constants in
/// `crates/rawdaw-synth-drum/src/voices/snare.rs`. The body has its own
/// pitch envelope (a faster, narrower drop than the kick); `noise_mix`
/// crossfades between body (0.0) and noise (1.0). `noise_hp_hz` /
/// `noise_hp_q` shape the high-pass on the noise stream so the bottom
/// end doesn't muddy the body.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SnarePatchData {
    pub body_start_hz: f32,
    pub body_end_hz: f32,
    pub body_pitch_decay_s: f32,
    pub noise_mix: f32,
    pub noise_hp_hz: f32,
    pub noise_hp_q: f32,
    pub amp: AdsrParamsData,
}

impl Default for SnarePatchData {
    fn default() -> Self {
        Self {
            body_start_hz: 240.0,
            body_end_hz: 130.0,
            body_pitch_decay_s: 0.030,
            noise_mix: 0.7,
            noise_hp_hz: 1500.0,
            noise_hp_q: 0.7,
            amp: AdsrParamsData {
                attack_s: 0.001,
                decay_s: 0.140,
                sustain_level: 0.0,
                release_s: 0.020,
            },
        }
    }
}

/// Hi-hat patch: high-passed noise + amp envelope.
///
/// Mirror of `crates/rawdaw-synth-drum/src/voices/hat.rs`. The closed
/// and open hats share this shape; only the `amp.decay_s` differs
/// between them (40 ms vs 300 ms in the v0 defaults).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HatPatchData {
    pub hp_hz: f32,
    pub hp_q: f32,
    pub amp: AdsrParamsData,
}

impl HatPatchData {
    /// Default closed-hat shape (short ~40 ms decay).
    pub fn default_closed() -> Self {
        Self {
            hp_hz: 6000.0,
            hp_q: 0.7,
            amp: AdsrParamsData {
                attack_s: 0.0005,
                decay_s: 0.040,
                sustain_level: 0.0,
                release_s: 0.020,
            },
        }
    }

    /// Default open-hat shape (long ~300 ms decay).
    pub fn default_open() -> Self {
        Self {
            hp_hz: 6000.0,
            hp_q: 0.7,
            amp: AdsrParamsData {
                attack_s: 0.0005,
                decay_s: 0.300,
                sustain_level: 0.0,
                release_s: 0.020,
            },
        }
    }
}

impl Default for HatPatchData {
    fn default() -> Self {
        // Arbitrary pick — concrete patches should select closed or
        // open explicitly. `default_closed` is the more common case
        // (a hit per quarter note) so we use that.
        Self::default_closed()
    }
}

/// Drum-synth patch: one sub-patch per voice type.
///
/// Today the drum synth has four voice slots (Kick, Snare, ClosedHat,
/// OpenHat) and a GM classifier maps MIDI notes (36/38/42/46) to them.
/// v1+ growth (more voices, kit-loader format) will extend this struct
/// or move to a map; the explicit struct shape is fine while the voice
/// inventory stays small and stable.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DrumPatchData {
    pub format_version: u32,
    pub kick: KickPatchData,
    pub snare: SnarePatchData,
    pub closed_hat: HatPatchData,
    pub open_hat: HatPatchData,
}

impl Default for DrumPatchData {
    fn default() -> Self {
        Self {
            format_version: DRUM_PATCH_FORMAT_VERSION,
            kick: KickPatchData::default(),
            snare: SnarePatchData::default(),
            closed_hat: HatPatchData::default_closed(),
            open_hat: HatPatchData::default_open(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_drum_patch_pins_v0_constants() {
        let p = DrumPatchData::default();

        assert_eq!(p.format_version, 1);

        assert_eq!(p.kick.start_hz, 110.0);
        assert_eq!(p.kick.end_hz, 45.0);
        assert_eq!(p.kick.pitch_decay_s, 0.060);
        assert_eq!(p.kick.amp.decay_s, 0.250);

        assert_eq!(p.snare.body_start_hz, 240.0);
        assert_eq!(p.snare.body_end_hz, 130.0);
        assert_eq!(p.snare.noise_mix, 0.7);
        assert_eq!(p.snare.noise_hp_hz, 1500.0);

        // Closed-hat is the short one (40 ms), open-hat is the long
        // one (300 ms). The asymmetric default selects them
        // explicitly via `default_closed` / `default_open`.
        assert_eq!(p.closed_hat.amp.decay_s, 0.040);
        assert_eq!(p.open_hat.amp.decay_s, 0.300);
        assert_eq!(p.closed_hat.hp_hz, 6000.0);
        assert_eq!(p.open_hat.hp_hz, 6000.0);
    }

    #[test]
    fn ron_round_trip_preserves_default_drum_patch() {
        let original = DrumPatchData::default();
        let s = ron::ser::to_string(&original).expect("serialize");
        let restored: DrumPatchData = ron::de::from_str(&s).expect("deserialize");
        assert_eq!(original, restored);
    }
}
