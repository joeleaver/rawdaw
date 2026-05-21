# Pattern editor plan (v1)

> **Status (close-out, 2026-05-21):** All P0–P5 phases ✅ landed
> across commits `c076d20` (P0 plan + design lock) → `3d0d8ca`
> (P1 library CRUD) → `ccbd7b2` (P2 pitched piano-roll + per-note
> inspector) → `214b335` (P3 drum step-grid) → `6e74b00` (P4
> activation binding UI + variant scheduling) → `6497792` (P4.x
> variant-context activation editing) → `e405cfd` (P4.x merge over
> uncovered bars). The full pattern-editor surface is live: users
> can create / rename / duplicate / delete patterns; edit pitched
> patterns in a piano roll with the degree-relative inspector;
> edit drum patterns on the step grid; bind patterns to activation
> rows in the section editor; pin variants per-bar in the variant
> schedule with right-click split/merge; and edit non-base
> variants through `ActivationOverride::Replace` / `Silent`. 612
> workspace tests; clippy clean across default / no-default /
> cpal-driver. **Recommended next Tier-1 bite:** master-fx-chain
> X1 — its plan already exists at `docs/master-fx-chain-plan.md`
> and the soft-clipper replaces the audible `-12 dB` master-gain
> workaround. Section-editor completeness (block drag in the
> arrangement, inline rename, delete/duplicate) is the next
> alternative once the CL2.x drag-handle infrastructure (commit
> `71d4bd9`) is generalized to section-blocks. Out-of-scope items
> still apply — see the bottom of this doc.

The second Tier-1 plan after chord-loop-editing closed. Builds the
**pattern editor UI** on top of the same Tier-0 edit pump that
fed chord-loop editing.

Patterns are the next composition layer beneath chord loops in
Joe's top-down workflow ([[user-composition-workflow]]): chord
loops give the harmonic progression, patterns give the **parts**
(bass lines, melodies, drum grooves) that realize against that
progression. Without a pattern editor, the chord-loop work tops
out at "the realized strip shows my changes." With patterns, the
user can write a bass line in degree-relative space
(`PitchSpec::Chord { degree: Root, octave: … }`) and have it
re-realize automatically against any chord progression — the
headline payoff for the rawdaw composition model.

The model side is already shipped and well-typed:

- `rawdaw_model::pattern::{Pattern, PatternBody, PitchedEvent,
  DrumEvent, PitchSpec, OctaveSpec, EventHumanization,
  ArticulationTag, DrumVoice, …}` carries the full data model
  (see audit below).
- `rawdaw_model::realize` already realizes pitched + drum
  patterns against chord events and the project scale, honoring
  `PitchSpec::Scale / Chord / Absolute / Chromatic / Rest` +
  `OctaveSpec::{Nearest, Anchored, UpFromPrev, DownFromPrev,
  RelativeToRole}`.
- `rawdaw_model::activation::{ActivationEntry, …}` already
  binds a pattern to a track within a section with per-variant
  scheduling.
- `Project.id_allocators` already vends `PatternId` and `NoteId`
  with the durability contract ([[project-status]] §
  "Non-obvious conventions": NoteIds are never reused).

The work here is **purely UI**: editor surfaces that mutate
`Project.patterns` + `Project.sections[*].base.activations` +
the per-variant override maps through `apply_project_edit` so
the engine drains and re-arms in lockstep.

The mockup-side references are `docs/design/composition-model.md`
(activation hierarchy + variant scheduling), `docs/design/
realization.md` (how `PitchSpec` resolves), and `docs/design/
drum-patterns.md` (drum-grid UX expectations). Those docs are
the spec; this plan is the **build schedule** that gets us there
in phases, each ending with a green workspace.

**Engineering constraints** (from `CLAUDE.md`): architectural
correctness over shortcuts; unlimited time and budget; ~700-line
cap per source file; no `unwrap()` outside tests;
`forbid(unsafe_code)` in every crate.

**Cadence.** Each phase ends with `cargo test --workspace` green,
clippy clean across all three feature builds (default /
`--no-default-features` / `--features cpal-driver`), and a
one-line "done when" criterion observably met.

**Multi-session scope.** Larger than chord-loop-editing because
patterns carry two body types (pitched + drum), variants, and
activation binding into sections. **P0** is this doc. **P1** is
pattern library CRUD (mirrors CL1; least risky structural edit-
pump consumer). **P2** is the pitched-pattern piano-roll editor
(the meatiest piece). **P3** is the drum-pattern step-grid
editor (smaller surface but distinct interaction model). **P4**
is activation binding UI + variant scheduling per section. **P5**
closes out and points at the next Tier-1 bite (likely **section
editor completeness** — block drag, rename, delete/duplicate —
or **round-3 arrangement polish**).

