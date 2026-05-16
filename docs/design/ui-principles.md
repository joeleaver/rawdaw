# rawdaw UI principles

Durable. Every design round inherits these. If a mockup contradicts one of
these principles, the principle wins unless we explicitly revisit it here.

The principles lead with **intent** — what the user should feel, understand,
or be able to do. Visual treatment follows from intent, not the other way
around.

---

## 1. Structural before notes

**Intent.** The user composes top-down: key → form → chord loops → derived
melodies. The UI must reward that flow. When the app opens, the user should
see *the shape of their song*, not a piano roll.

**Therefore.** The centerpiece is the arrangement view: named sections,
their chord context, their pattern activations. The piano roll exists, but
as a drill-down detail view that the user opens deliberately when they
want to inspect or pin per-note overrides — never as the default surface.

**We won't.** Replicate Logic / Pro Tools / Reaper, where the piano-roll
or per-track-clip lane is the visual centerpiece and structure is a thin
overlay on top.

## 2. Edit once, propagate everywhere

**Intent.** The whole point of rawdaw's data model is that a chord change
in a chord loop, or a tweak to a drum pattern, propagates to every
section/clip that references it. The UI must make this visible — the user
should always be able to see "what does this change affect?" before they
make it, and "where else did this come from?" after they touch a derived
result.

**Therefore.** Linked objects (a `PatternRef`, a `ChordLoopRef`, a
`SectionRef`) carry a consistent visual treatment that signals "this is a
reference, not a copy". Hovering or selecting a derived note in the piano
roll shows the source (which pattern event + which chord at that moment +
which override, if any). Editing a derived note offers two paths:
**pin as per-note override** (default) or **fork the activation into a
unique pattern** (explicit). Never silently copy-on-edit.

**We won't.** Use Ableton-style linked clips (which propagate literal
notes), or any visual treatment that makes references and copies look the
same.

## 3. Roman numerals primary, absolute on demand

**Intent.** The user thinks in functional harmony — I, V, vi, IV, V/V,
bVII. Absolute pitches (C, G, Am, F) are derived from the key. The UI
should let the user type and see the harmony the way they think about it
and surface absolute pitches only when explicitly requested (a hover, a
mode toggle).

**Therefore.** Chord-loop entry is text-first in Roman shorthand (`I`,
`V/V`, `b6`, `M7`, `/3`). The "Realized" preview strip shows the absolute
chord names beneath the symbolic ones, live, as the key changes.

**We won't.** Make the user click through a chord-quality dropdown +
root-note picker for every chord. Banner-style chord-name displays
(`C – G – Am – F`) appear only as derived previews, never as the
authoring surface.

## 4. Variants are tabs, not tree nodes

**Intent.** A "chorus" and a "chorus-final" share most of their structure
and differ in a small, knowable set of ways (drums dropped, one extra
build bar). The user thinks of them as two faces of the same thing. The
UI should reinforce that.

**Therefore.** Inside a section editor, variants live in a tab strip
(base + named variants). Each tab shows the same form, with fields
visually marked "inherited from base" vs. "overridden in this variant".
A revert-to-base affordance restores inheritance. Arrangement blocks
referencing a variant show the variant name as a small chip on the
block.

**We won't.** Hide variants in a tree view, a sub-modal, or a separate
window. Avoid any model that suggests variants are deeply nested
(variants-of-variants are explicitly out of scope; the data model is
flat).

## 5. Object identity is durable and visible

**Intent.** Sections, chord loops, patterns are *named reusable objects*.
The user should recognize them across views without re-reading labels —
"that's the same verse-progression I used in the bridge" should be a
glance, not a check.

**Therefore.** Each library object owns a stable color (derived
deterministically from its name or a user-assigned hue). The color
appears as a left stripe on the arrangement block, a chip in the
library, the header band of the pattern's piano-roll view, and the
hover-highlight when the user mouses over any reference to it.

**We won't.** Re-color objects across views; depend on icons alone to
identify objects; use color for status (selected, errored) in a way that
collides with object-identity color.

## 6. The library is a peer of the timeline, not a popup

**Intent.** Reusable patterns, chord loops, and section templates are
the working vocabulary of the composition. The user needs them visible
and dragable into the arrangement without breaking flow.

**Therefore.** The library is a side-docked panel (left of the
arrangement), always visible by default, with three lists: Patterns,
Chord Loops, Sections. Each item drag-drops onto a slot or a section
block. Filter / search is local to each list. Library items can be
collapsed but not modal.

**We won't.** Hide the library behind a button, a menu, or a tab that
the user has to find. Modal pickers ("choose a pattern…") are a
last-resort fallback for inspector-only contexts.

## 7. Density appropriate to a desktop pro tool

**Intent.** The user has used Ableton, Cakewalk, GarageBand. Comfortable
with information-dense screens but expects the typography and spacing
of 2026, not 2010. The default density is "more compact than
GarageBand, less compact than Ableton, breathable enough to scan."

**Therefore.** Default body text at 13–14 px; section labels at 12–13
px; numerics tabular. Padding inside controls is small but consistent
(8 px). Major panels separated by 1px borders or 2 px gutters, not
heavy shadows.

**We won't.** Skeuomorphic faders, heavy beveling, glowing neon. No
"plugin-style" aesthetic. The DAW chrome is functional and quiet so
the user's content (their song structure) is the visual subject.

## 8. Dark default, light supported

**Intent.** Composing happens in long sessions, often at night. Dark
default matches user expectation from every DAW they've used. Light
theme is offered for daytime work and screenshots.

**Therefore.** All mockups use a dark palette by default. The Rinch
theme system supports light/dark switching via tokens; design decisions
should pass through both. No view depends on a specific theme for its
information content.

**We won't.** Hardcode hex values that defeat the theme. Solid-black
backgrounds (`#000`); the dark base is a near-black gray that lets
elevated surfaces gain contrast through subtle lighter tints.

## 9. Use SVG for content geometry, not GIF tricks

**Intent.** Chord blocks, arrangement section blocks, the timeline
ruler, automation curves, drum-step cells, the piano-roll grid — these
are content geometry that scales, animates, hit-tests, and accepts
drag. They are not decorations.

**Therefore.** Mockups draw these regions as concrete SVG-style
geometry (rectangles, ticks, curves) with semantic class names. Rinch
paints SVG as first-class output through Vello, so anything mocked-up
as SVG translates directly. Don't use raster images for content
geometry.

**We won't.** Bitmap waveforms, raster piano-roll grids, or
emoji-as-icon. Icons are line-icons (Tabler set, available in Rinch).

## 10. Speak the model's vocabulary

**Intent.** The data model's nouns and verbs (Section, Variant,
ChordLoop, Pattern, Activation, NoteOverride) are not jargon — they are
the user's actual mental model. Renaming them in the UI to be "friendly"
loses the affordance that the user is thinking about the same thing the
software is thinking about.

**Therefore.** UI labels match the model: a section is "section", a
variant chip says "variant: chorus-final", an override badge says
"pinned" (the user-facing term for `per_note_overrides`). Tooltips can
add gentle clarification but the primary label is the technical term.

**We won't.** Invent marketing-style names ("scenes", "moments",
"blocks") that diverge from the data model. We won't dumb down "Roman
degree" into "chord number".
