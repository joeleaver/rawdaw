//! Canonical formatter — inverse of [`super::parse`].
//!
//! Canonical form rules (per `grammar.md` §"Aliases" and the
//! "Note: `format` prefers ..." line):
//!
//! - `m` over `min`/`-`
//! - `maj7` over `M7`/`Δ`
//! - `dim` over `°` (canonical ASCII)
//! - `m7b5` over `ø`
//! - Sharps over flats in pitch class names (matches the model's
//!   `PitchClass` "sharps preferred" convention)
//! - Roman case carries the quality default: lowercase for
//!   minor-flavored qualities (`Minor`, `Minor7`, `Minor6`,
//!   `MinorMajor7`, `HalfDiminished7`, `Diminished`, `Diminished7`);
//!   uppercase otherwise.
//! - Extensions before alterations, in declared model order.
//!
//! Round-trip guarantee: `format(parse(input).chord, parse(input).bass)
//! == canonical(input)` for every row of the **Normative test table**.

use rawdaw_model::chord::{
    Alteration, BassSpec, ChordQuality, ChordSpec, ChordSuffix, Extension, RomanDegree,
};
use rawdaw_model::pitch::PitchClass;
use rawdaw_model::scale::{Mode, Scale};

/// Format a `ChordSpec` (+ optional bass) into canonical shorthand.
/// `current_key` is used to render `in_key` as a degree relative to
/// the current tonic.
pub fn format(chord: &ChordSpec, bass: Option<&BassSpec>, current_key: &Scale) -> String {
    let mut out = match chord {
        ChordSpec::Functional {
            roman,
            suffix,
            in_key,
        } => format_functional(*roman, suffix, in_key.as_ref(), current_key),
        ChordSpec::Absolute { root, suffix } => format_absolute(*root, suffix),
    };
    if let Some(BassSpec::Absolute(pc)) = bass {
        out.push('/');
        out.push_str(format_pitch_class(*pc));
    }
    // Inversion / ChordDegree / ScaleDegree bass have no shorthand
    // form in v1 (grammar §"Formatter surface"); the inspector renders
    // those via the dedicated sub-editor display.
    out
}

// ─── Functional ─────────────────────────────────────────────────────────

fn format_functional(
    roman: RomanDegree,
    suffix: &ChordSuffix,
    in_key: Option<&Scale>,
    current_key: &Scale,
) -> String {
    let mut out = format_roman_with_case(roman, &suffix.quality);
    // When a 7th-implying extension (9/11/13) accompanies Dom7 or
    // Minor7, the quality suffix is implied by the extension — mirror
    // the parser's promotion rule by suppressing the "7"/"m7" prefix.
    // Without this, `V9` would format as `V79`.
    let quality_implicit = quality_is_case_default(&suffix.quality)
        || quality_implied_by_extension(&suffix.quality, &suffix.extensions);
    out.push_str(format_quality_suffix(&suffix.quality, quality_implicit));
    out.push_str(&format_extensions(&suffix.extensions));
    out.push_str(&format_alterations(&suffix.alterations));
    if let Some(scale) = in_key {
        out.push('/');
        out.push_str(&format_in_key(scale, current_key));
    }
    out
}

/// Mirrors the parser's implicit-Major → Dom7 promotion: if the
/// quality is one of the auto-promoted forms (`Dom7`, `Minor7`) and
/// the chord carries a 9/11/13 extension, the user input had no
/// explicit quality suffix and canonical output should match.
fn quality_implied_by_extension(quality: &ChordQuality, exts: &[Extension]) -> bool {
    let has_seventh_ext = exts
        .iter()
        .any(|e| matches!(e, Extension::Ninth | Extension::Eleventh | Extension::Thirteenth));
    has_seventh_ext && matches!(quality, ChordQuality::Dominant7 | ChordQuality::Minor7)
}