---

## Status

- P0 ✅ — this document. Landed as part of CL5 (commit `c076d20`).
- P1 ✅ — pattern library CRUD (commit `3d0d8ca`, 2026-05-20).
  Adds create / rename / duplicate / delete for pitched + drum
  patterns through the C2 edit pump. `selected_pattern` becomes
  the fourth selection axis on `AppState`.
- P2 ✅ — pitched-pattern piano-roll editor (commit `ccbd7b2`,
  2026-05-20). Piano-roll surface with degree-relative pitch
  axis, click-empty-bar insert, focused-note inspector
  (`PitchSpec` kind selector, `OctaveSpec` controls, articulation
  + humanization). Selection-axis state: `focused_pattern_note:
  Signal<Option<NoteId>>` + `focused_variant: Signal<Option<VariantId>>`.
- P3 ✅ — drum-pattern step-grid editor (commit `214b335`,
  2026-05-20). 16-step grid with click-to-toggle, per-voice
  rows, drag-to-set velocity deferred (P3 design decision).
- P4 ✅ — activation binding UI + variant scheduling (commit
  `6e74b00`, 2026-05-21). Pattern picker on every activation row
  (Active + CellInherit placeholder); per-bar variant scheduler
  with right-click split/merge ContextMenu; pure helpers in
  `pattern_actions/variant_schedule.rs` mirror CL4's chord-loop
  schedule contract.
- P4.x ✅ — variant-context activation editing (commit `6497792`,
  2026-05-21). Edits on a non-base section variant route to
  `Section.variants[v].activations` via `ActivationOverride::Replace`
  / `Silent`; schedule mutators auto-promote a clone of base to
  Replace when no override exists yet.
- P4.x ✅ — merge over uncovered bars (commit `e405cfd`,
  2026-05-21). The variant-schedule merge helpers treat
  default-fill ranges as first-class segments: right-clicking a
  default-fill bar and picking merge-left/right adopts the
  adjacent override's variant id; right-clicking an explicit
  entry adjacent to default-fill drops the entry.
- P5 ✅ — this close-out. 612 workspace tests; clippy clean across
  default / `--no-default-features` / `--features cpal-driver`;
  release build clean. Recommended next Tier-1 bite recorded in
  the top-of-doc paragraph (master-fx-chain X1).

**File-cap split (incidental):** `8d6bc7b` split the pitched
inspector and `pattern_actions` to keep every source file under
the ~700-line cap as the editor grew. The two remaining
pre-existing cap violations — `piano_roll.rs` (790) and
`regions/inspector/mod.rs` (705) — are not introduced by this
milestone and remain on the hygiene list.

---

## Model audit (background — does not block P0)

Captured from a fresh codebase walk so subsequent phases don't
re-derive it.

### Pattern types (`crates/rawdaw-model/src/pattern.rs`)

- `Pattern { id: PatternId, name: String, default_variant:
  VariantId, body: PatternBody }`.
- `PatternBody` is `Pitched(PitchedPatternBody)` or
  `Drum(DrumPatternBody)` — a single pattern is one or the other.
- `PitchedPatternBody { metadata: PitchedPatternMetadata,
  variants: BTreeMap<VariantId, Vec<PitchedEvent>> }`.
  `PitchedPatternMetadata { length: Duration }`.
- `PitchedEvent { note_id: NoteId, time: MusicalTime, duration:
  Duration, velocity: U7, articulation: Option<ArticulationTag>,
  humanization: EventHumanization, spec: PitchSpec }`.
- `PitchSpec` variants:
  - `Scale { degree: ScaleDegree, octave: OctaveSpec }`
  - `Chord { degree: ChordDegree, octave: OctaveSpec }`
  - `Absolute { pitch_class: PitchClass, octave: OctaveSpec }`
  - `Chromatic { semitones_from_prev: i32 }`
  - `Rest`
- `OctaveSpec` variants: `Nearest` (voice-leading), `Anchored(i8)`,
  `UpFromPrev`, `DownFromPrev`, `RelativeToRole(i8)`.
- `DrumPatternBody { metadata: DrumPatternMetadata, variants:
  BTreeMap<VariantId, Vec<DrumEvent>> }`.
  `DrumPatternMetadata { length: Duration, voices: Vec<DrumVoice> }`.
