# Chord Loops

Chord loops are named, reusable harmonic sequences. They live in the project library; sections reference them. A single edit propagates everywhere the loop is used. This doc covers the chord vocabulary, the functional-vs-absolute model, secondary dominants/borrowing/modulation, slash chords, and the section-level scheduling.

## Functional, not absolute (by default)

A chord is represented as a **functional Roman-numeral degree** in the current key. Absolute (pitch-class) representation is an escape hatch for the rare non-functional case.

Reasons:

- The composition workflow is key-first: a section has a tonal center, chord loops live inside sections.
- Changing the section's key transposes the loop automatically — no manual rewrite.
- Most progressions in practice are functional. Non-functional chord planing, jazz substitutions, and modernist harmony are minorities; `Absolute` events handle them.

**90% UX target:** in normal use, the user picks a global project key once, then composes entirely in Roman numerals. Absolute chord input is available but hidden behind a mode toggle / keyboard shortcut. The editor displays Roman numerals as the primary label; absolute (pitch-class) is shown on hover or as a small secondary line.

## Data model

```rust
struct ChordLoop {
    id: ChordLoopId,
    name: String,
    length: Duration,              // total length in bars/beats
    key: Option<Scale>,            // None = interpret in section's scale (default)
    events: Vec<ChordEvent>,
}

struct ChordEvent {
    time: MusicalTime,
    duration: Duration,
    chord: ChordSpec,
    bass: Option<BassSpec>,
    annotation: Option<Annotation>,
}

enum ChordSpec {
    Functional {
        roman: RomanDegree,        // I..VII with optional accidental (♭III, ♯IV, etc.)
        quality: ChordQuality,
        extensions: Vec<Extension>,
        alterations: Vec<Alteration>,
        in_key: Option<Scale>,     // override tonal center for this chord
    },
    Absolute {
        root: PitchClass,
        quality: ChordQuality,
        extensions: Vec<Extension>,
        alterations: Vec<Alteration>,
    },
}

enum RomanDegree {
    // Diatonic degrees; case-by-quality convention is rendering-only
    I, II, III, IV, V, VI, VII,
    // Chromatic / borrowed
    FlatII, FlatIII, FlatV, FlatVI, FlatVII,
    SharpI, SharpII, SharpIV, SharpV, SharpVI,
}

enum BassSpec {
    Inversion(u8),                 // 1st, 2nd, 3rd inversion (in-chord bass)
    ChordDegree(ChordDegree),      // chord tone in bass (root/3/5/7/9)
    ScaleDegree(ScaleDegree),      // scale tone not in chord (Cmaj/D)
    Absolute(PitchClass),          // explicit pitch class (Cmaj/D♯)
}

struct Annotation {
    cadence: Option<CadenceTag>,   // PAC, IAC, HC, deceptive, plagal, none
    comment: Option<String>,
}
```

The chord loop's `key` is usually `None` — the loop floats with the section's scale. Set it only when the loop is rigidly tied to a specific tonal center (rare; mostly useful for shared progressions that originated in a specific key and should be preserved if pasted into a different-key section).

## Chord vocabulary

```rust
enum ChordQuality {
    Major, Minor, Diminished, Augmented,
    Major7, Minor7, Dominant7, Diminished7, HalfDiminished7, MinorMajor7, AugmentedDom7,
    Major6, Minor6,
    Sus2, Sus4, Sus7, Sus9,
    Power,                         // root + 5th
    Custom { intervals: Vec<Interval> },  // semitones from root — escape hatch
}

enum Extension {
    Add9, Add11, Add13,
    Ninth, Eleventh, Thirteenth,   // implies the 7th and earlier extensions
}

enum Alteration {
    Flat5, Sharp5,
    Flat9, Sharp9,
    Sharp11, Flat13,
    NoFifth, NoThird,
}
```

This covers common pop/rock/jazz vocabulary. The combination of `quality + extensions + alterations` is expressive: `Csus2add9`, `Calt` (`Dominant7 + [Sharp9, Flat13]`), `Cmaj7♯11`, etc.

`Custom` is the escape hatch for unusual sonorities (whole-tone, quartal, planed clusters). We can grow the standard enum entries over time without breaking existing project data.

## Secondary dominants, modal interchange, modulation: `in_key`

