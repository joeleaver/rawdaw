# rawdaw — main window mockups · round 1

HTML mockup of the **main window at rest** + **with a section selected**,
implementing the brief in [`docs/design/ui-brief-r1.md`](../../docs/design/ui-brief-r1.md).
This package exists so the design intent is reviewable side-by-side with
the engineering work and so a Rinch port has a concrete, line-by-line
target.

**Round 1 covers only the information hierarchy of the main window**
(top bar, library, arrangement, inspector, bottom strip). The piano roll,
section editor internals, mixer, plugin browser, and modals are explicitly
deferred to later rounds.

---

## Quick start

Open `main-window.html` in any modern browser — no build, no install.
The two artboards (`A · At rest`, `B · Verse (base) selected`) sit on a
pannable design canvas; the **expand icon on each artboard header** opens
it fullscreen for a 1:1 review.

```
mockups/round-1/
├── README.md                       ← you are here
├── main-window.html                ← entry point (open this)
└── components/
    ├── data.js                     ← fixture (project / sections / patterns / chord loops / tracks)
    ├── parts.jsx                   ← shared bits — Icon, Pill, Roman, rgba()
    ├── topbar.jsx                  ← TopBar
    ├── library.jsx                 ← Library + Group + LibraryRow
    ├── arrangement.jsx             ← Arrangement, Ruler, ChordRibbon, SectionLane, SectionBlock
    ├── inspector.jsx               ← Inspector, VariantTabs, ActivationTable, …
    ├── main-window.jsx             ← MainWindow + BottomDetailStrip (composition root)
    └── design-canvas.jsx           ← starter — pan/zoom canvas for the two artboards
```

The component files are deliberately small (≪ 700-line cap) and split by
region so each one maps to one or two Rinch components.

---

## What's mocked, at a glance

| Region        | File                | Status                                                              |
|---------------|---------------------|---------------------------------------------------------------------|
| Top bar       | `topbar.jsx`        | Final intent for round 1. Real transport wiring is round 2.         |
| Library       | `library.jsx`       | Final intent. Drag affordance not shown (deferred per user).        |
| Arrangement   | `arrangement.jsx`   | Final intent: ruler + chord-loop ribbon + section lane. **One lane** — multi-lane sub-tracks are a later round (faint hint shown). |
| Inspector     | `inspector.jsx`     | Final intent for round 1. Activation table is **low-fidelity** by brief — it gets its own round. |
| Bottom strip  | `main-window.jsx`   | Collapsed-state only. Expanded piano roll is round 2+.              |

---

## Decisions made (where the brief left choices open)

These are the calls I made in round 1. Each is reversible — but defaulting
to one keeps the mockup buildable.

1. **Chord-loop ribbon placement → own row, between ruler and section lane.**
   Alternative considered: overlaying chord labels on the section blocks.
   Rejected because chord density and section length aren't always 1:1
   (a 4-bar loop in an 8-bar section repeats; multi-loop sections have
   uneven cells), and overlay collisions get ugly fast. Keeping it as a
   distinct horizontal band leaves the section block free to carry name,
   variant, length, and internal bar lines without competing for space.

2. **Section block fill → soft tint of identity color + 3 px left stripe
   + 1 px ring on selection.** Quietest of the three options we kicked
   around. The block reads as "the user's content," not as decorated
   chrome.

3. **Ribbon density → Roman primary (slightly larger) + absolute pitch
   on a smaller, dimmer line beneath, always visible.** At round-1 zoom
   (24 bars across ~1020 px) there's always room. When horizontal zoom
   tightens past a threshold the absolute line should hide; surface on
   hover.

4. **Identity palette → earthy / muted (the first option in the picker).**
   ~10 hues, moderate saturation; never used for status. See `palette` in
   `data.js`. Object-color assignments are hand-set in the fixture; in
   v1 they should be derived deterministically by name hash with manual
   override later.

5. **Case-by-quality Roman convention preserved in rendering.** The
   `<Roman>` helper uses heavier weight + tracked letter-spacing only —
   **no small-caps** — so a lowercase `vi` reads as minor and an
   uppercase `IV` reads as major at a glance. This is load-bearing per
   `composition-model.md`; a small-caps treatment would erase the
   distinction.

