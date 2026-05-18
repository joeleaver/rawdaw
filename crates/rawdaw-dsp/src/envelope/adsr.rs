//! Linear-ramp ADSR envelope.
//!
//! Five-stage state machine: `Idle` → `Attack` → `Decay` → `Sustain`
//! → `Release` → `Idle`. Ramps are linear in amplitude; exponential
//! ramps (Vital's default) are a v1 growth target.
//!
//! Retriggering: `note_on` from `Idle` starts at level 0; from any
//! other stage it preserves the current level and ramps to the peak
//! at the configured attack rate. This avoids the classic
//! "amplitude pop" on rapid retriggers.
//!
//! `note_off` transitions to `Release` from the current level — same
//! invariant: smooth, no pops.
//!
//! **RT-safety.** Construction allocates nothing; `tick` is a small
//! state machine with no allocations.

/// ADSR shape parameters. Times in seconds, sustain in `[0.0, 1.0]`.
///
/// Times are clamped to a tiny minimum (1 sample) at the rate-prep
/// step so a 0-second attack still produces a deterministic 1-sample
/// ramp instead of an undefined "instant" transition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdsrParams {
    pub attack_s: f32,
    pub decay_s: f32,
    pub sustain_level: f32,
    pub release_s: f32,
}

impl AdsrParams {
    /// Sensible defaults for a plucked-instrument envelope.
    pub fn default_pluck() -> Self {
        Self {
            attack_s: 0.005,
            decay_s: 0.080,
            sustain_level: 0.7,
            release_s: 0.200,
        }
    }
}

impl Default for AdsrParams {
    fn default() -> Self {
        Self::default_pluck()
    }
}

/// Current stage of the ADSR state machine. Public for tests and for
/// callers that want to inspect voice state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdsrStage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// Per-voice ADSR envelope generator.
#[derive(Debug, Clone, Copy)]
pub struct Adsr {
    params: AdsrParams,
    stage: AdsrStage,
    /// Current output level in `[0.0, 1.0]`.
    level: f32,
    /// Per-sample increment cached from current stage + sample rate.
    /// Sign carries the direction (positive for attack, negative for
    /// decay / release).
    delta_per_sample: f32,
    sample_rate: f32,
}

impl Adsr {
    pub fn new() -> Self {
        Self {
            params: AdsrParams::default(),
            stage: AdsrStage::Idle,
            level: 0.0,
            delta_per_sample: 0.0,
            sample_rate: 0.0,
        }
    }