- `DrumEvent { note_id: NoteId, time: MusicalTime, duration:
  Duration, voice: DrumVoice, velocity: U7, articulation, humanization }`.
- `EventHumanization` (assume time + velocity jitter ranges; treat
  as opaque from the UI side — inspector toggles, no rich curve UI
  in v1).

### Activations (`crates/rawdaw-model/src/activation.rs`)

- `ActivationEntry` binds a pattern to a track within a section.
  Carries variant schedule (`Vec<(BarRange, VariantId)>` — uncovered
  ranges fall back to `pattern.default_variant`) + realization
  parameters + per-`NoteId` overrides.
- Activations live on `Section.base.activations` and
  `Section.variants[VariantId].activations`. The activations vector
  is keyed per-track.

### Realization (`crates/rawdaw-model/src/realize/`)

- Realization walks the arrangement section-by-section, the
  activations per track, and emits timed MIDI events.
- `PitchSpec::Scale / Chord` resolve against the chord active at
  the event's start time (chord loops live above patterns in the
  realization stack).
- `PitchSpec::Absolute` pins MIDI note directly (escape hatch).
- `OctaveSpec::Nearest` needs the previous-event pitch state at
  realization time; the editor doesn't need to track this — render
  with a heuristic (anchor to a configurable preview octave;
  highlight the realized note relative to it).
- `in_key` on the active chord propagates down through chord-step
  resolution.

### Existing UI surface

- `regions/library/mod.rs` has a read-only Patterns group built by
  `build_pattern_rows()`. No click binding. P1 adds CRUD parallel
  to `regions/library/chord_loops.rs`.
- No pattern editor region exists. `regions/chord_loop_editor/`
  is the structural precedent.
- `AppState` carries `selected_idx`, `selected_track`,
  `selected_chord_loop`, `focused_chord_event_idx`. **No
  `selected_pattern` yet.** P1 adds it as the fourth selection
  axis with the same mutex (selecting a pattern clears track +
  chord-loop selection).
- `ProjectOverlay.pattern_color` + `pattern_meta` BTreeMaps are
  already in place — the color row in the Library Patterns group
  reads them today.

### Fixture coverage

`rawdaw_model::fixtures::round1` builds four patterns:
`bass-main` (2 variants: `main`, `fill`), `lead-main`,
`drums-main` (variants: `main`, `fill`), `pad-bed`. These are the
load-bearing fixtures the pattern editor must round-trip without
loss when displayed + re-saved through the C2 pump.

---

## Phase P0 — Plan + design decisions ◀ this doc

Lock the architectural choices in "Design decisions" below. No
code changes. The phase boundary criteria are documented here so
P1 has a concrete checklist.

**Done when:** This doc lands on `main`.

---

## Phase P1 — Pattern library CRUD

The structural-surgery phase. After P1, the user can add, rename,
delete, duplicate, and recolor patterns in the project library
through the existing Library panel — the editor surface lands in
P2 (pitched) and P3 (drum). P1 doesn't touch pattern *events* —
only the `Pattern` envelope (id, name, body kind, default
variant, color).

**App side:**

- Library panel's Patterns group grows:
  - A `+ pitched` / `+ drum` button pair (or a single "+" with a
    sub-menu). Body kind is locked at creation; converting a
    pitched pattern to a drum pattern is not supported.
  - Per-row context menu (`⋯` button mirroring chord loops) for
    Rename / Duplicate / Delete / Pick color.
  - Inline rename uses the same `NameControl` / `untracked` Effect
    pattern.
- New selection state: `AppState.selected_pattern:
  Signal<Option<PatternId>>` becomes the fourth selection axis.
  Mutex: setting it clears `selected_track` + `selected_chord_loop`
  + `selected_idx`; other axes clear it. MIDI sticky-target
  ([[K-series]]) is untouched.
- Color picker writes to `ProjectOverlay.pattern_color` — same
  pattern as chord-loop colors.
