# rawdaw — section editor · round 2

Round-2 starter focused on the **section editor**, and within it, the
**activation cell** — the (section × track) primitive where voicing,
OctaveSpec, humanization, per-pattern variant schedules, and per-note
overrides all live.

Background: the round-1 main window (in `mockups/round-1/`) shows the
arrangement, the library, and the inspector's low-fi activation table.
This round opens the section editor that the inspector links into, and
designs the activation cell at full fidelity.

---

## Quick start

Open `section-editor.html` in any modern browser. Two artboards sit on
the design canvas:

- **A · Verse / base** — clean baseline. Variant tabs visible (`base`,
  `stripped`). Activation cells for bass / lead / drums / pad with
  realization params populated.
- **B · Chorus / base · drum-fill schedule + voicing override** — the
  same surface for the 8-bar chorus, showing the **variant schedule
  timeline** (drums: `main` bars 1–7, `fill` bar 8) and an explicit
  voicing override (pad: `drop2` overriding role `pad`'s default
  `triad-open`).

```
mockups/round-2/
├── README.md                       ← you are here
├── section-editor.html             ← entry point
└── components/
    ├── data.js                     ← fixture (now with realization + variantSchedule)
    ├── parts.jsx                   ← shared bits (carries over from round-1)
    ├── topbar.jsx                  ← TopBar (carries over)
    ├── inspector.jsx               ← shared with round-1; used by section editor for some controls
    ├── activation-cell.jsx         ← NEW · the round-2 centerpiece
    ├── section-editor.jsx          ← NEW · section editor frame
    └── design-canvas.jsx           ← starter
```

The fixture (`data.js`) is the same one used in round-1, **extended**:

- `tracks[].role` is the typed role enum (`bass`, `melodic`, `pad`, `null`
  for drum tracks).
- `roleDefaults` table: per-role defaults for voicing / octave /
  humanization, used to compute the "↳ role: X" inheritance marker.
- Each `activation` now carries a `realization` block with
  `voicing` / `voicingFromRole`, `octave` / `octaveFromRole`,
  `humanization { velocity, timing, swing, seed }`.
