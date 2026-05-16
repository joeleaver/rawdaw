# Round-2 Rinch port plan

Implementation roadmap for porting the locked round-2 section-editor
mockup to a runnable Rinch surface in `crates/rawdaw-app/`.

**Spec.** The mockup at `docs/design/mockups/round-2/` is the canonical
spec. README decisions 1–24 (especially 19–24 from the rebuild) are
binding. Where this plan and the mockup disagree, the mockup wins.

**Engineering constraints** (from `CLAUDE.md`).
- Architectural correctness over shortcuts. Always.
- Unlimited time and budget — pay the cost of doing it right.
- ~700-line cap per source file. Split by concern when approached.
- UI work uses the `rinch:rinch` skill and the `rinch` MCP server.
- No `unwrap()` outside tests / proven-impossible cases. No silent
  swallowing. No fallbacks that hide invariant violations.

**Cadence.** Each phase ends with `cargo run -p rawdaw-app` showing
concrete visible progress, clippy clean (both default and `--features
cpal-driver`), and `cargo test --workspace` green. Invoke the
`rinch:rinch` skill at the start of each phase — the framework rules
are easy to forget between sessions.

---

## Phase 0 — Scaffold + navigation stub ✅ done

**Goal.** Establish module structure and a top-level reactive switch
between the arrangement view and the section editor.

**Steps.**
- Create `crates/rawdaw-app/src/section_editor/mod.rs` (single file at
  first; split as it grows toward the ~700-line cap).
- Add `enum EditorMode { Arrangement, SectionEditor { section_key:
  String, variant: String } }` and a `Signal<EditorMode>` shared
  via a Rinch store installed in `app::main_window`.
- Wire the round-1 inspector's "Open in editor" footer button to set
  `EditorMode::SectionEditor`. Wire a stub `Done` button to clear it.
- `section_editor::SectionEditor` renders `"section editor: <name> /
  <variant>"` + Done button only.

**Done when.** Clicking "Open in editor" with the verse@bar5 selection
swaps the content area below the top bar for the stub editor;
"Done" returns to the arrangement with the prior selection intact.

**Deviations from the original plan.**
- Used `section_key: String` instead of `section_idx: usize` — the
  existing fixture API is keyed by section name (`section_by_key`); idx
  would have required a parallel lookup table.
- Shared state lives in a `create_store(AppState)` (Rinch idiomatic for
  cross-tree state) rather than a Signal threaded through props. The
  Signal still exists — it's a field of `AppState` — but components
  fetch it by `use_store` instead of receiving it as a prop.
