# Composition writability plan (v1)

Make the UI's view of the project **mutable**. Today every UI surface
(`Arrangement`, `Library`, `TracksPane`, `Inspector`, `SectionEditor`) reads
through `fixture::round1()` — a `OnceLock<Round1>` populated once at boot
from `rawdaw_model::fixtures::build_round1_project()` plus a presentation
overlay. The synth editor sliders (U-series) and the MIDI device picker
(K-series) mutate runtime state, but nothing about the *song* — sections,
chord loops, patterns, tracks, arrangement — can be edited from the UI.

This milestone replaces the static fixture with a `Signal<Rc<Project>>` in
`AppState`, builds the **edit → re-realize → audio re-arm** pump, lands
**project save / load** UI on top of the existing `Project::save` /
`Project::load` model surface, and exercises the whole pipeline with one
proof-of-life editable control set in the top bar (tempo / key / project
name). After this, every Tier-1 editing surface (section editor, chord
loop editor, pattern editor, track manager, activation editor) plugs into
the same plumbing.

This is **Tier 0** of the composition-UI program. Tier 1 (the actual
editing surfaces) is gated on Tier 0 and will get its own plan(s) per
major surface — recommended first bite is chord-loop editing per Joe's
top-down workflow ([[user-composition-workflow]]).

**Engineering constraints** (from `CLAUDE.md`): architectural correctness
over shortcuts; unlimited time and budget; ~700-line cap per source file;
no `unwrap()` outside tests; `forbid(unsafe_code)` in every crate.

**Cadence.** Each phase ends with `cargo test --workspace` green, clippy
clean across all three feature builds (default / `--no-default-features` /
`--features cpal-driver`), and a one-line "done when" criterion observably
met.

**Multi-session scope.** Comparable to the K milestone (5 phases plus
this plan). C1 is the load-bearing one — touches every UI call site that
reads `fixture::round1()` (40+ hits across 14 files). C2 / C3 / C4 are
smaller and build on C1. C5 closes out.

---

## Status

- C0 — this document (when landed).
- C1 — not started.
- C2 — not started.
- C3 — not started.
- C4 — not started.
- C5 — not started.

---

## Phase C0 — Plan + design decisions ◀ this doc

Lock the architectural choices in "Design decisions" below. No code
changes. The fixture's current role and the read-site inventory are
documented here so C1 has a concrete checklist.

**Done when:** This doc lands on `main`.

---

## Phase C1 — `AppState.project: Signal<Rc<Project>>`; migrate every read site

The foundational refactor. After C1 the round-1 project lives in a
`Signal<Rc<Project>>` on `AppState`, every UI call site reads through it
instead of `fixture::round1()`, and `fixture/` is demoted to a one-shot
initial-state factory invoked once from `AppState::new()`.

**App side** (`rawdaw-app`):

- `AppState` gains:
  - `pub project: Signal<Rc<Project>>` — the live mutable project. Reads
    are O(refcount bump); mutations replace the `Rc` wholesale (see
    design decision 1 for why `Rc<Project>` over `Project` in the
    `Signal`). Initial value comes from
    `initial_project::build_initial()`.
  - `pub overlay: Signal<Rc<ProjectOverlay>>` — parallel store for
    UI-only decorations (colors, meta strings, per-cell realization
    annotations). Same `Rc`-wrap rationale.
- New module `crates/rawdaw-app/src/initial_project/` (rename of
  `fixture/`):
  - `mod.rs` — `pub fn build_initial() -> (Project, ProjectOverlay)`.
    Drives the round-1 demo project on app boot.
  - `overlay.rs` — `ProjectOverlay` struct with the UI-only fields the
    current `fixture::Overlay` carries (section colors, pattern colors,
    chord-loop colors, pattern meta strings, per-cell `Realization` +
    pinned-note count). Keyed by model IDs.
