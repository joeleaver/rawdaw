//! Wavetable oscillator — phase accumulator + linear interpolation read.
//!
//! [`Wavetable`] owns the sample buffer ([`TABLE_LEN`] f32 samples
//! representing one period of the waveform). [`WavetableOsc`] holds
//! just the per-voice state (phase + phase increment) and reads from
//! a borrowed `&Wavetable` each `tick`. This keeps the oscillator
//! struct small and Copy-clean so a voice pool can hold it without
//! lifetimes; the synth node owns the wavetable bank.
//!
//! **v0 limitation: single-table, no mip-mapping.** At notes whose
//! phase increment crosses 0.5/N (where N is the number of harmonics
//! in the table) the high harmonics fold back as aliasing. The
//! factory `Wavetable::saw_default` is band-limited to 128 harmonics
//! so the aliasing is audible but not extreme on top-octave notes.
//! Per-pitch mip-maps are a v1 growth target.
//!
//! **RT-safety.** Construction (`Wavetable::saw_default`) allocates
//! the sample buffer. `WavetableOsc::tick` is allocation-free.

use core::f32::consts::TAU;

/// Number of samples per wavetable period. 2048 is the standard
/// choice — its own Nyquist sits at harmonic 1024, leaving headroom
/// for the band-limited factory saw's 128 harmonics.
pub const TABLE_LEN: usize = 2048;

/// One period of a waveform sampled at [`TABLE_LEN`] points.
///
/// Stored as `Box<[f32]>` because the length is fixed at
/// construction time; `Vec<f32>`'s capacity overhead and reallocation
/// surface would be dead weight here.
#[derive(Debug, Clone)]
pub struct Wavetable {
    samples: Box<[f32]>,
}

impl Wavetable {
    /// Build a band-limited saw wavetable from `num_harmonics`
    /// sinusoids via additive synthesis. Amplitude follows the
    /// standard 1/h saw spectrum.
    ///
    /// `num_harmonics` is clamped to `TABLE_LEN / 2` (the table's
    /// own Nyquist limit) so a caller asking for more harmonics
    /// than the table can represent gets a defensively-bandlimited
    /// result instead of a corrupt one.
    pub fn band_limited_saw(num_harmonics: usize) -> Self {
        let h_max = num_harmonics.min(TABLE_LEN / 2);
        let mut samples = vec![0.0_f32; TABLE_LEN].into_boxed_slice();
        for (i, sample) in samples.iter_mut().enumerate() {
            let phase = i as f32 / TABLE_LEN as f32; // 0..1
            let mut s = 0.0_f32;
            for h in 1..=h_max {
                let h_f = h as f32;
                // Saw: -2/pi * sum_{h=1..} sin(2*pi*h*phase) / h.
                // The leading sign is folded into the sum: producing
                // -sin gives a saw that goes 1 → -1 then jumps back
                // up, which is the conventional "ramp" shape.
                s -= (TAU * h_f * phase).sin() / h_f;
            }
            // Normalize so peak amplitude is approximately 1.0 —
            // the band-limited saw's peak is roughly 2/pi * H(N) where
            // H(N) is the Nth harmonic number, and we don't want the
            // overall level to depend on the harmonic count.
            *sample = s * 2.0 / core::f32::consts::PI;
        }
        // Final normalize pass: scale so |max| == 1.0 exactly. The
        // analytical scaling above is approximate; this guarantees
        // headroom.
        let peak = samples
            .iter()
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max);
        if peak > 1e-9 {
            for s in samples.iter_mut() {
                *s /= peak;
            }
        }
        Self { samples }
    }

    /// v0 factory wavetable: band-limited saw with 128 harmonics.
    /// Bright but not aliasing through the floor.
    pub fn saw_default() -> Self {
        Self::band_limited_saw(128)
    }

    /// Linear-interpolated read at fractional phase `p` in `[0, 1)`.
    /// Phase values outside that range wrap modulo 1.
    pub fn sample(&self, phase: f32) -> f32 {
        // Wrap to [0, 1). `rem_euclid` handles negative phases too.
        let p = phase.rem_euclid(1.0);
        let scaled = p * TABLE_LEN as f32;
        let idx0 = scaled as usize;
        let idx1 = (idx0 + 1) % TABLE_LEN;
        let frac = scaled - idx0 as f32;
        let a = self.samples[idx0 % TABLE_LEN];
        let b = self.samples[idx1];
        a + (b - a) * frac
    }

    /// Borrowed sample slice — useful for tests / DSP that wants the
    /// raw buffer.
    pub fn as_slice(&self) -> &[f32] {
        &self.samples
    }
}

/// Per-voice oscillator state: phase + phase increment.
///
/// Holds no reference to the wavetable; the synth passes the table
/// to [`Self::tick`] each sample. This keeps the struct `Copy` and
/// lifetime-free so voice pools can hold it without juggling
/// references.
///
/// Frequency control: [`Self::set_frequency`] computes a phase
/// increment from `freq_hz / sample_rate`. Concrete users (the
/// wavetable synth's Voice) will typically have a precomputed
/// MIDI-note → frequency table built at prepare-time.
#[derive(Debug, Clone, Copy)]
pub struct WavetableOsc {
    phase: f32,
    phase_inc: f32,
    sample_rate: f32,
}

