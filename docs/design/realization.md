# Realization Pass

The realization pass turns rawdaw's structural composition model into concrete, sample-timed MIDI events the audio engine can dispatch. It's the bridge between "this pattern + chord loop + role" and "MIDI note 67 at sample 88200." Most of rawdaw's algorithmic work lives here.

## Where it runs

Realization is **not** on the RT audio thread. It runs on a worker thread and pushes results into a lock-free queue (`rtrb`). The audio thread consumes events from the queue and dispatches them sample-accurately within each processing block.

The pass operates over a **lookahead window** ahead of the playhead (target: 100–500ms; tunable). The worker re-runs the pass as the playhead advances and on structural edits.

## Interface

```rust
pub fn realize_range(
    project: &Project,
    range: TimeRange,                // absolute sample time
    cache: &mut RealizationCache,
) -> Vec<TimedEvent>;

pub struct TimedEvent {
    pub time: SampleTime,            // absolute, in engine clock
    pub target: TrackId,             // routes to that track's instrument
    pub message: Midi2Message,
    pub provenance: Provenance,
}

pub struct Provenance {
    pub pattern: PatternId,
    pub variant: VariantId,
    pub event_note_id: NoteId,
    pub override_applied: Option<NoteOverrideId>,
    pub section: SectionRefId,       // arrangement position
}
```

The function is conceptually pure: same `(project, range)` yields the same events. This is what makes caching, incremental re-realization, and unit testing tractable.

Provenance is **always populated**. Cost is small relative to the event payload, and the piano-roll detail view uses it to trace derived notes back to their structural source (clicking a note shows: "from pattern `verse-bass`, variant `main`, event 14, no override").

## Pipeline

For each section overlapping `range`:

1. Resolve the section's chord loops → a sequence of `(time, ResolvedChord)` pairs by walking `section.chord_loops: Vec<(BarRange, ChordLoopRef)>` and expanding each loop's events (looping the loop if its length is less than the range). For each `Functional` chord event, resolve it against `in_key` (if set) or the section's scale; apply the `bass` override if any.
2. Resolve the section's scale (section override or project default). Used for `ScaleDegree` pattern events.
3. For each track with an `ActivationEntry` in this section:
   1. Compute the variant schedule: which variant plays at each bar (`activation.variant_schedule`, falling back to `pattern.default_variant`).
   2. For each event in the active variant's body, in order:
      - **Pitched event** — resolution depends on the event variant:
        - `ScaleDegree` → look up the scale at event start time → resolve degree to pitch class → choose octave per `OctaveSpec` (with voice-leading state as the basis for `Nearest`).
        - `ChordDegree` → look up the chord at event start time → resolve degree to pitch class → choose octave per `OctaveSpec`.
        - `Absolute` → use the specified pitch class and octave directly. Voice-leading state still updates (so subsequent `Nearest` events follow this note).
        - `Chromatic` → take the previous event's resolved MIDI note and offset by `semitones_from_prev`. Updates voice-leading state.
        - `Rest` → emit nothing; voice-leading state unchanged.
      - **Drum event** (`voice`, `articulation?`): look up the voice + articulation in the track's drum kit → MIDI note (and per-note attribute if MIDI 2.0).
   3. Apply `realization` params: octave/voicing strategy, velocity curve, humanization (with seed).
   4. Apply matching `per_note_overrides` (matched by `NoteId`).
   5. Convert musical time → sample time via the tempo map.
   6. Emit a `TimedEvent` with provenance.

Pattern looping within an activation: if the pattern's length is shorter than the activation's bar range, the pattern repeats. Each repetition iterates `1..n` for note IDs internally but emits the same provenance `event_note_id` (so overrides apply consistently across loop iterations).

## Voice-leading

Voice-leading is **stateful minimal-motion** with **voicing-aware handling for explicit chord-block events**.

Per-track realization state during the pass:

```rust
struct VoiceLeadingState {
    last_pitch: Option<MidiNoteNumber>,
    last_chord_tones: Vec<MidiNoteNumber>,   // current voicing, if chord-block role
    section_root: Note,                       // for octave anchoring
}
```

Resolution rules:

