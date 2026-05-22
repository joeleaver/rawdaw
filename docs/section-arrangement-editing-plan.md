## Section + Arrangement editing plan (v1)

The third Tier-1 plan after composition-writability Tier 0 closed and
the chord-loop editing (CL) + pattern editor (P) milestones landed.
Builds the **section editor + arrangement editor UI surfaces** on top
of the C2 edit pump so the user can finally create a song from a
blank slate.

This is the missing outer layer of Joe's top-down composition
workflow ([[user-composition-workflow]]): key → form → chord loops →
patterns. Today every inner layer is editable end-to-end, but
**sections themselves and the arrangement that places them are
read-only** — the only sections in a project are whatever the
`initial_project` seeds, and the only arrangement is whatever the
seed wires up. The Library's `+ new section` row is currently
`onclick: move || {}` (`regions/library/mod.rs:135`); the
`regions/arrangement.rs` `SectionLane` renders blocks but has no
insert/remove/drag/reorder handlers; the section editor's meta-bar
shows `name` and `duration_bars` as read-only text.

The model side is already shipped and well-typed — `Section`,
`SectionBody`, `SectionVariantOverride`, `Arrangement`, `SectionRef`,
all in `rawdaw_model::section`. The work here is **purely UI +
edit-pump action wrappers**: pure helpers that mutate the model in
place, plus `AppState::apply_project_edit` consumers that route
through the C2 drain-without-rewinding flow so the engine re-arms
mid-edit without snapping the playhead.

The mockup-side references are `docs/design/section-variants.md`
(variant semantics, override chain), `docs/design/composition-model.md`
(top-level structure), and `docs/design/ui-brief-r1.md` (intended
panel layout). Those docs are the spec; this plan is the **build
schedule** that gets us there in phases, each ending with a green
workspace.

**Engineering constraints** (from `CLAUDE.md`): architectural
correctness over shortcuts; unlimited time and budget; ~700-line
cap per source file; no `unwrap()` outside tests;
`forbid(unsafe_code)` in every crate.

**Cadence.** Each phase ends with `cargo test --workspace` green,
clippy clean across all three feature builds (default /
`--no-default-features` / `--features cpal-driver`), and a one-line
"done when" criterion observably met via the `rinch` MCP server.

**Multi-session scope.** Comparable to the CL milestone in shape:
one design / plan phase, several build phases, one close-out. **S0**
is this doc. **S1** is the `section_actions` module (least risky;
pure structural edit-pump consumer, mirrors `chord_loop_actions` and
`pattern_actions`). **S2** is library wiring — `+ new section` +
row context menu for rename / duplicate / delete. **S3** is the
section editor meta-bar — inline-editable name, duration nudges,
variant tab strip CRUD. **S4** is the `arrangement_actions` module
covering step CRUD + variant override. **S5** is the arrangement
view editor — drag-to-move blocks, right-click context menu, append-
step affordance. **S6** closes out and points at the next bite (X,
master-fx-chain — its plan already exists).

---

## Status

- **S0** — this document.
- **S1** — Section actions module (pure helpers + edit-pump wrappers).
  - New module `crate::section_actions` mirroring the
    `chord_loop_actions` / `pattern_actions` shape: pure helpers
    that mutate a `&mut Project`, plus `apply_project_edit`
    consumers that route through the C2 pump.
  - **Primitives:** `create_section(name)` returns new `SectionId`;
    `rename_section(id, new_name)` with collision-suffix bump;
    `duplicate_section(id)` clones name+body+variants+default with
    a fresh `SectionId`; `delete_section(id)` refuses with
    reference list when any `Arrangement.sections` step references
    the section (mirrors chord-loop / pattern delete-refusal);
    `set_section_color(id, color)` writes overlay (no audio
    impact); `set_section_duration_bars(id, variant, bars)` dispatches
    to base vs variant override (creates Replace-style override if
    missing on a non-default variant, exactly like the activation
    auto-promote pattern from P4.x); `set_section_scale_override(id,
    variant, scale)` same dispatch.
  - **Variant CRUD:** `add_section_variant(id, name)`;
    `remove_section_variant(id, variant_id)` (forbid removing
    `default_variant`; surface error like delete-refusal);
    `rename_section_variant(id, old, new)` with collision check;
    `set_default_variant(id, variant_id)` (validate target exists).
  - **Chord-loop schedule editing** stays in the existing
    `chord_loop_bar` flow — CL4 + CL4.x already cover that surface.
    This phase doesn't touch it.
  - **Done when:** unit tests pin every primitive (≥30 tests:
    create / rename / duplicate / delete-refusal / variant CRUD /
    default-variant-protection / auto-promote-on-variant-override);
    primitives compile in `rawdaw-app` without UI consumers.
