# Chord-loop editing plan (v1)

The first Tier-1 plan after composition-writability Tier 0 closed.
Builds the chord-loop **editor UI** on top of the Tier-0 edit pump.

Chord loops are the load-bearing surface in Joe's top-down
composition workflow ([[user-composition-workflow]]): once the
project key and form are set, chord-loop authoring is the next
layer down before any pattern / melody work. The model side
(`rawdaw_model::chord::{ChordLoop, ChordEvent, ChordSpec,
ChordSuffix, RomanDegree, BassSpec, Annotation, …}`) is already
shipped and well-typed; the realization side
(`rawdaw_model::realize::resolve::{resolve_chord_spec_root,
resolve_chord_degree, …}`) already turns these into concrete
pitches. The work here is **purely UI**: editor surfaces that
mutate `Project.chord_loops` + `Project.sections[*].chord_loops`
through `AppState::apply_project_edit` so the engine drains and
re-arms in lockstep.

The mockup-side reference is `docs/design/chord-loops.md`, which
already covers the chord vocabulary, the functional-vs-absolute
default, the `in_key` story for secondary dominants / modal
interchange / modulation, the slash-chord story, and the editor
UX (timeline, quick-entry shorthand, realized strip, inspector).
That doc is the spec; this plan is the **build schedule** that
gets us there in phases, each ending with a green workspace.

**Engineering constraints** (from `CLAUDE.md`): architectural
correctness over shortcuts; unlimited time and budget; ~700-line
cap per source file; no `unwrap()` outside tests;
`forbid(unsafe_code)` in every crate.

**Cadence.** Each phase ends with `cargo test --workspace` green,
clippy clean across all three feature builds (default /
`--no-default-features` / `--features cpal-driver`), and a
one-line "done when" criterion observably met.

**Multi-session scope.** Comparable to the C milestone in shape:
one design / plan phase, several build phases, one close-out.
Realistically larger in absolute work because chord editing has
more surface area than tempo / name / key controls. **CL0** is
this doc. **CL1** is library CRUD (least risky; pure structural
edit pump consumer). **CL2** is the single-loop timeline editor
(the meaty piece). **CL3** is the quick-entry shorthand parser +
absolute/functional mode toggle. **CL4** is section binding
editing + the realized strip + annotation UI. **CL5** closes out
and points at the next Tier-1 bite.

---

## Status

