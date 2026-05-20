# Chord-shorthand grammar (v1)

This document specifies the quick-entry shorthand parsed by
`crate::chord_shorthand::parse` and serialized by
`crate::chord_shorthand::format`. It is the contract the parser
must honor — every example in the **Normative test table** at the
bottom round-trips; every malformed example is rejected.

The grammar is keyboard-friendly Roman / Nashville Number System
input for `rawdaw_model::chord::ChordSpec`. It lives in
`rawdaw-app` (CL0 design decision 7) and is not exposed to other
crates.

## Status

CL0 design decision 6 fixes the grammar before parser code lands.
This file is the canonical spec; if parser behavior diverges, the
spec is wrong, not the parser.

## Design decisions locked here

1. **Roman case carries quality default.** Uppercase Roman
   (`I`, `V`, `bVII`) defaults to major; lowercase Roman (`i`, `v`,
   `bvii`) defaults to minor. Arabic digits (`1`–`7`) default to
   major (Nashville convention favoring explicitness over
   key-context inference).

2. **`m` is redundant with lowercase case.** `v` and `vm` and `5m`
   all denote V-minor. `V` and `Vmaj` denote V-major. Explicit
   quality suffix wins over case if they disagree (`vM` → V-major).

3. **Slash semantics are letter-discriminated.**
   - `/<pitch-letter>` (A–G, optional `b`/`#`): `BassSpec::Absolute`.
   - Anything else after `/` (digit, Roman, with or without quality):
     sets `in_key` on the outer Functional chord. The slash content
     parses as a sub-chord whose root becomes the new tonic and
     whose quality default picks the mode.

   Consequence: there is **no shorthand for `BassSpec::Inversion`,
   `ChordDegree`, or `ScaleDegree` bass in v1**. Those are entered
   via the inspector's bass sub-editors. The design doc's earlier
   `/3 /5 /7` inversion shorthand is dropped from the parser; the
   inspector remains the source of truth for those bass kinds.

4. **Parser is key-aware.** `parse(input: &str, current_key: &Scale)
   -> Result<ChordSpec, ParseError>`. The `in_key` field of
   `ChordSpec::Functional` stores an absolute `Scale`; resolving
   `V/V` requires knowing the current tonic. The formatter is the
   inverse and takes the same `&Scale` for degree-relative
   rendering.

5. **Whitespace is ignored inside a chord token.** `bVI maj7` and
   `bVImaj7` parse identically. Spaces are not used as separators.

6. **Single-flat / single-sharp only.** Double-flat / double-sharp
   accidentals on degree tokens are out of scope for v1. Pitch-class
   tokens still accept `b`/`#` only (no `bb`/`##`).

7. **`Custom` quality and `Custom` mode are not addressable via
   shorthand.** They exist in the model for programmatic use; users
   enter them through the inspector.

## EBNF

```
ChordExpression  = Chord , [ "/" , SlashContent ] ;

Chord            = FunctionalChord | AbsoluteChord ;

FunctionalChord  = [ Accidental ] , DegreeToken , QualityBlock ;
AbsoluteChord    = PitchClass , QualityBlock ;

QualityBlock     = [ Quality ] , { Extension } , { Alteration } ;

DegreeToken      = RomanNumeral | DegreeDigit ;
RomanNumeral     = "I"  | "II"  | "III" | "IV"  | "V"  | "VI"  | "VII"
                 | "i"  | "ii"  | "iii" | "iv"  | "v"  | "vi"  | "vii" ;
DegreeDigit      = "1" | "2" | "3" | "4" | "5" | "6" | "7" ;

Accidental       = "b" | "#" ;
PitchLetter      = "A" | "B" | "C" | "D" | "E" | "F" | "G" ;
PitchClass       = PitchLetter , [ Accidental ] ;

Quality          = PlainQuality | SeventhQuality | SusQuality
                 | SixQuality   | PowerQuality   | DimAugQuality ;

PlainQuality     = "M"   | "maj" | "Maj" | "MAJ"        (* Major triad *)
                 | "m"   | "min" | "-"                  (* Minor triad *) ;

SeventhQuality   = "7"                                  (* Dominant7 *)
                 | "maj7" | "M7"   | "Δ"  | "Δ7"        (* Major7 *)
                 | "m7"   | "min7" | "-7"               (* Minor7 *)
                 | "mM7"  | "mMaj7" | "minMaj7" | "-Δ"  (* MinorMajor7 *)
                 | "dim7" | "°7"   | "o7"               (* Diminished7 *)
                 | "ø"    | "ø7"   | "m7b5" | "-7b5"
                   | "m7♭5"                             (* HalfDiminished7 *)
                 | "+7"   | "aug7"                      (* AugmentedDom7 *)
                 | "7sus4"                              (* Sus7 *) ;

SusQuality       = "sus"   | "sus4"                     (* Sus4 *)
                 | "sus2"                               (* Sus2 *)
                 | "sus9"                               (* Sus9 *) ;

SixQuality       = "6"                                  (* Major6 *)
                 | "m6"   | "min6" | "-6" ;             (* Minor6 *)

PowerQuality     = "5"                                  (* Power *) ;

DimAugQuality    = "dim"  | "°"    | "o"                (* Diminished *)
                 | "aug"  | "+" ;                       (* Augmented *)

Extension        = "add9"  | "add11" | "add13"
                 | "9"     | "11"    | "13" ;

Alteration       = "b5"  | "♭5"
                 | "#5"  | "♯5"
                 | "b9"  | "♭9"
                 | "#9"  | "♯9"
                 | "#11" | "♯11"
                 | "b13" | "♭13"
                 | "no3"
                 | "no5" ;

SlashContent     = PitchClass         (* → BassSpec::Absolute *)
                 | FunctionalChord ;  (* → in_key on outer chord *)
```

