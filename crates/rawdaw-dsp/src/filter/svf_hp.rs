//! State-variable highpass filter.
//!
//! Sibling of [`crate::filter::SvfLowpass`] — same Cytomic /
//! Andrew Simper trapezoidal-integrator topology, same coefficient
//! pipeline, but returns the highpass output. The SVF state machine
//! produces LP, BP, and HP from the same integrator memory; we keep
//! these as separate types for v0 instead of unifying behind one
//! "Svf with selectable output" struct. When more filter modes land
//! (BP, notch, shelves), a unifying refactor is the obvious cleanup.
//!
//! See `svf.rs` for the derivation and stability discussion.
//!
//! **RT-safety.** No allocations; `tick` is two multiply-adds plus
//! one extra subtraction over the LP path.

use core::f32::consts::PI;

/// Same clamps as the lowpass sibling — keep them in sync if the LP
/// definitions ever change.
const MIN_CUTOFF_HZ: f32 = 1.0;
const MAX_CUTOFF_FRAC: f32 = 0.49;
const MIN_Q: f32 = 0.5;

/// State-variable highpass filter.
#[derive(Debug, Clone, Copy)]
pub struct SvfHighpass {
    sample_rate: f32,
    cutoff_hz: f32,
    q: f32,

    g: f32,
    k: f32,
    a1: f32,
    a2: f32,

    ic1eq: f32,
    ic2eq: f32,
}

impl SvfHighpass {
    pub fn new() -> Self {
        Self {
            sample_rate: 0.0,
            cutoff_hz: 1000.0,
            q: 0.707,
            g: 0.0,
            k: 0.0,
            a1: 0.0,
            a2: 0.0,
            ic1eq: 0.0,
            ic2eq: 0.0,
        }
    }

    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate as f32;
        self.refresh_coefficients();
    }

    pub fn set_cutoff(&mut self, hz: f32) {
        let max = self.sample_rate * MAX_CUTOFF_FRAC;
        self.cutoff_hz = hz.clamp(MIN_CUTOFF_HZ, max.max(MIN_CUTOFF_HZ));
        self.refresh_coefficients();
    }

    pub fn set_resonance(&mut self, q: f32) {
        self.q = q.max(MIN_Q);
        self.refresh_coefficients();
    }

    pub fn reset_state(&mut self) {
        self.ic1eq = 0.0;
        self.ic2eq = 0.0;
    }

    /// Filter one sample and return the highpass output.
    pub fn tick(&mut self, input: f32) -> f32 {
        let v3 = input - self.ic2eq;
        let v1 = self.a1 * self.ic1eq + self.a2 * v3;
        let v2 = self.ic2eq + self.g * v1;
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;
        // HP = input - k*v1 - v2 (Simper's HP output formula).
        input - self.k * v1 - v2
    }

    fn refresh_coefficients(&mut self) {
        if self.sample_rate <= 0.0 {
            return;
        }
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

impl Default for SvfHighpass {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn fresh(cutoff: f32, q: f32) -> SvfHighpass {
        let mut f = SvfHighpass::new();
        f.prepare(SR);
        f.set_cutoff(cutoff);
        f.set_resonance(q);
        f
    }

    #[test]
    fn dc_blocked_at_high_cutoff() {
        // Drive with DC = 1.0. A highpass cut at 5 kHz should
        // strongly attenuate DC (essentially block it).
        let mut f = fresh(5000.0, 0.707);
        let mut last = 0.0;
        for _ in 0..10_000 {
            last = f.tick(1.0);
        }
        assert!(
            last.abs() < 0.05,
            "high-cutoff HP should block DC; got {last}",
        );
    }

    #[test]
    fn high_frequency_passes() {
        // 12 kHz sine through a 4 kHz HP should pass largely
        // unattenuated.
        let mut f = fresh(4000.0, 0.707);
        let freq = 12_000.0_f32;
        let dt = 1.0_f32 / SR as f32;
        // Warm up past the transient.
        for i in 0..4800 {
            let t = i as f32 * dt;
            let s = (2.0 * PI * freq * t).sin();
            let _ = f.tick(s);
        }
        let mut peak_in = 0.0_f32;
        let mut peak_out = 0.0_f32;
        for i in 0..480 {
            let t = i as f32 * dt;
            let s = (2.0 * PI * freq * t).sin();
            peak_in = peak_in.max(s.abs());
            peak_out = peak_out.max(f.tick(s).abs());
        }
        assert!(
            peak_out > 0.7 * peak_in,
            "12 kHz through 4 kHz HP should pass mostly intact; in={peak_in}, out={peak_out}",
        );
    }

    #[test]
    fn impulse_response_decays() {
        let mut f = fresh(4000.0, 0.707);
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
            "HP impulse response should decay; initial={initial_peak}, tail={tail_peak}",
        );
    }

    #[test]
    fn stable_at_high_resonance() {
        let mut f = fresh(1000.0, 10.0);
        let _ = f.tick(1.0);
        let mut max_abs = 0.0_f32;
        for _ in 0..10_000 {
            let y = f.tick(0.0);
            max_abs = max_abs.max(y.abs());
            assert!(y.is_finite());
        }
        assert!(
            max_abs < 100.0,
            "high-Q HP output should stay bounded; max={max_abs}",
        );
    }
}
