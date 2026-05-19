//! Modulation matrix value types — sources, destinations, slot
//! records, the per-sample [`Modulations`] bag, and the per-
//! destination native-unit `scale` constants.
//!
//! These are the "what flows" of the matrix. The "how the matrix is
//! built and validated" lives in [`super::engine`].

/// Number of oscillators per voice this matrix understands. Aligned
/// with `rawdaw-synth-wavetable::NUM_OSCS` (= 3); the matrix carries
/// per-osc destination arrays of this width.
///
/// This couples the matrix to a fixed-osc-count assumption — the
/// alternative (generic over osc count) propagates const generics
/// into every downstream type. v2 takes the coupling; v3 can lift
/// it if multi-osc-count synths arrive.
pub const NUM_OSCS_PER_VOICE: usize = 3;

/// Where a modulation value originates.
///
/// Sources output normalized values: envelopes in `[0.0, 1.0]`,
/// LFOs and oscillators in `[-1.0, 1.0]`. The matrix multiplies a
/// source's value by the slot's `amount`; the destination applies
/// its native scale (see [`ModDestination`] documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModSource {
    /// Inactive slot — the matrix skips it.
    #[default]
    None,
    /// Amp envelope (ENV1).
    Env1,
    /// Free modulation envelope 2 (ENV2).
    Env2,
    /// Free modulation envelope 3 (ENV3).
    Env3,
    /// Shared global LFO. v2 supports one LFO; Vital has four.
    Lfo1,
    /// Audio-rate output of oscillator 0.
    Osc0,
    /// Audio-rate output of oscillator 1.
    Osc1,
    /// Audio-rate output of oscillator 2.
    Osc2,
    /// K5 — incoming MIDI controller, by CC number (`0..=127`). The
    /// consumer reads `cc_state[cc as usize]` (normalized
    /// `[0.0, 1.0]`) at slot-eval time. Control-rate; not audio-rate.
    /// Out-of-range CC numbers (≥ 128) are sanitized to
    /// [`ModSource::None`] by [`super::ModMatrix::set_slots`] so the
    /// evaluator never indexes past the table.
    MidiCC(u8),
}

impl ModSource {
    /// `true` if this source's value is only available *during* the
    /// per-sample audio render (oscillator outputs). Audio-rate
    /// sources participate in the topological sort that determines
    /// oscillator render order.
    pub fn is_audio_rate(&self) -> bool {
        matches!(self, ModSource::Osc0 | ModSource::Osc1 | ModSource::Osc2)
    }

    /// For oscillator sources, the osc index `0..=2`. `None` for
    /// control-rate sources (envelopes, LFOs, MIDI CCs).
    pub fn osc_index(&self) -> Option<u8> {
        match self {
            ModSource::Osc0 => Some(0),
            ModSource::Osc1 => Some(1),
            ModSource::Osc2 => Some(2),
            _ => None,
        }
    }
}

/// A modulation target.
///
/// Each variant defines what `amount = 1.0` in a [`ModSlot`] means
/// in its native unit (see `docs/wavetable-synth-mod-matrix-plan.md`
/// for the canonical scale table):
///
/// | Variant | `amount = 1.0` ⇒ |
/// |---|---|
/// | `FilterCutoff` | + 4000 Hz cutoff swing |
/// | `FilterResonance` | + 1.0 Q (additive) |
/// | `OscLevel(i)` | + 1.0 mix level (additive) |
/// | `OscTune(i)` | + 24 semitones |
/// | `OscFineTune(i)` | + 100 cents |
/// | `PmAmountOf(i)` | + 1.0 phase swing |
/// | `AmAmountOf(i)` | + 1.0 amplitude coefficient |
/// | `RmAmountOf(i)` | + 1.0 ring-mix wet |
/// | `LfoRate` | + 4 Hz LFO rate |
///
/// `Copy + Default`. Default is `FilterCutoff` — an arbitrary safe
/// pick; slots with `source = ModSource::None` ignore their
/// destination anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModDestination {
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

impl ModDestination {
    /// The osc index for any osc-targeted destination. `None` for
    /// filter or LFO destinations.
    pub fn osc_index(&self) -> Option<u8> {
        match self {
            ModDestination::OscLevel(i)
            | ModDestination::OscTune(i)
            | ModDestination::OscFineTune(i)
            | ModDestination::PmAmountOf(i)
            | ModDestination::AmAmountOf(i)
            | ModDestination::RmAmountOf(i) => Some(*i),
            _ => None,
        }
    }

    /// `true` if this destination targets a valid oscillator index
    /// (`0..NUM_OSCS_PER_VOICE`) — or if it doesn't target an
    /// oscillator at all (filter, LFO).
    pub fn has_valid_osc_index(&self) -> bool {
        match self.osc_index() {
            None => true,
            Some(i) => (i as usize) < NUM_OSCS_PER_VOICE,
        }
    }
}

