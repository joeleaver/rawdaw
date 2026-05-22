## Pattern editor follow-ons plan (v1)

Three pattern-editor paper cuts deferred from the P milestone
(`docs/pattern-editor-plan.md`) bundled into a small follow-on. Each
gap was explicitly called out as a P-phase deviation; none of them
block creating songs end-to-end, but each one constrains what
patterns can express. This plan addresses them as a discrete
milestone after the section + arrangement editor lands but before
the round-3 visual redesign.

**Engineering constraints** (from `CLAUDE.md`): architectural
correctness over shortcuts; unlimited time and budget; ~700-line
cap per source file; no `unwrap()` outside tests;
`forbid(unsafe_code)` in every crate.

**Cadence.** Each phase ends with `cargo test --workspace` green,
clippy clean across all three feature builds (default /
`--no-default-features` / `--features cpal-driver`), and a one-line
"done when" criterion observably met via the `rinch` MCP server.

**Multi-session scope.** Smaller than CL / P / S. **G0** is this
doc. **G1** is configurable grid resolution. **G2** is drag-to-set-
velocity in the drum step grid. **G3** is `Extra(_)` voice creation
UI for drum patterns. **G4** closes out.

---

## Status

- **G0** — this document.
- **G1** — Configurable grid resolution (pitched + drum editors).
  - Today both pattern editors hardcode `STRAIGHT_SIXTEENTH`. The
    pitched piano-roll grid is laid out in
    `regions/pattern_editor/pitched/`; the drum step-grid in
    `regions/pattern_editor/drum/`. Triplet rhythms, 32nd-note
    fills, and dotted-eighth grooves are unrepresentable in the UI
    even though the model accepts any `MusicalTime`.
  - Design (P2 § 6): a `Signal<GridResolution>` on the editor
    region scopes the grid choice per pattern editor instance. The
    enum lives in `rawdaw_model::time` already as the existing
    `GridResolution` (`StraightSixteenth`, `StraightEighth`,
    `StraightQuarter`, `TripletEighth`, `TripletSixteenth`,
    `Sixtyfourth`, …). A toolbar dropdown above the piano-roll /
    step-grid surface selects the resolution; the grid SVG and
    snap-to-grid maths read the signal.
  - **Per-pattern vs per-session.** v1 is **per-pattern-editor-
    instance**, persisted to the pattern model as
    `Pattern.editor_grid_resolution: Option<GridResolution>`. None
    falls back to a project-default (which v1 hardcodes to
    `StraightSixteenth` for backwards compatibility). Persisting
    per-pattern means reopening a triplet pattern restores the
    triplet grid. Two-place edit: model field + UI toolbar.
  - **Done when:** both editors expose a grid-resolution dropdown
    (≥6 options); changing the dropdown re-renders the grid; new
    notes / steps snap to the new resolution; the choice persists
    across project save/load; MCP confirms the dropdown + snap
    behavior.
- **G2** — Drag-to-set velocity in drum step grid.
  - P3 shipped click-to-toggle cells at a fixed velocity (per
    P3 design decision: "drag-to-set-velocity deferred"). v1
    surface for velocity edits is the per-step inspector context
    menu. Users editing dynamics on a busy 32-step pattern have to
    click through each step.
  - **Surface.** Holding shift + clicking a step, or click-and-
    drag-vertically on a step, sets the velocity continuously.
    Pixel range maps to 1..=127 with linear scaling. While
    dragging, the cell renders a vertical fill bar reflecting the
    velocity (the existing fill shape that currently shows a
    binary on/off becomes proportional).
  - **Action wrapper.** New pure helper in `pattern_actions/
    drum_pattern.rs::set_step_velocity(pattern_id, voice, step_idx,
    velocity)` plus its `apply_project_edit` consumer. Cell
    toggle stays as today (click without modifier toggles on/off
    using the previously-set velocity or default 100). Tests pin
    the [1, 127] clamp + the no-op when targeting a step that
    doesn't exist.
  - **Done when:** vertical drag on a step cell adjusts velocity
    continuously, with live visual feedback; click-to-toggle
    behavior unchanged when no drag is initiated; MCP confirms a
    drag from one cell raises its velocity visibly; project
    save/load round-trips the velocity per step.