- Delete `crates/rawdaw-app/src/fixture/` (after migration). The
  `Round1` view type and the `section_by_key` / `pattern_by_name` /
  `chord_loop_by_name` / `track_by_id` / `instance_count` /
  `base_activation` / `variant_override` view helpers all go away. Call
  sites either:
  - Read the model directly: `project.sections.get(&sid)` — `BTreeMap`
    keys are already `SectionId` / `PatternId` / etc.
  - Use new thin extension methods on a `ProjectLookup` trait
    (`crates/rawdaw-app/src/project_lookup.rs`) for any lookup the
    callers still need that the model doesn't expose natively (e.g.
    `instance_count_of(&Project, SectionId) -> usize`).
- `AudioResources::build` currently calls `build_round1_project()`
  directly to seed `AudioResources.project: Rc<Project>`. Switch to
  reading the initial state from the same
  `initial_project::build_initial()` so the audio side and UI side
  share a single source of truth at boot. `AudioResources.project`
  stays an `Rc<Project>` snapshot for now; C2 introduces the
  project-update path that re-syncs both.
- `AppState` keeps deriving `Copy` (each `Signal` is `Copy`). Adding
  `Signal<Rc<Project>>` doesn't break that — `Signal<T>` is always
  `Copy` regardless of `T`.

**Read-site inventory (must be 0 after C1):**

```
$ rg 'fixture::(round1|section_by_key|pattern_by_name|chord_loop_by_name|track_by_id|instance_count|base_activation|variant_override)\b' crates/rawdaw-app/src/
```

Currently 40+ hits across `regions/{arrangement,library,topbar,tracks_pane}.rs`,
`regions/inspector/{mod,activation_table}.rs`,
`section_editor/{mod,meta_bar,chord_loop_bar,activations,variant_tabs,cell/mod}.rs`,
`state.rs`, `audio/{midi,graph}.rs`.

The `state.rs` docstring also references `fixture::section_by_key` — that
gets removed and the docstring updated when `section_key: String` migrates
to `section_id: SectionId` (see design decision 4).

**File-cap watch:**
- `fixture/data.rs` is ~530 lines today; the rename to
  `initial_project/data.rs` keeps it under 700, but if the C1 migration
  pushes it over, split by domain (`tracks.rs` / `patterns.rs` /
  `chord_loops.rs` / `sections.rs` / `arrangement.rs`).
- `state.rs` grows ~50 lines (the two new Signals + helper methods).
  Comfortable under the cap.

**Done when:**
- `rg 'fixture::' crates/rawdaw-app/src/` returns no hits.
- Round-1 demo project renders pixel-identically to pre-C1 (visual
  regression via `rinch` MCP screenshots saved at C0 baseline + diff'd
  at C1 close).
- `cargo test --workspace` green; clippy clean across all three feature
  builds.

---

## Phase C2 — Edit → re-realize → audio re-arm pump

C1 makes the project mutable but no edits flow to audio. C2 closes the
loop. After C2, calling `audio.apply_project_edit(|p| ...)` will:

1. Run the closure against a clone of the current project.
2. Replace `AppState.project` with the new `Rc<Project>` (UI re-renders
   observing components).
3. Re-realize the project's event list via `rawdaw_model::realize::*`.
4. Translate to MIDI `BlockEvent`s (existing path).
5. Drain the engine's song queue (existing K1.fix Stop→Play path) and
   push the new event list.