    /// Configure sample rate. Required before `tick`; otherwise the
    /// envelope is silent.
    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate as f32;
    }

    /// Replace the envelope shape parameters. The current stage's
    /// rate is recomputed so an in-flight ramp uses the new times
    /// from the next sample on.
    pub fn set_params(&mut self, params: AdsrParams) {
        self.params = params;
        self.refresh_stage_rate();
    }

    /// Inspect the current stage. Useful for tests and for the synth's
    /// "is this voice idle?" check.
    pub fn stage(&self) -> AdsrStage {
        self.stage
    }

    /// `true` iff the envelope has fully decayed and the voice can
    /// be freed.
    pub fn is_idle(&self) -> bool {
        self.stage == AdsrStage::Idle
    }

    /// Trigger Attack. Preserves the current level so a retrigger
    /// during release ramps up from where it was, not from 0.
    pub fn note_on(&mut self) {
        self.stage = AdsrStage::Attack;
        self.refresh_stage_rate();
    }

    /// Trigger Release. Preserves the current level so a release
    /// from mid-attack ramps down smoothly.
    pub fn note_off(&mut self) {
        if self.stage != AdsrStage::Idle {
            self.stage = AdsrStage::Release;
            self.refresh_stage_rate();
        }
    }

    /// Advance one sample and return the current level.
    pub fn tick(&mut self) -> f32 {
        match self.stage {
            AdsrStage::Idle => {
                self.level = 0.0;
            }
            AdsrStage::Attack => {
                self.level += self.delta_per_sample;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = AdsrStage::Decay;
                    self.refresh_stage_rate();
                }
            }
            AdsrStage::Decay => {
                self.level += self.delta_per_sample;
                if self.level <= self.params.sustain_level {
                    // Auto-Idle when the patch has no sustain — a
                    // `sustain_level == 0` ADSR becomes a one-shot AD
                    // envelope, which is what drum patches want
                    // (NoteOff is a no-op; the voice decays and
                    // releases the slot when the level hits zero).
                    if self.params.sustain_level <= 0.0 {
                        self.level = 0.0;
                        self.stage = AdsrStage::Idle;
                    } else {
                        self.level = self.params.sustain_level;
                        self.stage = AdsrStage::Sustain;
                    }
                    self.delta_per_sample = 0.0;
                }
            }
            AdsrStage::Sustain => {
                // Held at sustain_level until note_off arrives.
            }
            AdsrStage::Release => {
                self.level += self.delta_per_sample;
                if self.level <= 0.0 {
                    self.level = 0.0;
                    self.stage = AdsrStage::Idle;
                    self.delta_per_sample = 0.0;
                }
            }
        }
        self.level
    }

    /// Recompute the per-sample delta for the current stage. The
    /// direction (sign) and magnitude depend on stage + current level
    /// — release ramps from `self.level` to 0, not from 1 to 0,
    /// which is how the no-pop release-from-mid-attack invariant
    /// stays correct.
    fn refresh_stage_rate(&mut self) {
        if self.sample_rate <= 0.0 {
            self.delta_per_sample = 0.0;
            return;
        }
        // Clamp times to at least one sample so a 0-second stage
        // becomes a deterministic 1-sample ramp.
        let min_t = 1.0 / self.sample_rate;
        let span_samples = |t_seconds: f32| (t_seconds.max(min_t) * self.sample_rate).max(1.0);
        match self.stage {
            AdsrStage::Attack => {
                let span = span_samples(self.params.attack_s);
                // From self.level to 1.0.
                self.delta_per_sample = (1.0 - self.level) / span;
            }
            AdsrStage::Decay => {
                let span = span_samples(self.params.decay_s);
                // From self.level (1.0) to sustain_level.
                self.delta_per_sample = (self.params.sustain_level - self.level) / span;
            }
            AdsrStage::Release => {
                let span = span_samples(self.params.release_s);
                // From self.level to 0.0.
                self.delta_per_sample = (0.0 - self.level) / span;
            }
            AdsrStage::Idle | AdsrStage::Sustain => {
                self.delta_per_sample = 0.0;
            }
        }
    }
}

impl Default for Adsr {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn fresh(params: AdsrParams) -> Adsr {
        let mut e = Adsr::new();
        e.prepare(SR);
        e.set_params(params);
        e
    }

    #[test]
    fn starts_idle_with_level_zero() {
        let e = Adsr::new();
        assert_eq!(e.stage(), AdsrStage::Idle);
        assert!(e.is_idle());
    }

    #[test]
    fn note_on_ramps_to_peak_at_end_of_attack() {
        let mut e = fresh(AdsrParams {
            attack_s: 0.010, // 480 samples at 48 kHz
            decay_s: 0.100,
            sustain_level: 0.5,
            release_s: 0.100,
        });
        e.note_on();
        // Tick through attack span. Floor-add precision means we
        // expect ~480 ticks to reach 1.0, then the next tick
        // transitions into Decay.
        for _ in 0..479 {
            e.tick();
        }
        assert!(e.tick() >= 1.0 - 1e-5, "should reach peak at end of attack");
        assert_eq!(e.stage(), AdsrStage::Decay, "ends attack into decay");
    }

    #[test]
    fn decay_plateaus_at_sustain_level() {
        let mut e = fresh(AdsrParams {
            attack_s: 0.001, // tight attack
            decay_s: 0.010,
            sustain_level: 0.3,
            release_s: 0.100,
        });
        e.note_on();
        // Run for a generous duration past attack + decay
        // (~48 samples + 480 samples + margin).
        for _ in 0..1000 {
            e.tick();
        }
        let level = e.tick();
        assert!(
            (level - 0.3).abs() < 1e-4,
            "should plateau at sustain level 0.3, got {level}",
        );
        assert_eq!(e.stage(), AdsrStage::Sustain);
    }

