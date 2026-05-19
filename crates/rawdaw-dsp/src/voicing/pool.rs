//! Generic polyphonic voice pool.
//!
//! Holds N voices of a concrete `V: Voice` type. On `note_on`,
//! allocates an inactive voice if available; otherwise steals the
//! voice with the lowest age counter (= oldest active note). On
//! `note_off`, matches by MIDI note number and forwards the release
//! to the matching voice.
//!
//! Voices stay in the pool after release ramps complete — the
//! synth's per-block tick can check `is_active()` and skip silent
//! voices instead of compacting the array.
//!
//! **RT-safety.** The pool is constructed with all voices in place;
//! `note_on` / `note_off` / inspection methods never allocate.

/// Trait implemented by per-voice state types. Mirrors the existing
/// `SineNode` voice semantics so a wavetable synth's voice can drop
/// straight into the pool.
pub trait Voice {
    /// The MIDI note this voice is playing (if active). Used by the
    /// pool to match `note_off` events.
    fn note(&self) -> u8;

    /// Whether the voice is currently producing audio (anywhere from
    /// post-NoteOn attack through release). Inactive voices are
    /// preferred for allocation.
    fn is_active(&self) -> bool;

    /// Begin a new note. Called by the pool after deciding which
    /// voice slot to use.
    fn note_on(&mut self, note: u8, velocity: f32);

    /// Begin release. The pool already verified that `note_match`
    /// returns true for this voice.
    fn note_off(&mut self);
}

/// Polyphonic voice pool.
///
/// `voices.len()` sets the maximum polyphony. The default
/// constructor accepts a closure that builds a single fresh voice;
/// the pool calls it `polyphony` times.
pub struct VoicePool<V: Voice> {
    voices: Vec<V>,
    /// Per-voice age counter. Higher = newer note-on. We use this
    /// to find the oldest active voice for stealing. Parallel array
    /// (rather than a field on `V`) so `Voice` stays a small trait
    /// and individual voice impls don't have to thread age through.
    ages: Vec<u64>,
    /// Monotonic counter incremented on every successful `note_on`.
    /// Zero means "never had a note assigned" — the steal heuristic
    /// can treat zero-age voices as oldest in a pinch.
    age_counter: u64,
}

impl<V: Voice> VoicePool<V> {
    /// Build a pool of `polyphony` voices using `make_voice` to
    /// produce each one. `polyphony` must be ≥ 1; the constructor
    /// panics on 0 to surface the bug at install time.
    pub fn new(polyphony: usize, mut make_voice: impl FnMut() -> V) -> Self {
        assert!(polyphony > 0, "VoicePool polyphony must be ≥ 1");
        let mut voices = Vec::with_capacity(polyphony);
        for _ in 0..polyphony {
            voices.push(make_voice());
        }
        let ages = vec![0u64; polyphony];
        Self {
            voices,
            ages,
            age_counter: 0,
        }
    }

    /// Number of voice slots.
    pub fn polyphony(&self) -> usize {
        self.voices.len()
    }

    /// Borrow the voice array (mutable) — the synth's per-sample tick
    /// iterates over this and sums voice outputs.
    pub fn voices_mut(&mut self) -> &mut [V] {
        &mut self.voices
    }

    /// Borrow the voice array (shared) — useful for tests and for
    /// inspecting voice state.
    pub fn voices(&self) -> &[V] {
        &self.voices
    }

    /// Dispatch a MIDI note-on into the pool. Prefers an inactive
    /// voice; if none are available, steals the oldest active one.
    pub fn note_on(&mut self, note: u8, velocity: f32) {
        let idx = self.allocate_voice();
        self.voices[idx].note_on(note, velocity);
        self.age_counter += 1;
        self.ages[idx] = self.age_counter;
    }

