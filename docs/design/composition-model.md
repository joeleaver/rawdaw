# Composition Model

This is rawdaw's defining feature. The data model is a hierarchy of first-class, reusable musical objects. Edits to a shared object propagate to every place that references it.

## Motivating problem

Mainstream DAWs treat a MIDI clip as a bag of literal notes. Even "linked clips" propagate the literal notes, not the *structural decisions* underneath them. A composer who thinks in terms of "key → form → chord progression → derived parts" has to flatten that hierarchy into per-note edits across 16–32 piano-roll tracks, and any change at the structural level (move a chord, change a section length, swap a progression) becomes a cascading manual chore.

rawdaw represents the structural layer directly. The piano roll is a *detail view* on derived notes; the canonical edit lives at the structural level.

## Object hierarchy

```
Project
├── key/scale (default; sections may override)
├── tempo map
├── library
│   ├── ChordLoop[]    — named harmonic sequences, project-wide
│   ├── Pattern[]      — abstract part definitions, project-wide
│   └── Section[]      — named structural units
├── tracks: Track[]    — global track set; sections control activation
└── arrangement: Vec<SectionRef>  — the song, as an ordered list of section references
```

### Project

Owns the default key/scale, tempo map, library of reusable objects, the global track list, and the arrangement (ordered list of section references with section-level overrides).

The **project default key** is the "global key" — the starting tonal center the user picks when creating a project. Sections inherit from project unless they override their scale; chord loops interpret in the section's effective key unless the loop's own `key` is set. Most projects use one global key throughout — the Roman-numeral chord workflow flows naturally from this default.

### Section

A named structural unit (`intro`, `verse`, `pre-chorus`, `chorus`, `breakdown`, `outro`, etc.).

Carries:
- Duration (bars).
- Optional key/scale override.
- One or more `ChordLoopRef`s (typical case: one main loop; a section can layer or sequence loops, e.g. main progression + turnaround).
- Per-track **activation** entries: which tracks play in this section, with what pattern references, and what realization overrides. (See "Section ownership" below.)

Sections live in the project library. The arrangement is a list of references — placing the `chorus` section three times in the song does not create three sections, it creates three references. Edit `chorus` once, all three update.

A section can have **variants** (`chorus`, `chorus-final`) that share most properties but differ in some — implementation likely as either explicit variant relationships or by convention with copy-and-modify.

### ChordLoop

A named ordered sequence of harmonic events. Chord events are **functional (Roman numeral)** by default, with `Absolute` (pitch-class) as an escape hatch. Functional events interpret in the section's scale unless the chord event sets its own `in_key` override — that single mechanism handles secondary dominants, modal interchange, brief tonicizations, and chord-level modulation.

Chord events carry:

- Roman degree (or absolute root) + quality + extensions + alterations.
- Optional bass override (`BassSpec`: inversion, chord-degree, scale-degree, or absolute pitch).
- Optional `in_key` for tonicization/borrowing.
- Optional annotation (cadence tag, comment).

ChordLoops live in the project library. Sections reference them via a sequence: `Vec<(BarRange, ChordLoopRef)>` — most sections use one loop covering the whole range, but a section can chain multiple loops. Editing a loop updates everywhere it's used.

See `chord-loops.md` for the full data model, vocabulary, and editor UX.

### Pattern

A pattern is an **abstract part definition**, not a bag of literal notes. Patterns are sum-typed by kind and own a set of named variants. Every event in a pattern body carries a stable `NoteId` (durably allocated, never reused) so per-note overrides survive edits — see `realization.md`.

```rust
struct Pattern {
    id: PatternId,
    name: String,
    kind: PatternKind,
    shared: PatternMetadata,     // length, key/scale hints (pitched); voice list (drum)
    variants: BTreeMap<VariantId, PatternBody>,
    default_variant: VariantId,  // typically "main"
}

enum PatternKind {
    Pitched(PitchedPatternBody),
    Drum(DrumPatternBody),
}
```

A **pitched pattern body** is a sequence of events. Each event picks one of several pitch-specification modes, sharing common fields (`time`, `duration`, `velocity`, `articulation`, `NoteId`, humanization). The event variants span the spectrum from fully-derived to fully-written:

```rust
enum PitchedEvent {
    ScaleDegree {
        degree: ScaleDegree,          // 1-7 + accidental
        octave: OctaveSpec,
        ...
    },
    ChordDegree {
        degree: ChordDegree,          // root/3/5/7/9/11/13 + alteration
        octave: OctaveSpec,
        ...
    },
    Absolute {
        pitch: AbsolutePitch,         // pitch class + explicit octave
        ...
    },
    Chromatic {
        semitones_from_prev: i8,      // offset relative to previous event in pattern
        ...
    },
    Rest { duration: Duration },
}

enum OctaveSpec {
    Nearest,                          // voice-leading minimal-motion (default)
    Anchored(Octave),                 // pin to specific octave
    UpFromPrev,                       // force upward leap
    DownFromPrev,                     // force downward leap
    RelativeToRole,                   // use role's default register
}
```

