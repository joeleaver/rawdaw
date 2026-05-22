# Section Variants

Sections support named variants the same way patterns do, but with different mechanics — a section is an aggregation of references (chord loops, activations) rather than a flat body, so variants share by inheritance rather than independent copy.

## Motivating use cases

- `chorus` vs. `chorus-final` — same length and most content, but the final chorus has a chord substitution in the last bar, an active strings track, and a bigger drum fill.
- `intro-short` (4 bars) vs. `intro-long` (8 bars) — different lengths, otherwise similar.
- `verse` vs. `verse-stripped` — same structure with drums and bass silent.
- `bridge` vs. `bridge-modulated` — same shape, key change.

The shared goal: edit the structural common ground once (chord loop, base activations) and have it propagate, while specific differences are preserved per variant.

## Model

A section has a `base` definition plus a flat list of named variants. Each variant is a **sparse override** on the base.

```rust
struct Section {
    id: SectionId,
    name: String,
    base: SectionBody,
    variants: BTreeMap<VariantId, SectionVariantOverride>,
    default_variant: VariantId,   // typically "base"
}

struct SectionBody {
    duration_bars: u32,
    scale_override: Option<Scale>,
    chord_loops: Vec<ChordLoopRef>,
    activations: HashMap<TrackId, ActivationEntry>,
}

struct SectionVariantOverride {
    duration_bars: Option<u32>,                          // None = inherit
    scale_override: Option<Option<Scale>>,               // outer = inherit; inner None = clear
    chord_loops: Option<Vec<ChordLoopRef>>,              // None = inherit; Some = replace
    activations: HashMap<TrackId, ActivationOverride>,   // sparse: only changed tracks
}

enum ActivationOverride {
    Replace(ActivationEntry),    // different activation for this variant
    Silent,                       // track doesn't play in this variant
    // Inherit is implicit — absent key means inherit from base
}
```

## Merge semantics: shallow