All three are handled by the same `in_key` mechanism on `Functional` chord events. The chord's Roman degree is interpreted in `in_key` instead of the section's scale.

| Phenomenon          | Example (in C major)              | Encoding                                                         |
|---------------------|------------------------------------|------------------------------------------------------------------|
| Secondary dominant  | V/V (D⁷ resolving to G)            | `Functional { roman: V, quality: Dom7, in_key: Some(G major) }`  |
| Secondary dominant  | V/vi (E⁷ resolving to Am)          | `Functional { roman: V, quality: Dom7, in_key: Some(A minor) }`  |
| Modal interchange   | borrowed iv (F minor)              | `Functional { roman: IV, quality: Minor, in_key: Some(C minor) }` |
| Modal interchange   | ♭VI (A♭ major)                     | `Functional { roman: FlatVI, quality: Major }` (no in_key needed) |
| Brief tonicization  | ii-V-I to a new key                | each chord in the passage carries `in_key: Some(new_key)`        |
| Persistent modulation (chord-level only) | progression resolves in new key | events from modulation point onward carry `in_key: Some(new_key)` |

`FlatVI`, `FlatVII`, etc. as direct `RomanDegree` values handle chromatic-but-conventional borrowings without needing `in_key`. Use `in_key` when the harmonic context is genuinely a temporary key center.

**Note:** `in_key` affects chord-level interpretation only. The *section's scale* (used by `ScaleDegree` pattern events for melodies) is unchanged. If a melody should also re-key for a modulating passage, the user should split the section. (Section-internal scale ranges are a v2 direction — see `open-questions.md`.)

## Slash chords / inversions

`BassSpec` is independent of `ChordSpec` — the upper structure and bass note can be specified separately. This covers:

- **Standard inversions**: `Inversion(1)` for first inversion (3rd in bass). Identical to `ChordDegree(Third)`.
- **Chord-tone bass**: `ChordDegree(Seventh)` for third inversion of a 7th chord. Equivalent forms allowed.
- **Functional slashes**: `ScaleDegree(2)` for Cmaj/D — bass is a scale tone not in the chord.
- **Foreign slashes**: `Absolute(DSharp)` for Cmaj/D♯ — bass is a specific non-scale pitch.

If `bass` is `None`, the bass note is the chord's root in root position.

## No voicing hints on chord events

Chord events specify *harmony*, not *arrangement*. Voicing strategy (close, drop2, shell, rootless, etc.) belongs to the realization side. Precedence at realization time:

1. Pattern event's voicing strategy (per-event).
2. Activation entry's default voicing.
3. Track role's default voicing.
4. Realization fallback (`triad-close` for triads, `four-way-close` for 7th-bearing chords).

Keeping chord events pure-harmonic means swapping a chord loop into a different arrangement doesn't drag voicing assumptions with it.

## Section-level scheduling

Sections reference chord loops via a sequence:

```rust
struct SectionBody {
    chord_loops: Vec<(BarRange, ChordLoopRef)>,
    // ...
}
```

- **Single-loop section** (the common case): `[(0..section_length, loop_a)]`. Loop auto-repeats to fill the range.
- **Multi-loop section**: `[(0..8, verse_prog), (8..12, pre_chorus_prog), (12..16, chorus_tease_prog)]`. Each loop plays for its assigned range, looping internally if shorter.
- **Uncovered ranges** play no chord events. Chord-degree-dependent pattern events realize as silent in those ranges (with a UI warning when the user is editing).

Ranges do not overlap; layered/polychordal chord stacking is not supported in v1.

## Harmonic rhythm

Implicit, not explicit. The chord loop's events are at arbitrary positions; events can be spaced at bar-rate, half-bar, beat-rate, eighth-rate, or irregularly. No special "harmonic rhythm" parameter — the user's event spacing determines it.

## Annotations: cadence tags and comments

```rust
enum CadenceTag {
    PerfectAuthentic,    // V → I, both root position, soprano on tonic
    ImperfectAuthentic,  // V → I, weaker resolution
    Half,                // ends on V
    Plagal,              // IV → I
    Deceptive,           // V → vi (or other deceptive resolution)
    Phrygian,            // iv6 → V in minor
}
```

Cadence tags are optional and not load-bearing in v1. Uses:

