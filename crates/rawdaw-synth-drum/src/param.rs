//! Drum synth parameter addressing.
//!
//! [`DrumParam`] is the per-synth parameter path that flows from the
//! host UI through the engine's [`BlockMessage::Param`] channel into
//! the drum synth's `apply_event`. Mirrors the wavetable synth's
//! [`WavetableParam`](rawdaw_synth_wavetable::WavetableParam) shape:
//! one tag byte per parameter, value in the parameter's native
//! units, encoder + decoder + applier living together.
//!
//! Variants are flat (one per (voice, field) pair). The drum synth's
//! parameter surface is small enough that a flat enum is clearer than
//! nesting target + field; if the synth grows more voices (toms,
//! claps, cymbals) the encoding scheme will likely switch to a
//! `(byte0 = target, byte1 = field)` pair.

use crate::patch::DrumPatch;
use rawdaw_dsp::AdsrParams;

/// One typed parameter address on a [`DrumSynthNode`](crate::DrumSynthNode).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrumParam {
    // ── Kick (tags 0..=6) ────────────────────────────────────────
    KickStartHz,
    KickEndHz,
    KickPitchDecayS,
    KickAmpAttackS,
    KickAmpDecayS,
    KickAmpSustain,
    KickAmpRelease,
    // ── Snare (tags 7..=16) ──────────────────────────────────────
    SnareBodyStartHz,
    SnareBodyEndHz,
    SnareBodyPitchDecayS,
    SnareNoiseMix,
    SnareNoiseHpHz,
    SnareNoiseHpQ,
    SnareAmpAttackS,
    SnareAmpDecayS,
    SnareAmpSustain,
    SnareAmpRelease,
    // ── Closed hat (tags 17..=22) ────────────────────────────────
    ClosedHatHpHz,
    ClosedHatHpQ,
    ClosedHatAmpAttackS,
    ClosedHatAmpDecayS,
    ClosedHatAmpSustain,
    ClosedHatAmpRelease,
    // ── Open hat (tags 23..=28) ──────────────────────────────────
    OpenHatHpHz,
    OpenHatHpQ,
    OpenHatAmpAttackS,
    OpenHatAmpDecayS,
    OpenHatAmpSustain,
    OpenHatAmpRelease,
}

impl DrumParam {
    fn tag(self) -> u8 {
        // Mirror of the enum's declared order. Could be replaced by
        // `unsafe { mem::transmute::<_, u8>(self) }` for zero cost,
        // but the crate forbids unsafe_code and `match` is fine.
        match self {
            Self::KickStartHz => 0,
            Self::KickEndHz => 1,
            Self::KickPitchDecayS => 2,
            Self::KickAmpAttackS => 3,
            Self::KickAmpDecayS => 4,
            Self::KickAmpSustain => 5,
            Self::KickAmpRelease => 6,
            Self::SnareBodyStartHz => 7,
            Self::SnareBodyEndHz => 8,
            Self::SnareBodyPitchDecayS => 9,
            Self::SnareNoiseMix => 10,
            Self::SnareNoiseHpHz => 11,
            Self::SnareNoiseHpQ => 12,
            Self::SnareAmpAttackS => 13,
            Self::SnareAmpDecayS => 14,
            Self::SnareAmpSustain => 15,
            Self::SnareAmpRelease => 16,
            Self::ClosedHatHpHz => 17,
            Self::ClosedHatHpQ => 18,
            Self::ClosedHatAmpAttackS => 19,
            Self::ClosedHatAmpDecayS => 20,
            Self::ClosedHatAmpSustain => 21,
            Self::ClosedHatAmpRelease => 22,
            Self::OpenHatHpHz => 23,
            Self::OpenHatHpQ => 24,
            Self::OpenHatAmpAttackS => 25,
            Self::OpenHatAmpDecayS => 26,
            Self::OpenHatAmpSustain => 27,
            Self::OpenHatAmpRelease => 28,
        }
    }

    /// Encode into the engine's opaque 8-byte path form.
    pub fn encode(self) -> [u8; 8] {
        let mut out = [0u8; 8];
        out[0] = self.tag();
        out
    }