- **S2** — Library wiring for section CRUD.
  - Carve a new `regions/library/sections.rs` out of
    `regions/library/mod.rs` matching the pattern set by
    `chord_loops.rs` (CL1) and `patterns.rs` (P1). The Sections
    group becomes interactive: `+ new section` button wires to
    `apply_project_edit(create_section(...))` + selects the new
    section. Each row gets the same `⋯` action menu with Rename
    (inline-edit) / Duplicate / Delete + a 10-color palette + Default
    swatch. Delete uses model dry-run to produce a useful "referenced
    by: verse, chorus" message.
  - `SectionRow` carries per-row state (editing toggle, name input
    buffer, menu open) via `#[component]` so each row's Signals
    survive reorder under `key: id.get()` (rinch Rule 9). Inline
    rename mirrors C4 `NameControl`'s untracked-Effect peek.
  - **Done when:** user can create / rename / duplicate / delete /
    recolor a section from the Library; delete refuses with a
    reference list when arrangement references exist;
    `regions/library/mod.rs` stays under the 700-line cap; rinch
    MCP confirms the new-section workflow visually.
- **S3** — Section-editor meta-bar editing + variant tab strip CRUD.
  - `section_editor/meta_bar.rs` gains inline-editable name (same
    `NameControl` shape) and a `duration_bars` numeric nudger
    (− / + buttons + click-to-type). Duration writes route through
    `set_section_duration_bars` and respect the active variant tab
    (base vs override).
  - **Variant tab strip** (already rendered for navigation) becomes
    interactive: each tab gains a hover `⋯` menu for Rename /
    Delete; a trailing `+` tab opens an inline-rename for a new
    variant. The default-variant tab is decorated and protected
    against delete. Right-click on any tab opens the same menu via
    rinch `ContextMenu`.
  - **Scale override** picker (per variant tab) lands here too —
    `Option<Option<Scale>>` semantics surface as a three-way picker:
    "inherit base" / "no scale override" / "use scale …".
  - **Done when:** all four meta-bar fields (name, duration, scale,
    default-variant) are editable; variant tab strip supports
    add / rename / delete with default-variant protection; the
    `chord_loop_bar` row stays untouched (CL4 + CL4.x covers it);
    `section_editor/meta_bar.rs` stays under the cap (split into
    `meta_bar/mod.rs` + `variants.rs` if needed).