6. **Linked highlighting on selection.** Clicking a `SectionRef` lifts
   every other arrangement block pointing at the **same `SectionId`**
   (subtle background tint + identity-color ring), and emphasizes the
   chord-loop ribbon cells over those bars. Reinforces "edit once,
   propagate everywhere."

7. **Variant chip omitted when the block uses the section's default
   variant.** So `verse / base` shows nothing special and `verse /
   stripped` carries the chip — the deviation is what's worth shouting.

8. **Bar boundary lines inside section blocks** use the section's
   identity color at low alpha so the user can count bars without
   consulting the ruler. Subtle enough not to compete with the block
   itself.

9. **Inspector header carries the same color stripe + name treatment
   as the selected block,** plus a small "N instances in arrangement"
   readout and a bar range. This is the most important "what is
   selected" signal in the UI.

10. **`status` state on activation rows** uses a separate sage-green /
    gray palette (`active` / `silent` / `inherit`) — never identity
    colors — to honor "color is for identity, not status."

11. **Unified inheritance / override signal: asterisk + tooltip.** Any
    inspector field or activation row that deviates from base in the
    current variant carries a single asterisk with a `title=` tooltip
    explaining the relationship. The form-level italic hint that used
    to live under inherited fields is gone — one signal everywhere.

12. **`Duplicate placement`** (not just "Duplicate") in the inspector
    footer. The button operates on the selected `SectionRef` instance,
    not on the section template. Duplicating the template (creating a
    new entry in the library) is a library-side action and belongs in
    that surface.

---

## Map to Rinch primitives

Rinch ships Mantine-style primitives. The mockup leans on these where
they map cleanly, and flags the gaps inline. Below is the explicit
crosswalk so the Rinch port has no guessing.

| Mockup element                          | Rinch primitive (or "GAP")                          |
|-----------------------------------------|-----------------------------------------------------|
| Top bar layout                          | `Group`, `Stack`, `Paper`                           |
| Project name / playhead / BPM readouts  | `TextInput` (editable), `Text` w/ tabular numerics  |
| Transport buttons                       | `ActionIcon` with Tabler icons                      |
| Library panel root                      | `Paper` + `Stack` (left-docked)                     |
| Library groups (collapsible)            | `Accordion`                                         |
| Library row (color swatch + name + meta)| Custom row inside `Group`; Rinch ships `Tree` but a flat row reads better here |
| Library search                          | `TextInput` w/ left section icon                    |
| **Timeline ruler with bar/beat ticks**  | **GAP — custom SVG (Vello-paint-ready)**            |
| **Chord-loop ribbon cells**             | **GAP — custom SVG**                                |
| **Draggable, resizable arrangement block** | **GAP — custom drag layer; the block visual is `Paper`-ish but the interaction model is custom** |
| Section block (visual)                  | `Paper` styled or raw `<div>` w/ SVG inside         |
| Variant chip on a block                 | `Badge` (small)                                     |
| Playhead                                | Custom 1 px line + caret triangle, sibling of ruler |
| Inspector header                        | `Paper` w/ a colored left border                    |
| Variant tab strip                       | `Tabs`                                              |
| Inspector form fields                   | `NumberInput`, `Select`, `TextInput`                |
| Chord-loop row in inspector             | Custom row; uses `Badge`-like swatch                |
| Activation table                        | Plain grid for round 1; `Table` is overkill         |
| State pill (active / silent / inherit)  | `Badge` variants                                    |
| Bottom detail strip                     | `Paper` + chevron `ActionIcon`                      |
| Pane splitters                          | **GAP — `FloatingPanel` exists; resizable/dockable splitters do not** |

### Rinch gaps (priority-ordered for the port)

1. **Timeline ruler** — SVG primitive with configurable major/minor tick
   cadence, tabular-numeric label rendering, scroll-locked to the
   arrangement viewport.
2. **Arrangement block** — drag (move), resize (right edge), select,
   multi-select (later), hover-highlight-linked-instances. Visual is
   trivial; interaction is the work.
