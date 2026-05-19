//! Serialized wavetable-synth patch types.
//!
//! These are **model-layer mirrors** of the runtime types in
//! `rawdaw-dsp` / `rawdaw-synth-wavetable`. They use only model-level
//! primitives (`f32`, `i8`, `u8`, fixed-size arrays) so `rawdaw-model`
//! stays free of any `rawdaw-dsp` dependency. The conversion to the
//! runtime types lives in the synth crate's `From` impls (lands in
//! U3 alongside the synth-side decoders).
//!
//! ## Format versioning
//!
//! Every patch carries a `format_version: u32` so future schema changes
//! don't break old project files. v1 is the initial format. New
//! variants of [`ModSource`] / [`ModDestination`] / new patch fields
//! bump the version; the migration path will live alongside whatever
//! save/load plumbing the future project-file plan introduces.
//!
//! ## Default
//!
//! [`WavetablePatchData::default()`] returns the M5 default patch — the
//! Vital-flavored saw + 3-osc PM stack + plucked-filter envelope + slow
//! modulation accent. The constants are bit-equal to the hardcoded
//! patch in `crates/rawdaw-synth-wavetable/src/lib.rs` (U2 lifts those
//! constants to data; U3 will have the synth crate read this `default`
//! and verify audio is byte-identical).

use serde::{Deserialize, Serialize};

/// Current wavetable-patch format version. v2 adds the
/// [`ModSourceData::MidiCC`] variant (K5 of the MIDI input plan).
pub const WAVETABLE_PATCH_FORMAT_VERSION: u32 = 2;

/// Number of oscillators per voice. Mirrors `rawdaw-dsp::NUM_OSCS_PER_VOICE`.
pub const NUM_OSCS: usize = 3;

/// Number of envelopes per voice. ENV1 (amp) + ENV2/ENV3 (free).
pub const NUM_ENVS: usize = 3;

/// Number of mod matrix slots. Mirrors `rawdaw-synth-wavetable::MOD_MATRIX_SLOTS`.
pub const MOD_MATRIX_SLOTS: usize = 16;

/// Per-oscillator static params.
///
/// Mirror of `rawdaw-dsp::oscillator::WavetableOscParams`. Tune is
/// integer semitones in `-24..=24`; fine cents in `-100..=100`; level
/// is the mix gain into the voice sum.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WavetableOscParamsData {
    pub tune_semitones: i8,
    pub fine_cents: i8,
    pub level: f32,
}

impl Default for WavetableOscParamsData {
    fn default() -> Self {
        Self {
            tune_semitones: 0,
            fine_cents: 0,
            level: 0.0,
        }
    }
}

/// ADSR envelope params.
///
/// Mirror of `rawdaw-dsp::envelope::AdsrParams`. `attack_s` / `decay_s`
/// / `release_s` are in seconds; `sustain_level` is a normalized
/// amplitude in `0.0..=1.0`. A `sustain_level` of 0 turns the envelope
/// into a one-shot AD shape (used by every drum-synth amp envelope).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AdsrParamsData {
    pub attack_s: f32,
    pub decay_s: f32,
    pub sustain_level: f32,
    pub release_s: f32,
}

impl Default for AdsrParamsData {
    fn default() -> Self {
        // Inert default — never triggered. Patches override with real
        // shapes (see `WavetablePatchData::default()`).
        Self {
            attack_s: 0.0,
            decay_s: 0.0,
            sustain_level: 1.0,
            release_s: 0.0,
        }
    }
}

/// Modulation source.
///
/// Mirror of `rawdaw-dsp::modulation::ModSource`. The discriminants
/// match the dsp enum 1:1 — the synth crate's `From` impl is a plain
/// match.
///
/// `None` is the inactive-slot sentinel; a [`ModSlotData`] with
/// `source = None` contributes nothing regardless of its destination
/// or amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ModSourceData {
    #[default]
    None,
    Env1,
    Env2,
    Env3,
    Lfo1,
    Osc0,
    Osc1,
    Osc2,
    /// K5 — incoming MIDI controller, by CC number (`0..=127`). The
    /// runtime mirror is `rawdaw_dsp::ModSource::MidiCC(u8)`. CC
    /// numbers outside `0..=127` are sanitized to
    /// [`ModSourceData::None`] when the runtime matrix is built.
    MidiCC(u8),
}