- The chorus's `drums` activation has a `variantSchedule:
  Vec<{ range:[a,b], variant }>` matching `composition-model.md`'s
  sub-section model.

---

## The activation cell — anatomy

Each cell is a 3-column grid:

```
┌── Identity / Pattern / State ──┬── Realization ─────────┬── Variant schedule + overrides ──┐
│ ● bass  Pitched  role: bass    │ Voicing  [power     ▿] │  Variant schedule                │
│                       [active] │ Octave   [Role def. ▿] │ ┌──────────────────────────────┐ │
│                                │ Humanize                │ │ main (whole section)         │ │
│ [▮ bass-main · Pitched] [▸]    │  vel ▒░ 4%  tim ░ 4t   │ │                              │ │
│                                │  sw ░ 0%    seed 1742⟲ │ │  1   2   3   4   5   …       │ │
│ no per-note overrides          │                         │ └──────────────────────────────┘ │
│                                │ ↳ role: bass (inherit) │ legend: [main · default]         │
└────────────────────────────────┴────────────────────────┴──────────────────────────────────┘
```

### Column 1 — identity, pattern, state

- Track name + kind (Pitched / Drum) + role pill.
- Pattern ref card (with identity-color swatch) and an "open in pattern
  editor" affordance (the `▸` chevron).
- `state` pill (active / silent / inherit) with the round-1 unified
  asterisk-on-override convention.
- Per-note overrides footer: "N pinned ▸" when N > 0, otherwise muted
  "no per-note overrides."

### Column 2 — realization

- **Voicing** (`triad-close` / `four-way-close` / `drop2` / `shell` /
  …) — appears only on pitched tracks. Drum cells show
  "drums are pitch-symbolic — voicing & octave do not apply" in its
  place.
- **OctaveSpec** (`Nearest` / `Role default` / `Anchored · N` /
  `Up from prev` / `Down from prev`) — same treatment.
- **Humanize row** — four micro-sliders (velocity %, timing ticks,
  swing %, plus the seed). Seed has a re-roll button per `realization.md`'s
  "re-roll generates a new seed, reshuffling everything in that
  activation."
- **Two inheritance signals** sit on this column, both lightweight and
  consistent with round-1's vocabulary:
  - `↳ role: bass` tag → field value is the role default; this is the
    **inherited** case. Tooltip explains the source.
  - `*` asterisk → field value has been **overridden** from the role
    default in this activation. Tooltip explains.
  - Exactly one of the two appears next to each row; absence of both
    on a field means "no role default applies."

### Column 3 — variant schedule + overrides

- **Per-pattern variant schedule** (distinct from section variants): a
  horizontal timeline scaled to the section's effective duration, with
  colored segments showing which pattern variant plays at each bar
  range. Empty schedule = "default variant only" badge.
  - Default-variant segments render at lower alpha.
  - Non-default segments (e.g. `fill`) render at higher alpha + diagonal
    hatching so they read as "not the baseline groove."
  - Click a sub-range to assign / change its variant; drag a segment edge
    to resize (round-3 mechanics — affordance shown).
- **Legend chips** under the timeline: one chip per variant in play, with
  the default variant marked. Range labels appear for non-default
  segments.

---

## Decisions made (round-2 specific)

11 (round-1) ended at "Duplicate placement." Numbering continues:

13. **Cell = horizontal 3-column grid, not a row of stacked controls.**
    The three columns (identity · realization · schedule) carry distinct
    cognitive load and the user reads them top-to-bottom within each
    column rather than left-to-right across them. Stacked-row designs
    pushed the variant-schedule timeline off-screen or to a tooltip,
    which hides the most interesting per-activation property.

14. **One activation cell = one track in one section.** Cells stack
    vertically; no horizontal grid across tracks. A track that doesn't
    appear in this section's activations (round-2 fixture: pad in
    `verse`) renders as a dashed-border inherit placeholder — the user
    still sees the track, knows it's not playing here, and can click to
    add an entry.

15. **Pattern-variant schedule is a timeline, not a list.** A list of
    `(BarRange, VariantId)` is the data structure, but the UI shows it
    as a bar-aligned horizontal strip — same vocabulary as the
    arrangement view, same visual rules. Editing a schedule from a
    table-with-bar-numbers would force the user to mentally re-project
    onto the section's time.

16. **Inherited vs. overridden is two distinct signals.** Round-1 used a
    single asterisk for "this differs from base in this variant."
    Round-2's cell needs to talk about **two** inheritance axes:
    (a) variant override of section base, (b) cell override of track
    role. The role-inheritance case is more common than the variant case
    (most activations in a base variant inherit voicing from role), so
    we surface it explicitly with a `↳ role:` tag rather than relying on
    asterisks-by-absence.

17. **Realization controls only show when meaningful.** Voicing and
    OctaveSpec are hidden on drum cells (they have no chord/scale
    context). Humanization shows on every cell. This honors principle
    10 (speak the model's vocabulary) — the cell doesn't pretend a
    voicing exists on drums.

18. **The variant schedule's default segment is flat-filled; non-default
    segments add a hatch.** Texture, not color, distinguishes
    main-from-fill so the user can use one identity color (the
    pattern's color) across the whole schedule without losing
    legibility. Color stays object-identity, hatch is the modulator.

---

## Open round-2 questions

These are decisions that look right but should be exercised before
porting:

- **Where does "voicing-not-applicable" live for `melodic` /
  `countermelody` patterns?** The brief implies single-voice patterns
  ignore voicing, but the role defaults table still has a value. Right
  now we surface voicing on every pitched cell. Should single-voice
  roles also hide it? Probably yes; the round-2 cell can read a per-role
  "uses voicings" flag.
- **Schedule editor interactions.** Click-to-set-variant, drag-to-resize
  segment, shift-click-to-split — none of these are designed yet.
  Affordance is shown; mechanics are round-3 territory.
- **Per-note overrides drill-in.** "N pinned ▸" jumps where? The piano
  roll, filtered to this activation's pinned notes, is the obvious
  answer — designed in round 3.
- **Section meta inheritance markers when viewing `stripped`.** The bar
  shows `↳ base` tags on inherited fields. Should the tag also be
  clickable to *break* inheritance (turn into an explicit override)?
  Probably yes; round-3 specifies.

---

## Rinch component crosswalk (additions over round-1)

| Mockup element                              | Rinch primitive (or GAP)                                  |
|---------------------------------------------|-----------------------------------------------------------|
| Section editor frame                        | `Stack` + `Paper`                                         |
| Variant tabs                                | `Tabs`                                                    |
| Section meta bar (Duration / Scale / Loops) | `Group` w/ `NumberInput` + `Select` + custom strip        |
| Chord-loop tile strip                       | **GAP — custom SVG**, descended from round-1 ChordRibbon  |
| Activation cell                             | `Paper` + grid layout (or `SimpleGrid` cols)              |
| Voicing / Octave dropdowns                  | `Select`                                                  |
| Inheritance tag (`↳ role: X`)               | `Badge` w/ subtle border                                  |
| Humanize micro-sliders                      | `Slider` (Mantine-style); compact variant                 |
| Seed input + re-roll                        | `NumberInput` + `ActionIcon`                              |
| **Variant-schedule timeline**               | **GAP — custom SVG**, shares ruler primitive w/ round-1   |
| Variant legend chips                        | `Badge`                                                   |

### Rinch port priorities (round-2 additions)

1. **Variant-schedule timeline component.** This is essentially a tiny
   arrangement view scoped to one activation. Shares the ruler/scaling
   primitive from round-1's arrangement; segments are colored,
   drag-resizable, click-to-assign. Build once, reuse.
2. **`Select` with inheritance metadata.** A `Select` that displays a
   secondary "↳ source" label when the current value matches an
   inheritance source. Lightweight wrapper over Mantine `Select`.
3. **`SegmentedHumanize` row.** A 4-up compact slider strip + seed
   input. Could just be plain `Group` + `Slider` + `NumberInput` +
   `ActionIcon`; no new primitive needed.
4. **Cell container.** A `Paper` with a left-edge color stripe (same
   primitive as the round-1 inspector header — generalize it).

---

## Port-time notes (carry-over + new)

These are for the Rust/Rinch port:

- **Variant schedule is `Vec<(BarRange, VariantId)>` in the model**
  (see `composition-model.md` and `section-variants.md`). The UI here
  hoists the "default" segment to a first-class visual range; in the
  data, that range is **absent** — uncovered ranges play the pattern's
  `default_variant`. Don't introduce a phantom "default" entry in the
  binding; compute the default fill at render time.
- **`voicingFromRole` / `octaveFromRole` are display-only flags** in the
  fixture. The Rust binding doesn't need them — the component should
  compare the activation's realization value against
  `Project.tracks[ti].role`'s default. If they match, render the `↳
  role:` tag; if not, render the override mark.
- **Inheritance asterisks are computed, not stored.** Same rule as
  round-1's `inherit` state — it's a derived UI signal.
- **Humanization seed is `u64` (per `realization.md`).** Display
  as a 5-6 digit decimal in the fixture for readability; engineering
  should expose the full `u64` or a base-36 short string in the real
  control.

---

## Out of scope for round 2 (still later rounds)

- Pattern editor internals (per-event voicing, OctaveSpec per note,
  NoteId visualization). Round 3.
- Piano-roll detail view (per-note overrides drill-in, provenance
  overlay). Round 3.
- Mixer view (drum multi-out routing, return tracks, plugin chain).
  Round 4.
- Instrument browser, sample browser. Round 5+.
- Tempo map editor. Later.
- Live-MIDI input integration. Engine round, not UI.
