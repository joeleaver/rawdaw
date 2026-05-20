#![allow(dead_code)] // accretive helper module; consumers land region by region in C1c

//! Chord-name display helpers — model `ChordEvent` → UI label strings.
//!
//! Moved out of `fixture/chord_naming.rs` in C1c so non-fixture regions
//! (library, arrangement, inspector, section editor) can render chord
//! labels directly from the model without depending on the fixture
//! adapter. The fixture module re-imports from here while it still
//! exists.
//!
//! The mockup encodes chord quality through case on the Roman numeral
//! (`I` major, `vi` minor) and shows an absolute name beneath (`C`,
//! `Am`, `G7`). The model carries quality as a `ChordQuality` enum and
//! the roman as a separate `RomanDegree`; these helpers bridge the two.

use rawdaw_model::chord::{ChordQuality, ChordSpec, RomanDegree};
use rawdaw_model::pitch::PitchClass;
use rawdaw_model::scale::Scale;

/// Render the Roman-numeral part of a chord label, applying case-by-quality.
/// Major-leaning qualities stay uppercase, minor-leaning lower; diminished
/// and half-diminished append `°` and `ø` respectively.
pub fn roman_label(roman: RomanDegree, quality: &ChordQuality) -> String {
    let upper = roman_upper(roman);
    let lowered = matches!(
        quality,
        ChordQuality::Minor
            | ChordQuality::Minor7
            | ChordQuality::Minor6
            | ChordQuality::MinorMajor7
            | ChordQuality::Diminished
            | ChordQuality::Diminished7
            | ChordQuality::HalfDiminished7
    );
    let base = if lowered {
        upper.to_lowercase()
    } else {
        upper.to_string()
    };
    match quality {
        ChordQuality::Diminished | ChordQuality::Diminished7 => format!("{base}°"),
        ChordQuality::HalfDiminished7 => format!("{base}ø"),
        _ => base,
    }
}

fn roman_upper(roman: RomanDegree) -> &'static str {
    match roman {
        RomanDegree::I => "I",
        RomanDegree::II => "II",
        RomanDegree::III => "III",
        RomanDegree::IV => "IV",
        RomanDegree::V => "V",
        RomanDegree::VI => "VI",
        RomanDegree::VII => "VII",
        RomanDegree::FlatII => "♭II",
        RomanDegree::FlatIII => "♭III",
        RomanDegree::FlatV => "♭V",
        RomanDegree::FlatVI => "♭VI",
        RomanDegree::FlatVII => "♭VII",
        RomanDegree::SharpI => "♯I",
        RomanDegree::SharpII => "♯II",
        RomanDegree::SharpIV => "♯IV",
        RomanDegree::SharpV => "♯V",
        RomanDegree::SharpVI => "♯VI",
    }
}

/// Resolve a Roman numeral against a scale to its absolute root pitch
/// class, then append a textual suffix for the quality.
pub fn absolute_label(roman: RomanDegree, quality: &ChordQuality, scale: &Scale) -> String {
    let root_semitone = roman_semitone(roman, scale);
    let pc =
        PitchClass::from_semitones_mod12(scale.tonic.semitones_from_c() as i32 + root_semitone);
    format!("{}{}", pitch_class_name(pc), quality_suffix(quality))
}

/// Diatonic degree index 0..=6 for the major-mode-friendly Roman numerals.
/// Flats / sharps move the resulting semitone by ±1. Pentatonic and other
/// short modes wrap modulo `intervals.len()` — round-1 doesn't exercise
/// these but the fallback keeps the function total.
fn roman_semitone(roman: RomanDegree, scale: &Scale) -> i32 {
    let intervals = scale.mode.intervals();
    let nth = |idx: usize| intervals[idx % intervals.len()] as i32;
    match roman {
        RomanDegree::I => nth(0),
        RomanDegree::II => nth(1),
        RomanDegree::III => nth(2),
        RomanDegree::IV => nth(3),
        RomanDegree::V => nth(4),
        RomanDegree::VI => nth(5),
        RomanDegree::VII => nth(6),
        RomanDegree::FlatII => nth(1) - 1,
        RomanDegree::FlatIII => nth(2) - 1,
        RomanDegree::FlatV => nth(4) - 1,
        RomanDegree::FlatVI => nth(5) - 1,
        RomanDegree::FlatVII => nth(6) - 1,
        RomanDegree::SharpI => nth(0) + 1,
        RomanDegree::SharpII => nth(1) + 1,
        RomanDegree::SharpIV => nth(3) + 1,
        RomanDegree::SharpV => nth(4) + 1,
        RomanDegree::SharpVI => nth(5) + 1,
    }
}

pub fn pitch_class_name(pc: PitchClass) -> &'static str {
    match pc {
        PitchClass::C => "C",
        PitchClass::CSharp => "C#",
        PitchClass::D => "D",
        PitchClass::DSharp => "D#",
        PitchClass::E => "E",
        PitchClass::F => "F",
        PitchClass::FSharp => "F#",
        PitchClass::G => "G",
        PitchClass::GSharp => "G#",
        PitchClass::A => "A",
        PitchClass::ASharp => "A#",
        PitchClass::B => "B",
    }
}

pub fn quality_suffix(q: &ChordQuality) -> &'static str {
    match q {
        ChordQuality::Major => "",
        ChordQuality::Minor => "m",
        ChordQuality::Diminished => "dim",
        ChordQuality::Augmented => "aug",
        ChordQuality::Major7 => "maj7",
        ChordQuality::Minor7 => "m7",
        ChordQuality::Dominant7 => "7",
        ChordQuality::Diminished7 => "dim7",
        ChordQuality::HalfDiminished7 => "m7♭5",
        ChordQuality::MinorMajor7 => "mMaj7",
        ChordQuality::AugmentedDom7 => "aug7",
        ChordQuality::Major6 => "6",
        ChordQuality::Minor6 => "m6",
        ChordQuality::Sus2 => "sus2",
        ChordQuality::Sus4 => "sus4",
        ChordQuality::Sus7 => "7sus4",
        ChordQuality::Sus9 => "9sus4",
        ChordQuality::Power => "5",
        ChordQuality::Custom { .. } => "",
    }
}

/// Combined `(roman, absolute)` labels for one chord event, resolved
/// against the supplied scale (typically the section's effective scale
/// or the chord's `in_key` override for functional events).
pub fn event_labels(chord: &ChordSpec, scale: &Scale) -> (String, String) {
    match chord {
        ChordSpec::Functional {
            roman,
            suffix,
            in_key,
        } => {
            let effective = in_key.as_ref().unwrap_or(scale);
            (
                roman_label(*roman, &suffix.quality),
                absolute_label(*roman, &suffix.quality, effective),
            )
        }
        ChordSpec::Absolute { root, suffix } => (
            String::new(),
            format!(
                "{}{}",
                pitch_class_name(*root),
                quality_suffix(&suffix.quality),
            ),
        ),
    }
}