/// Modulation destination.
///
/// Mirror of `rawdaw-dsp::modulation::ModDestination`. The `u8` index
/// in `OscLevel` / `OscTune` / `OscFineTune` / `PmAmountOf` /
/// `AmAmountOf` / `RmAmountOf` selects an oscillator in `0..=2`.
///
/// `FilterCutoff` is the safe default — slots with `source = None`
/// ignore their destination, so the default is arbitrary; we pick the
/// most common one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ModDestinationData {
    #[default]
    FilterCutoff,
    FilterResonance,
    OscLevel(u8),
    OscTune(u8),
    OscFineTune(u8),
    PmAmountOf(u8),
    AmAmountOf(u8),
    RmAmountOf(u8),
    LfoRate,
}

/// One mod matrix slot.
///
/// Mirror of `rawdaw-dsp::modulation::ModSlot`. `amount` is signed and
/// dimensionless; the destination defines what `amount = 1.0` means in
/// its native units (see the design-decisions section of
/// `docs/wavetable-synth-mod-matrix-plan.md`).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ModSlotData {
    pub source: ModSourceData,
    pub destination: ModDestinationData,
    pub amount: f32,
}

/// A complete serializable wavetable-synth patch.
///
/// See module docs for the conversion contract. The default is the M5
/// patch (saw + 3-osc PM stack + ENV2 plucked-filter envelope + slow
/// ENV3 modulation accent + LFO cutoff wobble).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WavetablePatchData {
    pub format_version: u32,
    pub osc_params: [WavetableOscParamsData; NUM_OSCS],
    pub env_params: [AdsrParamsData; NUM_ENVS],
    pub lfo_rate_hz: f32,
    pub filter_cutoff_hz: f32,
    pub filter_resonance: f32,
    pub matrix: [ModSlotData; MOD_MATRIX_SLOTS],
}