- **S4** — Arrangement actions module (pure helpers + wrappers).
  - New module `crate::arrangement_actions` covering: `append_step
    (section_id, variant)`, `insert_step_after(index, section_id,
    variant)`, `remove_step(index)`, `move_step(from, to)`,
    `duplicate_step(index)`, `set_step_section(index, section_id,
    variant)`, `set_step_variant(index, variant)`.
  - **Start-time recomputation.** `SectionRef.start` is absolute
    `MusicalTime` (see `rawdaw_model::section::SectionRef`).
    Any insert / remove / move / duration-change requires
    recomputing later starts. The v1 model contract: **no gaps,
    no overlaps** — every step starts where the previous one
    ends. `arrangement_actions::recompute_starts(&mut Project)` is
    the canonical helper, called after every step mutation. Pure;
    fully testable. Reads each step's effective duration via the
    section + variant lookup (variant override's `duration_bars`
    if `Some`, else base `duration_bars`).
  - **Default fill on append.** `append_step` with no explicit
    section_id picks the first section in `project.sections` (or
    surfaces an error if none exist). Library S2 already covers the
    "no sections" case by gating the `+ append` button.
  - **Done when:** unit tests pin every primitive (≥25 tests
    covering ordering invariants, start recomputation, no-gap
    contract, move-to-same-index no-op, remove-last-step empties
    `Arrangement.sections`); primitives compile without UI consumers.
- **S5** — Arrangement view interactive editor.
  - `regions/arrangement.rs` gains: a `+ append` button anchored to
    the right of the section lane (opens a section picker + variant
    picker, commits via `append_step`); a right-click ContextMenu on
    every `SectionBlock` with Duplicate / Insert before / Insert
    after / Delete / Change variant…; drag-to-move on `SectionBlock`
    reusing the CL2.x `Drag::absolute()` pattern documented in
    [[project-next-session-pickup]] (register from inside onclick,
    `on_move(|x, y|)` updates a `DragPreview` signal,
    `on_end(|x, y|)` commits via `move_step`). Drag preview pattern
    matches CL2.x — block style reads the signal each render and
    applies `transform: translateX(...)`.
  - **No resize on arrangement blocks.** Step duration is the
    underlying section variant's `duration_bars`; resizing on the
    arrangement would entail per-step duration overrides which are
    not in the model (and intentionally so per the variant semantics
    in `docs/design/section-variants.md`). To change a step's
    length, the user edits the section's variant duration in the
    section editor (S3) — that propagates through `recompute_starts`
    on the next mutation.
  - **Variant picker chip.** The existing variant chip on each
    `SectionBlock` becomes clickable — opens a `Select` with the
    section's variants + "↳ default" semantics.
  - **Anticipate file-cap split.** `regions/arrangement.rs` was
    already over 600 lines pre-S5. Split into
    `regions/arrangement/mod.rs` (top-level surface + Ruler +
    ChordRibbon) + `regions/arrangement/section_lane.rs`
    (SectionLane + SectionBlock + drag wiring) + `regions/
    arrangement/append_action.rs`.
  - **Done when:** user can append / duplicate / delete / reorder /
    revariant steps from the arrangement view; drag preview shows
    the block moving live; commit lands on `on_end`; rinch MCP
    confirms each affordance visually; the playhead does not snap
    to bar 1 on any edit (C2 drain-without-rewind invariant).
- **S6** — Close-out.
  - Update this plan doc's Status section with commit hashes,
    record deviations from S0 decisions if any, finalize the
    next-Tier-1-bite recommendation (X1 master-fx-chain model
    surgery).
  - Update `docs/design/section-variants.md` and `docs/design/
    composition-model.md` with a "UI semantics shipped with the
    section + arrangement editor (S5)" section if any UI-level
    semantics deviate from the model spec.
  - Update [[project-status]] + [[project-next-session-pickup]]
    memories.
  - **Done when:** plan doc + design-docs reflect what shipped;
    memory pointers updated.

---

## Out of scope (for this milestone)

- **Per-step duration override.** The model does not represent it,
  and adding it would conflict with the variant-as-canonical-
  duration contract. Step length follows the section variant.
- **Drag-from-library to arrangement.** v1 uses an `+ append` button
  + section picker. Drag-from-library is later UX polish.
- **Visual / styling pass on the arrangement view.** Functional
  surface only. Joe's redesign pass ([[project-ui-redesign-pending]])
  will revisit visuals across the app.
- **Section reordering in the Library.** Section rows appear in
  insertion order (matching how Patterns and Chord Loops behave).
  Drag-to-reorder in the Library is a future paper-cut bite.
