//! Per-oscillator parameters + tuning helpers.
//!
//! Configuration types for multi-oscillator voices, lifted here so
//! every synth crate (wavetable, future physical, future FM-operator)
//! can share the per-osc parameter shape without each redefining it.
//! Pure data + tiny helpers; no DSP state lives in this module.
//!
//! ## v2 shape
//!
//! v1 carried per-osc `mod_source` / `mod_mode` / `mod_amount` fields
//! describing PM/AM/RM routing inline with each oscillator's params.
//! v2 removes those — modulation routing moved to the slot-based
//! [`crate::modulation::ModMatrix`]. `WavetableOscParams` now carries
//! only the per-osc state that's an intrinsic property of the
//! oscillator's identity in the patch (pitch + mix level), not
//! anything about how it's routed.

/// Standard concert tuning. A4 = 440 Hz at MIDI note 69. Pinned in
/// this module so the synth crates don't each redefine it; variable-
/// A4 / micro-tuning is a v2+ growth item.
const A4_HZ: f32 = 440.0;
const A4_NOTE: f32 = 69.0;

/// Frequency in Hz for `base_note` shifted by `semi` semitones and
/// `cents` cents, in standard concert tuning.
///
/// `semi` is a coarse offset (conventional range `-24..=24`);
/// `cents` is a fine offset (conventional range `-100..=100`). Out-
/// of-range values are accepted but produce out-of-pitch results —
/// no clamping, no panic.
///
/// Allocation-free; performs one `powf`. Intended for `note_on`-time
/// precomputation into a per-voice frequency cache, not per-sample
/// use.
pub fn note_offset_hz(base_note: u8, semi: i8, cents: i8) -> f32 {
    let offset_semis = semi as f32 + cents as f32 / 100.0;
    let n = base_note as f32 + offset_semis - A4_NOTE;
    A4_HZ * 2.0_f32.powf(n / 12.0)
}

/// Per-oscillator configuration for a multi-oscillator voice.
///
/// `Copy + Default`. Default is `tune_semitones = 0`, `fine_cents =
/// 0`, `level = 0.0` — an inactive oscillator that ticks at the
/// played note but contributes nothing to the audio mix.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WavetableOscParams {
    /// Coarse pitch offset in semitones relative to the played MIDI
    /// note. Conventional range is `-24..=24`.
    pub tune_semitones: i8,
    /// Fine pitch offset in cents. Conventional range is
    /// `-100..=100`.
    pub fine_cents: i8,
    /// Mix gain into the voice sum, in `[0.0, 1.0]`. `0.0` makes the
    /// oscillator silent in the mix — useful for modulator-only
    /// oscillators that contribute via matrix slots (PM, AM, RM,
    /// audio-rate tune) but not the audio sum.
    pub level: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_offset_hz_at_a4_is_440() {
        let hz = note_offset_hz(69, 0, 0);
        assert!(
            (hz - 440.0).abs() < 1e-3,
            "MIDI 69 with zero offset should be 440 Hz, got {hz}",
        );
    }

    #[test]
    fn note_offset_hz_octave_up_doubles() {
        let base = note_offset_hz(60, 0, 0);
        let octave = note_offset_hz(60, 12, 0);
        let ratio = octave / base;
        assert!(
            (ratio - 2.0).abs() < 1e-5,
            "tune_semitones = 12 should double the frequency; ratio = {ratio}",
        );
    }

    #[test]
    fn note_offset_hz_semitone_matches_powf() {
        // A semitone is 2^(1/12). Pin the math by cross-checking
        // against the same formula expressed via base_note instead
        // of semi: note_offset_hz(60, 1, 0) == note_offset_hz(61, 0, 0).
        let via_semi = note_offset_hz(60, 1, 0);
        let via_note = note_offset_hz(61, 0, 0);
        assert!(
            (via_semi - via_note).abs() < 1e-5,
            "semi offset and note offset should agree: {via_semi} vs {via_note}",
        );
    }

    #[test]
    fn note_offset_hz_cents_match_hundredth_of_semitone() {
        // 100 cents = 1 semitone. Pin: note_offset_hz(60, 0, 100)
        // matches note_offset_hz(60, 1, 0).
        let via_cents = note_offset_hz(60, 0, 100);
        let via_semi = note_offset_hz(60, 1, 0);
        assert!(
            (via_cents - via_semi).abs() < 1e-5,
            "100 cents == 1 semitone: {via_cents} vs {via_semi}",
        );
    }

    #[test]
    fn default_params_are_inactive() {
        let p = WavetableOscParams::default();
        assert_eq!(p.tune_semitones, 0);
        assert_eq!(p.fine_cents, 0);
        assert_eq!(p.level, 0.0);
    }
}