    /// Dispatch a MIDI note-off to **every** active voice with a
    /// matching MIDI note.
    ///
    /// Why "every," not "first": a voice in its Release stage is
    /// still `is_active()` (the amp envelope hasn't reached Idle
    /// yet — Release is the last 100–1000 ms of any patch with
    /// a non-zero `release_s`). If the user plays the same note
    /// twice quickly, the second NoteOn allocates a *new* slot
    /// while the first voice is still tail-releasing. When the
    /// matching NoteOff arrives, returning after the first match
    /// would hit the already-releasing voice (a no-op, since
    /// [`Adsr::note_off`] only transitions from non-Idle stages)
    /// and orphan the second voice in Sustain forever.
    ///
    /// Releasing every matching voice is robust to:
    /// - rapid same-note retrigger with overlapping release tails;
    /// - keyboards that send multiple NoteOns for one key press;
    /// - the rare "release all stuck instances of this note" gesture.
    ///
    /// [`Adsr::note_off`] is idempotent on a Release-stage voice,
    /// so re-releasing a voice that's already on its way out has
    /// no audible effect.
    ///
    /// Stray NoteOffs for notes that aren't playing are silently
    /// ignored (no matches → no-op), matching MIDI tolerance.
    pub fn note_off(&mut self, note: u8) {
        for v in self.voices.iter_mut() {
            if v.is_active() && v.note() == note {
                v.note_off();
            }
        }
    }

