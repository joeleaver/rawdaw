//! One-shot pitch decay envelope.
//!
//! Designed for percussive synthesis: starts at `start_hz` on
//! `note_on`, decays linearly to `end_hz` over `decay_s`, then holds
//! at `end_hz`. Useful for kick / snare / tom voices where the pitch
//! drops dramatically on hit.
//!
//! Linear ramp keeps the math trivial — exponential decay is the
//! more "natural" choice for some drums and is a v1 growth target.
//!
//! **RT-safety.** No allocations; `tick` is one branch + one add.

#[derive(Debug, Clone, Copy)]
pub struct PitchEnvelope {
    start_hz: f32,
    end_hz: f32,
    /// Per-sample increment (negative when decaying down, positive
    /// when sweeping up — `tick` follows whichever sign was set).
    delta_per_sample: f32,
    current_hz: f32,
    sample_rate: f32,
    decay_s: f32,
    /// `true` once the ramp has reached `end_hz`; subsequent `tick`s
    /// just return `end_hz` without advancing.
    finished: bool,
}

impl PitchEnvelope {
    pub fn new() -> Self {
        Self {
            start_hz: 0.0,
            end_hz: 0.0,
            delta_per_sample: 0.0,
            current_hz: 0.0,
            sample_rate: 0.0,
            decay_s: 0.0,
            finished: true,
        }
    }

    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate as f32;
        self.recompute_delta();
    }

    /// Configure the ramp endpoints + duration. Updates are picked up
    /// on the next `note_on`.
    pub fn set_shape(&mut self, start_hz: f32, end_hz: f32, decay_s: f32) {
        self.start_hz = start_hz;
        self.end_hz = end_hz;
        self.decay_s = decay_s.max(0.0);
        self.recompute_delta();
    }

    /// Retrigger the envelope at `start_hz`. The ramp restarts; any
    /// in-flight decay is discarded.
    pub fn note_on(&mut self) {
        self.current_hz = self.start_hz;
        self.finished = false;
    }

    /// Tick one sample and return the current pitch in Hz.
    pub fn tick(&mut self) -> f32 {
        if self.finished {
            return self.end_hz;
        }
        let next = self.current_hz + self.delta_per_sample;
        // Detect ramp completion. Direction depends on the sign of
        // `delta_per_sample`: decaying down means we crossed below
        // `end_hz`; ramping up means we crossed above.
        let crossed = if self.delta_per_sample <= 0.0 {
            next <= self.end_hz
        } else {
            next >= self.end_hz
        };
        if crossed {
            self.current_hz = self.end_hz;
            self.finished = true;
        } else {
            self.current_hz = next;
        }
        self.current_hz
    }

    fn recompute_delta(&mut self) {
        if self.sample_rate <= 0.0 {
            self.delta_per_sample = 0.0;
            return;
        }
        let min_t = 1.0 / self.sample_rate;
        let span_samples = (self.decay_s.max(min_t) * self.sample_rate).max(1.0);
        self.delta_per_sample = (self.end_hz - self.start_hz) / span_samples;
    }
}

impl Default for PitchEnvelope {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn fresh(start: f32, end: f32, decay_s: f32) -> PitchEnvelope {
        let mut e = PitchEnvelope::new();
        e.prepare(SR);
        e.set_shape(start, end, decay_s);
        e
    }

    #[test]
    fn starts_at_start_on_note_on() {
        let mut e = fresh(100.0, 50.0, 0.050);
        e.note_on();
        let first = e.tick();
        // First tick advances one sample; with 50 Hz / 50 ms decay
        // the per-sample drop is tiny, so `first` should be ~start.
        assert!(
            (first - 100.0).abs() < 1.0,
            "first tick should be near start; got {first}",
        );
    }

    #[test]
    fn reaches_end_after_decay_window() {
        let mut e = fresh(100.0, 50.0, 0.050);
        e.note_on();
        // 50 ms at 48 kHz = 2400 samples. Run a tiny margin past.
        for _ in 0..2400 {
            e.tick();
        }
        let v = e.tick();
        assert!(
            (v - 50.0).abs() < 0.1,
            "after decay window should rest at end; got {v}",
        );
    }

    #[test]
    fn holds_at_end_indefinitely() {
        let mut e = fresh(100.0, 50.0, 0.050);
        e.note_on();
        for _ in 0..10_000 {
            e.tick();
        }
        // Still pinned at end, not overshooting.
        for _ in 0..1000 {
            assert!((e.tick() - 50.0).abs() < 1e-3);
        }
    }

    #[test]
    fn upward_ramp_works_too() {
        // Pitch envelope as a "rise" — start low, end high. The same
        // primitive should work for snare body rises if a patch wants
        // that.
        let mut e = fresh(100.0, 200.0, 0.020);
        e.note_on();
        for _ in 0..960 {
            e.tick();
        }
        let v = e.tick();
        assert!(
            (v - 200.0).abs() < 0.1,
            "upward ramp should reach end; got {v}",
        );
    }

    #[test]
    fn retrigger_resets_to_start() {
        let mut e = fresh(100.0, 50.0, 0.050);
        e.note_on();
        for _ in 0..1000 {
            e.tick();
        }
        let mid = e.tick();
        assert!(mid > 50.0 && mid < 100.0);
        e.note_on();
        let after = e.tick();
        assert!(
            (after - 100.0).abs() < 1.0,
            "after retrigger should be near start; got {after}",
        );
    }
}
