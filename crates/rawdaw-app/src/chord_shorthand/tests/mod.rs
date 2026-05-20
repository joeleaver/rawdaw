//! Table-driven tests for the chord-shorthand parser and formatter.
//!
//! Cases derive directly from `grammar.md`'s **Normative test
//! table**. If you add a row there, add one here. If a test
//! fails, the spec is the contract — fix the parser, not the
//! expected value.
//!
//! Split into two submodules to stay under the ~700-line file cap:
//! - `parse_cases` — parser inputs → expected `ChordSpec` (one big
//!   table covering Functional / Absolute / in_key / bass /
//!   aliases / malformed inputs).
//! - `round_trip_cases` — `parse → format` round-trip canonical
//!   form checks.

#![allow(clippy::too_many_lines)]

mod parse_cases;
mod round_trip_cases;

// Shared helpers — used by both submodules via `super::`.

use rawdaw_model::chord::{
    Alteration, BassSpec, ChordQuality, ChordSpec, ChordSuffix, Extension, RomanDegree,
};
use rawdaw_model::pitch::PitchClass;
use rawdaw_model::scale::Scale;

use super::format;
use super::parse;
use super::parse::ParsedChord;

pub(super) fn c_major() -> Scale {
    Scale::major(PitchClass::C)
}

pub(super) fn functional(
    roman: RomanDegree,
    quality: ChordQuality,
    exts: &[Extension],
    alts: &[Alteration],
    in_key: Option<Scale>,
) -> ChordSpec {
    ChordSpec::Functional {
        roman,
        suffix: ChordSuffix {
            quality,
            extensions: exts.to_vec(),
            alterations: alts.to_vec(),
        },
        in_key,
    }
}

pub(super) fn absolute(
    root: PitchClass,
    quality: ChordQuality,
    exts: &[Extension],
    alts: &[Alteration],
) -> ChordSpec {
    ChordSpec::Absolute {
        root,
        suffix: ChordSuffix {
            quality,
            extensions: exts.to_vec(),
            alterations: alts.to_vec(),
        },
    }
}

#[track_caller]
pub(super) fn check(input: &str, expected: ChordSpec, bass: Option<BassSpec>) {
    let parsed = parse(input, &c_major())
        .unwrap_or_else(|e| panic!("`{}` failed to parse: {:?}", input, e));
    assert_eq!(
        parsed,
        ParsedChord {
            chord: expected,
            bass
        },
        "input `{}`",
        input
    );
}

#[track_caller]
pub(super) fn check_err(input: &str, fragment: &str) {
    let err = parse(input, &c_major())
        .err()
        .unwrap_or_else(|| panic!("`{}` should have failed but parsed", input));
    assert!(
        err.message.contains(fragment),
        "input `{}`: expected error containing `{}`, got `{}`",
        input,
        fragment,
        err.message
    );
}

#[track_caller]
pub(super) fn round_trip(input: &str, canonical: &str) {
    let key = c_major();
    let parsed = parse(input, &key).unwrap_or_else(|e| panic!("`{}` failed: {:?}", input, e));
    let rendered = format(&parsed.chord, parsed.bass.as_ref(), &key);
    assert_eq!(
        rendered, canonical,
        "input `{}` formatted to `{}`, expected `{}`",
        input, rendered, canonical
    );
}