- Section editor replaces the **entire content area** below the top bar
  (matches the mockup's `Shell` composition), not just the inspector
  pane. The original wording was ambiguous; the mockup's visual is
  unambiguous.

---

## Phase 1 — Fixture extension ✅ done

**Goal.** Extend `fixture.rs` with the data the section editor needs.
Defer `rawdaw-model::Project` integration (the round-1 punt) — it's out
of scope for this port and risks bloating the round.

**Steps.**
- Add `RoleDefaults` table: per-role voicing / octave / humanization.
- Add `Pattern.default_variant: &'static str`.
- Add `Activation { pattern, state, realization: Option<Realization>,
  variant_schedule: &[(BarRange, Option<&str>)], per_note_overrides:
  u32 }`.
- Add `Realization { voicing, octave: OctaveSpec, humanization:
  Humanization }`.
- Add `Section.variant_overrides: BTreeMap<&str,
  SectionVariantOverride>` with the `Silent` / `Replace(Activation)`
  shape from `section-variants.md`.
- Populate verse, chorus, and the stripped variant override exactly as
  the JS fixture does — pad-absent-from-verse and
  implicit-default schedule entries included.

**Port-time rules to honor.**
- Variant schedule = `Vec<(BarRange, VariantId)>`. **No phantom
  "default" entries.** Compute implicit-default fills at render time.
- Sub-range silences = `(BarRange, None)` in the same `Vec`, not a
  separate collection.

**Done when.** Unit tests round-trip the fixture and assert:
- pad has no verse entry,
- stripped lead's schedule has exactly one entry (`(3..4, None)`),
- chorus drums' schedule has exactly one entry (`(7..8,
  Some("fill"))`).

**Deviations from the original plan.**
- `fixture.rs` was approaching the ~700-line cap by the end of the
  phase, so split it into `fixture/mod.rs` (types + lookup helpers,
  ~406 lines) and `fixture/data.rs` (static tables + invariant tests,
  ~382 lines). Per CLAUDE.md rule 3 — refactor *before* you cross the
  line. Tests moved into `data.rs` since they assert the data shape.
- Round-1's existing `Activation` and `VariantOverride` shapes were
  extended in place rather than forked. Existing callers (the
  inspector's `activation_table`) keep working because the new fields
  (`realization`, `variant_schedule`, `per_note_overrides`) are
  optional / sparse and the inspector ignores them.
- `Section` dropped `Eq` (only `PartialEq`) because it transitively
  contains `Humanization`'s `f32` fields. Section identity should be
  by-id anyway, so structural Eq was never load-bearing.
- Added five fixture-invariant tests (the three plan-required ones
  plus `role_defaults_match_round_2_table` and
  `patterns_all_carry_default_variant` to pin the new tables).

---

## Phase 2 — Section editor frame (stock primitives) ✅ done

**Goal.** Build the chrome using Rinch primitives that already ship.

**Steps.**
- `section_editor::header` — back chevron, breadcrumb text,
  section-color stripe, name, "section editor" subtitle, `Duplicate
  section` / `Done` buttons. Reuse round-1's color-stripe approach.
- `section_editor::variant_tabs` — Rinch `Tabs`; italic "default
  variant" suffix on the section's default tab; `+ New variant`
  affordance trailing the tab list.
- `section_editor::meta_bar` — 200/240/1fr grid; `NumberInput` for
  duration; `Select` for scale; placeholder for the chord-loop strip;
  `↳ base` inheritance pill on each field when `current_variant !=
  default_variant && variant_override_for_that_field.is_none()`
  (computed, not stored).
- `section_editor::activations_header` — "Activations · 4 project
  tracks" + `New track to project` affordance.

**Done when.** verse@base renders the full chrome (header, two tabs,
meta bar with empty chord-loop slot, activations header). No cells
yet. Switching to `stripped` swaps the `↳ base` markers in.

**Deviations from the original plan.**
- The plan put variant-switching wiring in Phase 7. Phase 2's "swap
  `↳ base` markers in" check required reactive tab handling now, so
  variant click → `AppState::set_variant` landed here. Phase 7 can
  retire that line item.
- **Rinch closure-boundary gotcha** discovered: writing `style: {move
  || ...}` inside rsx double-wraps the user's closure inside the
  macro's own effect closure, which forces `FnOnce` semantics on
  String captures (the inner closure consumes them via move). The
  fix: drop the `move ||` and provide a bare expression — the macro's
  outer effect already tracks signal reads inside and the captures
  get borrowed each invocation. Same rule for rsx `if` conditions:
  write `if matches!(...)` directly, not `if { let ...; match ... }`.
  Captured this in code comments at the two affected sites.
- `SectionMetaBar` no longer takes `current_variant` as a prop; it
  reads `editor_mode` from the store internally so the `↳ base`
  markers stay reactive without the component re-mounting.
- `+ Loop range` action lives in the chord-loops field header (per
  README decision 22). Pulled forward from Phase 3 since it slotted
  naturally into the meta-bar code.

---

## Phase 3 — ChordLoopBar (small custom SVG)

**Goal.** The lighter of the two custom-SVG pieces. Reuse where
possible.

**Steps.**
- `section_editor::chord_loop_bar` — tile a `ChordLoop`'s events across
  the section duration; each cell shows `Roman` + small absolute label;
  first cell of each loop iteration carries a heavier left stripe in
  the loop's identity color.
- Multi-loop schedule: render only `chord_loops[0]` for now and add a
  TODO comment pointing at the round-3 multi-loop design question. The
  JS mockup has the same gap; we are not regressing.
- Reuse `Roman` from round-1 if it exists; otherwise port —
  case-preservation is principle 3, do not lowercase or small-caps.

**Done when.** verse meta bar shows `I V vi IV` with the first-cell
stripe; chorus shows `vi IV I V | vi IV I V` with two stripes.

---

## Phase 4 — Activation cell container + identity column

**Goal.** Build the 3-column cell shell and the leftmost column.
Generalize the "Paper with left-edge color stripe" into a reusable
primitive (Rinch port priority #4).

**Steps.**
- New primitive: `app::components::stripe_paper` — Paper with
  configurable left-stripe color + width. Replace round-1's bespoke
  inspector-header treatment with this in a separate small commit.
- `section_editor::cell::activation_cell` — 3-column grid (320 / 1fr /
  520), wraps `stripe_paper`, dims when `state == Silent`.
- `section_editor::cell::identity_column` — track row (kind icon + name
  + kind tag + optional `role: X` pill + state pill), pattern card
  with color swatch + `Open in pattern editor (round 3)` chevron,
  footer with pinned-pill / "no pinned notes" hint and the
  right-aligned `silenced in this variant` / `replaced in this variant`
  tag when present.
- `section_editor::cell::cell_inherit` — dashed-border placeholder for
  tracks without an entry, with `+ Add activation` button.
- The pinned chip uses the single warm-accent color — only place that
  color appears on a cell (per README decision 23). Plumb through
  `theme.rs`.

**Done when.** All four rows render on verse@base — three full active
cells (bass / lead / drums) and one dashed pad-inherit placeholder.
Realization and schedule columns are empty placeholders. Linked-highlight
from the arrangement view still works.

---

## Phase 5 — Realization column + InheritanceTag

**Goal.** Where computed inheritance rules become real. Honor the
port-time rule strictly: don't store inheritance flags — compute by
comparison.

**Steps.**
- New primitive: `app::components::inheritance_tag` — small bordered
  badge rendering `↳ role default` (or `↳ base`, etc.). Takes a
  `Source` enum, renders text + tooltip.
- `section_editor::cell::realization_column` — `Voicing` and `Octave`
  dropdowns (stock Select), `Humanize` row.
  - Compute "matches role default" by comparing
    `activation.realization.voicing` against
    `role_defaults[track.role].voicing`. Equal → `inheritance_tag`;
    different → asterisk (`InheritMark::Overridden`).
  - Drum tracks render the "drums are pitch-symbolic — voicing &
    octave do not apply" replacement string.
- `section_editor::cell::humanize_row` — four micro-controls. Per
  README port-time note, `seed` is `u64` in the model; display as a
  5–6 digit decimal in the fixture but plan the Rinch input to take
  the full `u64`. Stock Slider + NumberInput + ActionIcon.

**Done when.** lead@verse@base shows `triad-close` with `↳ role
default`, `Anchored · 4` with `*`, humanize row populated. Drum cell
shows the replacement text instead of dropdowns. Pad cell at chorus
shows `drop2` + `*` (overrides role's `triad-open`).

---

## Phase 6 — Variant-schedule timeline (the centerpiece)

**Goal.** The Rinch port priority #1 piece. Build it as a standalone
reusable widget — round 3 will need the same primitive for sub-range
editing.

**Steps.**
- `app::components::schedule_timeline` — stateless render. Takes
  `total_bars: u32`, `segments: &[Segment]`, `pattern_color: Color`,
  `silent: bool`.
  - Background: per-bar grid lines.
  - Segments — three visual modes:
    - **default**: flat fill, identity color, low alpha;
    - **non-default variant**: higher alpha + diagonal hatch (repeating
      linear gradient);
    - **silenced** (`variant == None`): dashed border + slashed
      pattern + italic "silent" label.
  - Bar number ticks in their own lane below the segment band — no
    clipping. `H=36`, segment band y=3..23, tick lane y=23..36.
- `section_editor::cell::schedule_column` — wraps `schedule_timeline`
  and the legend.
  - Segment builder: walk the schedule, gap-fill implicit-default
    segments at render time. **Do not** introduce phantom default
    entries in the model side.
  - Legend chips: filter by `!is_default`. Silenced chips use eye-off
    icon + dashed border. Single-bar ranges render as `bar N`,
    multi-bar as `bar A–B`.
  - Show the "click a sub-range to assign / silence a pattern variant"
    hint only when `has_non_default`.

**Done when.**
- chorus drums: plain bars 1–7 + hatched fill bar 8 + single `fill bar
  8` legend chip.
- stripped lead: plain bars 1–3 + dashed silent bar 4 + single silent
  legend chip with eye-off.
- verse base cells: plain `main` across the full duration with no
  legend.

---

## Phase 7 — Wire it all up

**Goal.** Static surfaces are done; connect navigation and variant
switching.

**Steps.**
- Click "Open in editor" → set `EditorMode::SectionEditor { section_idx,
  variant }`.
- Tab click in the variant strip → mutate `variant` in the editor
  state; all cells re-resolve their effective state from the variant
  override map.
- "Done" → return to `EditorMode::Arrangement` with the prior selection
  intact.
- Default-variant tab placement uses `Section.default_variant` per
  `section-variants.md`.

**Done when.** Clicking around the arrangement, opening verse,
switching from `base` to `stripped` and back, returning via Done,
opening chorus — all work and produce visuals matching artboards A /
B / C without restart.

---

## Phase 8 — Visual parity + sweep

**Goal.** Use the `rinch` MCP to compare the live app against the
mockup PNGs side-by-side; close every gap that isn't an explicit
round-3 deferral.

**Steps.**
- Screenshot the running app on each of the three states (verse/base,
  chorus/base, verse/stripped) at 1600×900 to match the mockup artboard
  sizes.
- Diff against `mockups/round-2/` rendered output.
- Verify the ~700-line cap: split `section_editor/` into submodules if
  any single file is over.
- `cargo clippy --workspace --all-targets --all-features` — zero
  warnings.
- `cargo test --workspace` — green.
- `cargo clippy --workspace --no-default-features`, then again with
  `--features cpal-driver` — both clean. Same discipline as round 1.

**Done when.** Screenshots match. Project-status memory updated.

---

## Out of scope (round 3 or later)

Don't backfill these in this port; they have their own design rounds
queued.

- Click-to-assign / drag-resize on schedule segments (round-3
  mechanics).
- Pattern editor (round 3).
- Piano roll drill-in for pinned notes (round 3).
- `rawdaw-model::Project` integration (still punted — see status
  memory).
- Multi-loop chord-loop schedule UI (gap from the mockup, flagged in
  Phase 3 with a TODO).
- Keyboard shortcuts.
- Click-to-break-inheritance on `↳ base` meta tags (round-3 open
  question).

---

## Risks / decisions worth flagging up front

- **`stripe_paper` refactor.** Phase 4 generalizes round-1's
  inspector-header stripe into a primitive. If the refactor turns out
  to require more invasive changes to the round-1 inspector than
  expected, deliver the stripe inline on the cell first and split the
  primitive out in a follow-up.
- **Variant switching state.** Phase 7 assumes a single shared
  `Signal<EditorState>` works cleanly with Rinch reactivity inside
  nested components. If it doesn't (e.g. closure-capture issues per the
  `rinch:rinch` skill notes), fall back to passing reactive state
  through props rather than restructuring data flow.
- **Fixture vs Project punt.** If during Phase 1 the fixture data
  shape feels like it's reinventing too much of `rawdaw-model`, we
  revisit the cross-crate fixture-sharing question. Right now keep
  extending constants.
