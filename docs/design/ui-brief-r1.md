# rawdaw UI brief — round 1: main window at rest

Audience: claude.ai/design (HTML mockups). The output will be translated
into [Rinch](https://github.com/joeleaver/rinch) components.

This is the first design round. **Read [`ui-principles.md`](ui-principles.md)
first** — every choice in this brief flows from those principles. If a
principle and this brief conflict, the principle wins; flag the conflict.

Read [`overview.md`](overview.md), [`composition-model.md`](composition-model.md),
[`section-variants.md`](section-variants.md), and
[`chord-loops.md`](chord-loops.md) for domain context — vocabulary
(Section, Variant, Pattern, ChordLoop, Activation, NoteOverride), the
edit-once-propagate-everywhere semantics, and the Roman-numeral chord
language. The brief assumes you've internalized those.

---

## What this round is for

We want to **lock in the information hierarchy of the main window before
designing any individual editor**. Concretely, this round answers:

1. What lives on screen at rest when the user opens a project?
2. How do the four major regions (top bar, library, arrangement, inspector)
   share space — and which is the visual subject?
3. How does a section block in the arrangement *read* (chord context,
   variant, length, color identity)?
4. How is the chord-loop ribbon rendered on top of / underneath the section
   timeline so the user can see harmonic context at a glance?
5. What does the library panel look like at the level of one entry —
   enough to convey color identity, name, type, and drag-handle?
6. Where does the inspector sit, and how does it announce *which object*
   it's inspecting?

We are **not** designing the piano roll, the mixer, the section editor's
internals, plugin/instrument UI, transport details, or any modal flow yet.
Those are rounds 2+.

---

## Intent of the main window

When the user opens a project, the intent is:

- **Anchor.** They should immediately see the shape of their song — a
  named sequence of sections with their lengths, variants, and color
  identities — without having to scroll, click, or expand anything.
- **Harmonic context.** They should see, at a glance, what harmony is
  in play across the arrangement (the chord loops scheduled into each
  section). Roman-numeral primary; absolute chord names available on
  hover.
- **Vocabulary.** They should see the reusable library (Patterns, Chord
  Loops, Sections) as a peer of the timeline — visible, draggable, not
  hidden behind a button.
- **Quiet.** The chrome should recede. No glowing accents, no
  skeuomorphism. The user's structural content is the visual subject;
  the app's UI is the background.

When the user has clicked on something (a section block, a library
entry, a chord), the **inspector announces what is selected**, with the
object's color and name in its header, and offers the most common
edits.

---

## Layout: regions and their intent

A desktop window, target ~1600×900 minimum, scales up. The window has
four regions:

```
+------------------------------------------------------------------+
| Top bar — project identity + transport + key/tempo               |
+--------+---------------------------------------------+-----------+
|        |                                             |           |
| Library|              Arrangement                    | Inspector |
| (left  |              (centerpiece)                  |  (right   |
|  pane) |                                             |   pane)   |
|        |                                             |           |
|        |  - Section blocks lane                      |           |
|        |  - Chord-loop ribbon                        |           |
|        |  - Timeline ruler (bars)                    |           |
|        |                                             |           |
+--------+---------------------------------------------+-----------+
| (Optional) Detail strip — closed by default; opens to piano roll |
+------------------------------------------------------------------+
```

Region intents, in priority order:

### Arrangement (centerpiece)

**Intent.** Show the song's structure: named sections in time order,
their harmonic context, their variant assignments. This is the largest,
brightest region. Everything else exists to support it.

**Content.**

- **Timeline ruler** at the top of the region. Bars are the primary unit
  (1, 5, 9, 13… for a 4/4 song); beats are minor ticks; current playhead
  position is a thin vertical line. The ruler is SVG and scales with
  horizontal zoom (round 1: assume a fixed zoom; the controls live on the
  top bar but aren't the focus of this round).
- **Chord-loop ribbon** directly under the ruler. A horizontal band
  showing the Roman-degree chord symbols (`I  V  vi  IV  | I  V  vi  IV`)
  aligned to their bar positions. When a section has a chord loop
  assigned, the ribbon spans that section's bar range. Roman primary;
  absolute chord names (`C  G  Am  F`) appear in a smaller, dimmer line
  beneath each symbol — always visible if there's room, otherwise
  surfaced on hover.
- **Section lane.** One horizontal lane (round 1; multi-lane sub-tracks
  are a later round) populated with **section blocks** placed at their
  start bars, with widths proportional to their duration.

**Section block design — this is the most important visual primitive
in the app.**

A section block carries:

- A 3-pixel **color stripe** on its left edge: the section's stable
  object-identity color.
- A **name** ("verse", "chorus", "bridge"), 13 px semibold, dark text
  on a light tint of the identity color.
- A **variant chip** in the top-right corner if the block uses a non-
  base variant ("variant: chorus-final"). Small, rounded, slightly
  inset.
- A **length readout** in the bottom-right corner ("4 bars"), tabular
  numerics, dimmer.
- A **subtle vertical line at each internal bar boundary** so the user
  can count bars without consulting the ruler — but light enough not to
  shout.
- **Hover state.** The block lifts slightly (subtle border, subtle
  background lift) and every other arrangement block that references
  the **same SectionId** also subtly highlights (intent: "show me where
  else this section is used"). Same for the linked chord loop ribbon.

Selection: clicking a section block selects it. The inspector on the
right updates. Selection state: a 1-px ring in the identity color, no
fill change.

### Library (left pane)

**Intent.** Reusable objects, always visible, drag-and-droppable into
the arrangement. Three categories that match the data model exactly:
**Patterns**, **Chord Loops**, **Sections**.

**Content.**

- A vertical stack with three collapsible groups. Each group's header
  shows the type name and a count: "Patterns (12)".
- Each entry in a group is one row:
  - A small color swatch (the object's identity color)
  - The object's name ("bass-main", "verse-progression", "verse")
  - A meta line below (small, dimmer): for Patterns, the kind +
    variant count ("Pitched · 2 variants"); for Chord Loops, the length
    ("4 bars"); for Sections, the base duration ("4 bars").
  - A drag handle is implicit — the whole row is grabbable.
- A search input at the top of the library, filters all three groups
  simultaneously.
- At the bottom of each group, a small `+` button to create a new
  object of that type. (Behavior is round-2; show the affordance.)

Width: ~260 px default, resizable, collapsible to a 36-px icon strip.

### Inspector (right pane)

**Intent.** Show what's selected; offer the most common edits without
opening a modal.

**Content for round 1 — show the inspector with a section block
selected** (the most-common case).

- Header: the section's identity color stripe + name + variant chip,
  matching the block design.
- A **variant tab strip** (`base`, `stripped`, `chorus-final`, …) — the
  user clicks a tab to switch which variant the selected SectionRef
  uses.
- Below the tabs, a vertical form:
  - **Duration.** Numeric input + "bars" unit.
  - **Scale override.** Dropdown ("Inherit project key" by default).
  - **Chord loops.** A small list of `(BarRange, ChordLoopRef)` pairs,
    with a `+ add chord loop` button.
  - **Activations.** A compact table: row per track, column for
    pattern, column for state (active / silent / inherit). Round 1:
    sketch the table at low fidelity — the activation matrix gets its
    own design round.
- A footer area with secondary actions: "duplicate section", "open in
  editor".

When nothing is selected, the inspector shows a placeholder ("Select a
section, pattern, or chord to inspect.") — keep it quiet, not animated,
not a marketing card.

Width: ~320 px default, resizable, collapsible.

### Top bar

**Intent.** Project identity (so the user knows what they're in) and
the global controls that have to be available always (transport,
tempo, key).

**Content.**

- Left: project name (editable on click). Beneath it in a smaller
  dimmer line: the project's default key and time signature
  ("C major · 4/4").
- Center: transport controls — play, stop, return-to-zero, record-toggle
  (record is gray/disabled in v1; we don't record audio, but the
  affordance signals "this is where playback control lives"). A
  playhead position readout in tabular numerics ("Bar 5 · Beat 2").
- Right: tempo (BPM, editable), then a compact zoom-out / zoom-in pair
  for the arrangement, then an overflow menu (gear icon) for project
  settings.

Height: ~48 px.

### Bottom detail strip (optional)

**Intent.** A future drill-down (piano roll, pattern editor) lives
here when invoked. Round 1: show the *collapsed state* — a thin
1-row bar at the bottom of the window with a label like "No detail
view open" and a small chevron to expand. Don't design the expanded
state in this round.

Height (collapsed): ~32 px.

---

## Visual identity

**Theme.** Dark default. Background: near-black gray (`#0F1115`-ish).
Elevated surfaces: slightly lighter tint. Borders: 1 px in a desaturated
mid-gray. Text: high-contrast off-white for primary, ~60% opacity for
secondary, ~40% for tertiary / metadata.

**Object-identity colors.** A palette of ~10 hues, saturation moderate,
not neon. Each library object owns one (assigned deterministically by
name hash in v1; user-editable later). Identity color appears as the
left stripe of section blocks, the swatch in the library, the chip in
chord-loop ribbon entries that derive from a named chord loop, and the
hover/selection ring on any reference. Identity color is **never used
for status** (selected, hovered, errored, muted) — those use a separate
neutral or semantic palette.

**Typography.** Single sans-serif font family. Tabular numerics for
all bar / beat / sample / time displays. Roman numerals in chord
contexts may use a slightly weighted small-caps treatment to be more
distinct from running text — but stay legible.

**Icons.** Tabler line icons (Rinch ships with these).

---

## Reference points

The user's DAW background is **Ableton, Cakewalk, GarageBand**. Lean
into:

- **Ableton's** clip-grid color identity discipline (objects keep their
  color across views).
- **Cakewalk's** comfortable density and bar-numbered arrangement
  ruler.
- **GarageBand's** restraint with chrome — the user's content reads
  more loudly than the app's UI.

Avoid:

- Logic / Pro Tools / Reaper piano-roll-first layouts.
- Plugin-style aesthetics (knobs, faders, metal, glow).
- Web-app marketing aesthetics (gradients, hero areas, oversized cards).

---

## Rinch component reality

Rinch has Mantine-style primitives: Stack, Group, Container, Tabs,
Accordion, TextInput, NumberInput, Select, Card, Paper, Divider,
Tooltip, Modal, Drawer, Popover, ContextMenu, VirtualList, Tree,
Tabler icons, Theme provider with dark/light. Use these in spirit;
the design output is HTML/CSS but components should map cleanly back
to Rinch.

Rinch does **not** ship with the following — we'll build these on top.
Design *as if they exist* (we want the design to drive the build), but
flag any place a mockup leans on one of these so we know it'll require
custom work:

- Timeline ruler with bar/beat ticks.
- Draggable + resizable arrangement block.
- Splitter / resizable / dockable panes (only `FloatingPanel` exists).
- Piano-roll grid.
- Knob / rotary (Slider is the only continuous input).
- VU meter / waveform display.

SVG is first-class in Rinch (paths, rects, circles, lines painted via
Vello). Section blocks, chord-loop ribbon, timeline ruler should all be
mocked up as concrete SVG geometry so the conversion is direct.

---

## Output we want from this round

For Claude Design specifically:

1. One **HTML mockup of the main window at rest**, dark theme, with
   placeholder content that reflects the tiny-project fixture (verse /
   stripped-verse / verse arrangement; I-V-vi-IV chord loop; bass +
   lead + drums tracks; a small library of 3 patterns + 1 chord loop +
   1 section).
2. A **second variant of the same window** with a section block
   selected — the inspector populated, the linked section instances
   subtly highlighted across the arrangement, the chord-loop ribbon
   showing the section's harmony.
3. (Optional, if it fits) a **third variant** showing the library with
   the user mid-drag of a pattern onto an empty section slot — to
   probe the drag-affordance design.

Annotations welcome. Where a decision is debatable (e.g. where exactly
the chord-loop ribbon sits — above the section lane, below the ruler,
overlaid on the blocks), produce one strong recommendation and call out
the alternative inline.

---

## Out of scope for round 1

- Piano-roll detail editor.
- Section editor's internal layout (the inspector sketch here is enough
  for now).
- Pattern editor.
- Mixer.
- Plugin / instrument browser.
- Transport details beyond play/stop/zero.
- Settings, preferences, project-wide configuration.
- Onboarding, empty states for a brand-new project.
- Keyboard shortcut overlay.
- Right-click context menus (a separate round).
- Light theme (designed-in via tokens; not mocked in round 1).