- For a single-pitch event with `OctaveSpec::Nearest` (most pitched-pattern events): given the target pitch class, pick the octave whose MIDI note is closest to `last_pitch`. Bias by the track's role (`bass` defaults low, `melodic` defaults mid, `pad` defaults sustained-and-wide). Update `last_pitch`.
- For `OctaveSpec::Anchored(o)`: use the specified octave directly. Update state.
- For `OctaveSpec::UpFromPrev` / `DownFromPrev`: pick the nearest valid octave *strictly above* (resp. below) `last_pitch`. Forces directional leaps for hook notes.
- For `OctaveSpec::RelativeToRole`: use the role's default register, ignoring `last_pitch`. Useful for resetting after a leap.
- For `Absolute` events: pitch is fully specified; state updates to the new pitch.
- For `Chromatic` events: pitch = `last_pitch + semitones_from_prev`. State updates.
- For a chord-block event: construct the voicing using the configured strategy (see below); ideally voice-lead from `last_chord_tones` to minimize total motion across voices.
- State **resets at section boundaries**. Sections are self-contained for caching and reasoning.
- State **continues across variant changes within a section**. Feels like the same player switching figures.
- State **resets when an activation entry ends**. Different activation = fresh start.

Full counterpoint analysis or AI voice-leading is out of scope. The simple rules above produce musical results for the vast majority of cases and remain debuggable.

## Voicings (chord-block events)

Named voicing strategies available v1:

- `triad-close` — root, 3rd, 5th packed within an octave.
- `triad-open` — root, 5th, 3rd spread over two octaves.
- `four-way-close` — root, 3rd, 5th, 7th (or extension) in close position.
- `drop2` — four-way close with the 2nd-highest note dropped an octave.
- `drop3` — four-way close with the 3rd-highest note dropped an octave.
- `shell` — root + 3rd + 7th. Jazz comping.
- `rootless` — 3rd + 7th + 9th. Jazz comping when a bass has the root.
- `power` — root + 5th. Rock/metal.

Strategy is selectable per pattern event (in the event itself) or as a default on the activation entry's `realization` block. The voicing module is structured so additional strategies can be added without touching the realization core.

## Chord-change crossing

When a pattern event's duration spans a chord change, the event is **interpreted only at its start time**. No mid-note re-interpretation. v1 simplification, with two consequences:

