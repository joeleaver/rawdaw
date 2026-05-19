//! Wavetable synth parameter addressing.
//!
//! [`WavetableParam`] is the per-synth parameter path that flows from
//! the host UI through the engine's [`BlockMessage::Param`] channel
//! into the wavetable synth's `apply_event`. The 8-byte
//! [`crate::ParamEvent::path`] is the encoded form; the synth crate
//! owns the encoder, the decoder, and the application logic that
//! mutates the runtime [`WavetablePatch`](crate::WavetablePatch).
//!
//! ## Encoding
//!
//! Byte 0 is the variant tag (0..=12, see [`WavetableParam`]'s
//! discriminant order). Byte 1 is the sub-index (osc 0..=2 / env
//! 0..=2 / matrix slot 0..=15) when the variant carries one, else 0.
//! Bytes 2..=7 are reserved (zero on encode; ignored on decode).
//!
//! The `value` field of the `ParamEvent` carries the numeric payload
//! in the parameter's native units (Hz, seconds, semitones, …).
//! For `MatrixSource` / `MatrixDestination` the value is the encoded
//! enum ordinal cast to `f32`; decoding rounds + range-checks.

use rawdaw_dsp::{AdsrParams, ModDestination, ModSource};

use crate::patch::WavetablePatch;

/// One typed parameter address on a [`WavetableSynthNode`](crate::WavetableSynthNode).
///
/// Discriminants are stable wire identifiers — adding new variants
/// only appends; existing variants never renumber. The encoder uses
/// `byte0 == discriminant ordinal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavetableParam {
    /// `osc_params[i].tune_semitones`. `i` ∈ 0..=2.
    OscTune(u8),
    /// `osc_params[i].fine_cents`. `i` ∈ 0..=2.
    OscFineCents(u8),
    /// `osc_params[i].level`. `i` ∈ 0..=2.
    OscLevel(u8),
    /// `env_params[i].attack_s`. `i` ∈ 0..=2.
    EnvAttackS(u8),
    /// `env_params[i].decay_s`. `i` ∈ 0..=2.
    EnvDecayS(u8),
    /// `env_params[i].sustain_level`. `i` ∈ 0..=2.
    EnvSustain(u8),
    /// `env_params[i].release_s`. `i` ∈ 0..=2.
    EnvReleaseS(u8),
    /// `lfo_rate_hz`.
    LfoRateHz,
    /// `filter_cutoff_hz`.
    FilterCutoffHz,
    /// `filter_resonance`.
    FilterResonance,
    /// `matrix[s].source` — value is the encoded [`ModSource`]
    /// ordinal cast to `f32`. `s` ∈ 0..=15.
    MatrixSource(u8),
    /// `matrix[s].destination` — value is the encoded
    /// [`ModDestination`] flat-index cast to `f32`. `s` ∈ 0..=15.
    MatrixDestination(u8),
    /// `matrix[s].amount`. `s` ∈ 0..=15.
    MatrixAmount(u8),
}

impl WavetableParam {
    fn discriminant(&self) -> u8 {
        match self {
            Self::OscTune(_) => 0,
            Self::OscFineCents(_) => 1,
            Self::OscLevel(_) => 2,
            Self::EnvAttackS(_) => 3,
            Self::EnvDecayS(_) => 4,
            Self::EnvSustain(_) => 5,
            Self::EnvReleaseS(_) => 6,
            Self::LfoRateHz => 7,
            Self::FilterCutoffHz => 8,
            Self::FilterResonance => 9,
            Self::MatrixSource(_) => 10,
            Self::MatrixDestination(_) => 11,
            Self::MatrixAmount(_) => 12,
        }
    }

    fn sub_index(&self) -> u8 {
        match self {
            Self::OscTune(i)
            | Self::OscFineCents(i)
            | Self::OscLevel(i)
            | Self::EnvAttackS(i)
            | Self::EnvDecayS(i)
            | Self::EnvSustain(i)
            | Self::EnvReleaseS(i)
            | Self::MatrixSource(i)
            | Self::MatrixDestination(i)
            | Self::MatrixAmount(i) => *i,
            Self::LfoRateHz | Self::FilterCutoffHz | Self::FilterResonance => 0,
        }
    }

    /// Encode the param address into the engine's opaque 8-byte
    /// [`crate::ParamEvent::path`] form.
    pub fn encode(self) -> [u8; 8] {
        let mut out = [0u8; 8];
        out[0] = self.discriminant();
        out[1] = self.sub_index();
        out
    }