- **G3** — `Extra(_)` voice creation UI for drum patterns.
  - The model supports `DrumVoice::Extra(name)` for user-defined
    voices on top of the canonical GM set (Kick / Snare /
    ClosedHat / OpenHat). P3 didn't ship UI for it — the only way
    to get an `Extra` voice today is editing the project file
    directly.
  - **Surface.** Drum pattern editor's voice-row strip gains a
    trailing `+ add voice` row. Click → inline rename → commits
    with `add_extra_voice(pattern_id, name)`. Each `Extra` row
    gets a `⋯` menu with Rename / Delete. Canonical rows
    (Kick / Snare / etc.) don't get the menu — they're always
    present.
  - **MIDI routing.** Extra voices have no GM mapping — they're
    transcribed into the realization stream as additional drum
    notes with a synthesized note number (e.g., 60 + extra-index
    to stay out of the GM range). The synth-side already routes
    "any unknown drum note" to a fallback / classifier-default; G3
    doesn't touch synth code. Future work: per-extra-voice synth
    routing. Out of scope here.
  - **Action wrappers.** `pattern_actions/drum_pattern.rs::
    add_extra_voice(id, name)`, `rename_extra_voice(id, old,
    new)`, `remove_extra_voice(id, name)`. Rename rewrites all
    drum-pattern step references with the old name; remove
    forbids when steps reference the voice or surfaces a
    "referenced by: 5 steps" message. Mirrors the delete-
    refusal pattern from chord-loop / pattern / section CRUD.
  - **Done when:** user can add an extra voice from the drum
    editor, place steps on it, rename / delete it (with refusal
    when referenced); project save/load round-trips. MCP confirms
    the add + place + delete flow.
- **G4** — Close-out.
  - Plan doc + design-doc deviations + memory pointers, same
    cadence as P5 / S6.

---

## Out of scope (for this milestone)

- **Per-extra-voice synth routing.** Adding a custom voice to a
  pattern doesn't get you a new synth voice today — it produces a
  fallback-classified drum note. A separate small milestone
  (probably in the `rawdaw-synth-drum` crate) wires per-voice
  synth-side routing.
- **Variable grid per row** (e.g., kick on quarters, hat on 16ths).
  Multiple resolutions per pattern is round-3 UX. v1 has one
  grid per pattern.
- **Velocity ramp / random** humanization. The model already
  supports per-activation humanize parameters (`ActivationEntry`);
  the inspector surface lands in a later round.
- **Polyrhythm grids** (5/4, 7/8, …). Tempo + meter still constant
  per project.

---

## Phase G0 — Design decisions (this document)

### 1. Resolution choice persists per-pattern

A user opening a triplet swing pattern in the editor wants to see
the triplet grid immediately. Persisting per-pattern via a new
model field `Pattern.editor_grid_resolution: Option<GridResolution>`
gives that for free. `None` → project default → v1 hardcoded
`StraightSixteenth`. Backwards compatible (serde defaults to None
on missing field for pre-G1 projects).

### 2. Velocity drag uses vertical-only motion

Horizontal motion is reserved for future "drag to extend step
length" semantics if we add variable step lengths. v1 vertical-
only drag avoids axis ambiguity. Shift-click as an alternate
modifier for users who prefer click-to-set is also wired (single-
click + shift opens an inline numeric input).

### 3. Extra-voice rows live below the GM rows

Voice ordering in the drum editor: canonical GM voices first
(Kick / Snare / ClosedHat / OpenHat) in fixed order; Extra voices
appended in insertion order. The trailing `+ add voice` row anchors
at the bottom. Reordering Extra voices is out of scope for v1
(future paper cut).

### 4. Extra-voice name collisions

`add_extra_voice("hat")` when an Extra voice named "hat" already
exists rejects with `Conflict`. Auto-suffix (`hat-2`, `hat-3`, …)
follows the same convention as section / chord-loop / pattern name
conflicts. Distinct from GM voice names: an Extra voice can be
named "Kick" without colliding with the canonical kick (they're
keyed differently in the model — `DrumVoice::Kick` vs
`DrumVoice::Extra("Kick".to_string())`). UI disambiguates with a
visual marker on the Extra row.

### 5. Tests live in pattern_actions/drum_pattern.rs

All three G phases extend `pattern_actions/drum_pattern.rs` (which
already houses the drum step CRUD). Anticipate splitting into
`pattern_actions/drum_pattern/{steps.rs, velocity.rs, voices.rs}`
if the file approaches cap during G2 / G3.

---

## Phase G1 — Configurable grid resolution

**Scope.** Toolbar dropdown + per-pattern persistence + snap-to-
grid maths threaded through both editors.

**Files.**
- `rawdaw_model::pattern::Pattern` gains
  `editor_grid_resolution: Option<GridResolution>` field with serde
  `#[serde(default, skip_serializing_if = "Option::is_none")]`.
  Migration: `Project::migrate_to_current` no-op (additive optional
  field).