    /// Decode an 8-byte path back into a `DrumParam`. Returns
    /// `None` on an unrecognized tag.
    pub fn decode(bytes: &[u8; 8]) -> Option<Self> {
        match bytes[0] {
            0 => Some(Self::KickStartHz),
            1 => Some(Self::KickEndHz),
            2 => Some(Self::KickPitchDecayS),
            3 => Some(Self::KickAmpAttackS),
            4 => Some(Self::KickAmpDecayS),
            5 => Some(Self::KickAmpSustain),
            6 => Some(Self::KickAmpRelease),
            7 => Some(Self::SnareBodyStartHz),
            8 => Some(Self::SnareBodyEndHz),
            9 => Some(Self::SnareBodyPitchDecayS),
            10 => Some(Self::SnareNoiseMix),
            11 => Some(Self::SnareNoiseHpHz),
            12 => Some(Self::SnareNoiseHpQ),
            13 => Some(Self::SnareAmpAttackS),
            14 => Some(Self::SnareAmpDecayS),
            15 => Some(Self::SnareAmpSustain),
            16 => Some(Self::SnareAmpRelease),
            17 => Some(Self::ClosedHatHpHz),
            18 => Some(Self::ClosedHatHpQ),
            19 => Some(Self::ClosedHatAmpAttackS),
            20 => Some(Self::ClosedHatAmpDecayS),
            21 => Some(Self::ClosedHatAmpSustain),
            22 => Some(Self::ClosedHatAmpRelease),
            23 => Some(Self::OpenHatHpHz),
            24 => Some(Self::OpenHatHpQ),
            25 => Some(Self::OpenHatAmpAttackS),
            26 => Some(Self::OpenHatAmpDecayS),
            27 => Some(Self::OpenHatAmpSustain),
            28 => Some(Self::OpenHatAmpRelease),
            _ => None,
        }
    }

    /// Apply this parameter change to a runtime drum patch.
    /// Out-of-range values are clamped to the parameter's accepted
    /// span (Hz fields clamp to audio range; amp envelope times
    /// clamp to 0..=10 s; sustain clamps to 0..=1; mix to 0..=1).
    pub fn apply(self, patch: &mut DrumPatch, value: f32) {
        match self {
            Self::KickStartHz => patch.kick.start_hz = value.clamp(20.0, 20_000.0),
            Self::KickEndHz => patch.kick.end_hz = value.clamp(20.0, 20_000.0),
            Self::KickPitchDecayS => patch.kick.pitch_decay_s = value.clamp(0.001, 5.0),
            Self::KickAmpAttackS => set_amp(&mut patch.kick.amp, |a| &mut a.attack_s, value, 0.0, 10.0),
            Self::KickAmpDecayS => set_amp(&mut patch.kick.amp, |a| &mut a.decay_s, value, 0.0, 10.0),
            Self::KickAmpSustain => set_amp(&mut patch.kick.amp, |a| &mut a.sustain_level, value, 0.0, 1.0),
            Self::KickAmpRelease => set_amp(&mut patch.kick.amp, |a| &mut a.release_s, value, 0.0, 10.0),
            Self::SnareBodyStartHz => patch.snare.body_start_hz = value.clamp(20.0, 20_000.0),
            Self::SnareBodyEndHz => patch.snare.body_end_hz = value.clamp(20.0, 20_000.0),
            Self::SnareBodyPitchDecayS => patch.snare.body_pitch_decay_s = value.clamp(0.001, 5.0),
            Self::SnareNoiseMix => patch.snare.noise_mix = value.clamp(0.0, 1.0),
            Self::SnareNoiseHpHz => patch.snare.noise_hp_hz = value.clamp(20.0, 20_000.0),
            Self::SnareNoiseHpQ => patch.snare.noise_hp_q = value.clamp(0.0, 10.0),
            Self::SnareAmpAttackS => set_amp(&mut patch.snare.amp, |a| &mut a.attack_s, value, 0.0, 10.0),
            Self::SnareAmpDecayS => set_amp(&mut patch.snare.amp, |a| &mut a.decay_s, value, 0.0, 10.0),
            Self::SnareAmpSustain => set_amp(&mut patch.snare.amp, |a| &mut a.sustain_level, value, 0.0, 1.0),
            Self::SnareAmpRelease => set_amp(&mut patch.snare.amp, |a| &mut a.release_s, value, 0.0, 10.0),
            Self::ClosedHatHpHz => patch.closed_hat.hp_hz = value.clamp(20.0, 20_000.0),
            Self::ClosedHatHpQ => patch.closed_hat.hp_q = value.clamp(0.0, 10.0),
            Self::ClosedHatAmpAttackS => set_amp(&mut patch.closed_hat.amp, |a| &mut a.attack_s, value, 0.0, 10.0),
            Self::ClosedHatAmpDecayS => set_amp(&mut patch.closed_hat.amp, |a| &mut a.decay_s, value, 0.0, 10.0),
            Self::ClosedHatAmpSustain => set_amp(&mut patch.closed_hat.amp, |a| &mut a.sustain_level, value, 0.0, 1.0),
            Self::ClosedHatAmpRelease => set_amp(&mut patch.closed_hat.amp, |a| &mut a.release_s, value, 0.0, 10.0),
            Self::OpenHatHpHz => patch.open_hat.hp_hz = value.clamp(20.0, 20_000.0),
            Self::OpenHatHpQ => patch.open_hat.hp_q = value.clamp(0.0, 10.0),
            Self::OpenHatAmpAttackS => set_amp(&mut patch.open_hat.amp, |a| &mut a.attack_s, value, 0.0, 10.0),
            Self::OpenHatAmpDecayS => set_amp(&mut patch.open_hat.amp, |a| &mut a.decay_s, value, 0.0, 10.0),
            Self::OpenHatAmpSustain => set_amp(&mut patch.open_hat.amp, |a| &mut a.sustain_level, value, 0.0, 1.0),
            Self::OpenHatAmpRelease => set_amp(&mut patch.open_hat.amp, |a| &mut a.release_s, value, 0.0, 10.0),
        }
    }
}