Each top-level property is either **inherited** (use base's value) or **replaced** as a whole. There is no deep merge within nested types.

| Field             | Variant says...                    | Effective value                           |
|-------------------|-------------------------------------|-------------------------------------------|
| `duration_bars`   | `None`                              | base's duration                           |
| `duration_bars`   | `Some(n)`                           | `n`                                       |
| `scale_override`  | `None`                              | base's scale (which may itself be `None`) |
| `scale_override`  | `Some(None)`                        | explicitly clear the scale override       |
| `scale_override`  | `Some(Some(scale))`                 | use `scale`                               |
| `chord_loops`     | `None`                              | base's chord loops                        |
| `chord_loops`     | `Some(vec)`                         | use `vec` wholesale                       |
| `activations[T]`  | (key absent)                        | base's activation for `T` (if any)        |
| `activations[T]`  | `Silent`                            | track `T` silent in this variant          |
| `activations[T]`  | `Replace(activation)`               | use `activation` wholesale for `T`        |

"Shallow" means: if you want to override one field inside an `ActivationEntry` (say, the `realization.seed`), you replace the whole activation. The cost is some surgical-edit ergonomics; the benefit is predictable, debuggable merge semantics and a UI that can show "inherited / overridden" at a single level.

## Use at the arrangement layer

The arrangement is a sequence of section *references* with a variant pick:

```rust
struct SectionRef {
    section: SectionId,
    variant: VariantId,    // defaults to the section's default_variant on placement
}
```

When the user drags a section into the arrangement, the section's `default_variant` is selected. The user can switch the variant from a dropdown on the section block.

**No third-level overrides** at the arrangement layer in v1. If you need a unique variation of an already-variant section, make another variant of the section. Variants are cheap; three-level merge (base → variant → instance) is not worth the complexity.

## Composition with pattern variants

Section variants and pattern variants are orthogonal:

- The **section variant** picks which `ActivationEntry` plays for each track.
- The **pattern variant** (selected via the activation's `variant_schedule`) picks which pattern body plays at each bar.

They compose with no special handling. A `chorus-final` section variant might `Replace` the drums activation with one whose `variant_schedule` puts the `fill` pattern variant in the last bar.

UI vocabulary distinguishes the two: "section variant" appears on section blocks in the arrangement; "pattern variant" appears in the pattern editor and on activation block sub-segments.

## Length variants

Variants can change the section's duration. Two interactions to spell out:

- **Chord loops auto-loop** to fill the variant's duration. A 4-bar chord loop in an 8-bar variant plays twice. Same rule as patterns looping inside an activation — consistent across the model.
- **Activation `variant_schedule` bar indices are absolute** (relative to bar 0 of the section). A schedule of `[(bar 3, fill)]` in a 4-bar base places the fill at the end; in an 8-bar variant of the same activation, the fill lands at bar 3 — halfway through. If that's wrong for the long variant, the user overrides that activation in the long variant explicitly.

We may add relative-to-end addressing (e.g. `last_bar`) in a future version if absolute indices become painful. For v1, absolute is simple and predictable.

## Inheritance chains

Variants only inherit from the section's `base`. No multi-level inheritance (no `chorus-final` inheriting from `chorus` inheriting from `chorus-base`). Flat hierarchy keeps the merge story simple.

## Chord-loop variants?

**No** (v1). Chord loops are cheap — they're short sequences of harmonic events. Cloning a chord loop to make a substituted version costs little, and the section variant can reference the alternative loop in its `chord_loops` override. Adding variants to chord loops adds a third place where variant machinery lives (after patterns and sections) for marginal benefit.

If chord-loop cloning becomes painful in practice, we revisit.

## UX

**Section editor** has a variant-tabs strip at the top: `base`, `chorus-final`, `chorus-stripped`. Switching tabs shows that variant's effective view. Properties that are inherited from base display dimmed with an "inherited" marker; click to override (turning the field from inherit into replace). A "revert to base" affordance reverses the override.

**Arrangement view** shows each section block with its variant name (e.g. `chorus / final`). Clicking the block opens a small inspector for switching variants. The library panel groups variants under their parent section:

```
sections/
  chorus  ▾
    base       (default)
    final
    stripped
  verse  ▾
    base       (default)
    bridge-tail
```

**Default variant on placement** uses the section's `default_variant`. Setting per-section defaults is part of the section editor.

## Resolved questions

- **Override granularity?** Shallow — each top-level property inherits or replaces as a whole.
- **Length variants and bar drift?** Absolute bar indices in `variant_schedule`. Override activations explicitly if the variant's length needs different scheduling.
- **Default variant on arrangement placement?** Section's `default_variant`, settable per section.
- **Inheritance chains?** No — flat, base-only inheritance.
- **Chord-loop variants?** No (v1). Clone chord loops instead.
- **Per-arrangement-instance overrides?** No — make another variant.

## UI semantics shipped with the pattern editor (P4.x)

The activation cell's editor surface lays a few interpretive
decisions on top of the model that aren't directly visible from
the type definitions:

- **Picker walks the override chain.** The pattern dropdown on a
  non-base variant row shows the *effective* pattern: variant
  `Replace.pattern_ref` if present, `None` for `Silent`, base's
  `pattern_ref` otherwise.
- **"(no pattern)" semantics differ by tab.** On the base tab it
  keeps the entry and nulls `pattern_ref` ("placed but silent").
  On a non-base variant tab it installs `ActivationOverride::Silent`
  for that track — the variant-level "silence this track here"
  affordance.
- **Schedule edits auto-promote a base clone.** Right-clicking a
  bar in the variant schedule on a non-base tab that has no
  override yet clones the base entry into a `Replace` and applies
  the schedule mutation to the clone. Base stays untouched.
  `Silent` overrides remain Silent (no schedule edit promotes
  them); rows with no base entry no-op.
- **Default-fill ranges are first-class for merge.** Right-click
  merge-left/right on a default-fill bar adopts the adjacent
  segment's variant id (extending coverage); the inverse
  (explicit entry adjacent to default-fill) drops the entry back
  to default. `clear-range` on default-fill is a no-op since
  the bar is already cleared.
- **`merge_range_right` conservatively no-ops at the end of the
  schedule** when no entry exists past the source — the pure
  helper doesn't take `total_bars`, so it can't tell whether the
  trailing range has anywhere to merge into.

## UI semantics shipped with the section + arrangement editor (S5)

The S1–S5 milestone (close-out `2026-05-22`,
`docs/section-arrangement-editing-plan.md`) made section
variants editable end-to-end. A few interpretive decisions on
top of the model that aren't directly visible from the type
definitions:

- **Variant tab strip is the section-editor primary axis.**
  Each `SectionVariantOverride` key gets a tab; the section's
  `default_variant` tab is decorated with a marker and
  protected against delete. Tab strip lives at
  `section_editor/variant_tabs.rs`. The active tab drives
  every meta-bar edit through `set_section_duration_bars(id,
  variant, bars)` etc. — non-default-variant edits auto-
  promote a sparse override (the activation-auto-promote
  pattern P4.x established applies symmetrically here).
- **Variant chip on arrangement blocks is a clickable
  `DropdownMenu` button**, not a separate inspector pane.
  Each `SectionBlock` in `regions/arrangement/section_lane.rs`
  renders its `SectionRef.variant` as a chip in the top-right
  corner; clicking the chip opens a dropdown listing the
  section's variants. Picking a variant commits via
  `arrangement_actions::set_step_variant(index, variant)` —
  no popup, no separate inspector. The "↳ default" picker
  semantics from the plan's §4 hold: picking the
  default-variant entry writes the section's *current*
  `default_variant` rather than a sentinel; later
  `default_variant` changes do NOT retarget existing
  `SectionRef.variant` values.
- **Block-level actions surface in two places.** Each
  `SectionBlock` exposes both a top-left `⋯` button that
  opens a `DropdownMenu`, AND a right-click `ContextMenu`
  on the block body. Both fire the same items: Duplicate /
  Insert section before… / Insert section after… / Delete.
  The `⋯` button is the more discoverable affordance; the
  right-click ContextMenu is the power-user shortcut. (The
  ⋯ alone shipped with S5 because rinch v0.3's
  `display: contents` ContextMenu wrappers collapsed
  percent-positioned children to 0×0; the ContextMenu came
  back with the rinch issue #25 fix in commit `0bf4680`.)
- **"Insert section before/after" reads from
  `AppState.selected_section`.** Picking the menu item
  inserts the currently-selected Library section at the
  chosen position; if no section is selected, the action
  `eprintln!`s a hint. An inline section picker popover is a
  future paper-cut bite.
- **`+ append` is the only "add to arrangement" affordance in
  v1.** Lives at the right end of the section lane in the
  arrangement toolbar (post-F4 pixel-positioning refactor:
  `regions/arrangement/toolbar.rs`). Opens a section picker
  `DropdownMenu`; commits via
  `arrangement_actions::append_step(section_id, section.
  default_variant)`. Disabled when `project.sections` is
  empty. Drag-from-library is a later UX polish.
- **Variant tab CRUD lives entirely in the section editor.**
  `+ new variant` tab → inline rename (a fresh, empty
  `SectionVariantOverride` is created on commit); per-tab
  context menu → Rename / Delete / Set as default.
  Default-tab delete is rejected (must change the default
  first); arrangement-referenced variant delete is rejected
  with a referenced-by list.

## Open questions

- **Variant rename / delete propagation.** If a variant is deleted, what happens to `SectionRef`s pointing at it in the arrangement? Probably fall back to `default_variant` with a warning. Same for rename — by ID, not name, so renames are safe.
- **Variant-level realization overrides at the section scope.** Should a section variant be able to override realization params for *all* its activations at once (e.g. "in the stripped variant, humanization tightens by 50% across the board")? Probably overkill for v1.