fn format_roman_with_case(roman: RomanDegree, quality: &ChordQuality) -> String {
    let (numeral_upper, accidental) = roman_to_numeral_accidental(roman);
    let lower = is_minor_flavored(quality);
    let numeral = if lower {
        numeral_upper.to_lowercase()
    } else {
        numeral_upper.to_string()
    };
    let mut out = String::new();
    if let Some(a) = accidental {
        out.push(a);
    }
    out.push_str(&numeral);
    out
}

/// Returns the uppercase Roman numeral and an optional accidental
/// character for a given `RomanDegree`.
fn roman_to_numeral_accidental(roman: RomanDegree) -> (&'static str, Option<char>) {
    use RomanDegree::*;
    match roman {
        I => ("I", None),
        II => ("II", None),
        III => ("III", None),
        IV => ("IV", None),
        V => ("V", None),
        VI => ("VI", None),
        VII => ("VII", None),
        FlatII => ("II", Some('b')),
        FlatIII => ("III", Some('b')),
        FlatV => ("V", Some('b')),
        FlatVI => ("VI", Some('b')),
        FlatVII => ("VII", Some('b')),
        SharpI => ("I", Some('#')),
        SharpII => ("II", Some('#')),
        SharpIV => ("IV", Some('#')),
        SharpV => ("V", Some('#')),
        SharpVI => ("VI", Some('#')),
    }
}

/// Whether the quality belongs to the "lowercase Roman default"
/// group used by analytical conventions. The formatter renders the
/// numeral in lowercase iff this is true.
fn is_minor_flavored(quality: &ChordQuality) -> bool {
    matches!(
        quality,
        ChordQuality::Minor
            | ChordQuality::Minor7
            | ChordQuality::Minor6
            | ChordQuality::MinorMajor7
            | ChordQuality::HalfDiminished7
            | ChordQuality::Diminished
            | ChordQuality::Diminished7
    )
}

/// Whether the quality is "case-implied" — i.e. the Roman case alone
/// communicates it and no quality suffix is needed.
fn quality_is_case_default(quality: &ChordQuality) -> bool {
    matches!(quality, ChordQuality::Major | ChordQuality::Minor)
}

// ─── Absolute ───────────────────────────────────────────────────────────

fn format_absolute(root: PitchClass, suffix: &ChordSuffix) -> String {
    let mut out = format_pitch_class(root).to_string();
    // Absolute always emits an explicit quality suffix (case can't
    // carry it for pitch classes), except for plain Major which is
    // the universal default.
    out.push_str(format_quality_suffix(
        &suffix.quality,
        matches!(suffix.quality, ChordQuality::Major),
    ));
    out.push_str(&format_extensions(&suffix.extensions));
    out.push_str(&format_alterations(&suffix.alterations));
    out
}

fn format_pitch_class(pc: PitchClass) -> &'static str {
    use PitchClass::*;
    match pc {
        C => "C",
        CSharp => "C#",
        D => "D",
        DSharp => "D#",
        E => "E",
        F => "F",
        FSharp => "F#",
        G => "G",
        GSharp => "G#",
        A => "A",
        ASharp => "A#",
        B => "B",
    }
}

// ─── Quality suffix ─────────────────────────────────────────────────────

/// Render the canonical quality suffix. If `case_default` is true,
/// the quality is implied by Roman case and emits nothing.
fn format_quality_suffix(quality: &ChordQuality, case_default: bool) -> &'static str {
    if case_default {
        return "";
    }
    match quality {
        ChordQuality::Major => "",
        ChordQuality::Minor => "m",
        ChordQuality::Diminished => "dim",
        ChordQuality::Augmented => "aug",
        ChordQuality::Major7 => "maj7",
        ChordQuality::Minor7 => "m7",
        ChordQuality::Dominant7 => "7",
        ChordQuality::Diminished7 => "dim7",
        ChordQuality::HalfDiminished7 => "m7b5",
        ChordQuality::MinorMajor7 => "mMaj7",
        ChordQuality::AugmentedDom7 => "aug7",
        ChordQuality::Major6 => "6",
        ChordQuality::Minor6 => "m6",
        ChordQuality::Sus2 => "sus2",
        ChordQuality::Sus4 => "sus4",
        ChordQuality::Sus7 => "7sus4",
        ChordQuality::Sus9 => "sus9",
        ChordQuality::Power => "5",
        ChordQuality::Custom { .. } => "",
    }
}

