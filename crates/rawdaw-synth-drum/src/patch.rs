//! Runtime drum patch — the in-memory form the drum synth's voices
//! read at prepare time + at note-on for the per-style hat lookup.
//!
//! Mirrors `rawdaw-model::patch::drum::DrumPatchData` field for
//! field. The conversion seam (`From<DrumPatchData>`) lives here so
//! the synth crate owns the bridge between serialized data and
//! runtime types.

use rawdaw_dsp::AdsrParams;
use rawdaw_model::patch::drum::{
    DrumPatchData, HatPatchData, KickPatchData, SnarePatchData,
};

/// Kick-drum runtime patch.
#[derive(Debug, Clone, Copy)]
pub struct KickPatch {
    pub start_hz: f32,
    pub end_hz: f32,
    pub pitch_decay_s: f32,
    pub amp: AdsrParams,
}

impl From<KickPatchData> for KickPatch {
    fn from(d: KickPatchData) -> Self {
        Self {
            start_hz: d.start_hz,
            end_hz: d.end_hz,
            pitch_decay_s: d.pitch_decay_s,
            amp: AdsrParams {
                attack_s: d.amp.attack_s,
                decay_s: d.amp.decay_s,
                sustain_level: d.amp.sustain_level,
                release_s: d.amp.release_s,
            },
        }
    }
}

/// Snare runtime patch.
#[derive(Debug, Clone, Copy)]
pub struct SnarePatch {
    pub body_start_hz: f32,
    pub body_end_hz: f32,
    pub body_pitch_decay_s: f32,
    pub noise_mix: f32,
    pub noise_hp_hz: f32,
    pub noise_hp_q: f32,
    pub amp: AdsrParams,
}

impl From<SnarePatchData> for SnarePatch {
    fn from(d: SnarePatchData) -> Self {
        Self {
            body_start_hz: d.body_start_hz,
            body_end_hz: d.body_end_hz,
            body_pitch_decay_s: d.body_pitch_decay_s,
            noise_mix: d.noise_mix,
            noise_hp_hz: d.noise_hp_hz,
            noise_hp_q: d.noise_hp_q,
            amp: AdsrParams {
                attack_s: d.amp.attack_s,
                decay_s: d.amp.decay_s,
                sustain_level: d.amp.sustain_level,
                release_s: d.amp.release_s,
            },
        }
    }
}

/// Hi-hat runtime patch. The drum-synth `DrumPatch` carries two of
/// these (one closed, one open); the voice receives whichever one
/// matches the incoming note's MIDI mapping.
#[derive(Debug, Clone, Copy)]
pub struct HatPatch {
    pub hp_hz: f32,
    pub hp_q: f32,
    pub amp: AdsrParams,
}

impl From<HatPatchData> for HatPatch {
    fn from(d: HatPatchData) -> Self {
        Self {
            hp_hz: d.hp_hz,
            hp_q: d.hp_q,
            amp: AdsrParams {
                attack_s: d.amp.attack_s,
                decay_s: d.amp.decay_s,
                sustain_level: d.amp.sustain_level,
                release_s: d.amp.release_s,
            },
        }
    }
}

/// Full drum patch — one sub-patch per voice type.
#[derive(Debug, Clone, Copy)]
pub struct DrumPatch {
    pub kick: KickPatch,
    pub snare: SnarePatch,
    pub closed_hat: HatPatch,
    pub open_hat: HatPatch,
}

impl DrumPatch {
    /// The v0 default drum patch — bit-equal to the constants the
    /// drum voices have been shipping since the D-phases.
    pub fn default_v0() -> Self {
        DrumPatchData::default().into()
    }
}

impl Default for DrumPatch {
    fn default() -> Self {
        Self::default_v0()
    }
}

impl From<DrumPatchData> for DrumPatch {
    fn from(d: DrumPatchData) -> Self {
        Self {
            kick: d.kick.into(),
            snare: d.snare.into(),
            closed_hat: d.closed_hat.into(),
            open_hat: d.open_hat.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_runtime_drum_patch_matches_default_data() {
        let runtime = DrumPatch::default_v0();
        let data = DrumPatchData::default();

        assert_eq!(runtime.kick.start_hz, data.kick.start_hz);
        assert_eq!(runtime.kick.amp.decay_s, data.kick.amp.decay_s);

        assert_eq!(runtime.snare.body_start_hz, data.snare.body_start_hz);
        assert_eq!(runtime.snare.noise_mix, data.snare.noise_mix);
        assert_eq!(runtime.snare.noise_hp_hz, data.snare.noise_hp_hz);

        assert_eq!(runtime.closed_hat.amp.decay_s, data.closed_hat.amp.decay_s);
        assert_eq!(runtime.open_hat.amp.decay_s, data.open_hat.amp.decay_s);
        assert_ne!(
            runtime.closed_hat.amp.decay_s,
            runtime.open_hat.amp.decay_s,
            "closed and open hats should have different decays"
        );
    }
}