impl Default for WavetablePatchData {
    fn default() -> Self {
        // Numbers are bit-equal to the constants in
        // `crates/rawdaw-synth-wavetable/src/lib.rs` (M5 default patch).
        // U3 will replace the synth-side constants with reads from this
        // default and verify audio byte-identity.

        let osc_params = [
            // osc[0]: saw at played pitch, full level — principal voice.
            WavetableOscParamsData {
                tune_semitones: 0,
                fine_cents: 0,
                level: 1.0,
            },
            // osc[1]: octave above, 0.4 mix.
            WavetableOscParamsData {
                tune_semitones: 12,
                fine_cents: 0,
                level: 0.4,
            },
            // osc[2]: perfect twelfth above, modulator-only (level 0).
            WavetableOscParamsData {
                tune_semitones: 19,
                fine_cents: 0,
                level: 0.0,
            },
        ];

        let env_params = [
            // ENV1 (amp): 5 ms / 80 ms / 0.7 / 200 ms.
            AdsrParamsData {
                attack_s: 0.005,
                decay_s: 0.080,
                sustain_level: 0.7,
                release_s: 0.200,
            },
            // ENV2 (filter pluck): 5 ms / 250 ms / 0.0 / 50 ms — decays
            // into nothing for classic plucked-filter motion.
            AdsrParamsData {
                attack_s: 0.005,
                decay_s: 0.250,
                sustain_level: 0.0,
                release_s: 0.050,
            },
            // ENV3 (modulation accent): 5 ms / 400 ms / 0.3 / 200 ms.
            AdsrParamsData {
                attack_s: 0.005,
                decay_s: 0.400,
                sustain_level: 0.3,
                release_s: 0.200,
            },
        ];

        // Default matrix: five M5 slots + one K5 expressive slot, the
        // remaining 10 empty.
        let mut matrix = [ModSlotData::default(); MOD_MATRIX_SLOTS];
        matrix[0] = ModSlotData {
            source: ModSourceData::Lfo1,
            destination: ModDestinationData::FilterCutoff,
            amount: 0.1,
        };
        matrix[1] = ModSlotData {
            source: ModSourceData::Env2,
            destination: ModDestinationData::FilterCutoff,
            amount: 0.6,
        };
        matrix[2] = ModSlotData {
            source: ModSourceData::Osc1,
            destination: ModDestinationData::PmAmountOf(0),
            amount: 0.3,
        };
        matrix[3] = ModSlotData {
            source: ModSourceData::Osc2,
            destination: ModDestinationData::PmAmountOf(1),
            amount: 0.15,
        };
        matrix[4] = ModSlotData {
            source: ModSourceData::Env3,
            destination: ModDestinationData::PmAmountOf(0),
            amount: -0.15,
        };
        // K5 — mod wheel → filter cutoff is the universal default
        // mapping every hardware/software synth ships with. At
        // amount = 0.8 the mod wheel can lift the cutoff by ~3200 Hz,
        // giving the user instant expressive control without having
        // to touch the matrix editor. Slot is inert until the wheel
        // moves (CC table boots at zero).
        matrix[5] = ModSlotData {
            source: ModSourceData::MidiCC(1),
            destination: ModDestinationData::FilterCutoff,
            amount: 0.8,
        };

        Self {
            format_version: WAVETABLE_PATCH_FORMAT_VERSION,
            osc_params,
            env_params,
            lfo_rate_hz: 4.0,
            filter_cutoff_hz: 800.0,
            filter_resonance: 2.5,
            matrix,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_patch_pins_m5_constants() {
        // These numeric pins exist to catch any drift between this
        // mirror's `default()` and the synth-crate constants. If a
        // tuning change to the M5 patch lands, both sides have to
        // update in lockstep — U3's audio byte-identity test will
        // catch any miss in the unlikely event this one slips.
        let p = WavetablePatchData::default();

        assert_eq!(p.format_version, 2);
        assert_eq!(p.lfo_rate_hz, 4.0);
        assert_eq!(p.filter_cutoff_hz, 800.0);
        assert_eq!(p.filter_resonance, 2.5);

        assert_eq!(p.osc_params[0].tune_semitones, 0);
        assert_eq!(p.osc_params[0].level, 1.0);
        assert_eq!(p.osc_params[1].tune_semitones, 12);
        assert_eq!(p.osc_params[1].level, 0.4);
        assert_eq!(p.osc_params[2].tune_semitones, 19);
        assert_eq!(p.osc_params[2].level, 0.0);

        assert_eq!(p.env_params[0].sustain_level, 0.7);
        assert_eq!(p.env_params[1].sustain_level, 0.0);
        assert_eq!(p.env_params[2].sustain_level, 0.3);

        // Matrix slots [0..5] are the M5 routes; [5] is the K5 mod
        // wheel slot; [6..16] are empty.
        assert_eq!(p.matrix[0].source, ModSourceData::Lfo1);
        assert_eq!(p.matrix[0].destination, ModDestinationData::FilterCutoff);
        assert_eq!(p.matrix[0].amount, 0.1);
        assert_eq!(p.matrix[1].source, ModSourceData::Env2);
        assert_eq!(p.matrix[1].amount, 0.6);
        assert_eq!(p.matrix[4].source, ModSourceData::Env3);
        assert_eq!(p.matrix[4].destination, ModDestinationData::PmAmountOf(0));
        assert_eq!(p.matrix[4].amount, -0.15);
        assert_eq!(p.matrix[5].source, ModSourceData::MidiCC(1));
        assert_eq!(p.matrix[5].destination, ModDestinationData::FilterCutoff);
        assert_eq!(p.matrix[5].amount, 0.8);
        for slot in &p.matrix[6..] {
            assert_eq!(slot.source, ModSourceData::None);
        }
    }

    #[test]
    fn empty_mod_slot_default_is_inactive() {
        let s = ModSlotData::default();
        assert_eq!(s.source, ModSourceData::None);
        assert_eq!(s.amount, 0.0);
    }

    #[test]
    fn ron_round_trip_preserves_default_patch() {
        // The model crate uses ron for project serialization (see
        // `Project::save_ron`); patches go through the same format.
        let original = WavetablePatchData::default();
        let s = ron::ser::to_string(&original).expect("serialize");
        let restored: WavetablePatchData = ron::de::from_str(&s).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn ron_round_trip_preserves_custom_patch() {
        let mut original = WavetablePatchData::default();
        // Mutate every category so the round-trip exercises each field.
        original.osc_params[1].fine_cents = -33;
        original.env_params[2].attack_s = 1.5;
        original.filter_cutoff_hz = 2222.0;
        original.matrix[7] = ModSlotData {
            source: ModSourceData::Env1,
            destination: ModDestinationData::OscLevel(2),
            amount: 0.75,
        };

        let s = ron::ser::to_string(&original).expect("serialize");
        let restored: WavetablePatchData = ron::de::from_str(&s).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn ron_round_trip_preserves_midi_cc_source() {
        // K5: a MidiCC source slot must survive serialization. Cover
        // the keyboard-controller subset (mod wheel CC1, breath CC2,
        // volume CC7, pan CC10, expression CC11) plus the boundary
        // values 0 and 127.
        for cc in [0u8, 1, 2, 7, 10, 11, 64, 127] {
            let mut original = WavetablePatchData::default();
            original.matrix[8] = ModSlotData {
                source: ModSourceData::MidiCC(cc),
                destination: ModDestinationData::FilterCutoff,
                amount: 0.5,
            };
            let s = ron::ser::to_string(&original).expect("serialize");
            let restored: WavetablePatchData = ron::de::from_str(&s).expect("deserialize");
            assert_eq!(restored.matrix[8].source, ModSourceData::MidiCC(cc));
            assert_eq!(original, restored);
        }
    }
}