    /// Decode an 8-byte path back into a `WavetableParam`. Returns
    /// `None` on an unrecognized discriminant or on a sub-index that
    /// falls outside the variant's accepted range.
    pub fn decode(bytes: &[u8; 8]) -> Option<Self> {
        let tag = bytes[0];
        let i = bytes[1];
        let osc_ok = i < 3;
        let env_ok = i < 3;
        let slot_ok = i < 16;
        match tag {
            0 if osc_ok => Some(Self::OscTune(i)),
            1 if osc_ok => Some(Self::OscFineCents(i)),
            2 if osc_ok => Some(Self::OscLevel(i)),
            3 if env_ok => Some(Self::EnvAttackS(i)),
            4 if env_ok => Some(Self::EnvDecayS(i)),
            5 if env_ok => Some(Self::EnvSustain(i)),
            6 if env_ok => Some(Self::EnvReleaseS(i)),
            7 => Some(Self::LfoRateHz),
            8 => Some(Self::FilterCutoffHz),
            9 => Some(Self::FilterResonance),
            10 if slot_ok => Some(Self::MatrixSource(i)),
            11 if slot_ok => Some(Self::MatrixDestination(i)),
            12 if slot_ok => Some(Self::MatrixAmount(i)),
            _ => None,
        }
    }

    /// Apply this parameter change to a runtime patch. Out-of-range
    /// values are clamped to the parameter's accepted span (e.g.
    /// `filter_cutoff_hz` clamps to 20 Hz..20 kHz); unknown
    /// `MatrixSource` / `MatrixDestination` ordinals are no-ops so
    /// a bad value can't drive the patch into an invalid state.
    pub fn apply(self, patch: &mut WavetablePatch, value: f32) {
        match self {
            Self::OscTune(i) => {
                if let Some(slot) = patch.osc_params.get_mut(i as usize) {
                    slot.tune_semitones = value.round().clamp(-24.0, 24.0) as i8;
                }
            }
            Self::OscFineCents(i) => {
                if let Some(slot) = patch.osc_params.get_mut(i as usize) {
                    slot.fine_cents = value.round().clamp(-100.0, 100.0) as i8;
                }
            }
            Self::OscLevel(i) => {
                if let Some(slot) = patch.osc_params.get_mut(i as usize) {
                    slot.level = value.clamp(0.0, 1.0);
                }
            }
            Self::EnvAttackS(i) => set_env(&mut patch.env_params, i, |e| &mut e.attack_s, value, 0.0, 10.0),
            Self::EnvDecayS(i) => set_env(&mut patch.env_params, i, |e| &mut e.decay_s, value, 0.0, 10.0),
            Self::EnvSustain(i) => set_env(&mut patch.env_params, i, |e| &mut e.sustain_level, value, 0.0, 1.0),
            Self::EnvReleaseS(i) => set_env(&mut patch.env_params, i, |e| &mut e.release_s, value, 0.0, 10.0),
            Self::LfoRateHz => patch.lfo_rate_hz = value.clamp(0.0, 40.0),
            Self::FilterCutoffHz => patch.filter_cutoff_hz = value.clamp(20.0, 20_000.0),
            Self::FilterResonance => patch.filter_resonance = value.clamp(0.0, 10.0),
            Self::MatrixSource(s) => {
                if let (Some(slot), Some(src)) =
                    (patch.matrix.get_mut(s as usize), decode_mod_source(value))
                {
                    slot.source = src;
                }
            }
            Self::MatrixDestination(s) => {
                if let (Some(slot), Some(dst)) = (
                    patch.matrix.get_mut(s as usize),
                    decode_mod_destination(value),
                ) {
                    slot.destination = dst;
                }
            }
            Self::MatrixAmount(s) => {
                if let Some(slot) = patch.matrix.get_mut(s as usize) {
                    slot.amount = value.clamp(-2.0, 2.0);
                }
            }
        }
    }
}

fn set_env<F>(
    envs: &mut [AdsrParams; crate::patch::NUM_ENVS],
    i: u8,
    field: F,
    value: f32,
    lo: f32,
    hi: f32,
) where
    F: FnOnce(&mut AdsrParams) -> &mut f32,
{
    if let Some(env) = envs.get_mut(i as usize) {
        *field(env) = value.clamp(lo, hi);
    }
}