- **"New project" / File-menu UI.** Existing save/load works (C2);
  but there's no explicit "start from blank" UI control. Adjacent
  to this milestone but a separate small bite.
- **Section variant inheritance UI for chord_loops.** CL4 + CL4.x
  already cover this surface; this plan doesn't touch it.
- **EventStrip-layout-reflects-time** for the chord-loop editor.
  Tracked separately as a CL paper cut.

---

## Phase S0 — Design decisions (this document)

### 1. Mirror the established CRUD action-module pattern

`chord_loop_actions` (CL1) and `pattern_actions` (P1) defined the
canonical shape: pure helpers in a top-level module that take
`&mut Project` and return result-or-error, plus thin `AppState`
methods that route through `apply_project_edit`. The C2 edit pump
takes care of engine drain + re-arm without snapping the playhead.

We follow the same pattern exactly. `section_actions` carries
section + variant CRUD; `arrangement_actions` carries step CRUD.
Splitting them keeps each module under the cap and pins
responsibilities cleanly.

### 2. Delete refusal mirrors chord-loop / pattern policy

`delete_section(id)` refuses when any `Arrangement.sections` step
references the section, and produces a "referenced by: step 0
(verse @ bar 0), step 4 (verse @ bar 32)" message. The model
dry-run runs before the edit-pump commit; on refusal, no edit
lands. Surface is `eprintln!` until the toast/alert primitive
ships (matches CL1 + P1).

`remove_section_variant(id, vid)` refuses when:
- `vid == section.default_variant` (the default is protected — must
  change the default first), OR
- any `SectionRef.variant == vid` exists in the arrangement.

### 3. Variant duration override uses the auto-promote pattern

`set_section_duration_bars(id, variant, bars)`: when `variant ==
default_variant`, mutates `section.base.duration_bars` directly.
When `variant` is non-default and the override exists with
`duration_bars: Some(_)`, mutates in place. When the override
exists with `duration_bars: None` (or no override entry), auto-
promote: create / extend a `SectionVariantOverride` with
`duration_bars: Some(bars)`. This matches the activation auto-
promote pattern from P4.x (see [[project-next-session-pickup]]).

Setting a non-default variant's duration back to the base value
does NOT auto-clear the override — explicit clearing is a separate
"inherit from base" action surfaced in S3's three-way scale picker
(applies symmetrically here, can add a "match base" affordance to
the duration nudger).

### 4. Default-variant change does not rewrite SectionRefs

`SectionRef.variant: VariantId` is the variant in use, stored
explicitly. Changing `Section.default_variant` does not retarget
existing `SectionRef`s — they keep pointing at whatever they
pointed at. The "↳ default" chip in the arrangement variant
picker reflects "this step is targeting whatever's currently
default" semantics, which means picking "↳ default" writes the
section's *current* default_variant, not a sentinel. Re-checking
on `default_variant` change does NOT rewrite existing steps; the
chip just visually updates.

(If we wanted "always-track-default" semantics we'd need a
sentinel `VariantId::DEFAULT` distinct from concrete variant ids.
The current model doesn't have that, and CL4 already established
the precedent of storing explicit variant ids in
`Section.chord_loops` entries via `SectionVariantOverride`.)

### 5. No-gap arrangement contract enforced UI-side

The model permits `SectionRef.start` to be arbitrary `MusicalTime`,
which would technically allow gaps or overlaps. **The v1 editor
enforces a no-gap, no-overlap contract** via
`recompute_starts(&mut Project)` after every mutation:

```text
let mut t = MusicalTime::ZERO;
for step in &mut project.arrangement.sections {
    step.start = t;
    t = t + step_duration(project, step);
}
```

`step_duration` resolves variant override's `duration_bars` first,
else base. The helper is pure, tested, and called from every
`arrangement_actions` mutator.

