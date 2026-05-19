//! Runtime wavetable patch — the in-memory form the synth node + its
//! voices read every sample.
//!
//! Mirrors `rawdaw-model::patch::wavetable::WavetablePatchData` field
//! for field, but holds `rawdaw-dsp` types directly
//! (`WavetableOscParams`, `AdsrParams`, `ModSlot`, …) so the hot path
//! reads the same memory layout the audio thread already uses.
//!
//! `From<WavetablePatchData>` is the conversion seam: the host loads /
//! mutates the serialized form, then constructs the runtime form once
//! at synth construction (and again on whole-patch replace — preset
//! switching in U8). Individual parameter changes flow through the
//! parameter event channel and land on this runtime patch directly,
//! not through the data form.

use rawdaw_dsp::{AdsrParams, ModDestination, ModSlot, ModSource, WavetableOscParams};
use rawdaw_model::patch::wavetable::{
    AdsrParamsData, ModDestinationData, ModSlotData, ModSourceData, WavetableOscParamsData,
    WavetablePatchData,
};

use crate::{MOD_MATRIX_SLOTS, NUM_OSCS};

/// Number of envelopes per voice (ENV1 amp + ENV2/ENV3 free).
pub(crate) const NUM_ENVS: usize = 3;

/// Runtime wavetable patch — owned by [`WavetableSynthNode`](crate::WavetableSynthNode)
/// and mirrored into every voice at construction + on patch replace.
#[derive(Debug, Clone, Copy)]
pub struct WavetablePatch {
    pub osc_params: [WavetableOscParams; NUM_OSCS],
    pub env_params: [AdsrParams; NUM_ENVS],
    pub lfo_rate_hz: f32,
    pub filter_cutoff_hz: f32,
    pub filter_resonance: f32,
    pub matrix: [ModSlot; MOD_MATRIX_SLOTS],
}

impl WavetablePatch {
    /// The M5 default patch — same bytes the synth has been shipping
    /// since the M-phases. Constructed via the `From` conversion from
    /// the model's [`WavetablePatchData::default`] so this side and the
    /// serialized side share a single source of truth.
    pub fn default_m5() -> Self {
        WavetablePatchData::default().into()
    }
}

impl Default for WavetablePatch {
    fn default() -> Self {
        Self::default_m5()
    }
}

impl From<WavetablePatchData> for WavetablePatch {
    fn from(data: WavetablePatchData) -> Self {
        Self {
            osc_params: data.osc_params.map(WavetableOscParams::from_data),
            env_params: data.env_params.map(AdsrParams::from_data),
            lfo_rate_hz: data.lfo_rate_hz,
            filter_cutoff_hz: data.filter_cutoff_hz,
            filter_resonance: data.filter_resonance,
            matrix: data.matrix.map(ModSlot::from_data),
        }
    }
}

// ── Per-field conversions ────────────────────────────────────────────
//
// Sit on the runtime types via free functions rather than `From` impls,
// because `impl From<T> for U` requires U and T to be in the same crate
// (orphan rule). The runtime types live in rawdaw-dsp; the data types
// live in rawdaw-model. This crate owns the bridge, so the functions
// are private helpers exposed via `WavetablePatch::from(data)` above.

trait FromData<T> {
    fn from_data(data: T) -> Self;
}

impl FromData<WavetableOscParamsData> for WavetableOscParams {
    fn from_data(data: WavetableOscParamsData) -> Self {
        Self {
            tune_semitones: data.tune_semitones,
            fine_cents: data.fine_cents,
            level: data.level,
        }
    }
}

impl FromData<AdsrParamsData> for AdsrParams {
    fn from_data(data: AdsrParamsData) -> Self {
        Self {
            attack_s: data.attack_s,
            decay_s: data.decay_s,
            sustain_level: data.sustain_level,
            release_s: data.release_s,
        }
    }
}

impl FromData<ModSourceData> for ModSource {
    fn from_data(data: ModSourceData) -> Self {
        match data {
            ModSourceData::None => Self::None,
            ModSourceData::Env1 => Self::Env1,
            ModSourceData::Env2 => Self::Env2,
            ModSourceData::Env3 => Self::Env3,
            ModSourceData::Lfo1 => Self::Lfo1,
            ModSourceData::Osc0 => Self::Osc0,
            ModSourceData::Osc1 => Self::Osc1,
            ModSourceData::Osc2 => Self::Osc2,
            ModSourceData::MidiCC(cc) => Self::MidiCC(cc),
        }
    }
}