// ─── Extensions and alterations ─────────────────────────────────────────

fn format_extensions(exts: &[Extension]) -> String {
    let mut out = String::new();
    for ext in exts {
        out.push_str(match ext {
            Extension::Add9 => "add9",
            Extension::Add11 => "add11",
            Extension::Add13 => "add13",
            Extension::Ninth => "9",
            Extension::Eleventh => "11",
            Extension::Thirteenth => "13",
        });
    }
    out
}

fn format_alterations(alts: &[Alteration]) -> String {
    let mut out = String::new();
    for alt in alts {
        out.push_str(match alt {
            Alteration::Flat5 => "b5",
            Alteration::Sharp5 => "#5",
            Alteration::Flat9 => "b9",
            Alteration::Sharp9 => "#9",
            Alteration::Sharp11 => "#11",
            Alteration::Flat13 => "b13",
            Alteration::NoThird => "no3",
            Alteration::NoFifth => "no5",
        });
    }
    out
}

// ─── In_key rendering ───────────────────────────────────────────────────

/// Express `in_key` as a Roman/digit degree relative to `current_key`.
/// Picks lowercase Roman for Aeolian/Locrian, uppercase for Ionian
/// (matches the parser's mode-for-quality table). Falls back to
/// uppercase if the scale's mode is exotic.
fn format_in_key(in_key: &Scale, current_key: &Scale) -> String {
    let semitone_diff = (in_key.tonic.semitones_from_c() as i32
        - current_key.tonic.semitones_from_c() as i32)
        .rem_euclid(12);
    let intervals = current_key.mode.intervals();
    // Find diatonic match first; if none, prefer flat-of-higher-
    // degree (bVII) over sharp-of-lower-degree (#VI) for the same
    // target — flats are the analytical default for borrowed degrees.
    for (idx, st) in intervals.iter().enumerate() {
        if *st as i32 == semitone_diff {
            return numeral_for_degree(idx + 1, None, in_key_case(&in_key.mode));
        }
    }
    for (idx, st) in intervals.iter().enumerate() {
        if (*st as i32 - 1).rem_euclid(12) == semitone_diff {
            return numeral_for_degree(idx + 1, Some('b'), in_key_case(&in_key.mode));
        }
    }
    for (idx, st) in intervals.iter().enumerate() {
        if (*st as i32 + 1).rem_euclid(12) == semitone_diff {
            return numeral_for_degree(idx + 1, Some('#'), in_key_case(&in_key.mode));
        }
    }
    // Last-resort: emit pitch class name. Not strictly grammar-legal
    // for in_key but better than a panic. Round-trip won't match
    // these exotics — document elsewhere.
    format_pitch_class(in_key.tonic).to_string()
}

fn in_key_case(mode: &Mode) -> RomanCase {
    match mode {
        Mode::Aeolian
        | Mode::Locrian
        | Mode::Phrygian
        | Mode::HarmonicMinor
        | Mode::MelodicMinor
        | Mode::MinorPentatonic => RomanCase::Lower,
        _ => RomanCase::Upper,
    }
}

#[derive(Clone, Copy)]
enum RomanCase {
    Upper,
    Lower,
}

fn numeral_for_degree(degree: usize, accidental: Option<char>, case: RomanCase) -> String {
    let upper = match degree {
        1 => "I",
        2 => "II",
        3 => "III",
        4 => "IV",
        5 => "V",
        6 => "VI",
        7 => "VII",
        _ => return String::new(),
    };
    let numeral = match case {
        RomanCase::Upper => upper.to_string(),
        RomanCase::Lower => upper.to_lowercase(),
    };
    let mut out = String::new();
    if let Some(a) = accidental {
        out.push(a);
    }
    out.push_str(&numeral);
    out
}