- CL0 ✅ — this document (`7cbee35`).
- CL1 ✅ — chord-loop library CRUD.
  - **New module `crate::chord_loop_actions`** (353 lines + tests):
    `create_chord_loop`, `rename_chord_loop`, `duplicate_chord_loop`,
    `delete_chord_loop` (refuses-with-reference-list when sections
    reference the loop in base or any variant override),
    `set_chord_loop_color` (writes overlay; doesn't drive audio).
    `unique_loop_name` helper handles `"untitled" → "untitled-2"`
    suffix bumping. 11 unit tests pin the contracts.
  - **`AppState::select_chord_loop`** added as the third selection
    axis. The mutex extends: `set_selected_idx(Some)` /
    `select_track(Some)` clear chord-loop selection;
    `select_chord_loop(Some)` clears both. MIDI sticky-target is
    untouched (per [[K-series]]). 5 new selection-mutex tests.
  - **`regions/library.rs` split** into `regions/library/`:
    `mod.rs` (312 lines — Library shell + SearchBar + Patterns /
    Sections groups + shared chrome) and `chord_loops.rs`
    (455 lines — interactive Chord Loops group). Forced by the CL1
    additions pushing the original file over the 700 cap.
  - **`ChordLoopRow`** carries per-row state (editing toggle,
    name input buffer, menu open) via `#[component]` so each row's
    Signals survive reorder/refresh under `key: id.get()`
    (rinch Rule 9). Inline rename mirrors the C4 `NameControl`
    pattern (untracked-Effect peek). The `⋯` action menu surfaces
    Rename / Duplicate / Delete plus 10 palette colors + Default.
    Selection writes [`AppState::selected_chord_loop`]; the row's
    `background` + `border-left` reactively highlight when
    selected (CL2's editor will key off the same signal).
  - **Delete refusal** uses a model dry-run before the edit-pump
    commit so the user gets a useful "referenced by: verse, chorus"
    message; never silently corrupts the project (per CL1 contract).
    UI surface is `eprintln!` until the toast/alert primitive ships.
  - **`+ new chord loop`** click creates an empty loop via the
    edit pump, then selects the new id immediately.
  - 395 workspace tests (was 381; +16 net: +11 chord_loop_actions,
    +5 selection-mutex). Clippy clean across default /
    `--no-default-features` / `--features cpal-driver`; release
    build clean.
- CL2 — not started.
- CL3 — not started.
- CL4 — not started.
- CL5 — not started.

---

## Phase CL0 — Plan + design decisions ◀ this doc

Lock the architectural choices in "Design decisions" below. No code
changes. The phase boundary criteria are documented here so CL1 has
a concrete checklist.

**Done when:** This doc lands on `main`.

---

## Phase CL1 — Chord-loop library CRUD

The structural-surgery phase. After CL1, the user can add, rename,
delete, duplicate, and recolor chord loops in the project library
through the existing Library panel; the timeline / event editor
lands in CL2. CL1 doesn't touch chord *events* — only the
`ChordLoop` envelope (id, name, length, key override, color).

**App side:**

- Library panel currently lists chord loops (read-only) under its
  "Chord Loops" section. CL1 grows the section with:
  - A `+` button to create a new chord loop. The new loop carries
    a fresh `ChordLoopId` (via the existing
    `Project.id_allocators.alloc_chord_loop()` allocator), a
    default name like `"untitled-N"` (next available numeric
    suffix), and a sensible default length (4 bars). Events list
    starts empty.
  - Per-row context affordance (right-click menu or `⋯` button)
    for Rename / Duplicate / Delete / Pick color.
  - Inline rename uses the same `NameControl` pattern as the C4
    project name: local `Signal<String>` buffer, Effect with
    `untracked` peek, Enter commits via `apply_project_edit`,
    blank reverts.
- New selection state: `AppState.selected_chord_loop:
  Signal<Option<ChordLoopId>>` so other surfaces (the CL2 editor)
  can observe which loop is "open". Library selection writes it;
  CL2 reads it.
- Color picker writes to `ProjectOverlay.chord_loop_color`. Same
  pattern as section / pattern colors today.
- Delete confirms before commit if the loop is referenced by any
  section (`Project.sections[*].base.chord_loops` /
  `variants[*].…` — walk the project once). If unreferenced,
  delete unconditionally. The confirm dialog can be a stub
  `eprintln!("would delete in-use loop, confirm UI TBD")` for CL1
  if the toast / alert primitive hasn't landed yet — but
  **deletion that breaks references must not silently corrupt the
  project**. Either confirm-then-delete or refuse-with-message;
  no third option.
- Duplicate clones the chord loop with a fresh id + suffixed name
  (`"verse"` → `"verse copy"`).

**File-cap watch:**
- Library panel is in `regions/library.rs`. Current size: check
  at start of CL1; expect the chord-loop section to grow ~60-80
  lines. If `library.rs` crests ~600, split into
  `regions/library/{mod,patterns,chord_loops,sections}.rs`
  proactively.

**Done when:**
- Creating a fresh chord loop adds an entry to
  `Project.chord_loops` reachable from the library; arrangement +
  section views still render correctly (an empty loop renders an
  empty chord ribbon — fine).
- Rename / duplicate / delete round-trip through save → reopen.
- `cargo test --workspace` green; clippy clean.

---

## Phase CL2 — Single-loop timeline editor

The meaty phase. After CL2, clicking a chord loop in the library
opens an event-timeline editor where chord events can be added,
removed, moved, resized, and edited via a per-event inspector
pane. The shorthand parser arrives in CL3; CL2 is "click-driven"
editing with dropdowns + chips.

**App side:**

- New region or inspector tab: `regions/chord_loop_editor/`.
  Resolves where the editor lives — see design decision 1.
- Top of the editor: read-only summary (loop name from library
  selection, loop length editable as a numeric input, optional
  `key` override).
- Main surface: a timeline of bars scaled to the loop's `length`.
  Chord events render as labeled blocks (Roman + quality + a
  quality-coded small absolute label, mirroring round-2's
  `ChordLoopBar` cell). Each block is draggable along the
  timeline (changes `time`) and resize-able from the right edge
  (changes `duration`). Empty timeline ranges are click-targets
  to insert a new event at that beat.
- Per-event inspector pane (right side or below): dropdowns for
  `RomanDegree`, `ChordQuality`, chip lists for `Vec<Extension>`
  and `Vec<Alteration>`, dropdown for `BassSpec` kind +
  conditional value editor, optional `in_key` dropdown (reuses
  C4's `KeyDropdown` widget). Cadence tag dropdown (optional;
  `CadenceTag` enum). Comment text input (optional).
- Edits commit via `app.apply_project_edit(|p| { let cl =
  p.chord_loops.get_mut(&id)?; cl.events[i].chord = ...; })`
  patterns. Every chord-event mutation is a single commit through
  the C2 edit pump, so realization re-runs and audio re-arms in
  lockstep (the section editor's chord ribbon updates audibly +
  visibly on every edit).
- Drag interactions snap to a beat grid by default (configurable
  later; v1 uses 1-beat snap on the bar timeline scale + a held
  modifier for fine-grained ticks).
- CL2 ships **functional events only**. The `Absolute` variant of
  `ChordSpec` is constructible by the model but not exposed in
  the v1 inspector — CL3 adds the mode toggle.

**File-cap watch:**
- New `chord_loop_editor/` directory expected to fan out into 4-6
  files (~150 lines each): `mod.rs` (top-level), `timeline.rs`
  (event block layout + drag handlers), `event_block.rs` (one
  cell), `inspector.rs` (per-event field editors), `helpers.rs`
  (event ordering, beat snapping). Stay under the 700 cap per
  file from day one.
- Add at most one new chord-display helper to
  `crate::chord_display` (e.g. `format_extensions`,
  `format_alterations`). Resist growing that module — the
  read-only labels it ships today are the contract.

**Done when:**
- Creating + editing chord events in a chord loop visibly +
  audibly changes playback: the section editor's chord ribbon
  re-tiles with the new chords, and the realized engine output
  reflects the new chord progression on the next playback block.
- Editing a chord loop that's referenced from multiple sections
  updates every reference (single source of truth — the model
  already guarantees this; CL2 just exercises it).
- `cargo test --workspace` green; clippy clean.

---

## Phase CL3 — Quick-entry shorthand parser + `Absolute` mode toggle

The "1, 5/5, b6m7, /3" phase. Replaces or augments the dropdown
inspector with text-typed chord shorthand per
`docs/design/chord-loops.md` § "Chord-loop editor UX". The parser
is the load-bearing piece; lives in its own module + has its own
unit-test crate (or test module).

**App side (or new crate):**

- New `crates/rawdaw-app/src/chord_shorthand/` (or, if the parser
  is reusable enough, a sibling crate `rawdaw-chord-shorthand` —
  see design decision 7) with:
  - `parse.rs` — the grammar implementation. Returns
    `Result<ChordSpec, ParseError>` with span-precise errors so
    the UI can highlight the failing token.
  - `format.rs` — the inverse: serialize a `ChordSpec` back into
    its shorthand. Round-trip tested.
  - `tests/` — table-driven test suite covering every documented
    case from the design doc (`5/5`, `bVImaj7add9`, `Vsus4/3`,
    etc.) plus malformed input rejection cases.
- The CL2 per-event inspector grows a text input that bidirects
  with the dropdown set: typing parses into the dropdown values;
  changing a dropdown updates the text. Use the C4 `untracked`
  Effect pattern to break the feedback loop.
- `Absolute` mode toggle: a per-event chip switches the event
  between `ChordSpec::Functional` and `ChordSpec::Absolute`. The
  dropdown set and the text input swap to absolute-pitch
  representation (e.g. `Gmaj7` instead of `Vmaj7`).
- The grammar must be specified before code lands — see design
  decision 6. A short BNF + the grammar's normative test cases
  live alongside the parser in `chord_shorthand/grammar.md`.

**Done when:**
- Every documented shorthand example from
  `docs/design/chord-loops.md` parses + formats round-trip.
- Mode toggle round-trips `Functional` ↔ `Absolute` losslessly
  for chords that have absolute representations (most do; rare
  exotics may diverge — accept and document).
- `cargo test --workspace` green; clippy clean.

---

## Phase CL4 — Section binding + realized strip + annotations

After CL4, the user can attach a chord loop to a section across
specific bar ranges (multi-loop sections — bars 1–4 use one loop,
5–8 use another), see the concrete pitches each chord realizes to
under the current key, and tag chord events with cadence
annotations and comments.

**App side:**

- The section editor's existing `chord_loop_bar.rs` becomes
  editable. Each cell click opens a small affordance ("pick a
  loop for this range"). Splitting / merging ranges is a
  right-click affordance.
- The chord-loop editor (CL2) grows a "Realized" strip beneath
  the timeline showing concrete pitches per event under the
  effective key. Effective key resolution:
  1. Chord event's `in_key` if set.
  2. Chord loop's `key` if set.
  3. Section's scale override if the loop is being previewed in
     a section context.
  4. Project's `default_key` otherwise.
- Reuses `rawdaw_model::realize::resolve::{resolve_chord_spec_root,
  resolve_chord_degree}` to compute pitches; no new realization
  code.
- Inspector grows the annotation row: cadence-tag dropdown
  (`CadenceTag` enum), comment text input. Cadence tags are
  display-only in v1 — drum-fill placement hints + variant
  suggestion uses are tracked as future work.

**Done when:**
- Multi-loop sections render correctly in the section editor's
  chord ribbon (CL2's edits to one loop don't bleed into the
  other's bar range).
- The realized strip updates audibly + visibly when the user
  changes the project key (C4's `KeyDropdown` already commits
  through the same pump — this is a downstream consumer).
- Cadence annotations + comments round-trip through save / load
  via `Project::save` / `Project::load`.
- `cargo test --workspace` green; clippy clean.

---

## Phase CL5 — Close-out + next Tier-1 bite

- Update `project_status` memory: chord-loop editing milestone
  ✅; point at the recommended next Tier-1 bite (likely
  **pattern editor** per the realize-the-melody story below).
- Update `project_next_session_pickup` with the chosen next
  bite's plan-doc path.
- Workspace test count + clippy / release clean state recorded
  in the Status section above with commit hashes per phase.
- Re-check `docs/design/chord-loops.md` against what shipped;
  capture any deviations in the design doc itself (not in this
  plan).
- One paragraph at the top of this plan summarizing what landed
  and the open items (the Out of Scope list below).

**Likely next Tier-1 bite:** **pattern editor** — chord loops
give you progressions; patterns give you parts. Without a
pattern editor, the chord-loop work tops out at "the realized
strip shows my changes." With patterns + activations, the user
can write a bassline that re-realizes against any chord
progression — the headline payoff for the rawdaw composition
model. Alternative: **section binding completeness** (drag
section blocks in the arrangement, rename sections,
delete/duplicate), which is smaller but less foundational.

**Done when:**
- Memory updated.
- All CL phases ✅ in the Status section above with commit
  hashes.
- The next Tier-1 plan doc exists (`docs/<next>.md`) with at
  least a CL0-equivalent design phase written.

---

## Design decisions locked in CL0

### 1. Where the chord-loop editor lives in the round-1 UI

**Decision:** new full-width region that replaces the
`Inspector` / `SectionEditor` panel when a chord loop is
selected from the Library. The Library remains visible on the
side; the editor takes the center stage. Closing the editor (×
button or selecting a section / track in the library) reverts
to the previous inspector content.

Rationale: the chord-loop editor needs horizontal real estate
for the timeline + per-event inspector. Cramming it into the
existing 600px-wide inspector pane would feel cramped, and the
section editor's full-region pattern is already the precedent
for "structural editor takes center stage."

Defer the visual chrome to the design pass
([[project-ui-redesign-pending]]). CL2 ships a placeholder
layout; the pass tightens it.

### 2. Functional as the default mode

Per `docs/design/chord-loops.md` § "Functional, not absolute (by
default)". The CL2 inspector ships functional-only; CL3 adds the
opt-in `Absolute` toggle. New chord events default to
`Functional { roman: RomanDegree::I, suffix:
ChordSuffix::new(ChordQuality::Major), in_key: None }`.

### 3. Quick-entry text vs. dropdown UI: ship both, link them bidirectionally

The dropdowns are reach-for-mouse-input affordances; the
shorthand text input is the keyboard-driven flow. CL2 ships
dropdowns first (lower risk, no parser); CL3 layers the text
input on top with bidirectional sync via the same `untracked`
Effect pattern C4 used for BPM / Name.

### 4. Chord-loop variants — deferred from v1

`docs/design/chord-loops.md` § "Open questions" flags chord-loop
variants as "skipped in v1; revisit if cloning chord loops
becomes painful." This plan inherits that decision. Duplicate
+ rename is the v1 path; if a verse needs a variant, duplicate
the loop and edit the copy.

### 5. Realized strip computation: reuse model `realize::resolve`

The realized strip in CL4 doesn't introduce a new realization
pass. It calls the same helpers
(`resolve_chord_spec_root` + `resolve_chord_degree`) that the
main realization pipeline uses, against the effective key
computed via the precedence rule in CL4. This keeps the
realized strip + actual playback in lockstep — they cannot
drift because they share the resolver.

### 6. Shorthand grammar specified before parser code

The grammar is not invented during CL3; it lands as
`chord_shorthand/grammar.md` (or section in
`docs/design/chord-loops.md`) at the start of CL3, reviewed
before the parser lands. Open questions to resolve before
locking the grammar:

- Should `5` mean "V" or "Cmaj/G" (5th in bass)? The design
  doc's examples use `5` for V; lock that.
- Does `m` always mean minor, or is it absorbed by case-coded
  roman (`v` = V minor)? Lock: case-coded romans are the
  primary; `m` is allowed redundantly (`Vm` == `v`).
- Inversion: `/3` (third in bass) vs. `/E` (E in bass) — both
  supported; `/N` where N is a digit is chord-tone, where N is
  a pitch class is absolute.
- `in_key` shorthand: `5/5` is V/V. Is `5/[V]` allowed as
  long-form? Defer to grammar doc.

### 7. Shorthand parser: stay in `rawdaw-app` for v1

The parser produces `ChordSpec` (a model type), so it lives in a
crate that depends on `rawdaw-model`. Options:

- Sibling crate `rawdaw-chord-shorthand`: cleaner separation,
  reusable.
- Module `crates/rawdaw-app/src/chord_shorthand/`: simpler, no
  inter-crate plumbing.

**Decision: module in `rawdaw-app` for v1.** Promote to a
sibling crate if a second consumer materializes (e.g., a
command-palette macro, a CLI chord-loop converter). The
parser's tests can live in `tests/` of the app crate.

### 8. Voicing UI stays out of the chord-loop editor

Per `docs/design/chord-loops.md` § "No voicing hints on chord
events". Voicing belongs to realization, exposed through the
activation editor (future Tier-1 milestone). The chord-loop
editor never shows or edits voicing fields.

### 9. CRUD goes through the C2 edit pump, no shortcuts

Every chord-loop mutation — add, rename, delete, duplicate,
event add/move/edit, section binding change — goes through
`AppState::apply_project_edit(|p| ...)` so the engine drains +
re-arms in lockstep with the host signal swap. No direct
mutation of `app.project` via `Signal::set` from any chord-loop
editor handler. The C2 contract is the law.

### 10. Selection model: extend `AppState`, not invent a new pattern

The selected chord-loop id becomes a new field on `AppState`:

```rust
pub selected_chord_loop: Signal<Option<ChordLoopId>>,
```

Mirrors the existing `selected_track` / `selected_idx` pattern.
Library selection writes it; the chord-loop editor reads it.
Selecting a different surface (section, track) clears it.
`AppState::new` defaults it to `None`.

### 11. File-cap discipline: split eagerly

The chord-loop editor will fan into several files
(`chord_loop_editor/{mod,timeline,event_block,inspector,helpers}.rs`)
from day one of CL2 rather than starting in a single file and
splitting at 600+ lines. Same lesson as `topbar/` post-C4.

### 12. Length editing: keep `Duration` as the model type

`ChordLoop.length: Duration` already exists. The library-row
length editor (CL1) edits this as bars + beats; the chord-loop
editor (CL2) re-renders the timeline against it. No new model
type; CL1 / CL2 use the existing `Duration` constructors
(`Duration::bars`, `Duration::beats`).

---

## Out of scope (Tier-1 follow-ons and beyond)

These belong to follow-on plans, not CL1–CL5:

- **Pattern editing.** Pattern model + body editor (piano roll,
  drum step grid). Likely the next Tier-1 plan after CL5.
- **Activation editing.** Drop patterns onto section × track
  cells; variant scheduling; realization params. Depends on
  pattern editing.
- **Section editing.** Add / rename / duplicate / delete
  sections; drag arrangement blocks; resize. Some of this can
  ride along with CL4's section-binding work; the full surface
  is its own milestone.
- **Track management.** Add / rename / delete tracks; pick synth
  assignment; mute / solo.
- **Per-note overrides + clip fork.** Round-3 territory.
- **Chord-loop variants.** Symmetric with pattern + section
  variants; design-decision 4 above defers v1.
- **Section-internal scale ranges.** `docs/design/chord-loops.md`
  § "Open questions" defers — split the section for v1.
- **Display preference: Nashville Number System.** Render-only
  toggle, no data-model change. Defer to UI redesign pass.
- **Drum-fill placement hints from cadence tags.** v1 ships
  cadence as display-only.
- **MIDI input → chord-loop event capture.** Future tooling
  (the K-series live-MIDI groundwork is in place; capture-to-
  loop is its own design).

[[user-composition-workflow]]
[[project-status]]
[[feedback-commit-cost-calibration]]
[[project-ui-redesign-pending]]