impl FromData<ModDestinationData> for ModDestination {
    fn from_data(data: ModDestinationData) -> Self {
        match data {
            ModDestinationData::FilterCutoff => Self::FilterCutoff,
            ModDestinationData::FilterResonance => Self::FilterResonance,
            ModDestinationData::OscLevel(i) => Self::OscLevel(i),
            ModDestinationData::OscTune(i) => Self::OscTune(i),
            ModDestinationData::OscFineTune(i) => Self::OscFineTune(i),
            ModDestinationData::PmAmountOf(i) => Self::PmAmountOf(i),
            ModDestinationData::AmAmountOf(i) => Self::AmAmountOf(i),
            ModDestinationData::RmAmountOf(i) => Self::RmAmountOf(i),
            ModDestinationData::LfoRate => Self::LfoRate,
        }
    }
}

impl FromData<ModSlotData> for ModSlot {
    fn from_data(data: ModSlotData) -> Self {
        Self {
            source: ModSource::from_data(data.source),
            destination: ModDestination::from_data(data.destination),
            amount: data.amount,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_runtime_patch_matches_default_data() {
        // Property pin: every field of the runtime patch round-trips
        // through `From<WavetablePatchData::default()>`. Catches any
        // drift between the model's serialized default and the runtime
        // constructor's values.
        let runtime = WavetablePatch::default_m5();
        let data = WavetablePatchData::default();

        assert_eq!(runtime.lfo_rate_hz, data.lfo_rate_hz);
        assert_eq!(runtime.filter_cutoff_hz, data.filter_cutoff_hz);
        assert_eq!(runtime.filter_resonance, data.filter_resonance);

        for i in 0..NUM_OSCS {
            assert_eq!(runtime.osc_params[i].tune_semitones, data.osc_params[i].tune_semitones);
            assert_eq!(runtime.osc_params[i].fine_cents, data.osc_params[i].fine_cents);
            assert_eq!(runtime.osc_params[i].level, data.osc_params[i].level);
        }

        for i in 0..NUM_ENVS {
            assert_eq!(runtime.env_params[i].attack_s, data.env_params[i].attack_s);
            assert_eq!(runtime.env_params[i].decay_s, data.env_params[i].decay_s);
            assert_eq!(runtime.env_params[i].sustain_level, data.env_params[i].sustain_level);
            assert_eq!(runtime.env_params[i].release_s, data.env_params[i].release_s);
        }

        // Slots [0..5] are the M5 routes; slot 5 is the K5 mod-wheel
        // default; the rest are inactive.
        assert_eq!(runtime.matrix[0].source, ModSource::Lfo1);
        assert_eq!(runtime.matrix[1].source, ModSource::Env2);
        assert_eq!(runtime.matrix[2].source, ModSource::Osc1);
        assert_eq!(runtime.matrix[3].source, ModSource::Osc2);
        assert_eq!(runtime.matrix[4].source, ModSource::Env3);
        assert_eq!(runtime.matrix[5].source, ModSource::MidiCC(1));
        for slot in &runtime.matrix[6..] {
            assert_eq!(slot.source, ModSource::None);
        }
    }

    #[test]
    fn from_data_preserves_custom_patch_values() {
        let mut data = WavetablePatchData::default();
        data.osc_params[2].level = 0.42;
        data.env_params[1].attack_s = 0.123;
        data.filter_cutoff_hz = 1234.0;
        data.matrix[6] = ModSlotData {
            source: ModSourceData::Env1,
            destination: ModDestinationData::OscLevel(2),
            amount: 0.55,
        };

        let runtime: WavetablePatch = data.into();
        assert_eq!(runtime.osc_params[2].level, 0.42);
        assert_eq!(runtime.env_params[1].attack_s, 0.123);
        assert_eq!(runtime.filter_cutoff_hz, 1234.0);
        assert_eq!(runtime.matrix[6].source, ModSource::Env1);
        assert_eq!(runtime.matrix[6].destination, ModDestination::OscLevel(2));
        assert_eq!(runtime.matrix[6].amount, 0.55);
    }
}
