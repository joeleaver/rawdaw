//! State-variable lowpass filter (Cytomic / Andrew Simper formulation).
//!
//! Two-integrator topology with trapezoidal integration. 12 dB/oct
//! slope. Coefficients update only when cutoff or resonance change,
//! so per-sample `tick` is three multiply-adds.
//!
//! See <https://cytomic.com/files/dsp/SvfLinearTrapOptimised2.pdf>
//! for the original derivation. We carry only the lowpass output
//! here; other modes (HP / BP / notch / shelf) reuse the same state
//! and become a v1 growth target.
//!
//! **Stability.** Trapezoidal integration is stable for all
//! cutoff < Nyquist and Q ≥ 0.5. We clamp cutoff to
//! `[1, 0.49 * sample_rate]` defensively so callers driving the
//! cutoff with an LFO can't push the filter into instability.
//!
//! **RT-safety.** No allocations; `tick` is two multiply-adds plus
//! two state updates.

use core::f32::consts::PI;

/// Minimum cutoff in Hz. Below this we stop the filter from
/// approaching DC where the integrators stagnate.
const MIN_CUTOFF_HZ: f32 = 1.0;

/// Cutoff is clamped to `MAX_CUTOFF_FRAC * sample_rate` to keep the
/// filter stable near Nyquist.
const MAX_CUTOFF_FRAC: f32 = 0.49;

/// Minimum resonance (Q). The Cytomic formulation uses `k = 1/Q`;
/// `Q = 0.5` is Butterworth-flat-passband (the standard "no
/// resonance" reference). Q < 0.5 is mathematically fine but
/// musically pointless.
const MIN_Q: f32 = 0.5;

/// State-variable lowpass filter.
#[derive(Debug, Clone, Copy)]
pub struct SvfLowpass {
    sample_rate: f32,
    cutoff_hz: f32,
    q: f32,

    // Pre-computed coefficients refreshed on parameter change.
    g: f32,
    k: f32,
    a1: f32,
    a2: f32,

    // Integrator states.
    ic1eq: f32,
    ic2eq: f32,
}

impl SvfLowpass {
    pub fn new() -> Self {
        Self {
            sample_rate: 0.0,
            cutoff_hz: 1000.0,
            q: 0.707, // butterworth-ish default
            g: 0.0,
            k: 0.0,
            a1: 0.0,
            a2: 0.0,
            ic1eq: 0.0,
            ic2eq: 0.0,
        }
    }

    /// Configure sample rate. Required before `tick`.
    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate as f32;
        self.refresh_coefficients();
    }

    /// Set cutoff frequency in Hz. Clamped to a safe range relative
    /// to the sample rate.
    pub fn set_cutoff(&mut self, hz: f32) {
        let max = self.sample_rate * MAX_CUTOFF_FRAC;
        self.cutoff_hz = hz.clamp(MIN_CUTOFF_HZ, max.max(MIN_CUTOFF_HZ));
        self.refresh_coefficients();
    }

    /// Set resonance (Q). Clamped at `MIN_Q` from below; there's no
    /// hard upper limit because the trapezoidal integrator stays
    /// stable, but extreme Q values produce very narrow peaks that
    /// can ring for a long time.
    pub fn set_resonance(&mut self, q: f32) {
        self.q = q.max(MIN_Q);
        self.refresh_coefficients();
    }

    /// Reset integrator state — useful for re-arming the filter on
    /// a fresh note without any leftover ringing.
    pub fn reset_state(&mut self) {
        self.ic1eq = 0.0;
        self.ic2eq = 0.0;
    }

    /// Filter one sample and return the lowpass output.
    pub fn tick(&mut self, input: f32) -> f32 {
        // Cytomic linear trapezoidal SVF, lowpass output.
        let v3 = input - self.ic2eq;
        let v1 = self.a1 * self.ic1eq + self.a2 * v3;
        let v2 = self.ic2eq + self.g * (self.a1 * self.ic1eq + self.a2 * v3);
        // Update integrator memory.
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;
        v2
    }

    fn refresh_coefficients(&mut self) {
        if self.sample_rate <= 0.0 {
            return;
        }
        // g = tan(pi * cutoff / sample_rate)
        let g = (PI * self.cutoff_hz / self.sample_rate).tan();
        let k = 1.0 / self.q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        self.g = g;
        self.k = k;
        self.a1 = a1;
        self.a2 = a2;
    }
}