    /// Pick the slot for a new voice — prefer any inactive voice,
    /// otherwise steal the oldest active voice (lowest age).
    fn allocate_voice(&self) -> usize {
        if let Some((i, _)) = self
            .voices
            .iter()
            .enumerate()
            .find(|(_, v)| !v.is_active())
        {
            return i;
        }
        // All active — steal oldest. `min_by_key` is fine for 16-ish
        // voices; we'd reach for a heap only if pool sizes grow past
        // hundreds.
        self.ages
            .iter()
            .enumerate()
            .min_by_key(|(_, age)| **age)
            .map(|(i, _)| i)
            .expect("pool is non-empty by construction")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal Voice impl for testing the pool's allocation /
    /// stealing logic in isolation.
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct TestVoice {
        note: u8,
        active: bool,
    }

    impl TestVoice {
        fn new() -> Self {
            Self {
                note: 0,
                active: false,
            }
        }
    }

    impl Voice for TestVoice {
        fn note(&self) -> u8 {
            self.note
        }
        fn is_active(&self) -> bool {
            self.active
        }
        fn note_on(&mut self, note: u8, _velocity: f32) {
            self.note = note;
            self.active = true;
        }
        fn note_off(&mut self) {
            self.active = false;
        }
    }

    fn pool(polyphony: usize) -> VoicePool<TestVoice> {
        VoicePool::new(polyphony, TestVoice::new)
    }

    #[test]
    fn allocates_inactive_voice_first() {
        let mut p = pool(4);
        p.note_on(60, 1.0);
        // First voice should now hold note 60; rest still inactive.
        assert!(p.voices()[0].is_active());
        assert_eq!(p.voices()[0].note(), 60);
        for v in p.voices().iter().skip(1) {
            assert!(!v.is_active());
        }
    }

    #[test]
    fn note_off_matches_by_note() {
        let mut p = pool(4);
        p.note_on(60, 1.0);
        p.note_on(64, 1.0);
        p.note_on(67, 1.0);
        p.note_off(64);
        // Voice with note 64 should now be inactive; the others remain.
        let active: Vec<u8> = p.voices().iter().filter(|v| v.is_active()).map(|v| v.note()).collect();
        assert_eq!(active.len(), 2);
        assert!(active.contains(&60));
        assert!(active.contains(&67));
        assert!(!active.contains(&64));
    }

    #[test]
    fn unmatched_note_off_is_a_noop() {
        let mut p = pool(4);
        p.note_on(60, 1.0);
        p.note_off(99); // not playing this note
        assert!(p.voices()[0].is_active());
        assert_eq!(p.voices()[0].note(), 60);
    }

    #[test]
    fn steals_oldest_when_full() {
        let mut p = pool(3);
        p.note_on(60, 1.0); // age 1
        p.note_on(62, 1.0); // age 2
        p.note_on(64, 1.0); // age 3
        // All voices active; next note_on must steal the oldest (age 1, note 60).
        p.note_on(67, 1.0); // age 4 — steals slot 0 (note 60)
        let notes: Vec<u8> = p.voices().iter().map(|v| v.note()).collect();
        assert!(notes.contains(&67), "new note must be installed");
        assert!(!notes.contains(&60), "oldest (60) must have been stolen");
        assert!(notes.contains(&62));
        assert!(notes.contains(&64));
    }

    #[test]
    fn re_allocation_to_inactive_works_after_release() {
        let mut p = pool(2);
        p.note_on(60, 1.0);
        p.note_on(62, 1.0);
        p.note_off(60); // slot 0 now inactive
        p.note_on(64, 1.0); // should land in slot 0 (preferred inactive)
        assert_eq!(p.voices()[0].note(), 64);
        assert!(p.voices()[0].is_active());
        assert_eq!(p.voices()[1].note(), 62);
    }

    #[test]
    #[should_panic(expected = "polyphony must be ≥ 1")]
    fn zero_polyphony_panics() {
        let _ = VoicePool::new(0, TestVoice::new);
    }

    /// Voice that models a release tail: `note_off` doesn't immediately
    /// flip `is_active` to false — it records that release was
    /// requested. A subsequent `finish_release` simulates the
    /// envelope decaying to Idle. This mirrors the real
    /// `WavetableVoice` where `is_active` returns true for the entire
    /// Release stage.
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct ReleasingVoice {
        note: u8,
        active: bool,
        releasing: bool,
    }

    impl ReleasingVoice {
        fn new() -> Self {
            Self {
                note: 0,
                active: false,
                releasing: false,
            }
        }

        fn finish_release(&mut self) {
            if self.releasing {
                self.active = false;
                self.releasing = false;
            }
        }
    }

    impl Voice for ReleasingVoice {
        fn note(&self) -> u8 {
            self.note
        }
        fn is_active(&self) -> bool {
            self.active
        }
        fn note_on(&mut self, note: u8, _velocity: f32) {
            self.note = note;
            self.active = true;
            self.releasing = false;
        }
        fn note_off(&mut self) {
            // Voice stays active until `finish_release` simulates the
            // envelope reaching Idle.
            self.releasing = true;
        }
    }

    #[test]
    fn note_off_releases_all_matching_voices_even_when_one_is_already_in_release() {
        // K1.fix2 regression: with the old "first match + return"
        // logic, a NoteOff on a note that has multiple active voices
        // (because an earlier voice's release tail overlaps a fresh
        // NoteOn) would only release the older voice — leaving the
        // newer one stuck in Sustain forever.
        //
        // The trace from Joe's KeyLab MkII showed exactly this:
        // NoteOn 53 → NoteOff 53 → NoteOn 53 → NoteOff 53, with the
        // second pair allocating voice slot 1 while slot 0 was still
        // in its release tail.
        let mut p = VoicePool::<ReleasingVoice>::new(4, ReleasingVoice::new);

        // First press-release cycle. Voice 0 enters "releasing" but
        // stays active.
        p.note_on(53, 1.0);
        p.note_off(53);
        assert!(p.voices()[0].is_active(), "voice 0 still tail-releasing");
        assert!(p.voices()[0].releasing);

        // Second press: voice 0 still active in release tail, so a
        // fresh slot (voice 1) is allocated.
        p.note_on(53, 1.0);
        assert!(p.voices()[1].is_active());
        assert!(!p.voices()[1].releasing, "voice 1 not yet released");

        // Second release: under the buggy "first match + return"
        // semantics, this would target voice 0 (already releasing)
        // and orphan voice 1. The fix iterates every matching voice,
        // so voice 1 must end up `releasing == true` too.
        p.note_off(53);
        assert!(
            p.voices()[1].releasing,
            "voice 1 must be released by the second NoteOff",
        );
        // Voice 0 is still releasing (was already releasing; the
        // re-release is idempotent per Adsr::note_off semantics).
        assert!(p.voices()[0].releasing);

        // Both voices reach Idle after their envelopes finish.
        for v in p.voices_mut() {
            v.finish_release();
        }
        for v in p.voices() {
            assert!(!v.is_active(), "all voices must be idle");
        }
    }
}