fn set_amp<F>(amp: &mut AdsrParams, field: F, value: f32, lo: f32, hi: f32)
where
    F: FnOnce(&mut AdsrParams) -> &mut f32,
{
    *field(amp) = value.clamp(lo, hi);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_variants() -> Vec<DrumParam> {
        (0..=28u8)
            .map(|tag| {
                let mut bytes = [0u8; 8];
                bytes[0] = tag;
                DrumParam::decode(&bytes).unwrap_or_else(|| panic!("tag {tag} should decode"))
            })
            .collect()
    }

    #[test]
    fn encode_decode_round_trips_every_variant() {
        for p in all_variants() {
            let bytes = p.encode();
            let decoded = DrumParam::decode(&bytes)
                .unwrap_or_else(|| panic!("decode failed for {p:?}"));
            assert_eq!(p, decoded);
        }
    }

    #[test]
    fn variants_have_unique_tags() {
        // Ensures the tag() function and the decode() match don't
        // drift apart silently.
        let mut tags: Vec<u8> = all_variants().into_iter().map(|p| p.tag()).collect();
        tags.sort();
        let mut deduped = tags.clone();
        deduped.dedup();
        assert_eq!(tags, deduped, "every variant must have a unique tag");
    }

    #[test]
    fn decode_rejects_unknown_tag() {
        assert!(DrumParam::decode(&[100, 0, 0, 0, 0, 0, 0, 0]).is_none());
    }

    #[test]
    fn apply_kick_start_hz_clamps_to_audio_range() {
        let mut patch = DrumPatch::default();
        DrumParam::KickStartHz.apply(&mut patch, 50_000.0);
        assert_eq!(patch.kick.start_hz, 20_000.0);
        DrumParam::KickStartHz.apply(&mut patch, -10.0);
        assert_eq!(patch.kick.start_hz, 20.0);
    }

    #[test]
    fn apply_snare_noise_mix_clamps_to_unit_range() {
        let mut patch = DrumPatch::default();
        DrumParam::SnareNoiseMix.apply(&mut patch, 1.5);
        assert_eq!(patch.snare.noise_mix, 1.0);
        DrumParam::SnareNoiseMix.apply(&mut patch, -0.5);
        assert_eq!(patch.snare.noise_mix, 0.0);
        DrumParam::SnareNoiseMix.apply(&mut patch, 0.3);
        assert_eq!(patch.snare.noise_mix, 0.3);
    }

    #[test]
    fn apply_open_hat_decay_only_affects_open() {
        let mut patch = DrumPatch::default();
        let closed_before = patch.closed_hat.amp.decay_s;
        DrumParam::OpenHatAmpDecayS.apply(&mut patch, 0.5);
        assert_eq!(patch.open_hat.amp.decay_s, 0.5);
        assert_eq!(patch.closed_hat.amp.decay_s, closed_before);
    }
}