impl Default for SvfLowpass {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn fresh(cutoff: f32, q: f32) -> SvfLowpass {
        let mut f = SvfLowpass::new();
        f.prepare(SR);
        f.set_cutoff(cutoff);
        f.set_resonance(q);
        f
    }

    #[test]
    fn dc_passes_at_high_cutoff() {
        let mut f = fresh(20_000.0, 0.707);
        // Drive with DC = 1.0 for many samples; output should converge
        // to ~1.0 (the lowpass passes DC unattenuated).
        let mut last = 0.0;
        for _ in 0..1000 {
            last = f.tick(1.0);
        }
        assert!(
            (last - 1.0).abs() < 0.05,
            "high-cutoff LP should pass DC; got {last}",
        );
    }

    #[test]
    fn low_cutoff_attenuates_high_frequency() {
        // Sine at 8 kHz fed through a filter cut at 200 Hz should be
        // strongly attenuated.
        let mut f = fresh(200.0, 0.707);
        let mut peak_in = 0.0_f32;
        let mut peak_out = 0.0_f32;
        let freq = 8000.0_f32;
        let dt = 1.0_f32 / SR as f32;
        // Warm up to past the transient.
        for i in 0..4800 {
            let t = i as f32 * dt;
            let s = (2.0 * PI * freq * t).sin();
            let _ = f.tick(s);
        }
        for i in 0..480 {
            let t = i as f32 * dt;
            let s = (2.0 * PI * freq * t).sin();
            peak_in = peak_in.max(s.abs());
            peak_out = peak_out.max(f.tick(s).abs());
        }
        assert!(
            peak_out < 0.1 * peak_in,
            "8 kHz through 200 Hz LP should be ≥ -20 dB; in={peak_in}, out={peak_out}",
        );
    }

    #[test]
    fn impulse_response_decays() {
        let mut f = fresh(2000.0, 0.707);
        let mut output = vec![f.tick(1.0)];
        for _ in 0..2000 {
            output.push(f.tick(0.0));
        }
        let initial_peak = output
            .iter()
            .take(50)
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max);
        let tail_peak = output
            .iter()
            .skip(1500)
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max);
        assert!(initial_peak > 0.0);
        assert!(
            tail_peak < initial_peak * 0.01,
            "impulse response should decay; initial={initial_peak}, tail={tail_peak}",
        );
    }

    #[test]
    fn stable_at_high_resonance() {
        // Q=10 is high but musically common (resonant filter sweep).
        // The trapezoidal SVF should stay bounded.
        let mut f = fresh(1000.0, 10.0);
        // Hit it with an impulse + long zero tail; output must not
        // grow unboundedly or produce NaN / Inf.
        let mut max_abs = 0.0_f32;
        let _ = f.tick(1.0);
        for _ in 0..10_000 {
            let y = f.tick(0.0);
            max_abs = max_abs.max(y.abs());
            assert!(y.is_finite(), "filter output must stay finite");
        }
        assert!(
            max_abs < 100.0,
            "high-Q filter output should stay bounded; max={max_abs}",
        );
    }

    #[test]
    fn cutoff_above_nyquist_is_clamped() {
        // Driving cutoff past Nyquist must not produce NaN / Inf.
        let mut f = SvfLowpass::new();
        f.prepare(SR);
        f.set_cutoff(100_000.0);
        f.set_resonance(0.707);
        let y = f.tick(0.5);
        assert!(y.is_finite());
    }

    #[test]
    fn cutoff_below_minimum_is_clamped() {
        // Negative cutoff and 0 must not produce NaN / Inf.
        let mut f = SvfLowpass::new();
        f.prepare(SR);
        f.set_cutoff(-50.0);
        f.set_resonance(0.707);
        let y = f.tick(0.5);
        assert!(y.is_finite());
    }
}