- **Drum-fill placement hints** — fills tend to land just before authentic cadences.
- **Variant suggestion** — if a loop is marked with an `Authentic` cadence, the editor can offer a `final` variant of the chord loop or section.
- **Visual labeling** in the chord-loop editor.

Comments are for the user's own notes.

## Chord-loop editor UX

- Timeline of bars/beats, scaled to the loop's `length`.
- Chord events as labeled blocks. **Primary label is Roman numeral** (`V⁷`, `♭VI`, `ii⁷/V`). Absolute pitch (`G⁷`, `A♭`, `Am⁷`) shown on hover or as a small secondary line beneath when expanded.
- Quick-entry palette and keyboard shortcuts. Typing into an event accepts shorthand:
  - `1`, `4`, `5`, `6` etc. for diatonic degrees (case-sensitive: `1` = I major in major key, `6` = vi minor in major key).
  - `5/5` for V of V (entering `in_key` automatically).
  - `b6`, `#4`, etc. for chromatic degrees.
  - `7`, `M7`, `m7`, `°7`, `ø7`, `+7`, `sus4`, `add9`, etc. as quality/extension/alteration suffixes.
  - `/3`, `/5`, `/7` for inversions; `/[note]` for explicit bass.
- A "Realized" strip beneath the chord lane shows the concrete pitches each chord becomes in the current section's key. Updates live as the key changes.
- An inspector pane edits the focused chord's quality, extensions, alterations, `in_key`, bass, annotation.
- Mode toggle for `Absolute` vs `Functional` event types. Default is functional; absolute is an explicit mode the user opts into for a chord.

## Project-level key

A project carries a default key (default tonal center). This is the "global key" — the 90%-case starting point. Sections inherit from project unless overridden; chord loops interpret in the section's effective key unless their own `key` is set.

```rust
struct Project {
    default_key: Scale,
    // ...
}
```

When the user creates a project, the first step in the empty-project UX is "pick your key." From there, all chord input is Roman-numeral by default.

## Realization interaction

The realization pass (see `realization.md`) uses chord events to resolve `ChordDegree` pattern events and chord-block voicings. It looks up the chord *in effect at the event's start time*:

1. Find the section overlapping the event's time.
2. Find the `(BarRange, ChordLoopRef)` covering that time within the section.
3. Within the chord loop, find the chord event at the event's start time (modulo loop length).
4. Resolve the chord against `in_key` (if set) or the section's scale.
5. Apply the bass override if any.
6. Pass the resolved chord (root pitch, quality, intervals, bass) to the pitched-event resolver.

Re-evaluation across chord changes during a held note follows the rule in `realization.md`: events are interpreted at their start time only.

## Resolved questions

- **Functional vs. absolute representation?** Functional as canonical, absolute as escape hatch. 90% of UX shows only Roman numerals.
- **Secondary dominants / borrowing / modulation?** Single `in_key` mechanism handles all three.
- **Slash chords / inversions?** Independent `BassSpec` with four kinds: inversion, chord-degree, scale-degree, absolute.
- **Voicing hints on chord events?** No — voicing belongs to realization (pattern/activation/role).
- **Section-internal modulation (scale ranges)?** Deferred to v2; split the section for v1.
- **Multiple chord loops per section?** Yes, sequential via `Vec<(BarRange, ChordLoopRef)>`. No layering.
- **Harmonic rhythm?** Implicit in chord event spacing.

## Open questions

- **Roman shorthand grammar.** The quick-entry parser needs a precise grammar (`5/5`, `bVImaj7add9`, `Vsus4/3`). Specify before the chord-loop editor is built.
- **Chord-loop variants (deferred from earlier).** Symmetric with pattern/section variants. Skipped in v1; revisit if cloning chord loops becomes painful.
- **Chord-loop transposition relative to its declared `key`.** If a chord loop has its own `key` set and is placed in a section whose scale differs, do the loop's chords transpose to the section's key, or stay in the loop's key? Probably stay in the loop's key (that's why it's set), but the rendered concrete pitches will be "wrong" relative to the section's scale — needs UI clarity.
- **Display preference for chord names.** Some users prefer Nashville Number System over classical Roman numerals. Could be a render-only setting; data model is unchanged.