- Delete refusal: if the pattern is referenced by any
  `ActivationEntry` in any section's base or variants, refuse with
  the reference list (mirrors CL1's `delete_chord_loop`). The dry-
  run uses a new `pattern_actions::pattern_references(project, id)
  -> Vec<(SectionName, VariantId, TrackName)>` helper.
- Duplicate clones the pattern with a fresh id + suffixed name
  (`"bass-main"` → `"bass-main copy"`); all NoteIds are
  freshly-allocated (durability says don't reuse, even for a clone
  the new notes get new ids).
- `+ pitched` creates an empty `PitchedPatternBody` with a
  default `length` of 4 bars + a single `default_variant` of
  `VariantId::new("main")` + an empty event vec. `+ drum` creates
  a drum body with the four-voice default
  (`Kick/Snare/ClosedHat/OpenHat`) — matches the drum-synth's GM
  classifier.

**Module layout (P1 lands these):**

- `crates/rawdaw-app/src/pattern_actions.rs` — `create_pitched_pattern`,
  `create_drum_pattern`, `rename_pattern`, `duplicate_pattern`,
  `delete_pattern`, `set_pattern_color`, `pattern_references`,
  `unique_pattern_name`. ~350–450 lines + unit tests.
- `crates/rawdaw-app/src/regions/library/patterns.rs` — interactive
  Patterns group, parallel to `chord_loops.rs`. Forces splitting
  `regions/library/mod.rs` (already a multi-file dir post-CL1).

**Done when:**
- Library can create, rename, duplicate, delete, recolor pitched +
  drum patterns. Delete is refused-with-reference-list when bound;
  delete unbinds when free.
- `AppState.selected_pattern` exists and is exercised by the row's
  click handler + visual selection highlight.
- `cargo test --workspace` green; clippy clean across default /
  `--no-default-features` / `--features cpal-driver`; release
  build clean.

---

## Phase P2 — Pitched-pattern piano-roll editor

The meaty phase. After P2, the user can author monophonic and
polyphonic pitched patterns through a piano-roll surface that
mirrors the chord-loop editor's compositional model: degree-
relative input by default, an Absolute escape hatch per-note, a
per-note inspector that exposes the full `PitchedEvent` field set.

**App side:**

- New region `regions/pattern_editor/` fanned into files from
  day one (per design decision 10 below):
  - `mod.rs` — top-level surface + EditorHeader (rename + length
    nudge + variant tabs + close button). Dispatches on
    `PatternBody::Pitched` vs `Drum` (`Drum` arm is a deferral
    banner until P3 lands).
  - `pitched/mod.rs` — pitched-editor shell.
  - `pitched/piano_roll.rs` — the grid surface itself: pitch axis
    + time axis + drawn notes. Likely the largest file; split
    further if it crests ~500 lines (cells / axes / interaction
    handlers each their own module).
  - `pitched/note_block.rs` — one cell with reactive focused-
    state highlight, parallels CL2's `event_block.rs`.
  - `pitched/helpers.rs` — pure helpers: `snap_time_to_grid`,
    `default_pitched_event`, `insert_note_sorted`,
    `delete_note_by_id`, `next_free_note_position`. Unit-tested.
  - `pitched/inspector/` — per-note inspector split by field
    group: `spec.rs` (PitchSpec kind toggle + sub-editors for
    Scale / Chord / Absolute / Chromatic / Rest),
    `timing.rs` (time + duration nudges), `dynamics.rs` (velocity
    + articulation), `humanization.rs` (humanization toggles).
- **Pitch axis** renders 25 semitones of vertical space by default
  (~2 octaves, configurable). Labels show degree (1, 2, 3, …)
  under the project's default key with chromatic accidentals.
  `PitchSpec::Absolute` notes render labeled with their absolute
  pitch class on the corresponding row.
- **Chord-context preview picker** in the editor header: a small
  dropdown sets "the chord this pattern previews against" so the
  realized strip beneath the roll can show concrete pitches. Default
  is `I` in the project's default key. Changing the picker live-
  re-renders the realized strip; does not mutate the pattern.
- **Realized strip** beneath the roll mirrors the chord-loop
  editor's CL4 realized strip: one cell per note, width-
  proportional to duration, showing the realized absolute pitch
  under the current chord-context + project key.
- **Variant tabs** at the top of the editor: one tab per
  `VariantId` in `pattern.variants`; `+` button creates a new
  variant (asks for a name; defaults to `untitled-N`); right-click
  rename / duplicate / delete. Tabs read+write
  `AppState.focused_variant: Signal<Option<VariantId>>` (P2 adds
  this signal alongside `selected_pattern`). Default variant is
  bolded.
- **Note-block interactions (v1 scope):** click empty space →
  insert default note at snapped time; click note → select +
  populate inspector; Delete → remove selected; click+drag on
  note body → defer to P2.x (out of v1, same as CL2's deferred
  drag handlers).

**Pure helpers (P2 lands these as unit-tested helpers):**

- `snap_time_to_grid(time: MusicalTime, grid: GridSpec)
  -> MusicalTime` — quantizes to the chosen grid (`1/16`, `1/8`,
  `1/4`, `triplet-*` variants).
- `insert_note_sorted(events: &mut Vec<PitchedEvent>, new:
  PitchedEvent)` — keeps the event vec sorted by `time` then
  stable-by-NoteId.
- `default_pitched_event(time: MusicalTime, project_key:
  &Scale) -> PitchedEvent` — `PitchSpec::Scale { degree: 1,
  octave: OctaveSpec::Anchored(3) }`, velocity `64`, duration =
  one grid cell.

**Selection model:**

- `AppState.focused_pattern_note: Signal<Option<NoteId>>` —
  parallel to `focused_chord_event_idx` but keyed by durable
  NoteId (patterns are mutable and id-stable; idx-keying would
  break under insert/delete). The inspector's value-fn dispatchers
  look up the note by id on every read.

**Done when:**
- Creating + editing notes in a pitched pattern visibly + audibly
  changes playback: when the pattern is bound to a track in a
  section, the engine emits the new notes on the next playback
  block.
- Editing a pattern that's referenced by multiple activations
  updates every reference (single source of truth — the model
  already guarantees this; P2 just exercises it).
- Variant tabs round-trip: switching variants reveals the
  per-variant event vec; edits in variant A don't leak into
  variant B.
- `cargo test --workspace` green; clippy clean.

---

## Phase P3 — Drum-pattern step-grid editor

The drum-grid phase. After P3, the user can author drum patterns
through a step grid + per-cell velocity editing. Smaller scope
than P2 — voices are one-shot (no duration UX), and pitch is
fixed per row.

**App side:**

- `regions/pattern_editor/mod.rs` dispatch arm for `PatternBody::
  Drum` activates:
  - `drum/mod.rs` — drum-editor shell.
  - `drum/step_grid.rs` — the grid surface. Rows =
    `DrumPatternMetadata.voices` (in declaration order — first
    voice is the bottom row, matching DAW convention). Columns =
    grid steps (default 16 = one bar in `1/16` resolution;
    configurable per design decision 6).
  - `drum/cell.rs` — one step cell: click toggles on/off, drag
    sets velocity (vertical drag maps to U7 0..127). Inactive
    cells render dimmed; active cells render at velocity-derived
    opacity.
  - `drum/inspector/` — per-event inspector for the focused step
    (timing nudge, velocity slider, articulation tag,
    humanization toggles). Parallels P2's pitched inspector but
    drops the PitchSpec sub-editors.
  - `drum/helpers.rs` — `step_to_time(step: usize, grid:
    GridSpec, length: Duration) -> MusicalTime`,
    `time_to_step(...)`, `toggle_step(events, voice, step)`,
    `set_step_velocity(events, voice, step, velocity)`. Pure;
    unit-tested.
- **Voice management** lives in the editor header: rename voice,
  add voice (defaults to `Kick`), remove voice, reorder by drag
  (deferred to P3.x if drag is non-trivial). Adding a voice
  writes to `metadata.voices`; removing one purges all `DrumEvent`
  with that voice from every variant.
- **Variant tabs** behave identically to P2's — the variant signal
  is shared across pitched + drum.

**Done when:**
- Creating + editing drum steps in a drum pattern visibly +
  audibly changes playback.
- Per-cell velocity edits propagate to the engine through the
  edit pump.
- Variants behave identically to pitched patterns.
- `cargo test --workspace` green; clippy clean.

---

## Phase P4 — Activation binding UI + variant scheduling

The "wire patterns to sections" phase. After P4, the user can bind
patterns to per-track lanes within a section, schedule which
variant plays where, and the section editor shows the binding
state visibly.

**App side:**

- The section editor grows a per-track **activations lane** beneath
  the existing `chord_loop_bar`. Each lane row is one track in the
  project; each cell within the lane shows the bound
  `ActivationEntry` for the bar range it covers + the active
  `VariantId` for that range.
- Cell click opens a `Select` (same primitive CL4 used per the
  DropdownMenu workaround note in [[chord-loops-design]]) with
  the list of patterns + "(no pattern)". Selecting a pattern
  writes an `ActivationEntry` for the bar range through
  `apply_project_edit`.
- Per-cell variant picker (secondary affordance — right-click,
  or a sub-line beneath the pattern name) selects the
  `VariantId` for that bar range from the bound pattern's
  variants. Default = `pattern.default_variant`.
- **Pure schedule mutation** `set_activation_for_bar(activations,
  track, bar, new_entry) -> Vec<(BarRange, ActivationEntry)>`
  — same split/merge contract as CL4's `set_loop_for_bar`.
  Unit-tested with the same 8-case coverage.
- Right-click "merge with left/right" / "split at this beat" is
  deferred to a P4.x follow-up alongside the chord-loop CL4.x
  follow-up (the helpers compose; the UI is the gap).

**Section-editor changes:**

- `crates/rawdaw-app/src/section_editor/` grows
  `activation_lane.rs` (paralleling `chord_loop_bar.rs`). The
  section editor's outer layout becomes:
  `meta_bar.rs` → `chord_loop_bar.rs` → one `activation_lane.rs`
  per track.

**Done when:**
- Binding a pattern to a track range emits the realized MIDI on
  the next playback block.
- Variant-pinning a bar range plays that variant on the next
  playback block.
- Removing a binding silences that track for that range.
- Realization correctly drops `ChordDegree` / `ScaleDegree` events
  in bar ranges that have no chord-loop coverage (already a model
  guarantee — P4 just exercises the boundary).
- `cargo test --workspace` green; clippy clean.

---

## Phase P5 — Close-out + next Tier-1 bite

- Update `project_status` memory: pattern editor milestone ✅;
  point at the recommended next Tier-1 bite.
- Update `project_next_session_pickup` with the chosen next bite's
  plan-doc path.
- Workspace test count + clippy / release clean state recorded in
  the Status section above with commit hashes per phase.
- Re-check `docs/design/composition-model.md` +
  `docs/design/realization.md` +
  `docs/design/drum-patterns.md` against what shipped; capture
  any deviations in those design docs (not in this plan).
- One paragraph at the top of this plan summarizing what landed
  and the open items (the Out of Scope list below).

**Likely next Tier-1 bite candidates:**

- **Section-editor completeness** — block drag in the arrangement,
  rename sections inline, delete/duplicate. Smaller; brings the
  arrangement surface up to library parity. CL2.x's drag handlers
  + CL4.x's split/merge UI are natural co-residents of this bite.
- **Round-3 arrangement polish** — visual chrome pass on the
  arrangement row + tracks pane after the synth-editor design pass
  picks a token palette. [[project-ui-redesign-pending]] gates
  this; defer until the synth-editor pass lands.
- **Master-fx chain X1–X8** — the soft-clipper milestone has a
  plan already (`docs/master-fx-chain-plan.md`). Independent of
  composition surfaces; sits orthogonally.

**Done when:**
- Memory updated.
- All P phases ✅ in the Status section above with commit hashes.
- The next Tier-1 plan doc exists (or this plan's close-out
  paragraph names the chosen bite if it's the master-fx X plan
  doc that already exists).

---

## Design decisions locked in P0

### 1. Where the pattern editor lives in the round-1 UI

**Decision:** new full-width region that replaces the
`Inspector` / `SectionEditor` panel when a pattern is selected
from the Library — same model as CL2's chord-loop editor. The
Library remains visible on the side; the editor takes center
stage. Closing the editor (× button or selecting a section /
track / chord-loop in the library) reverts to the previous
inspector content.

Rationale: the piano roll needs horizontal real estate for the
time axis + pitch axis + per-note inspector. Cramming it into the
existing inspector pane is a non-starter. The structural-editor-
takes-center-stage pattern is already the precedent
(`chord_loop_editor`, `section_editor`).

Defer the visual chrome to the design pass
([[project-ui-redesign-pending]]). P2/P3 ship a placeholder
layout; the pass tightens it.

### 2. Pitched + drum get separate editors, dispatched by `PatternBody`

**Decision:** the `pattern_editor::mod.rs` top-level region
dispatches on `PatternBody::Pitched` vs `Drum` and renders one of
two distinct sub-editors. They share the editor header (name +
length + variant tabs + close) and the C2 edit-pump plumbing,
but the interaction model differs sharply (piano roll vs. step
grid) and cramming both into one surface obscures both.

Precedent: the synth editor dispatches on `Track::synth` between
the Wavetable + Drum editors with the same shape (shared header,
distinct bodies). U5/U7 established this; P2/P3 follow it.

### 3. Degree-relative input is the default; Absolute is the escape hatch

**Decision:** new pitched notes default to `PitchSpec::Scale
{ degree: 1, octave: OctaveSpec::Anchored(3) }`. The per-note
inspector's first field is the `PitchSpec` kind selector
(`Scale` / `Chord` / `Absolute` / `Chromatic` / `Rest`), and
switching kinds preserves as much state as makes sense (e.g.
`Scale { degree: 5, octave: Anchored(3) }` → `Absolute { pitch_class
: G, octave: Anchored(3) }` under a C-major project key — the
realized pitch carries over verbatim).

Rationale: this mirrors chord-loops' Functional-as-default + the
[[user-composition-workflow]] motivation. Joe composes top-down;
the chord-loop layer is functional Roman numerals; the pattern
layer naturally rides on the same degree-relative space so a part
written in C transposes when the section's key changes.

The piano roll's pitch axis labels primarily by *degree* (with
chromatic accidentals labeled `b3`, `#4`, etc.); a settings toggle
flips to *absolute* pitch labels for the rare cases where the
user wants the keyboard-style view. Default is degree.

