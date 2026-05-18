//! xorshift32 white noise generator.
//!
//! Marsaglia's xorshift32 — three xors + three shifts per sample.
//! Deterministic given a seed; reproducible across runs. Good enough
//! for percussion noise; not cryptographically secure.
//!
//! Returns samples in `[-1.0, 1.0]` via integer-to-float scaling.
//!
//! **RT-safety.** No allocations. `tick` is two arithmetic ops + a
//! float cast.

#[derive(Debug, Clone, Copy)]
pub struct NoiseSource {
    state: u32,
}

impl NoiseSource {
    /// Construct from a seed. Seed 0 would lock xorshift at 0
    /// forever, so a 0 seed gets bumped to a safe default.
    pub fn new(seed: u32) -> Self {
        let state = if seed == 0 { 0xACE1_BEEF } else { seed };
        Self { state }
    }

    /// Produce one sample of white noise in `[-1.0, 1.0]`.
    pub fn tick(&mut self) -> f32 {
        // Marsaglia xorshift32.
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        // Map u32 [0, u32::MAX] → f32 [-1.0, 1.0].
        // Subtract 0.5 then double; the constant is precomputed so the
        // hot path is one mul + one sub.
        (x as f32) * (2.0 / u32::MAX as f32) - 1.0
    }
}

impl Default for NoiseSource {
    /// Construct with a fixed default seed. Two `NoiseSource`s built
    /// via `default()` will produce the same sequence; pass a unique
    /// seed to get independent streams.
    fn default() -> Self {
        Self::new(0xACE1_BEEF)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_in_unit_range() {
        let mut n = NoiseSource::new(1);
        for _ in 0..100_000 {
            let s = n.tick();
            assert!(
                (-1.0..=1.0).contains(&s),
                "noise sample {s} should be in [-1, 1]",
            );
        }
    }

    #[test]
    fn zero_seed_does_not_lock_at_zero() {
        // Pathological seed handling: xorshift32 with state=0 stays 0
        // forever, so a 0 seed gets bumped. Verify output is non-zero.
        let mut n = NoiseSource::new(0);
        let mut nonzero_count = 0;
        for _ in 0..100 {
            if n.tick() != 0.0 {
                nonzero_count += 1;
            }
        }
        assert!(nonzero_count > 50, "expected nonzero samples; got {nonzero_count}");
    }

    #[test]
    fn deterministic_for_same_seed() {
        let mut a = NoiseSource::new(42);
        let mut b = NoiseSource::new(42);
        for _ in 0..100 {
            assert_eq!(a.tick(), b.tick());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = NoiseSource::new(1);
        let mut b = NoiseSource::new(2);
        let mut differ = 0;
        for _ in 0..100 {
            if a.tick() != b.tick() {
                differ += 1;
            }
        }
        assert!(
            differ > 90,
            "different seeds should produce different streams; only {differ}/100 differed",
        );
    }

    #[test]
    fn rough_zero_mean_over_long_window() {
        // White noise should have ~zero mean. Average over 100k
        // samples should be well under 0.05.
        let mut n = NoiseSource::new(7);
        let mut sum = 0.0_f64;
        let count = 100_000;
        for _ in 0..count {
            sum += n.tick() as f64;
        }
        let mean = sum / count as f64;
        assert!(
            mean.abs() < 0.05,
            "white noise mean should be ~0; got {mean}",
        );
    }
}