/// Decode a `ModSource` ordinal cast to `f32`. Ordinals match
/// [`rawdaw_model::patch::wavetable::ModSourceData`] discriminants.
fn decode_mod_source(value: f32) -> Option<ModSource> {
    let n = value.round() as i32;
    match n {
        0 => Some(ModSource::None),
        1 => Some(ModSource::Env1),
        2 => Some(ModSource::Env2),
        3 => Some(ModSource::Env3),
        4 => Some(ModSource::Lfo1),
        5 => Some(ModSource::Osc0),
        6 => Some(ModSource::Osc1),
        7 => Some(ModSource::Osc2),
        _ => None,
    }
}

/// Decode a `ModDestination` flat-index cast to `f32`. The UI sees
/// a flat list (FilterCutoff, FilterResonance, OscLevel(0), …,
/// LfoRate); this function maps the chosen index back to the
/// runtime enum. Out-of-range values return `None` so a malformed
/// event leaves the slot's destination unchanged.
fn decode_mod_destination(value: f32) -> Option<ModDestination> {
    let n = value.round() as i32;
    match n {
        0 => Some(ModDestination::FilterCutoff),
        1 => Some(ModDestination::FilterResonance),
        2..=4 => Some(ModDestination::OscLevel((n - 2) as u8)),
        5..=7 => Some(ModDestination::OscTune((n - 5) as u8)),
        8..=10 => Some(ModDestination::OscFineTune((n - 8) as u8)),
        11..=13 => Some(ModDestination::PmAmountOf((n - 11) as u8)),
        14..=16 => Some(ModDestination::AmAmountOf((n - 14) as u8)),
        17..=19 => Some(ModDestination::RmAmountOf((n - 17) as u8)),
        20 => Some(ModDestination::LfoRate),
        _ => None,
    }
}

#[allow(dead_code)] // Used by U4+ UI code; kept here for symmetry.
pub(crate) fn encode_mod_source(src: ModSource) -> f32 {
    match src {
        ModSource::None => 0.0,
        ModSource::Env1 => 1.0,
        ModSource::Env2 => 2.0,
        ModSource::Env3 => 3.0,
        ModSource::Lfo1 => 4.0,
        ModSource::Osc0 => 5.0,
        ModSource::Osc1 => 6.0,
        ModSource::Osc2 => 7.0,
    }
}

#[allow(dead_code)] // Used by U4+ UI code; kept here for symmetry.
pub(crate) fn encode_mod_destination(dst: ModDestination) -> f32 {
    match dst {
        ModDestination::FilterCutoff => 0.0,
        ModDestination::FilterResonance => 1.0,
        ModDestination::OscLevel(i) => 2.0 + i as f32,
        ModDestination::OscTune(i) => 5.0 + i as f32,
        ModDestination::OscFineTune(i) => 8.0 + i as f32,
        ModDestination::PmAmountOf(i) => 11.0 + i as f32,
        ModDestination::AmAmountOf(i) => 14.0 + i as f32,
        ModDestination::RmAmountOf(i) => 17.0 + i as f32,
        ModDestination::LfoRate => 20.0,
    }
}

