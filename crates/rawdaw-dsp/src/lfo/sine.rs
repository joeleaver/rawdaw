//! Free-running sine LFO.
//!
//! Phase-accumulator with `sin()` per tick. Returns values in
//! `[-1.0, 1.0]`. Tempo sync, retrigger-from-note, and non-sine
//! waveforms are v1 growth targets.
//!
//! **RT-safety.** No allocations; `tick` is one `sin()` call plus a
//! phase update.

use core::f32::consts::TAU;

#[derive(Debug, Clone, Copy)]
pub struct SineLfo {
    sample_rate: f32,
    phase: f32,
    phase_inc: f32,
}

impl SineLfo {
    pub fn new() -> Self {
        Self {
            sample_rate: 0.0,
            phase: 0.0,
            phase_inc: 0.0,
        }
    }

    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate as f32;
    }

    /// Set the LFO rate in Hz. `prepare` must have been called.
    pub fn set_rate_hz(&mut self, hz: f32) {
        if self.sample_rate > 0.0 {
            self.phase_inc = hz / self.sample_rate;
        }
    }

    /// Reset the phase to 0. Useful for note-retrigger if a future
    /// growth pass wants per-voice LFOs that restart on NoteOn.
    pub fn reset_phase(&mut self) {
        self.phase = 0.0;
    }

    /// Advance one sample and return the LFO output in `[-1.0, 1.0]`.
    pub fn tick(&mut self) -> f32 {
        let s = (self.phase * TAU).sin();
        self.phase += self.phase_inc;
        if self.phase >= 1.0 || self.phase < 0.0 {
            self.phase = self.phase.rem_euclid(1.0);
        }
        s
    }
}

impl Default for SineLfo {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    #[test]
    fn output_in_unit_range() {
        let mut lfo = SineLfo::new();
        lfo.prepare(SR);
        lfo.set_rate_hz(5.0);
        for _ in 0..96_000 {
            let v = lfo.tick();
            assert!((-1.0 - 1e-5..=1.0 + 1e-5).contains(&v));
        }
    }

    #[test]
    fn one_hz_completes_one_period_per_second() {
        // At 1 Hz / 48 kHz, after 48_000 samples we should be back at
        // phase 0 → sin = 0.
        let mut lfo = SineLfo::new();
        lfo.prepare(SR);
        lfo.set_rate_hz(1.0);
        for _ in 0..(SR - 1) {
            lfo.tick();
        }
        let final_sample = lfo.tick();
        // Float phase accumulation over 48k samples drifts by ~5e-3
        // at f32 precision — well below any audible threshold for an
        // LFO.
        assert!(
            final_sample.abs() < 1e-2,
            "after one full period should return to ~0; got {final_sample}",
        );
    }

    #[test]
    fn quarter_period_reaches_peak() {
        // At 1 Hz / 48 kHz, after 12_000 samples (1/4 cycle) sin should be ~1.
        let mut lfo = SineLfo::new();
        lfo.prepare(SR);
        lfo.set_rate_hz(1.0);
        for _ in 0..11_999 {
            lfo.tick();
        }
        let v = lfo.tick();
        assert!(
            (v - 1.0).abs() < 1e-3,
            "1/4-period sample should reach ~+1; got {v}",
        );
    }

    #[test]
    fn phase_continuous_across_blocks() {
        let mut a = SineLfo::new();
        a.prepare(SR);
        a.set_rate_hz(3.0);
        let mut b = a;
        let one_shot: Vec<f32> = (0..512).map(|_| a.tick()).collect();
        let mut chunked: Vec<f32> = Vec::with_capacity(512);
        for _ in 0..256 {
            chunked.push(b.tick());
        }
        for _ in 0..256 {
            chunked.push(b.tick());
        }
        for i in 0..512 {
            assert!(
                (one_shot[i] - chunked[i]).abs() < 1e-6,
                "phase continuity broken at sample {i}: {} vs {}",
                one_shot[i],
                chunked[i],
            );
        }
    }
}