### 4. Chord-context preview picker is editor-local, not project-state

**Decision:** the "what chord am I previewing this pattern
against" picker lives in the editor header as a UI-local signal;
it does NOT serialize to the project. Default = the pattern's
home key's `I` chord. Changing it re-renders the realized strip
only; nothing about the pattern mutates.

Rationale: a pattern previews against many chords across an
arrangement. Pinning a "preview chord" into the model would be a
display-state leak. Keep it transient.

The picker offers: `I`, `ii`, `iii`, `IV`, `V`, `vi`, `vii°`
(or whatever the project key's diatonic set is) + a "custom"
escape for any `ChordSpec`. v1 ships the diatonic set only;
custom-chord preview is out of scope.

### 5. Variant tabs at the top, scheduling lives in the section editor

**Decision:** the pattern editor surfaces variant **creation** and
**editing** (tabs at top, +button, rename, delete, duplicate);
the section editor surfaces variant **scheduling** (per-bar
binding of which variant plays in this section's variant context,
landed in P4's activation lane).

Rationale: variants are content; scheduling is context. Editing
a variant's notes is a pattern-editor concern; deciding which
variant plays during the chorus is a section-editor concern.
Mirrors how the chord-loop editor edits a loop's chords while
the section editor binds the loop to a bar range.

### 6. Grid resolution is per-pattern, configurable

**Decision:** `GridSpec` carries `{ subdivision: 1/4 | 1/8 | 1/16
| 1/32, triplet: bool }` — the editor's note-snap resolution is a
pattern-editor-local Signal, defaulting to `1/16` straight for
pitched and `1/16` straight for drum (configurable in the editor
header). Not serialized; transient.

Rationale: snap resolution is a UX preference, not a property of
the music. Snapping at `1/16` doesn't constrain note positions —
nothing in the model rounds to a grid — but it makes click-input
predictable.

### 7. CRUD goes through the C2 edit pump, no shortcuts

**Decision:** every pattern mutation flows through
`AppState::apply_project_edit`. No direct mutex pokes; no
side-channel writes. The pump's `request_song_queue_drain` is
the only path that keeps the audio thread in lockstep.

Rationale: chord-loop editing (CL1–CL4) proved this works; P1–P4
inherit the same plumbing for free. Any "fast path" temptation
(e.g. velocity drag without an edit-pump round-trip) gets a
firm "no" — the next note-on after the drag picks up the new
velocity from the song queue, which is exactly the contract.

### 8. Selection model: extend `AppState`, not invent a new pattern

**Decision:** add `selected_pattern: Signal<Option<PatternId>>`
+ `focused_pattern_note: Signal<Option<NoteId>>` +
`focused_variant: Signal<Option<VariantId>>` to `AppState`.
Selection mutex grows to cover all four selection axes (track,
chord-loop, pattern, arrangement-block) — only one selectable
at a time. Focus signals (chord event idx, pattern note,
variant) are tied to their parent selection — selecting a new
pattern clears `focused_pattern_note` + resets `focused_variant`
to the pattern's `default_variant`.

Rationale: same shape CL2 + CL3 used; one source of truth for
selection means the UI never gets into "two things highlighted"
states.

### 9. NoteId durability matters; key UI by NoteId, not idx

**Decision:** any UI Signal that points at a `PitchedEvent` or
`DrumEvent` uses `NoteId`, not vector index. Inspector value-fn
dispatchers re-fetch by id on every read; insert/delete operations
preserve other notes' ids; clone-via-duplicate allocates fresh
ids.

Rationale: NoteId is a model-level durability contract
([[project-status]] § "Non-obvious conventions"). Per-note
overrides (articulation, humanization, future plug-in
parameters) hang off NoteId; an idx-keyed UI would break under
insert/delete. CL2 documented this gotcha for chord events
(focus by idx is fine when events are sorted-by-time and the
inspector tolerates idx drift); patterns are stricter because
per-note state grows.

### 10. File-cap discipline: split eagerly

**Decision:** `regions/pattern_editor/` lands with its file
fanout from day one (per CLAUDE.md § 3 — ~700-line cap is a
trigger, not a target). The directory layout (mod / pitched /
drum / inspector / helpers) is the precedent from CL2's
`regions/chord_loop_editor/`.

Rationale: cramming a piano roll + a step grid + two inspector
trees into one file is the obvious path to a 2000-line refactor
later. Pay the structural cost up front.

### 11. Length editing: keep `Duration` as the model type

**Decision:** the editor header's length nudge writes
`PitchedPatternMetadata.length` (or `DrumPatternMetadata.length`)
directly through `apply_project_edit`. Shrinking the length
**does not** delete out-of-range events — they're retained but
not realized (mirrors how chord-loop CL2 handled length shrink).
Restoring the length restores them.

Rationale: lossless edits beat lossy "convenience" deletes.
Users can manually delete trailing notes if they want; the
inverse (recovering deleted notes after a length nudge) isn't
possible.

### 12. Audition / preview is out of scope for v1

**Decision:** v1 has no "play this pattern in isolation against a
preview chord" affordance. The user previews patterns by binding
them to a section and pressing Play.

Rationale: isolated audition requires a side audio path (or a
"solo this activation" toggle); both are non-trivial relative to
v1's scope. The chord-context preview picker (decision 4) is
enough to *visualize* the realized pitches; *hearing* them
requires the section binding workflow, which is already P4.

### 13. Octave-input UX: separate from PitchSpec kind selector

**Decision:** the inspector's octave row is its own sub-editor
(`OctaveSpec::{Nearest, Anchored, UpFromPrev, DownFromPrev,
RelativeToRole}`). The default for new notes is
`OctaveSpec::Anchored(3)` (one of the simpler-to-reason-about
variants). `Nearest` is the realization-side default for voice-
leading-aware lines; the inspector lets the user opt in.

Rationale: keeping the octave story explicit in the UI surfaces
voice-leading as an editable property rather than burying it in
the realization pass. Joe's composition workflow treats melodic
contour as a first-class concern; the editor should too.

---

## Out of scope (Tier-1 follow-ons and beyond)

- **Drag-to-move + resize-edge handlers** for pitched + drum
  events. Same scope cut as CL2; defer to P2.x / P3.x.
- **Right-click ContextMenu** for note ops (delete, duplicate,
  cut/paste). v1 uses the Delete key + the inspector. P2.x / P3.x.
- **Step recording from live MIDI input.** The K-series MIDI path
  ([[K-series]]) already feeds notes into the engine; routing
  those into a recording buffer that writes to the focused
  pattern is its own piece of work. Tier-2.
- **MPE / per-note expression curves.** `EventHumanization` has
  the data slots; the UI doesn't ship rich shaping. Tier-2.
- **Quantize tools beyond grid-snap.** Iterative quantize, swing
  templates, groove import — all Tier-2.
- **Articulation-tag rich editor.** v1 ships a dropdown of the
  defined `ArticulationTag` variants. Shaping articulation into
  per-note envelope curves is Tier-2.
- **Pattern chord-context picker showing custom chords.** v1
  picks from the project key's diatonic set. Custom-chord
  preview is Tier-2.
- **Pattern variants linked to chord-loop variants.** v1 keeps
  variant scheduling per-track-per-section in P4's activation
  lane. Cross-binding pattern variants to chord-loop variants is
  Tier-2.
- **Cut / copy / paste of note ranges.** Single-note Delete +
  add-by-click are the v1 affordances. Tier-2.
- **Pattern templates / library.** v1 starts from blank patterns
  or duplicates of existing ones. Browseable templates are
  Tier-2.
- **Audition / solo modes.** See design decision 12.
- **Pitched-event chromatic-relative-to-prev mode (`PitchSpec::
  Chromatic`) inspector.** v1 ships the field but the inspector
  edits the `semitones_from_prev` value as a raw integer.
  Smarter UX (drag-up/down, melodic-contour hints) is Tier-2.

[[user-composition-workflow]]
[[project-status]]
[[project-next-session-pickup]]
[[chord-loop-editing-plan]]