- **`ScaleDegree`** / **`ChordDegree`** — pitch is *derived* from the section's scale or the current chord context. The melody/part transposes with key changes and re-realizes against new chord progressions. Use for the structural backbone of a part.
- **`Absolute`** — pitch is *written*, fixed regardless of context. Use when a specific note matters for its own sake (a hook leap, a deliberate non-diatonic note that's the *point*).
- **`Chromatic`** — pitch is relative to the previous event. Use for passing tones, ornaments, chromatic walks. Keeps the pattern readable as "the structural note, then a half step up, then resolve" without forcing every passing tone to be a degree-with-accidental.
- **`Rest`** — silence at a position.

This vocabulary covers basslines (`ChordDegree`-heavy), voicings (`ChordDegree` with chord-block voicing strategies), arpeggios (`ChordDegree` sequence), diatonic melodies (`ScaleDegree`-heavy), written melodies (`Absolute`-heavy), and the common mixed case (scale-degree structure with chromatic ornamentation).

`OctaveSpec` deserves a note: for most parts you want `Nearest` (voice-leading-driven). For melodic work the composer sometimes *wants* the leap — `UpFromPrev` for a hook, `Anchored` for "this F is structurally in octave 5." The pattern editor exposes this as a per-note toggle.

A **drum pattern body** uses symbolic voice names (`kick`, `snare`, `hat.closed`, etc.), not chord-relative degrees. See `drum-patterns.md`.

**Variants:** each pattern carries a set of named variants (`main`, `fill`, `build`, `breakdown`, `walk-down`, etc.). Variants share the pattern's metadata (length, voice list) but have independent bodies; they do not inherit. The `default_variant` plays when no variant is explicitly scheduled.

**Future:** parametric/generative pattern *kinds* (Euclidean rhythms, arpeggiators, motif-and-transform generators) as additional variants of `PatternKind`, sharing the variant machinery but with computed bodies instead of stored events.

Patterns live in the project library. Multiple clips can reference the same pattern.

### Track

A part with:
- A name.
- A **kind**: `Pitched` or `Drum`. Pitched tracks accept only pitched patterns; drum tracks accept only drum patterns and carry a drum-kit reference. See `drum-patterns.md`.
- A **role** (pitched tracks only): `bass`, `voicing`, `arp`, `melodic`, `pad`, `countermelody`, etc. The role hints at how a pattern's degrees should be interpreted (e.g. a `bass` track defaults to octave-down, `voicing` realizes chord-degree patterns as block chords) and provides defaults for realization knobs.
- An instrument (built-in synth, sampler, or drum kit in v1).
- A mixer position (inserts, sends, output routing — see `architecture.md`). Drum tracks with multi-out kits produce multiple mixer channels.

Tracks live at the project level. Sections do **not** own tracks — they own *activation entries* for tracks.

### Section ownership: activation, not ownership

Tracks are global to the project. Each section carries an activation table:

```
section.activations: Map<TrackId, ActivationEntry>
ActivationEntry {
  pattern_ref: Option<PatternRef>,   // None = silent in this section
  realization: RealizationParams,    // octave, voicing, humanization, velocity curve, etc.
  per_note_overrides: Vec<NoteOverride>,
}
```

This means moving the `chorus` section in time moves what every track plays in the chorus, without each section needing to re-declare the track list. Adding a new track gives it an empty activation in every section by default (silent until you place a pattern reference).

#### Sub-section ranges and variant scheduling

`ActivationEntry` supports sub-section time ranges so a single activation can play different content across different bars of the section. This is how drum fills, bass walk-downs, and other end-of-section variations are expressed:

```rust
struct ActivationEntry {
    pattern_ref: Option<PatternRef>,
    variant_schedule: Vec<(BarRange, VariantId)>,  // pinned variants per sub-range
    realization: RealizationParams,
    per_note_overrides: Vec<NoteOverride>,
}
```

If `variant_schedule` is empty, the pattern's `default_variant` plays for the full section. If it has entries, each entry pins a variant to a sub-section bar range; uncovered ranges fall back to the default variant. Sub-section ranges can also be used to silence a portion of an activation (`(BarRange, None)`).

The UX is: click a sub-range of an activation block in the song view, pick a variant from a dropdown. The activation block shows mini-segments visualizing the schedule.

### Clip

In rawdaw, "clip" is the user-facing word for an `ActivationEntry` instance — a pattern reference placed on a track in a section, with optional realization overrides and per-note overrides. There are no free-floating clips on the timeline; all clips belong to a (section × track) cell.

A clip's audible notes are *derived* at render time from:

```
realize(clip.pattern, section.chord_loop, section.scale, track.role, clip.realization)
  → concrete MIDI note events
  → apply clip.per_note_overrides
  → output
```

## Editing realized notes

When a user opens a clip's detail view, they see the *derived* notes (concrete pitches given the current chord context). Editing one of those notes has two possible modes:

1. **Pin as per-note override.** The edit attaches to that note position in the clip and survives chord-context changes (e.g. "this F always plays as F♯ in this clip even when the progression changes"). The pattern itself is unchanged.
2. **Fork.** Explicit user action: this clip becomes a literal-notes clip and loses its binding to the pattern. Use when "I just want to draw notes here."

v1 default: option (1) on direct edit, with a "fork from pattern" menu action for (2). We won't ship option (c) "edits rejected; pattern must be edited directly" — it's too restrictive.

Editing the *pattern* itself is done from the pattern editor (entered explicitly), and propagates to every clip that references it.

## What this changes about the UI

The piano roll is a *detail view*. The centerpiece of the UI is a **song-form / arrangement view**:

- Section blocks arranged in order along the timeline.
- Per-section overlays showing chord loop and active tracks.
- A library panel (probably side-docked) listing chord loops, patterns, and sections for drag/drop reuse.

The piano-roll-style editor opens when the user double-clicks a clip or pattern, and shows derived notes with structural context (the chord and scale at each beat shown above/below the notes).

The mixer is its own view, opened on demand.

## Resolved questions

- **Generate or constrain?** Generate. Notes are derived from structural layers. (Constrain-style snapping is a v2-or-never feature.)
- **Pattern model?** Rhythm + chord-degree / scale-degree sequence for pitched; symbolic voice names for drums (see `drum-patterns.md`).
- **Pattern kinds?** `Pitched` and `Drum` as a sum type; same variant + activation machinery.
- **Pattern variants?** First-class, named (`main`, `fill`, etc.), flat list under a parent pattern. Not deferred.
- **Sub-section time ranges on activations?** Supported; used for variant scheduling and partial-section silencing.
- **Section ownership of tracks?** Activation-based: tracks are global, sections specify activation.
- **Editing realized notes?** Default to per-note overrides; explicit "fork" available.
- **MIDI version?** Internal event model targets MIDI 2.0 (per-note controllers, high-resolution velocity) even if I/O surface is MIDI 1.0 initially.

## UI semantics shipped with the section + arrangement editor (S5)

The S1–S5 milestone (close-out `2026-05-22`,
`docs/section-arrangement-editing-plan.md`) made the
arrangement editable. A few interpretive decisions on top of
the model that aren't directly visible from the type
definitions:

- **No-gap, no-overlap arrangement contract.** `SectionRef.
  start: MusicalTime` is absolute, so the model technically
  permits gaps or overlaps. The v1 editor refuses both —
  `arrangement_actions::recompute_starts(&mut Project)` runs
  after every step mutation (append / insert / remove / move /
  duplicate / set-section / set-variant) and walks
  `project.arrangement.sections` packing each step's `start`
  flush against the previous step's end. Each step's
  effective duration is the active variant override's
  `duration_bars` if `Some`, else the base body's
  `duration_bars`. Steps cannot have explicit gaps in v1.
  Future support for silence / count-off would be a model-
  level change deferred to a later round.
- **Step duration follows section variant.** There is no
  per-step duration override on `SectionRef` — and
  intentionally so per `section-variants.md` (the section
  variant is the canonical knob for duration). To change a
  step's length, the user edits the section's variant
  duration in the section editor; `recompute_starts` then
  shifts every later step's `start` on the next mutation.
  Arrangement-view blocks therefore have no resize handle
  (vs. chord-loop bars and pattern notes, which do).
- **Selection axis stays narrow.** The arrangement editor does
  NOT add a new "selected arrangement step" axis to
  `AppState`. Block click does
  `app.set_selected_idx(Some(arrangement_step_idx))` —
  selecting the underlying section for editing in the
  section editor. Block actions (drag, ⋯ menu, right-click
  ContextMenu, variant chip) operate on click-time index
  via transient `arrangement_drag_preview` /
  per-component `menu_open` signals.
- **Drag commits go through the C2 drain-without-rewind
  pump.** Drag-to-move is registered via
  `Drag::absolute().on_move(...).on_end(...).start()` from
  inside the block's onclick (rinch fires onclick on
  mousedown). The `on_end` handler routes through
  `apply_project_edit(move_step(from, target))`, so the
  engine drains pending events and re-arms without resetting
  `sample_clock` to 0 — the playhead stays continuous during
  a move even with playback running.
- **F4 pixel-positioning is layout-only.** Post-S5, commit
  `6331a83` replaced the round-1 percent-based arrangement
  layout with pixel-based positioning + zoom (`pixels_per_bar:
  Signal<f32>` on AppState; native `overflow-x: auto` scroll;
  zoom toolbar at `regions/arrangement/toolbar.rs`). No model
  change — the underlying invariants above are unchanged. The
  Ruler / ChordRibbon / SectionLane / LaneFiller rows share
  a single pixel-width content wrapper inside the outer
  scrolling area so they always pan together.

## Open questions

See `open-questions.md` — keeping the running list there.