The EBNF is permissive: the parser accepts any ordering of
`Extension` and `Alteration` after `Quality` but **canonical
format** (what `format` emits) is `Extension*` first, then
`Alteration*`.

### Tokenizing ambiguities

- `5` standalone: V-major triad (degree).
- `V5`: V power chord (`PowerQuality` follows `DegreeToken`).
- `b6m7` → flat-VI degree + minor-7 quality. The `b` binds to `6`
  (it would be invalid as a quality lead).
- Pitch letter `B` is unambiguous because Roman tokens use only
  `I`/`V` letters.
- `M` vs `m`: case-sensitive. `M7` is Major7; `m7` is Minor7.
- `b` and `B`: `b` is the flat accidental; `B` is the pitch class.
  An absolute-mode token starting with `b` is invalid (pitch class
  `b` doesn't exist; users write `Bb` for B-flat).
- `Δ` (U+0394) is accepted as an alias for `maj7` when it follows
  a degree token. `°` (U+00B0) and `ø` (U+00F8) similarly accepted
  for diminished and half-diminished. Output prefers ASCII
  (`maj7`, `dim`, `ø` stays as `ø` since no ASCII single-glyph
  alias exists; output emits `m7b5` for half-diminished).

### Mode selection from `in_key`

When SlashContent parses as a FunctionalChord and sets `in_key`,
the new scale's `Mode` is derived from the sub-chord's quality
default:

| Sub-chord form        | Resulting mode               |
|-----------------------|------------------------------|
| Uppercase Roman / digit, major default | `Ionian`     |
| Lowercase Roman, minor default         | `Aeolian`    |
| Explicit `dim` / `°` quality           | `Locrian`    |
| Explicit `m` / minor quality on digit  | `Aeolian`    |
| Explicit `M` / `maj` quality on lowercase Roman | `Ionian` |

Extensions and alterations on the slash sub-chord are **ignored
for mode selection** in v1 — only the root degree and its quality
default determine the borrowed scale. (Future v2 may add modal
hints; out of scope here.)

### Pitch-class resolution for `in_key` tonic

The slash sub-chord's root degree resolves against
`current_key`'s mode using `Scale::intervals()`:

```
tonic_pitch = (current_key.tonic + intervals[degree-1]
               + accidental_offset) mod 12
```

`accidental_offset` is +1 for `#`, -1 for `b`, 0 otherwise.
`degree` is 1-indexed; `bVII` is degree 7 with offset −1.

## Normative test table

Each row is a documented case the parser must handle. Column 4
(canonical) is what `format` emits given the parsed result. All
rows in the **Round-trip** block satisfy
`format(parse(input)) == canonical`. Rows in the **Aliases** block
parse to the same `ChordSpec` as the canonical form on the right
but `format` emits the canonical spelling.

Current key for all rows is `C major` (`Scale::major(C)`) unless
noted.

### Round-trip examples (Functional)

| Input         | RomanDegree | Quality        | Extensions | Alterations | in_key       | Canonical     |
|---------------|-------------|----------------|------------|-------------|--------------|---------------|
| `I`           | I           | Major          | —          | —           | None         | `I`           |
| `i`           | I           | Minor          | —          | —           | None         | `i`           |
| `V`           | V           | Major          | —          | —           | None         | `V`           |
| `v`           | V           | Minor          | —          | —           | None         | `v`           |
| `5`           | V           | Major          | —          | —           | None         | `V`           |
| `5m`          | V           | Minor          | —          | —           | None         | `v`           |
| `bVI`         | FlatVI      | Major          | —          | —           | None         | `bVI`         |
| `#IV`         | SharpIV     | Major          | —          | —           | None         | `#IV`         |
| `bVII7`       | FlatVII     | Dominant7      | —          | —           | None         | `bVII7`       |
| `Vmaj7`       | V           | Major7         | —          | —           | None         | `Vmaj7`       |
| `Vm7`         | V           | Minor7         | —          | —           | None         | `Vm7`         |
| `viim7b5`     | VII         | HalfDiminished7| —          | —           | None         | `viim7b5`     |
| `Vsus4`       | V           | Sus4           | —          | —           | None         | `Vsus4`       |
| `Vsus2`       | V           | Sus2           | —          | —           | None         | `Vsus2`       |
| `V7sus4`      | V           | Sus7           | —          | —           | None         | `V7sus4`      |
| `I6`          | I           | Major6         | —          | —           | None         | `I6`          |
| `im6`         | I           | Minor6         | —          | —           | None         | `im6`         |
| `V5`          | V           | Power          | —          | —           | None         | `V5`          |
| `vii°`        | VII         | Diminished     | —          | —           | None         | `vii°`        |
| `vii°7`       | VII         | Diminished7    | —          | —           | None         | `vii°7`       |
| `Iadd9`       | I           | Major          | Add9       | —           | None         | `Iadd9`       |
| `V9`          | V           | Dominant7      | Ninth      | —           | None         | `V9`          |
| `V13`         | V           | Dominant7      | Thirteenth | —           | None         | `V13`         |
| `bVImaj7add9` | FlatVI      | Major7         | Add9       | —           | None         | `bVImaj7add9` |
| `V7b9`        | V           | Dominant7      | —          | Flat9       | None         | `V7b9`        |
| `V7#11`       | V           | Dominant7      | —          | Sharp11     | None         | `V7#11`       |
| `V7no5`       | V           | Dominant7      | —          | NoFifth     | None         | `V7no5`       |

### Round-trip examples (Functional with in_key)

Current key: `C major`. The `in_key` scale's tonic is in absolute
pitch classes; the canonical spelling re-renders it relative to
`current_key` via the degree shorthand.

| Input        | RomanDegree | Quality   | in_key                         | Canonical    |
|--------------|-------------|-----------|--------------------------------|--------------|
| `V/V`        | V           | Major     | `Scale { G, Ionian }`          | `V/V`        |
| `5/5`        | V           | Major     | `Scale { G, Ionian }`          | `V/V`        |
| `V/v`        | V           | Major     | `Scale { G, Aeolian }`         | `V/v`        |
| `V7/IV`      | V           | Dominant7 | `Scale { F, Ionian }`          | `V7/IV`      |
| `vii°7/V`    | VII         | Diminished7| `Scale { G, Ionian }`         | `vii°7/V`    |
| `ii/bVII`    | II          | Minor     | `Scale { Bb, Ionian }`         | `ii/bVII`    |
| `V/bIII`     | V           | Major     | `Scale { Eb, Ionian }`         | `V/bIII`     |

### Round-trip examples (Functional with absolute bass)

| Input         | Outer chord     | Bass                  | Canonical     |
|---------------|-----------------|-----------------------|---------------|
| `V/B`         | V major         | Absolute(B)           | `V/B`         |
| `V/Bb`        | V major         | Absolute(Bb / ASharp) | `V/Bb`        |
| `Imaj7/E`     | I Major7        | Absolute(E)           | `Imaj7/E`     |
| `bVI/F#`      | bVI Major       | Absolute(F# / FSharp) | `bVI/F#`      |

### Round-trip examples (Absolute)

| Input         | Root        | Quality        | Extensions | Alterations | Bass            | Canonical     |
|---------------|-------------|----------------|------------|-------------|-----------------|---------------|
| `C`           | C           | Major          | —          | —           | —               | `C`           |
| `Cm`          | C           | Minor          | —          | —           | —               | `Cm`          |
| `Cmaj7`       | C           | Major7         | —          | —           | —               | `Cmaj7`       |
| `F#m7`        | FSharp      | Minor7         | —          | —           | —               | `F#m7`        |
| `Bbm7b5`      | ASharp      | HalfDiminished7| —          | —           | —               | `Bbm7b5`      |
| `G7b9`        | G           | Dominant7      | —          | Flat9       | —               | `G7b9`        |
| `Cmaj7/E`     | C           | Major7         | —          | —           | Absolute(E)     | `Cmaj7/E`     |
| `Cmaj7add9#11`| C           | Major7         | Add9       | Sharp11     | —               | `Cmaj7add9#11`|

### Alias examples (parse → same `ChordSpec` as canonical column)

| Input         | Equivalent canonical |
|---------------|----------------------|
| `C-`          | `Cm`                 |
| `Cmin`        | `Cm`                 |
| `CM7`         | `Cmaj7`              |
| `CΔ`          | `Cmaj7`              |
| `Cdim`        | `C°` *(rendered with `°` only when input used it; otherwise `Cdim`)* |
| `Cø`          | `Cm7b5`              |
| `Cm7♭5`       | `Cm7b5`              |
| `V min 7`     | `Vm7`                |
| `vm`          | `v`                  |
| `vM`          | `V`                  |
| `5/5m`        | `V/v`                |

Note: `format` prefers `m` over `min`/`-`, `maj7` over `M7`/`Δ`,
`dim` over `°` (canonical ASCII), `m7b5` over `ø`. The renderer's
glyph choice is fixed; the parser is permissive.

### Malformed input (must be rejected with span info)

| Input         | Reason                                 |
|---------------|----------------------------------------|
| ``            | Empty                                  |
| `H`           | Unknown leading character              |
| `8`           | Degree out of range (1–7 only)         |
| `bbVI`        | Double-flat accidental not supported   |
| `V/`          | Slash with no content                  |
| `V//V`        | Empty middle segment                   |
| `V/bB`        | `b` before pitch letter is invalid     |
| `Vmaj7maj7`   | Quality specified twice                |
| `VsusX`       | Unknown sus variant                    |
| `Vadd14`      | Unknown extension degree               |
| `V7b14`       | Unknown alteration degree              |

### Lossy-but-accepted cases (parse succeeds; format may diverge)

| Input         | Behavior                                                 |
|---------------|----------------------------------------------------------|
| `Vm7b5`       | Parses as HalfDiminished7 (preferred over Minor7+Flat5). Format emits `Vm7b5`. |
| `Vm7 b5`      | Same — whitespace ignored.                               |
| `V/Bbb`       | Rejected (no double-flat) — or, when v2 adds double accidentals, parses as Absolute(A). |

## Parser surface

```rust
pub fn parse(
    input: &str,
    current_key: &rawdaw_model::scale::Scale,
) -> Result<ParsedChord, ParseError>;

pub struct ParsedChord {
    pub chord: rawdaw_model::chord::ChordSpec,
    pub bass: Option<rawdaw_model::chord::BassSpec>,
}

pub struct ParseError {
    pub span: std::ops::Range<usize>,  // byte offsets into input
    pub message: String,
}
```

`bass` is broken out separately from `ChordSpec` because the model
stores bass on `ChordEvent`, not on `ChordSpec` — the inspector
glue code sets `event.chord` and `event.bass` from one `ParsedChord`.

## Formatter surface

```rust
pub fn format(
    chord: &rawdaw_model::chord::ChordSpec,
    bass: Option<&rawdaw_model::chord::BassSpec>,
    current_key: &rawdaw_model::scale::Scale,
) -> String;
```

Bass kinds `Inversion`, `ChordDegree`, and `ScaleDegree` have no
shorthand form (decision 3); when formatting an event whose bass is
one of those, `format` emits the chord without a slash and the
inspector renders the bass via the dedicated sub-editor display.
This is documented behavior, not a bug.

## Out of scope (v2+)

- Double-flat / double-sharp accidentals.
- Chained secondaries (`V/V/V`).
- Modal-flavor hints on `in_key` beyond the quality-default table.
- `Custom` quality / `Custom` mode entry via shorthand.
- Inversion / ChordDegree / ScaleDegree bass shorthand.
- Nashville Number System rhythm marks (no `1.` vs `1` distinction).

[[project-status]]