    #[test]
    fn note_off_releases_to_zero() {
        let mut e = fresh(AdsrParams {
            attack_s: 0.001,
            decay_s: 0.001,
            sustain_level: 0.5,
            release_s: 0.010,
        });
        e.note_on();
        // Reach sustain.
        for _ in 0..200 {
            e.tick();
        }
        e.note_off();
        // Tick through release. ~480 samples at 48 kHz.
        for _ in 0..1000 {
            e.tick();
        }
        assert_eq!(e.stage(), AdsrStage::Idle);
        assert_eq!(e.tick(), 0.0);
    }

    #[test]
    fn release_from_mid_attack_does_not_pop() {
        // The "no pop" invariant: release should ramp from the
        // current level, not snap to peak first.
        let mut e = fresh(AdsrParams {
            attack_s: 1.0, // very long attack so we're definitely mid-ramp
            decay_s: 0.001,
            sustain_level: 0.5,
            release_s: 1.0, // long release for inspection
        });
        e.note_on();
        for _ in 0..100 {
            e.tick();
        }
        let mid_attack_level = e.tick();
        assert!(
            mid_attack_level < 0.5,
            "should be early in attack ramp; got {mid_attack_level}",
        );
        e.note_off();
        let just_released = e.tick();
        // Just-released level should be very close to mid_attack_level
        // — release advances exactly one sample of decay.
        assert!(
            (just_released - mid_attack_level).abs() < 1e-2,
            "release should preserve level; mid_attack={mid_attack_level}, after_release={just_released}",
        );
        assert_eq!(e.stage(), AdsrStage::Release);
    }

    #[test]
    fn retrigger_during_release_resumes_from_current_level() {
        let mut e = fresh(AdsrParams {
            attack_s: 0.100,
            decay_s: 0.001,
            sustain_level: 0.5,
            release_s: 0.100,
        });
        e.note_on();
        for _ in 0..1000 {
            e.tick(); // reach sustain
        }
        e.note_off();
        for _ in 0..100 {
            e.tick(); // partial release
        }
        let mid_release = e.tick();
        assert!(mid_release > 0.0 && mid_release < 0.5);
        e.note_on();
        let just_retrigger = e.tick();
        // After retrigger we're in Attack mode; the next sample should
        // be ≥ mid_release (climbing back up).
        assert!(
            just_retrigger >= mid_release - 1e-3,
            "retrigger should resume from current level: was {mid_release}, became {just_retrigger}",
        );
        assert_eq!(e.stage(), AdsrStage::Attack);
    }

    #[test]
    fn sustain_zero_makes_one_shot_ad_envelope() {
        // Drum patches use sustain_level = 0 to get a one-shot
        // attack-decay shape: the envelope plays Attack → Decay →
        // Idle automatically, no NoteOff required.
        let mut e = fresh(AdsrParams {
            attack_s: 0.001,
            decay_s: 0.010,
            sustain_level: 0.0,
            release_s: 0.020,
        });
        e.note_on();
        // 1ms attack + 10ms decay = 11ms = ~528 samples at 48 kHz.
        // Run with a generous margin and confirm the env auto-Idles.
        for _ in 0..1000 {
            e.tick();
        }
        assert_eq!(e.stage(), AdsrStage::Idle);
        assert!(e.is_idle());
    }

    #[test]
    fn zero_attack_time_still_works() {
        // Edge case: attack_s = 0 should produce a 1-sample ramp,
        // not a divide-by-zero or stall.
        let mut e = fresh(AdsrParams {
            attack_s: 0.0,
            decay_s: 0.001,
            sustain_level: 0.5,
            release_s: 0.001,
        });
        e.note_on();
        let first = e.tick();
        assert!(first >= 1.0 - 1e-5, "0-second attack should reach peak in 1 sample");
    }
}