impl WavetableOsc {
    /// Construct at zero phase. `prepare` must be called before
    /// `tick` so `sample_rate` is set.
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            phase_inc: 0.0,
            sample_rate: 0.0,
        }
    }

    /// Configure the sample rate. Subsequent [`Self::set_frequency`]
    /// calls use this rate. Resets the phase to 0.
    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate as f32;
        self.phase = 0.0;
    }

    /// Set the playback frequency in Hz. `prepare` must have been
    /// called; calling before `prepare` is a no-op (phase_inc stays 0).
    pub fn set_frequency(&mut self, hz: f32) {
        if self.sample_rate > 0.0 {
            self.phase_inc = hz / self.sample_rate;
        }
    }

    /// Reset the phase to 0 — useful for note retriggering where
    /// callers want a phase-aligned waveform start.
    pub fn reset_phase(&mut self) {
        self.phase = 0.0;
    }

    /// Read one sample from `wavetable` at the current phase and
    /// advance. Allocation-free.
    pub fn tick(&mut self, wavetable: &Wavetable) -> f32 {
        let sample = wavetable.sample(self.phase);
        self.phase += self.phase_inc;
        // Keep phase in [0, 1) without an unbounded growth that would
        // eventually destroy precision. `rem_euclid` handles negative
        // wraps too if set_frequency is ever passed a negative value.
        if self.phase >= 1.0 || self.phase < 0.0 {
            self.phase = self.phase.rem_euclid(1.0);
        }
        sample
    }
}

impl Default for WavetableOsc {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    #[test]
    fn saw_default_is_normalized() {
        let wt = Wavetable::saw_default();
        let peak = wt.as_slice().iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
        assert!(
            (peak - 1.0).abs() < 1e-5,
            "peak should be normalized to 1.0, got {peak}",
        );
    }

    #[test]
    fn saw_default_has_correct_length() {
        let wt = Wavetable::saw_default();
        assert_eq!(wt.as_slice().len(), TABLE_LEN);
    }

    #[test]
    fn sample_wraps_phase() {
        let wt = Wavetable::saw_default();
        // Three reads that should all map to the same wavetable index.
        let a = wt.sample(0.25);
        let b = wt.sample(1.25);
        let c = wt.sample(-0.75);
        assert!((a - b).abs() < 1e-6);
        assert!((a - c).abs() < 1e-6);
    }

    #[test]
    fn osc_produces_periodic_output_at_an_integer_period() {
        // 480 Hz at 48 kHz gives an exact 100-sample period — no
        // fractional drift, so phase 0 lines up exactly. Saw at A4
        // (440 Hz) would have a 109.09… sample period and drift
        // accumulates over a cycle, so we pin reproducibility on the
        // integer-period case.
        let wt = Wavetable::saw_default();
        let mut osc = WavetableOsc::new();
        osc.prepare(SR);
        osc.set_frequency(480.0);

        let period = 100;
        let samples: Vec<f32> = (0..(period * 3)).map(|_| osc.tick(&wt)).collect();
        // sample[i] should equal sample[i + period] to within float
        // precision since the period is integer.
        let mut max_diff = 0.0_f32;
        for i in 0..period {
            let diff = (samples[i] - samples[i + period]).abs();
            max_diff = max_diff.max(diff);
        }
        // Tolerance: accumulated float error over 100 samples of phase
        // addition is on the order of 1e-3 at f32 precision; that's
        // well below any audible threshold.
        assert!(
            max_diff < 1e-3,
            "integer-period samples should match within accumulated float error; max_diff = {max_diff}",
        );
    }

    #[test]
    fn osc_phase_is_continuous_across_block_boundaries() {
        // Equivalent to running 256 samples in one go vs. two 128-sample
        // blocks; the result should be sample-identical (or at least
        // phase-continuous within float noise).
        let wt = Wavetable::saw_default();
        let mut a = WavetableOsc::new();
        a.prepare(SR);
        a.set_frequency(220.0);
        let mut b = a;
        let one_shot: Vec<f32> = (0..256).map(|_| a.tick(&wt)).collect();
        let mut chunked: Vec<f32> = Vec::with_capacity(256);
        for _ in 0..128 {
            chunked.push(b.tick(&wt));
        }
        for _ in 0..128 {
            chunked.push(b.tick(&wt));
        }
        for i in 0..256 {
            assert!(
                (one_shot[i] - chunked[i]).abs() < 1e-6,
                "phase should be continuous; sample {i}: {} vs {}",
                one_shot[i],
                chunked[i],
            );
        }
    }

    #[test]
    fn osc_before_prepare_is_silent() {
        // Pre-prepare, phase_inc is 0 and the oscillator reads sample 0
        // of the wavetable forever. Defensive: shouldn't panic, even
        // though that's an API misuse.
        let wt = Wavetable::saw_default();
        let mut osc = WavetableOsc::new();
        // set_frequency is a no-op before prepare.
        osc.set_frequency(440.0);
        let a = osc.tick(&wt);
        let b = osc.tick(&wt);
        assert_eq!(a, b, "with phase_inc=0 the osc should read the same sample");
    }
}