3. **Splitter / resizable panes** — library/arrangement and
   arrangement/inspector both need draggable splitters with
   collapse-to-strip behavior at min width.
4. **Chord-loop ribbon** — derives directly from the timeline ruler
   primitive; tile chord cells aligned to bars, tooltip on hover for
   absolute name when the small line is hidden at tight zooms.
5. **Piano-roll grid** (round 2) and **knob / rotary** (effects round)
   are flagged but out of scope here.

### Port-time notes

These don't change the visual; they only affect how the Rust binding /
Vello paint path should be written:

- **Bar / beat tick lines** are rendered as individual `<line>` elements
  in the mockup so React diffing stays trivial. In the Vello port,
  collapse them to a **single `<path>`** with `M`/`L` commands per tick;
  redraws on horizontal zoom stay cheap and one tessellation pass covers
  the whole ruler.
- **`inherit` is a display state, not a data variant.** The data model
  has `ActivationOverride::{ Replace(ActivationEntry), Silent }` and
  "absent key means inherit." The mockup's activation table hoists
  `inherit` to a first-class third pill purely so the user sees something
  in the row — do **not** introduce a phantom `ActivationOverride::Inherit`
  variant in the Rust binding. The Rinch component should compute
  `inherit` from "no entry in the variant override map."

---

## Fixture data

`data.js` exports `window.RD` with the full round-1 fixture. The shape
mirrors `composition-model.md` and `section-variants.md` directly — it
should be straightforward to port to Rust structs for engine fixtures or
to use as a JSON test asset.

```text
RD.project       { name, key, timeSig, tempo, playhead: { bar, beat } }
RD.tracks        Track[]                       — 4 tracks: bass, lead, drums, pad
RD.patterns      { [name]: PatternMeta }       — 4 patterns
RD.chordLoops    { [name]: ChordLoop }         — 2 loops: verse-progression (I V vi IV),
                                                          chorus-progression (vi IV I V)
RD.sections      { [name]: Section }           — 3 sections: intro, verse, chorus
                                                 (verse has base + stripped variants)
RD.arrangement   SectionRef[]                  — 5 blocks across 24 bars
RD.totalBars     24
RD.tokens        { color tokens, font stacks, sizes }
RD.palette       { 10 identity hues }
```

**Why "stripped" is a variant of `verse` and not its own section.** The
brief calls out `verse / verse-stripped / verse` arrangement explicitly.
Modeling stripped as a sparse variant override on `verse` (silencing
bass + drums) demonstrates the inheritance machinery from
`section-variants.md`. In the arrangement, all three verse blocks point
at the same `SectionId`, and the linked-highlight in artboard B lights
all three.

---

## Annotations

Light callouts (yellow pins) are drawn outside the right edge of each
artboard, calling out the **debatable** choices (per the brief's "Where
a decision is debatable, produce one strong recommendation and call out
the alternative inline"). They are not part of the UI itself — strip
them when porting.

---

## Out of scope for round 1 (will land later)

- Piano-roll detail editor (round 2).
- Section editor internals — the inspector here is the sketch.
- Pattern editor.
- Mid-drag affordance — deferred per user; will revisit when the drag
  interaction model is being designed.
- Mixer / plugin browser / project settings.
- Light theme — tokens are in place but no mock.
- Empty-project state, onboarding, keyboard shortcut overlay.
- Right-click context menus.

---

## Conventions that should survive the port

- **Object identity color is durable and global.** Never re-color an
  object across views; never use identity color for status.
- **Roman numerals primary, absolute on demand.** The chord ribbon and
  any future chord-editor surface follow this. Case is load-bearing.
- **Variants are tabs, not tree nodes.** Inspector's variant strip is
  literal `Tabs`; arrangement blocks carry a variant chip when they
  deviate from default.
- **Library is a peer of the timeline, not a popup.** It's a docked
  panel; collapse to icon-strip is fine, hide behind a menu is not.
- **The chrome recedes.** Dark surfaces, 1 px lines, no shadows beyond
  micro-elevations. Identity colors are the only color on screen; status
  uses neutral / semantic.

If a Rinch port shortcut would violate one of these, surface it instead
of paving over it — the principles in `ui-principles.md` are the
arbiter.