/// One row in the modulation matrix.
///
/// `Copy + Default`. Default is an inactive slot (`source = None`,
/// `destination = FilterCutoff`, `amount = 0.0`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ModSlot {
    pub source: ModSource,
    pub destination: ModDestination,
    pub amount: f32,
}

impl ModSlot {
    /// `true` if this slot contributes to per-sample modulation.
    pub fn is_active(&self) -> bool {
        self.source != ModSource::None
    }

    /// `true` if this slot is an osc→osc audio-rate route — its
    /// modulator must render before its carrier.
    pub fn is_audio_rate_osc_route(&self) -> bool {
        self.source.is_audio_rate() && self.destination.osc_index().is_some()
    }
}

/// Per-sample modulation contributions, summed across all active
/// slots for a single sample.
///
/// Each consumer (filter, per-osc render) adds the appropriate
/// `*_offset` to its base value before applying it. Defaults to all
/// zero — an inactive matrix produces a `Modulations` that doesn't
/// alter the signal path.
#[derive(Debug, Clone, Copy, Default)]
pub struct Modulations {
    pub filter_cutoff_hz_offset: f32,
    pub filter_resonance_offset: f32,
    pub osc_level_offset: [f32; NUM_OSCS_PER_VOICE],
    pub osc_tune_offset: [f32; NUM_OSCS_PER_VOICE],
    pub osc_fine_offset: [f32; NUM_OSCS_PER_VOICE],
    pub pm_amount_offset: [f32; NUM_OSCS_PER_VOICE],
    pub am_amount_offset: [f32; NUM_OSCS_PER_VOICE],
    pub rm_amount_offset: [f32; NUM_OSCS_PER_VOICE],
    pub lfo_rate_offset: f32,
}

/// Per-destination scale: multiplier from "slot amount × source
/// value" (both in `[-1, 1]`-ish) into the destination's native
/// units (Hz, semitones, cents, dimensionless coefficients).
///
/// Centralizing these means a patch's `amount = 1.0` carries
/// portable meaning across all destinations.
pub mod scale {
    pub const FILTER_CUTOFF_HZ: f32 = 4000.0;
    pub const FILTER_RESONANCE: f32 = 1.0;
    pub const OSC_LEVEL: f32 = 1.0;
    pub const OSC_TUNE_SEMITONES: f32 = 24.0;
    pub const OSC_FINE_CENTS: f32 = 100.0;
    pub const PM_AMOUNT: f32 = 1.0;
    pub const AM_AMOUNT: f32 = 1.0;
    pub const RM_AMOUNT: f32 = 1.0;
    pub const LFO_RATE_HZ: f32 = 4.0;
}