Future support for explicit gaps (silence / count-off) is a model-
level question deferred to a later round.

### 6. Drag-to-move uses the CL2.x pattern, not HTML5 draggable

`Drag::absolute()` from `rinch::core::events` is the right primitive
for continuous-position drag — registered from inside an onclick
(which fires on mousedown in rinch's dispatch model), with
`.on_move(|x, y| ...)` updating a `DragPreview` signal and
`.on_end(|x, y| ...)` committing once. The HTML5 `draggable` +
`ondragstart` API only fires at start / end and is the wrong shape
([[reference-rinch-tooling]], [[project-next-session-pickup]]).

`AppState.arrangement_drag_preview: Signal<Option<ArrangementDrag
Preview>>` lives on the store; `SectionBlock` reads it each render
to apply a `translateX(...)` while the drag is live. On commit,
the action module routes through `move_step` and clears the
preview signal.

**Snap-to-bar.** The drag's `on_move` computes a target index by
mapping pointer x to a bar position, then clamping to the
[0, len) neighbour range. On commit, we move to that index. Pixel-
accurate sub-bar movement is not supported (and would conflict with
the no-gap-no-overlap contract anyway).

### 7. Variant CRUD inheritance

`add_section_variant(id, name)` creates an empty
`SectionVariantOverride { duration_bars: None, scale_override: None,
chord_loops: None, activations: {} }` — every field inherits from
base by default. `rename_section_variant(id, old, new)` is a rename
of the `BTreeMap` key + a rewrite of `default_variant` if the
renamed variant was the default; it does NOT touch existing
`SectionRef.variant` references in the arrangement (which would be
invalid post-rename if we didn't rewrite them, so we DO rewrite
them — same atomic edit-pump commit). Same for renames that affect
`Section.chord_loops` entries inside other variants — no, those
are per-section, not cross-section, so unaffected.

### 8. UI structure mirrors CL + P module layouts

```
crates/rawdaw-app/src/
├── section_actions/                 # NEW (S1)
│   ├── mod.rs                       # CRUD + variants
│   ├── duration.rs                  # set_duration + auto-promote
│   └── tests.rs
├── arrangement_actions/             # NEW (S4)
│   ├── mod.rs                       # step CRUD
│   ├── recompute_starts.rs          # canonical helper
│   └── tests.rs
├── regions/
│   ├── library/
│   │   └── sections.rs              # NEW (S2; carved out of mod.rs)
│   ├── arrangement/                 # SPLIT (S5; was a single file)
│   │   ├── mod.rs
│   │   ├── section_lane.rs
│   │   └── append_action.rs
│   └── ...
└── section_editor/
    └── meta_bar/                    # SPLIT (S3; was meta_bar.rs)
        ├── mod.rs
        ├── name_duration.rs
        ├── variants.rs              # variant tab strip CRUD
        └── scale.rs
```

Splits happen as files approach cap during each phase, not pre-
emptively.

### 9. Tests are pure-helper-heavy; UI verification via MCP

Every action module pure helper has a unit test. Variant CRUD's
collision suffix bumping (`untitled-2`, `untitled-3`, …) is pinned
by tests. `recompute_starts` is pinned across the matrix of
insert / remove / move / variant-change operations.

UI surfaces are verified interactively with the `rinch` MCP server:
launch the app, click the affordance, screenshot, inspect DOM /
computed style. Matches the verification cadence used in CL2 + P4.

### 10. Selection axes stay narrow

Existing selection axes (`selected_idx`, `selected_track`,
`selected_chord_loop`, `selected_pattern`, `selected_master_fx` once
X1 lands) cover everything we need. No new top-level axis for
"selected arrangement step" — the editor operates on click-time
index, mirroring how chord-loop event edits use a transient
`focused_chord_event_idx` signal (CL2).

Section block click stays as today: `app.set_selected_idx(Some
(arrangement_step_idx))` — i.e., clicking step N selects the
underlying section for editing in the section editor. The
arrangement context menu and drag use a transient
`arrangement_drag_preview` signal for the move handler.

### 11. Cap-violation hygiene runs alongside, not as a blocker

`piano_roll.rs` (790) and `regions/inspector/mod.rs` (705) are
pre-existing 700-line cap violations called out in
[[project-status]]. Not in this milestone's S-numbered phases —
tracked as the H milestone (see picklist + the H-hygiene block at
the end of this doc). H runs after S + X + G so we don't entangle
new code with a refactor.

---

## Phase S1 — Section actions module

**Scope.** Pure helpers + edit-pump wrappers for section CRUD +
variant CRUD. No UI consumers; primitives exist and compile.

**Files.** `crates/rawdaw-app/src/section_actions/{mod.rs,
duration.rs, tests.rs}`. Mod registration in `lib.rs` /
`main.rs`. Re-export from `apply_project_edit` consumer surface
on `AppState`.

**Primitives.**

```rust
pub fn create_section(project: &mut Project, name: &str) -> SectionId;
pub fn rename_section(project: &mut Project, id: SectionId, name: &str) -> Result<(), Conflict>;
pub fn duplicate_section(project: &mut Project, id: SectionId) -> SectionId;
pub fn delete_section(project: &mut Project, id: SectionId) -> Result<(), Vec<ArrangementRef>>;
pub fn set_section_color(project: &mut Project, id: SectionId, color: ColorSlot);

pub fn set_section_duration_bars(project: &mut Project, id: SectionId, variant: &VariantId, bars: u32);
pub fn set_section_scale_override(project: &mut Project, id: SectionId, variant: &VariantId, scale: Option<Scale>);
pub fn clear_variant_duration_override(project: &mut Project, id: SectionId, variant: &VariantId);
pub fn clear_variant_scale_override(project: &mut Project, id: SectionId, variant: &VariantId);

pub fn add_section_variant(project: &mut Project, id: SectionId, name: &str) -> Result<VariantId, Conflict>;
pub fn remove_section_variant(project: &mut Project, id: SectionId, variant: &VariantId) -> Result<(), RemoveVariantError>;
pub fn rename_section_variant(project: &mut Project, id: SectionId, old: &VariantId, new: &VariantId) -> Result<(), Conflict>;
pub fn set_default_variant(project: &mut Project, id: SectionId, variant: &VariantId) -> Result<(), MissingVariant>;
```

**Tests.** ≥30 unit tests covering: create / unique-name collision
suffix bump; rename collision; duplicate produces fresh SectionId +
preserves body+variants+default; delete refusal lists arrangement
refs; delete success removes from `project.sections`; color writes
overlay; duration writes base; duration writes variant override
with auto-promote-from-base; duration on variant clear; scale same
shape; add-variant collision; remove-variant default-protected;
remove-variant arrangement-ref-protected; rename-variant rewrites
`default_variant` if it matched; set-default validates target.

**Done when.** Tests green; primitives compile under all three
feature flags; clippy clean; primitives are NOT yet wired to any
UI (S2 / S3 do that). Workspace test count rises by ~30.

---

## Phase S2 — Library wiring for section CRUD

**Scope.** Make the Sections group in the Library interactive.

**Files.**
- `regions/library/sections.rs` (NEW; carved out of `mod.rs`).
- `regions/library/mod.rs` (updated to mount the new sub-module
  + register the Sections row builder).

**Surface.**
- `+ new section` button → `apply_project_edit(create_section
  ("untitled"))` + `select_section(new_id)`.
- Per-row `⋯` action menu: Rename (inline-edit field swap) /
  Duplicate / Delete + 10-color palette swatches + "Default" reset.
- Inline rename uses the same untracked-Effect peek pattern as
  C4's `NameControl` (see CL1 + P1 row implementations).
- Delete refusal surfaces as `eprintln!` (matches CL1 + P1 until
  the toast primitive ships).
- Click anywhere on a non-action area: select the section
  (`select_section(id)`).

**Component.** `SectionRow { section_id, name, color, is_selected,
editing }` keyed by `section_id`. Same `#[component]` shape as
`ChordLoopRow` and `PatternRow` so per-row Signals survive under
`key:`.

**Done when.** User can create / rename / duplicate / delete /
recolor a section from the Library. Delete refusal produces a
useful reference list. `regions/library/mod.rs` is under cap.
Verified via MCP: create → rename → recolor → duplicate → delete
flow visible.

---

## Phase S3 — Section-editor meta-bar editing

**Scope.** Make the section editor's meta-bar fields editable +
the variant tab strip CRUD-capable.

**Files.**
- `section_editor/meta_bar/mod.rs` (current `meta_bar.rs` split).
- `section_editor/meta_bar/name_duration.rs`.
- `section_editor/meta_bar/variants.rs` (variant tab strip CRUD).
- `section_editor/meta_bar/scale.rs` (three-way scale picker).

**Surface.**
- **Name.** Inline-editable via `NameControl` shape — click to
  edit, Enter to commit, Esc to cancel, focus loss commits.
- **Duration.** Numeric nudger (− / + buttons + click-to-type
  numeric input). Routes through `set_section_duration_bars` with
  the **active variant tab as the target**. On a non-default
  variant tab, edits create / mutate the variant override
  (auto-promote pattern).
- **Scale override.** Three-way picker: "inherit base" (sets
  `scale_override` to `None` on variant) / "no scale override"
  (`Some(None)`) / "use scale …" (`Some(Some(scale))`). On the
  base tab, two-way (no inherit option).
- **Variant tab strip.** Each tab → right-click ContextMenu with
  Rename / Delete / Set as default. Trailing `+ new variant` tab
  → inline rename + create. Default variant decorated (e.g., dot
  marker) + protected against delete.

**Done when.** All four meta-bar fields are editable. Variant tab
CRUD works including default-variant protection. MCP confirms the
inline-rename + duration nudges + variant add/delete flow. Meta-bar
files all under cap.

---

## Phase S4 — Arrangement actions module

**Scope.** Pure helpers + edit-pump wrappers for arrangement step
CRUD + start-time recomputation.

**Files.** `arrangement_actions/{mod.rs, recompute_starts.rs,
tests.rs}`. Mod registration in `lib.rs` / `main.rs`. Re-export
from `AppState` apply-project-edit surface.

**Primitives.**

```rust
pub fn append_step(project: &mut Project, section_id: SectionId, variant: VariantId) -> SectionRefId;
pub fn insert_step_after(project: &mut Project, index: usize, section_id: SectionId, variant: VariantId) -> SectionRefId;
pub fn remove_step(project: &mut Project, index: usize);
pub fn move_step(project: &mut Project, from: usize, to: usize);
pub fn duplicate_step(project: &mut Project, index: usize) -> SectionRefId;
pub fn set_step_section(project: &mut Project, index: usize, section_id: SectionId, variant: VariantId);
pub fn set_step_variant(project: &mut Project, index: usize, variant: VariantId);

pub fn recompute_starts(project: &mut Project);
pub fn step_duration(project: &Project, step: &SectionRef) -> MusicalTime;
```

`recompute_starts` runs after every mutating helper. `step_duration`
resolves the variant override's `duration_bars` first, else falls
back to the base body.

**Tests.** ≥25 unit tests covering: append-from-empty / append-with-
existing / insert-at-0 / insert-at-end / remove-first / remove-last /
remove-middle / move-forward / move-backward / move-same-no-op /
duplicate-clones-section+variant / set-section-recomputes-starts /
set-variant-recomputes-starts-when-duration-changes / set-variant-
no-shift-when-duration-same / start recomputation is monotonic
non-decreasing / start of first step is `MusicalTime::ZERO` /
variant override duration honored / no override falls back to base.

**Done when.** Tests green; primitives compile; no UI consumers yet.
Workspace test count rises by ~25.

---

## Phase S5 — Arrangement view interactive editor

**Scope.** Wire the arrangement view to `arrangement_actions`. Drag-
to-move, right-click context menu, append-step affordance,
variant-chip picker.

**Files.**
- `regions/arrangement/mod.rs` (split out from current single file).
- `regions/arrangement/section_lane.rs` (SectionLane + SectionBlock
  + drag wiring).
- `regions/arrangement/append_action.rs` (`+ append` button +
  section/variant picker popover).

**Surface.**
- **Block drag.** `SectionBlock`'s onclick registers a
  `Drag::absolute().on_move(...).on_end(...).start()`. `on_move`
  computes the target insert index from pointer x via the lane's
  bar-to-px scale and updates `AppState.arrangement_drag_preview`.
  `on_end` commits via `move_step(from, to)`. Snap-to-bar; clamp
  to `[0, sections.len())`.
- **Right-click ContextMenu** on `SectionBlock`: Duplicate / Insert
  section before… / Insert section after… / Change variant… /
  Delete. "Insert section before/after" opens the section picker
  popover. "Change variant" opens a `Select` of the section's
  variants.
- **`+ append`** button anchored to the right of the section lane.
  Disabled (`opacity: 0.4`, no handler) when `project.sections` is
  empty; otherwise opens a popover with section + variant pickers.
- **Variant chip on blocks** becomes clickable — same `Select`
  shape as the context-menu "Change variant" option, just inline
  on the block.
- **Drag preview style.** `SectionBlock` reads
  `arrangement_drag_preview.get()` each render; when its index is
  the active drag, applies `transform: translateX(delta_px); z-
  index: 2; opacity: 0.85; box-shadow: ...;` to lift it visually.

**Done when.** All five affordances above work. Drag preview
matches CL2.x visually. Commit path uses C2 drain-without-rewind
so the playhead doesn't snap to bar 1 on any edit (verify with
playback running during a move). MCP-verified end-to-end.