/// Convenience for the `propagate_patch_to_voices` analog in tests
/// and U4 UI code: apply a `(path, value)` pair to a patch without
/// going through the engine's event channel.
#[allow(dead_code)]
pub(crate) fn apply_encoded(
    patch: &mut WavetablePatch,
    path: &[u8; 8],
    value: f32,
) -> Option<()> {
    let param = WavetableParam::decode(path)?;
    param.apply(patch, value);
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_variants() -> Vec<WavetableParam> {
        let mut v = vec![
            WavetableParam::LfoRateHz,
            WavetableParam::FilterCutoffHz,
            WavetableParam::FilterResonance,
        ];
        for i in 0..3 {
            v.push(WavetableParam::OscTune(i));
            v.push(WavetableParam::OscFineCents(i));
            v.push(WavetableParam::OscLevel(i));
            v.push(WavetableParam::EnvAttackS(i));
            v.push(WavetableParam::EnvDecayS(i));
            v.push(WavetableParam::EnvSustain(i));
            v.push(WavetableParam::EnvReleaseS(i));
        }
        for s in 0..16 {
            v.push(WavetableParam::MatrixSource(s));
            v.push(WavetableParam::MatrixDestination(s));
            v.push(WavetableParam::MatrixAmount(s));
        }
        v
    }

    #[test]
    fn encode_decode_round_trips_every_variant() {
        for p in all_variants() {
            let bytes = p.encode();
            let decoded = WavetableParam::decode(&bytes)
                .unwrap_or_else(|| panic!("decode failed for {p:?}"));
            assert_eq!(p, decoded);
        }
    }

    #[test]
    fn decode_rejects_out_of_range_indices() {
        // Osc index 3 (only 0..=2 valid).
        assert!(WavetableParam::decode(&[0, 3, 0, 0, 0, 0, 0, 0]).is_none());
        // Env index 3.
        assert!(WavetableParam::decode(&[3, 3, 0, 0, 0, 0, 0, 0]).is_none());
        // Matrix slot 16.
        assert!(WavetableParam::decode(&[10, 16, 0, 0, 0, 0, 0, 0]).is_none());
    }

    #[test]
    fn decode_rejects_unknown_tag() {
        assert!(WavetableParam::decode(&[200, 0, 0, 0, 0, 0, 0, 0]).is_none());
    }

    #[test]
    fn apply_filter_cutoff_clamps_into_audio_range() {
        let mut patch = WavetablePatch::default();
        WavetableParam::FilterCutoffHz.apply(&mut patch, 50_000.0);
        assert_eq!(patch.filter_cutoff_hz, 20_000.0);
        WavetableParam::FilterCutoffHz.apply(&mut patch, -100.0);
        assert_eq!(patch.filter_cutoff_hz, 20.0);
        WavetableParam::FilterCutoffHz.apply(&mut patch, 4000.0);
        assert_eq!(patch.filter_cutoff_hz, 4000.0);
    }

    #[test]
    fn apply_osc_level_clamps_to_unit_range() {
        let mut patch = WavetablePatch::default();
        WavetableParam::OscLevel(0).apply(&mut patch, 5.0);
        assert_eq!(patch.osc_params[0].level, 1.0);
        WavetableParam::OscLevel(0).apply(&mut patch, -0.5);
        assert_eq!(patch.osc_params[0].level, 0.0);
        WavetableParam::OscLevel(1).apply(&mut patch, 0.42);
        assert_eq!(patch.osc_params[1].level, 0.42);
    }

    #[test]
    fn apply_matrix_source_changes_slot() {
        let mut patch = WavetablePatch::default();
        let original = patch.matrix[0].source;
        WavetableParam::MatrixSource(0).apply(&mut patch, encode_mod_source(ModSource::Env1));
        assert_eq!(patch.matrix[0].source, ModSource::Env1);
        assert_ne!(patch.matrix[0].source, original);
    }

    #[test]
    fn apply_matrix_destination_round_trip() {
        let mut patch = WavetablePatch::default();
        let target = ModDestination::OscFineTune(2);
        WavetableParam::MatrixDestination(3).apply(&mut patch, encode_mod_destination(target));
        assert_eq!(patch.matrix[3].destination, target);
    }

    #[test]
    fn apply_with_unknown_matrix_source_value_is_a_noop() {
        let mut patch = WavetablePatch::default();
        let original = patch.matrix[5].source;
        // Garbage value out of any ModSource ordinal range.
        WavetableParam::MatrixSource(5).apply(&mut patch, 999.0);
        assert_eq!(patch.matrix[5].source, original);
    }

    #[test]
    fn apply_out_of_range_osc_index_is_a_noop() {
        // Index 3 is invalid for a 3-osc voice; apply silently skips.
        // The matching variant `OscTune(3)` can only be constructed
        // directly (decode would reject it); this guards the apply
        // path in case a caller bypasses decode.
        let mut patch = WavetablePatch::default();
        let before = patch.osc_params;
        WavetableParam::OscTune(3).apply(&mut patch, 7.0);
        assert_eq!(patch.osc_params, before);
    }

    #[test]
    fn mod_source_round_trip_through_f32() {
        for src in [
            ModSource::None,
            ModSource::Env1,
            ModSource::Env2,
            ModSource::Env3,
            ModSource::Lfo1,
            ModSource::Osc0,
            ModSource::Osc1,
            ModSource::Osc2,
        ] {
            let encoded = encode_mod_source(src);
            assert_eq!(decode_mod_source(encoded), Some(src));
        }
    }

    #[test]
    fn mod_destination_round_trip_through_f32() {
        let destinations = [
            ModDestination::FilterCutoff,
            ModDestination::FilterResonance,
            ModDestination::OscLevel(0),
            ModDestination::OscLevel(2),
            ModDestination::OscTune(1),
            ModDestination::OscFineTune(0),
            ModDestination::PmAmountOf(2),
            ModDestination::AmAmountOf(1),
            ModDestination::RmAmountOf(0),
            ModDestination::LfoRate,
        ];
        for dst in destinations {
            let encoded = encode_mod_destination(dst);
            assert_eq!(decode_mod_destination(encoded), Some(dst));
        }
    }
}