6. Restore transport state (the user shouldn't notice a Stop→Play cycle).

**App side:**

- New `audio/edit_pump.rs` module:
  - `pub fn apply_project_edit<F>(&mut self, f: F) where F: FnOnce(&mut Project)`
    on `AudioResources`. Internally: clones the current project, runs
    `f`, atomically swaps `AudioResources.project` and the AppState
    `project` signal (via a shared mutation entry point), then re-arms.
  - `re_arm(&mut self)` is the lower-level helper: re-realize, drain
    queue, push events. Used by C3's load path too.
- AppState side: a single mutation method
  `app.apply_project_edit<F>(f)` that delegates to the audio side, so
  call sites don't have to plumb `AudioResources` themselves.
- Granularity: whole-project re-realize for v1 (round-1's event list is
  ~hundreds of events; release-build re-realize is microseconds).
  Incremental re-realization is a v2+ optimization (see design decision
  5).

**Audio thread coordination:**
- The audio thread never touches `Project` directly — it only ever sees
  the realized `BlockEvent` stream. So re-arm is purely a host-side
  data swap + a song-queue rewrite. No new SPSC queue is needed.
- If transport is `Playing` during an edit, the user hears a single-
  block gap (~6 ms at 44.1 kHz / 256 frames) while the queue drains
  and refills. Acceptable for v1; the visual playhead does *not* jump.

**Proof of life:** a `#[cfg(debug_assertions)]` "+1 BPM" button in the
TopBar (gated so it doesn't ship in release) bumps
`Project.tempo_map`, calls `apply_project_edit`, and the playhead
audibly speeds up while playing. Removed in C4 once the real tempo
control replaces it.

**File-cap watch:** `audio/mod.rs` is ~580 lines (post K-split). The
pump adds 50–100; split `audio/edit_pump.rs` out from the start rather
than growing `mod.rs`.

**Done when:**
- Debug "+1 BPM" button audibly changes playback tempo end to end.
- A `RecorderNode`-based integration test pins the re-arm path: apply
  an edit that changes a chord-loop event's quality, verify the next
  rendered block carries the new note pitches.
- `cargo test --workspace` green; clippy clean.

---

## Phase C3 — Project save / load (UI wiring)

The model already ships `Project::save()` → RON + version header and
`Project::load()` → version-checked deserialize with a typed
`LoadError`. C3 is **app-side UI wiring**: file dialog, overlay
bundling, error surfacing, file-menu plumbing. No model changes
required.

**New dep:** `rfd = "0.14"` (or latest at write time). Cross-platform
file dialog; on Linux uses zenity / kdialog / xdg-desktop-portal — no
GTK runtime dep.

**App side:**

- New module `crates/rawdaw-app/src/project_io/`:
  - `bundle.rs` — `SavedBundle { bundle_version: u32, project: Project, overlay: ProjectOverlay }`.
    `bundle_version` is independent of `Project.schema_version` (the
    overlay format can change without touching the model). Initial
    value: `1`.
  - `save.rs` — `save_to_path(bundle: &SavedBundle, path: &Path) -> io::Result<()>`.
    Uses `Project::save()` internally for the model half; serializes
    overlay alongside via RON.
  - `load.rs` — `load_from_path(path: &Path) -> Result<SavedBundle, LoadError>`.
    Wraps `Project::load`'s typed errors plus an overlay-bundle-
    version check.
  - `dialog.rs` — `pick_save_path() -> Option<PathBuf>` and
    `pick_open_path() -> Option<PathBuf>`. Each spawns a `std::thread`
    for the rfd call (rfd blocks); result lands via a one-shot
    `mpsc::sync_channel` polled by a small `Signal<Option<PathBuf>>`
    on `AppState` (PlayheadPoller pattern from E5).
- File-menu UI: TopBar grows a `Project ▾` button that opens a popover
  with `New / Open… / Save / Save As…`. Built on the same button +
  popover primitive as K2's MidiPicker — no new Rinch component
  primitive needed.
- File extension: `.rawd` (rawdaw project). `rfd` file-type filter
  uses this.
- Load goes through C2's pump (`audio.apply_project_load(bundle)`) so
  the audio thread re-arms against the loaded project's realized event
  list.

**Errors:**
- `LoadError::UnsupportedSchemaVersion` (already in model) — surface as
  a Rinch dialog with the version mismatch + a "this build understands
  vN" line.
- `LoadError::Parse` — surface the spanned error to the user;
  RON's parse errors carry line/column info that's actually useful.
- IO errors (file not found, permission denied) — surface verbatim.
- All paths: on error, the existing project is **untouched**. Atomic
  swap only after a successful parse + overlay-version check.

**Smoke test:**
- Boot the app, edit tempo via C2's debug button, `File → Save As →
  my.rawd`, quit, relaunch, `File → Open → my.rawd`, verify tempo
  restored and playback matches.
- `Save` with no open path falls through to `Save As`.

**File-cap watch:** `project_io/*.rs` should each stay tiny (<150
lines). Topbar grows ~80 lines for the menu; well under cap.

**Done when:**
- Save / load round-trips a project to byte-identity (RON
  re-serialize after load equals the saved string).
- Version-mismatch surfaces a readable error and leaves the live
  project untouched.
- `cargo test --workspace` covers the bundle's serde round-trip and
  the version-mismatch rejection.

---

## Phase C4 — Proof-of-life editable controls (tempo / key / project name)

Replace the C2 debug button with three real edits exposed through the
TopBar — three controls that exercise C1 + C2 + C3 end to end from
user clicks, no debug gates.

- **Project name** — TopBar currently has a project name readout sourced
  from the UI-side `fixture::Project` mirror; that mirror disappears in
  C1. C4 needs a real `name: String` field on `rawdaw_model::Project`
  (add in C4 as a one-line model change + serde + schema_version bump
  to `2`, plus a migration entry that defaults missing `name` to
  `"Untitled"`). Click → inline editable text → blur commits via
  `apply_project_edit(|p| p.name = new)`. No audio impact.
- **Tempo** — click the BPM readout → number-input + ±1 buttons →
  commits via `apply_project_edit(|p| p.tempo_map = TempoMap::constant(new_bpm))`.
  Replaces the C2 debug button.
- **Key** — click the key readout → dropdown of pitch classes × {major,
  minor} → commits via `apply_project_edit(|p| p.default_key = new)`.
  Re-realization re-derives every Roman-numeral chord under the new
  key; round-1's chord ribbon visibly + audibly updates.

**Schema-version bump in C4:**
- `SCHEMA_VERSION: u32` goes from `1` to `2`.
- Migration: a v1 project loaded into v2 build defaults `name` to
  `"Untitled"`. Add a `migrate_v1_to_v2` function in `rawdaw-model`
  with a test pinning the migration.
- The model already documents the change procedure in
  `project.rs`'s `SCHEMA_VERSION` docstring — follow it.

**File-cap watch:** TopBar is ~150 lines; +100–150 for inline-edit
controls + dropdown. If it crests 600, split into
`regions/topbar/{mod,project_meta,transport,midi_picker}.rs`.

**Done when:**
- All three controls round-trip a user edit through the store →
  re-realize → audio → save → reopen.
- Changing project key during playback audibly re-pitches the next
  block's chords.
- v1 project files load into the v2 build via the migration with a
  test pinning the migration path.
- `cargo test --workspace` green; clippy clean.

---

## Phase C5 — Close-out + handoff

- Update `project_status` memory: Tier 0 ✅; Tier 1 not started; point
  at the recommended first Tier-1 bite (chord-loop editing — see
  design decision 10).
- Update `project_next_session_pickup`: point at the first Tier-1 plan.
- Workspace test count + clippy clean status recorded in the Status
  section above with commit hashes per phase.
- Update `docs/design/composition-model.md` if any of the C1–C4 work
  forced a small clarification on the activation-vs-ownership story
  (likely not — Tier 0 doesn't add editing surfaces — but check).
- One paragraph at the top of this plan summarizing what landed and
  the open items required for an actual end-to-end "compose a song"
  workflow (the Tier-1 list in "Out of scope" below).

**Done when:**
- Memory updated.
- All Tier-0 phases ✅ in the Status section above with commit hashes.
- This plan and `project_status` agree on what's done vs pending for
  the composition-UI program.

---

## Design decisions locked in C0

### 1. Project state shape: `Signal<Rc<Project>>`, not `Signal<Project>` or per-field signals

`Project` is a deep struct (BTreeMaps of sections / patterns / chord
loops, vector of tracks, IdAllocators). Cloning it on every component
read is wasteful. Wrapping in `Rc` makes reads O(refcount bump);
mutations clone-on-write the inner `Project` and `Signal::set` a
fresh `Rc`.

Per-field signals (e.g. `Signal<Vec<SectionRef>>` for the arrangement,
`Signal<BTreeMap<SectionId, Section>>` for the section library) would
give finer reactive granularity but at the cost of coordinating cross-
field invariants (a single user edit often touches multiple fields —
new section + arrangement insert). Start with the whole-project
signal; add per-field signals later when render-cost profiling shows
it's needed. Matches the U-milestone's "start simple, optimize when
metrics demand" pattern.

### 2. Overlay separation: `Signal<Rc<ProjectOverlay>>` parallel to `Signal<Rc<Project>>`

Per CLAUDE.md rule 1 (modeling correctness), per-pattern color, per-
section color, the `Pitched · N variants` meta strings, and the round-
2 cell realization decorations do not belong in `rawdaw-model`. They
stay in `rawdaw-app` as a parallel signal keyed by model IDs. Same
lifecycle as the project (saved/loaded together in C3's `SavedBundle`).

Tier 1 may eventually push some overlay fields into the model if they
gain user-facing semantics (e.g. section color is arguably project
data the user picks). Tier 0 does not do that migration.

### 3. Re-realize granularity: whole-project per edit

Round-1's project produces a few hundred realized events. Re-realizing
on every edit is fast (microseconds, release build). SectionRef-
scoped incremental re-realization is a v2+ optimization that adds
substantial complexity (dirty-tracking, merge logic).

Failure mode to watch: when Tier 1 patterns produce 10× more events,
re-realize-on-every-edit may become perceptible. Profile when patterns
ship; optimize then.

### 4. `EditorMode::SectionEditor.section_key: String` → `SectionId`

The current `state.rs` carries `section_key: String` because the
fixture indexes sections by name. Once the live project is the source
of truth, the identifier should be the typed model `SectionId`. Same
for any other `*_key: String` that's really an ID. This is a small
clean-up that lands inside C1.

The renamed `selected_idx` (arrangement-block index) stays a `usize`
for now — it indexes into `Project.arrangement.sections: Vec<SectionRef>`,
which is order-dependent (the arrangement IS the song timeline). Tier
1's drag-to-reorder work may switch it to `SectionRefId`.

### 5. Engine re-arm strategy: drain song queue + replay (Stop→Play pattern)

Already the proven path. K1.fix established that the engine's song
queue (`event_tx`) can be drained-and-rearmed from the host. Re-arm
on every edit pushes the freshly realized event list at
`sample_clock + 1`. If transport is Playing, the user hears a brief
single-block gap (~6 ms at 44.1 kHz / 256 frames); the playhead does
*not* jump.

Delta replay (don't drain — surgically insert/remove individual
events) is a substantial new piece of audio-thread coordination not
justified at v1 edit rates (human-click cadence).

K5's lesson stands: param events stay on the separate host-event
queue; structural re-arm goes through the song queue. Do not
conflate them.

### 6. Save format: RON (already in place)

`Project::save()` / `Project::load()` already use
`ron::ser::to_string_pretty`. RON's tagged-enum representation is
friendlier than serde_json's for the sum-typed `PatternKind` /
`PitchedEvent` / etc. that Tier 1 will exercise. File extension
`.rawd`.

### 7. File dialog: `rfd` crate, spawned thread

Standard Rust cross-platform choice. No GTK runtime dependency on
Linux (uses zenity / kdialog / xdg-desktop-portal). rfd blocks the
calling thread, so it runs in a spawned `std::thread`; result lands
via an `mpsc::sync_channel` polled by a `Signal<Option<PathBuf>>` on
AppState (PlayheadPoller pattern from E5).

### 8. Default project on app boot: round-1 demo (no behavior change)

Boot continues to load the round-1 demo project so the app starts
visually identical to today. "Last opened project" comes when a
user-prefs file lands (post-C5).

`File → New` (creates an empty project) is a C4 menu entry but the
empty-project default has to render — an empty project with no
tracks would leave the inspector + tracks pane with nothing to render.
Define a "minimum sensible empty project" in C4: one Pitched track
(wavetable, default patch), one empty section "Untitled Section",
arrangement with one block pointing at it.

### 9. `fixture::round1()` → `initial_project::build_initial()` (flag-day rename)

The fixture's runtime accessor disappears in C1. The build function is
repurposed as the one-shot factory called once from
`AppState::new()`. The `Round1` view type and all `section_by_key` /
`pattern_by_name` view helpers are deleted; call sites read
`&Project` + `&ProjectOverlay` directly.

This is a flag-day refactor — no "both paths coexist" intermediate.
The C1 commit (or short series) is one bundled landing per
[[feedback-commit-cost-calibration]] unless the diff crests ~1000
lines changed.

### 10. Pattern decoration types stay in app crate

`Realization`, `OctaveSpec`, `Voicing`, `Humanization`,
`ScheduleEntry`, and the UI-side `Activation` mirror — these exist
in `fixture/data.rs` because the model doesn't yet implement patterns
beyond the activation placeholder (`ActivationEntry`). They migrate
into `ProjectOverlay` in C1; they move into the model proper when
patterns ship in Tier 1's pattern-editing milestone. Resist the urge
to grow the model with pattern types in Tier 0 — keep the surgery
contained.

### 11. AppState file cap

`state.rs` is currently ~250 lines (including tests). C1 grows it by
~50 (two Signals + a couple helper methods); C2 grows it by ~30 (the
`apply_project_edit` delegate). Comfortable under the 700 cap. If
Tier 1 pushes it over, split into
`state/{mod,selection,project_handle,editor_mode}.rs` proactively.

### 12. Recommended Tier-1 first bite: chord-loop editing

After C5, the next plan to write is `docs/chord-loop-editing-plan.md`.
Rationale: chord loops are the load-bearing surface for Joe's top-down
workflow ([[user-composition-workflow]]); the chord-loop model
(`rawdaw_model::chord::{ChordLoop, ChordEvent, ChordSpec, BassSpec}`)
already exists and is well-typed; the work is "build the editor UI"
plus one new edit-pump consumer, contained scope. Section editing /
track management can follow.

---

## Out of scope (Tier 1 and beyond)

These belong to follow-on plans, not C1–C5:

- **Section editing UI.** Add / rename / duplicate / delete sections;
  drag arrangement blocks; resize.
- **Chord loop editing UI.** Library CRUD; per-chord roman / quality /
  extensions / bass / `in_key` editor; attach to sections. Recommended
  Tier-1 first bite per design decision 12.
- **Track management UI.** Add / rename / delete tracks; pick synth
  assignment; mute / solo.
- **Pattern model + editor UI.** The model `Pattern` type currently
  carries activation-placeholder shapes; the full
  `PitchedEvent` / `OctaveSpec` / `PatternKind` story per
  `composition-model.md` needs implementing. Then the piano-roll
  detail view, drum step grid, pattern library CRUD, pattern variants.
  Almost certainly the largest single Tier-1 milestone.
- **Activation editing UI.** Drop patterns onto section×track cells;
  variant scheduling sub-ranges; realization params.
- **Per-note overrides + clip fork.** Round-3 territory.
- **Mixer + master FX chain.** X1–X8 (`docs/master-fx-chain-plan.md`)
  covers master FX. A track-level mixer is its own plan.
- **Visual design pass.** Picklist item 1 in `project_status`.

After Tier 0 lands, the order of Tier-1 surfaces is Joe's call. See
design decision 12 for the recommendation.

[[user-composition-workflow]]
[[project-status]]
[[feedback-commit-cost-calibration]]