---

## Phase S6 — Close-out

**Scope.** Documentation + memory pointers.

**Actions.**
- Update this plan doc's Status section with commit hashes,
  per-phase deviations, and a "Milestone closed" paragraph at top.
- Update `docs/design/section-variants.md` + `docs/design/
  composition-model.md` + `docs/design/ui-brief-r1.md` with a
  "UI semantics shipped in S5" section if anything deviates.
- Update [[project-status]] + [[project-next-session-pickup]].
- Recommend X1 (master-fx-chain model surgery) as next bite.

**Done when.** Plan doc + design docs reflect what shipped; memory
pointers updated; user confirms milestone closed.

---

## H — Cap-violation hygiene (no plan doc; runs after S + X + G)

Pre-existing 700-line cap violations from [[project-status]]:

- `crates/rawdaw-app/src/regions/inspector/piano_roll.rs` (790).
- `crates/rawdaw-app/src/regions/inspector/mod.rs` (705).

Split by concern:

- **`piano_roll.rs`** → carve `note_block.rs` (NoteBlock component
  + selection + drag) and `grid.rs` (background grid SVG + ruler)
  out of `piano_roll.rs`. Keep the top-level `PianoRoll` + per-
  note edit handlers in `mod.rs` under cap.
- **`regions/inspector/mod.rs`** → already has sub-modules
  (synth_editor / wavetable_editor / drum_editor / matrix_editor /
  preset_dropdown / activation_table). Move the remaining
  top-level body (track editor / chord-loop editor inspector
  dispatch / placeholder rows) into focused sub-modules
  (`track.rs`, `chord_loop.rs`, `empty_state.rs`) and shrink
  `mod.rs` to dispatch + shared chrome.

No new tests required (refactor only). Workspace stays green.