- `regions/pattern_editor/{pitched,drum}/mod.rs` host a new
  `GridResolutionToolbar` component.
- Existing snap-to-grid maths in both editors reads the signal
  rather than the hardcoded constant.

**Surface.** A `Select` dropdown above the editor surface listing 6
resolutions (subset of `GridResolution` enum; e.g.,
StraightQuarter / StraightEighth / StraightSixteenth /
TripletEighth / TripletSixteenth / Sixtyfourth). Choosing one
writes through `set_pattern_grid_resolution(pattern_id, res)`.

**Action wrapper.** `pattern_actions/grid.rs::set_pattern_grid_
resolution(pattern_id, resolution)` mutates the pattern field. Pure;
tested.

**Done when.** Dropdown shows 6 options; selection re-renders the
grid SVG; new notes snap to the new resolution; project save/load
preserves the choice; both editors honor the field. MCP confirms a
switch from 16ths to triplet-8ths visibly redraws the grid lines.

---

## Phase G2 — Drag-to-set velocity in drum step grid

**Scope.** Vertical drag on a step cell sets velocity continuously,
with live visual feedback.

**Files.**
- `regions/pattern_editor/drum/step_grid.rs` (or wherever the step
  cell currently lives — adjust on read).
- `pattern_actions/drum_pattern/velocity.rs` (NEW; pure helpers).

**Surface.** `StepCell` onclick registers a
`Drag::absolute().on_move(...).on_end(...).start()`. `on_move`
computes a velocity from pointer dy (clamped to 1..=127), updates a
`StepDragPreview` signal. `on_end` commits via `set_step_velocity`.
Click without sustained motion (drag distance < 4 px) falls back to
the existing click-to-toggle behavior.

**Visual.** Step cell renders a `linear-gradient` fill whose stop
position reflects the current velocity. Active steps with velocity
100 look as today; steps at velocity 30 render as a short bar.

**Action wrapper.** `set_step_velocity(pattern_id, voice, step_idx,
velocity)`. Pure; clamps to [1, 127]; no-ops when the step doesn't
exist (i.e., the cell is off). Setting velocity on an off step
turns it on at the chosen velocity.

**Tests.** Clamp at 0 and 128. No-op on missing step. Round-trip
through project save/load.

**Done when.** Vertical drag on a cell adjusts velocity continuously
with live visual feedback; click-to-toggle unchanged when no drag.
MCP confirms a drag from one cell raises its velocity visibly.

---

## Phase G3 — Extra-voice creation UI for drum patterns

**Scope.** Add / rename / delete Extra voices in the drum editor.

**Files.**
- `regions/pattern_editor/drum/voice_strip.rs` (or wherever the
  per-row strip currently renders — adjust on read).
- `pattern_actions/drum_pattern/voices.rs` (NEW; pure helpers).

**Surface.** Below the four GM voice rows, list Extra voices in
insertion order. Below those, a `+ add voice` row. Click it →
inline rename → Enter commits via `add_extra_voice`. Each Extra row
has a `⋯` menu with Rename + Delete. GM rows have no menu.

**Action wrappers.**

```rust
pub fn add_extra_voice(project: &mut Project, pattern_id: PatternId, name: &str) -> Result<(), Conflict>;
pub fn rename_extra_voice(project: &mut Project, pattern_id: PatternId, old: &str, new: &str) -> Result<(), Conflict>;
pub fn remove_extra_voice(project: &mut Project, pattern_id: PatternId, name: &str) -> Result<(), Vec<StepRef>>;
```

Rename rewrites all `DrumStep` entries referencing the old name to
the new name. Remove refuses with a "referenced by: 5 steps" list
when any step references the voice; on refusal, no edit lands.

**Tests.** Conflict on collision with another Extra (not GM); rename
rewrites step references; remove refuses with reference list; on
empty (no steps), remove succeeds.

**Done when.** User can add / rename / delete Extra voices from the
drum editor with delete refusal; project save/load round-trips.
MCP confirms add + place + rename + delete flow.

---

## Phase G4 — Close-out

**Scope.** Doc + memory updates, same cadence as P5 / S6.

**Actions.**
- Update this plan doc's Status section with commit hashes,
  per-phase deviations, and a "Milestone closed" paragraph at top.
- Update `docs/design/drum-patterns.md`'s "Editor scope shipped in
  P3" section with the G2 + G3 additions.
- Update [[project-status]] + [[project-next-session-pickup]].

**Done when.** Plan doc + design doc reflect what shipped; memory
pointers updated.