impl Modulations {
    /// Add a single source's contribution at this sample into the
    /// destination's slot. `source_value` is the modulator's
    /// normalized output; `amount` is the slot's signed depth.
    /// The destination's per-unit scale is applied here.
    pub fn add_contribution(
        &mut self,
        destination: ModDestination,
        source_value: f32,
        amount: f32,
    ) {
        let contribution = source_value * amount;
        match destination {
            ModDestination::FilterCutoff => {
                self.filter_cutoff_hz_offset += contribution * scale::FILTER_CUTOFF_HZ;
            }
            ModDestination::FilterResonance => {
                self.filter_resonance_offset += contribution * scale::FILTER_RESONANCE;
            }
            ModDestination::OscLevel(i) => {
                if let Some(slot) = self.osc_level_offset.get_mut(i as usize) {
                    *slot += contribution * scale::OSC_LEVEL;
                }
            }
            ModDestination::OscTune(i) => {
                if let Some(slot) = self.osc_tune_offset.get_mut(i as usize) {
                    *slot += contribution * scale::OSC_TUNE_SEMITONES;
                }
            }
            ModDestination::OscFineTune(i) => {
                if let Some(slot) = self.osc_fine_offset.get_mut(i as usize) {
                    *slot += contribution * scale::OSC_FINE_CENTS;
                }
            }
            ModDestination::PmAmountOf(i) => {
                if let Some(slot) = self.pm_amount_offset.get_mut(i as usize) {
                    *slot += contribution * scale::PM_AMOUNT;
                }
            }
            ModDestination::AmAmountOf(i) => {
                if let Some(slot) = self.am_amount_offset.get_mut(i as usize) {
                    *slot += contribution * scale::AM_AMOUNT;
                }
            }
            ModDestination::RmAmountOf(i) => {
                // RM uses `output = raw × (1 + rm_amount_offset)`
                // for consumer symmetry with AM, but the per-slot
                // formula is `(source - 1) × amount × scale` instead
                // of `source × amount × scale`. Why: v1's single-slot
                // RM is `output = raw × (m × a + (1 - a))`, which
                // simplifies to `raw × (1 + (m - 1) × a)`. Treating
                // each slot's contribution as `(m - 1) × a` and
                // summing them generalizes the v1 formula to multi-
                // slot RM with well-defined "more-wet" semantics.
                if let Some(slot) = self.rm_amount_offset.get_mut(i as usize) {
                    *slot += (source_value - 1.0) * amount * scale::RM_AMOUNT;
                }
            }
            ModDestination::LfoRate => {
                self.lfo_rate_offset += contribution * scale::LFO_RATE_HZ;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modulations_add_contribution_routes_to_right_field() {
        let mut m = Modulations::default();
        m.add_contribution(ModDestination::FilterCutoff, 1.0, 0.1);
        // 1.0 × 0.1 × 4000 Hz scale = 400 Hz offset.
        assert!((m.filter_cutoff_hz_offset - 400.0).abs() < 1e-3);

        m.add_contribution(ModDestination::PmAmountOf(0), 1.0, 0.5);
        // 1.0 × 0.5 × 1.0 PM scale = 0.5.
        assert!((m.pm_amount_offset[0] - 0.5).abs() < 1e-6);

        m.add_contribution(ModDestination::OscTune(2), 0.5, 1.0);
        // 0.5 × 1.0 × 24 semi scale = 12 semis.
        assert!((m.osc_tune_offset[2] - 12.0).abs() < 1e-4);
    }

    #[test]
    fn rm_contribution_uses_source_minus_one_formula() {
        // RM's contribution is `(source - 1) × amount × scale` so
        // that the consumer's `output = raw × (1 + rm_offset)`
        // reproduces v1's single-slot `raw × (m × a + (1 - a))`.
        let mut m = Modulations::default();
        // amount = 1.0, source = 1.0 ⇒ contribution (1 - 1) × 1 = 0.
        // Consumer: raw × (1 + 0) = raw. (Pure wet RM at modulator
        // peak passes the carrier through unchanged.)
        m.add_contribution(ModDestination::RmAmountOf(0), 1.0, 1.0);
        assert!((m.rm_amount_offset[0] - 0.0).abs() < 1e-6);

        // amount = 1.0, source = -1.0 ⇒ contribution (-1 - 1) × 1 = -2.
        // Consumer: raw × (1 - 2) = -raw. (Pure wet RM flips sign
        // when modulator is at -1 — classic four-quadrant ring mod.)
        let mut m = Modulations::default();
        m.add_contribution(ModDestination::RmAmountOf(0), -1.0, 1.0);
        assert!((m.rm_amount_offset[0] - (-2.0)).abs() < 1e-6);

        // amount = 0.5, source = 0.0 ⇒ contribution (0 - 1) × 0.5 = -0.5.
        // Consumer: raw × 0.5. (50% dry/wet at zero modulator gives
        // half-attenuated carrier — matches v1.)
        let mut m = Modulations::default();
        m.add_contribution(ModDestination::RmAmountOf(0), 0.0, 0.5);
        assert!((m.rm_amount_offset[0] - (-0.5)).abs() < 1e-6);

        // amount = 0.0, any source ⇒ contribution 0.
        // Consumer: raw × 1 = raw (full dry).
        let mut m = Modulations::default();
        m.add_contribution(ModDestination::RmAmountOf(0), 0.5, 0.0);
        assert!((m.rm_amount_offset[0] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn modulations_default_is_all_zero() {
        let m = Modulations::default();
        assert_eq!(m.filter_cutoff_hz_offset, 0.0);
        assert_eq!(m.filter_resonance_offset, 0.0);
        assert_eq!(m.osc_level_offset, [0.0; NUM_OSCS_PER_VOICE]);
        assert_eq!(m.pm_amount_offset, [0.0; NUM_OSCS_PER_VOICE]);
        assert_eq!(m.lfo_rate_offset, 0.0);
    }

    #[test]
    fn mod_source_audio_rate_classification() {
        assert!(!ModSource::None.is_audio_rate());
        assert!(!ModSource::Env1.is_audio_rate());
        assert!(!ModSource::Lfo1.is_audio_rate());
        assert!(ModSource::Osc0.is_audio_rate());
        assert!(ModSource::Osc1.is_audio_rate());
        assert!(ModSource::Osc2.is_audio_rate());
        // MIDI CCs are control-rate — they update from incoming MIDI
        // events, never from per-sample audio output.
        assert!(!ModSource::MidiCC(1).is_audio_rate());
        assert!(!ModSource::MidiCC(64).is_audio_rate());
        assert!(ModSource::MidiCC(0).osc_index().is_none());
    }

    #[test]
    fn mod_destination_osc_index_extraction() {
        assert_eq!(ModDestination::FilterCutoff.osc_index(), None);
        assert_eq!(ModDestination::LfoRate.osc_index(), None);
        assert_eq!(ModDestination::PmAmountOf(1).osc_index(), Some(1));
        assert_eq!(ModDestination::OscTune(2).osc_index(), Some(2));
    }
}