- A note held across a chord change keeps the pitch it had at the start of the note. This is musically realistic (it's a suspension or anticipation).
- Re-evaluation mid-note is not supported. If the user wants the held note to *change* on the new chord, they should write it as two events (one per chord) in the pattern.

## Per-note overrides via stable NoteIds

Every pattern event carries a stable `NoteId`:

```rust
type NoteId = u64;   // monotonically allocated; serialized; never reused
```

`NoteId`s are allocated when the user adds an event in the pattern editor, persisted into the project file, and not reused even after the event is deleted. Overrides reference `NoteId`s:

```rust
struct NoteOverride {
    id: NoteOverrideId,
    target: NoteId,
    transform: OverrideTransform,   // pin pitch, pin velocity, pin timing, etc.
}
```

When the realization pass processes an event, it looks up overrides keyed by `NoteId`. If an override exists, the transform is applied after resolution but before emission.

If a pattern event is deleted, overrides referencing its `NoteId` become **orphans**. The model layer detects orphans on pattern edit and surfaces them in the UI (warning + offer to drop). The realization pass simply ignores orphans (the lookup misses).

This is why `NoteId` allocation is durable across deletes: a user might want to undo a delete and have their overrides come back. Reused IDs would silently re-bind overrides to the wrong notes.

## Humanization

Humanization is **deterministic given a seed**. The seed lives on the `ActivationEntry`:

```rust
struct RealizationParams {
    voicing: Option<VoicingStrategy>,
    velocity_curve: Option<VelocityCurve>,
    humanization: Humanization,
    seed: u64,
    // ...
}

struct Humanization {
    velocity_jitter: f32,    // ±N% of velocity
    timing_jitter_ticks: i32,
    swing: f32,              // 0.0 = straight, 0.5 = full triplet swing
    accent_pattern: Option<AccentPattern>,
}
```

The RNG (small fast PRNG, e.g. `WyRand`) is seeded from `(activation_seed, pattern_event_note_id)` per event, so:
- Editing one event doesn't reshuffle randomness on other events.
- The same event in the same activation always humanizes the same way.
- A "re-roll" UI action generates a new `seed`, reshuffling everything in that activation.

## Tempo map

Pattern events are stored in musical time (bars/beats/ticks at PPQ 960). The realization pass converts to sample time using the project's `TempoMap`:

```rust
impl TempoMap {
    pub fn musical_to_sample(&self, musical: MusicalTime) -> SampleTime;
    pub fn sample_to_musical(&self, sample: SampleTime) -> MusicalTime;
}
```

The tempo map handles constant tempo, ramped tempo changes (linear or curved), and time-signature changes. The realization pass receives an immutable snapshot per invocation; if the tempo map changes mid-playback, the in-flight realization is cancelled and re-run.

## Caching and incremental re-realization

The cache is keyed by `(ActivationEntryId, hash_of(activation, chord_loop, scale, tempo_window))`. A cache hit returns the previously-realized events for that activation within the section. Cache invalidation:

- Edit to an activation entry → invalidate that activation only.
- Edit to a chord loop → invalidate every activation in every section referencing it.
- Edit to a scale or section duration → invalidate every activation in that section.
- Edit to a pattern body → invalidate every activation referencing that pattern.
- Edit to the tempo map within a time window → invalidate all activations overlapping that window.

Cache hit rate should be excellent in practice: most user edits are localized, and the structural model is built around shared references that the cache key naturally tracks.

## State across boundaries — summary table

| Boundary                                          | Voice-leading state | Notes                                       |
|---------------------------------------------------|---------------------|---------------------------------------------|
| Section boundary                                  | reset               | Self-contained for caching                  |
| Variant change within an activation               | continues           | Feels like same player switching figures    |
| Activation entry end → start of next (same track) | reset               | Different activation = fresh start          |
| Pattern loop iteration within an activation       | continues           | Pattern repeating, same player              |

## Crate organization

```
rawdaw-model/
└── src/
    ├── lib.rs
    ├── pattern.rs          // Pattern, PatternKind, variants, NoteId
    ├── chord.rs            // ChordEvent, ChordLoop, chord-tone resolution
    ├── scale.rs            // Scale, key, scale-degree resolution
    ├── role.rs             // Role enum, role defaults
    ├── voicing.rs          // Voicing strategies, voicing construction
    ├── activation.rs       // ActivationEntry, variant scheduling
    ├── override.rs         // NoteOverride, OverrideTransform
    ├── tempo.rs            // TempoMap, MusicalTime, SampleTime
    └── realize/
        ├── mod.rs          // realize_range() entry point, cache
        ├── pitched.rs      // pitched event realization w/ voice-leading
        ├── drum.rs         // drum event realization
        ├── voicing.rs      // voicing construction
        ├── humanize.rs     // seeded jitter, swing, accent
        └── cache.rs        // RealizationCache, invalidation
```

The realization module is the only piece that touches RT-adjacent queues; everything else is pure data and pure functions. This makes the model layer easy to unit-test without an audio engine.

## Open questions

- **Lookahead window size tuning.** 100–500ms target; actual sweet spot depends on edit-to-audible-change latency tolerance vs. CPU cost. Profile once a real playback path exists.
- **Voicing transitions across chord changes.** Current rule is "voice-lead each chord-block event from `last_chord_tones`." Some chord-change patterns (e.g. ii–V–I) have idiomatic voice-leading rules (`shell` → `shell` keeps 3rd/7th moving by step). Worth a v2 enhancement: per-voicing-strategy transition rules.
- **Phrase-level accent patterns.** Realization params currently support per-beat accents. Phrase-level (e.g. crescendo across 4 bars, accent on every downbeat) is doable but needs a richer accent representation.
- **Live MIDI input during playback.** If the user is playing a MIDI keyboard while the realization pass is running, those events need to merge with realized events into the engine queue. Probably a separate ingestion path on the worker thread; not realization's concern, but worth noting the interaction.
