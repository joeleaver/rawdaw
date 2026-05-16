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

## Open questions

- **Variant rename / delete propagation.** If a variant is deleted, what happens to `SectionRef`s pointing at it in the arrangement? Probably fall back to `default_variant` with a warning. Same for rename — by ID, not name, so renames are safe.
- **Variant-level realization overrides at the section scope.** Should a section variant be able to override realization params for *all* its activations at once (e.g. "in the stripped variant, humanization tightens by 50% across the board")? Probably overkill for v1.
