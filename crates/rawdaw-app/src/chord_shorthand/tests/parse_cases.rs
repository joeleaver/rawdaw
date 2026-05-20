//! Parser cases — input string → expected `ChordSpec` (+ optional
//! bass). Each `#[test]` covers one row group of `grammar.md`'s
//! Normative test table.

use rawdaw_model::chord::{Alteration, BassSpec, ChordQuality, Extension, RomanDegree};
use rawdaw_model::pitch::PitchClass;
use rawdaw_model::scale::{Mode, Scale};

use super::{absolute, check, check_err, functional};

// ─── Functional: degree + case-implied quality ──────────────────────────

#[test]
fn functional_uppercase_roman_defaults_major() {
    use RomanDegree::*;
    for (input, roman) in [
        ("I", I),
        ("II", II),
        ("III", III),
        ("IV", IV),
        ("V", V),
        ("VI", VI),
        ("VII", VII),
    ] {
        check(
            input,
            functional(roman, ChordQuality::Major, &[], &[], None),
            None,
        );
    }
}

#[test]
fn functional_lowercase_roman_defaults_minor() {
    use RomanDegree::*;
    for (input, roman) in [
        ("i", I),
        ("ii", II),
        ("iii", III),
        ("iv", IV),
        ("v", V),
        ("vi", VI),
        ("vii", VII),
    ] {
        check(
            input,
            functional(roman, ChordQuality::Minor, &[], &[], None),
            None,
        );
    }
}

#[test]
fn functional_arabic_digit_defaults_major() {
    use RomanDegree::*;
    for (input, roman) in [
        ("1", I),
        ("2", II),
        ("3", III),
        ("4", IV),
        ("5", V),
        ("6", VI),
        ("7", VII),
    ] {
        check(
            input,
            functional(roman, ChordQuality::Major, &[], &[], None),
            None,
        );
    }
}

#[test]
fn functional_digit_with_minor_suffix() {
    check(
        "5m",
        functional(RomanDegree::V, ChordQuality::Minor, &[], &[], None),
        None,
    );
}

#[test]
fn functional_explicit_quality_overrides_case() {
    // `vM` → V major (explicit M wins over lowercase v default).
    check(
        "vM",
        functional(RomanDegree::V, ChordQuality::Major, &[], &[], None),
        None,
    );
    // `Vm` → V minor (explicit m wins over uppercase V default).
    check(
        "Vm",
        functional(RomanDegree::V, ChordQuality::Minor, &[], &[], None),
        None,
    );
    // `vm` → V minor (redundant m allowed alongside lowercase).
    check(
        "vm",
        functional(RomanDegree::V, ChordQuality::Minor, &[], &[], None),
        None,
    );
}

// ─── Functional: accidentals ────────────────────────────────────────────

#[test]
fn functional_flat_accidental() {
    use RomanDegree::*;
    for (input, roman) in [
        ("bII", FlatII),
        ("bIII", FlatIII),
        ("bV", FlatV),
        ("bVI", FlatVI),
        ("bVII", FlatVII),
    ] {
        check(
            input,
            functional(roman, ChordQuality::Major, &[], &[], None),
            None,
        );
    }
}

#[test]
fn functional_sharp_accidental() {
    use RomanDegree::*;
    for (input, roman) in [
        ("#I", SharpI),
        ("#II", SharpII),
        ("#IV", SharpIV),
        ("#V", SharpV),
        ("#VI", SharpVI),
    ] {
        check(
            input,
            functional(roman, ChordQuality::Major, &[], &[], None),
            None,
        );
    }
}

// ─── Functional: quality suffixes ──────────────────────────────────────

#[test]
fn functional_seventh_qualities() {
    use ChordQuality::*;
    for (input, quality) in [
        ("V7", Dominant7),
        ("Vmaj7", Major7),
        ("VM7", Major7),
        ("VΔ7", Major7),
        ("VΔ", Major7),
        ("vm7", Minor7),
        ("vmin7", Minor7),
        ("V-7", Minor7),
        ("VmMaj7", MinorMajor7),
        ("VminMaj7", MinorMajor7),
        ("Vdim7", Diminished7),
        ("V°7", Diminished7),
        ("Vo7", Diminished7),
        ("Vø7", HalfDiminished7),
        ("Vø", HalfDiminished7),
        ("Vm7b5", HalfDiminished7),
        ("V-7b5", HalfDiminished7),
        ("V+7", AugmentedDom7),
        ("Vaug7", AugmentedDom7),
        ("V7sus4", Sus7),
    ] {
        check(
            input,
            functional(RomanDegree::V, quality, &[], &[], None),
            None,
        );
    }
}

#[test]
fn functional_triad_qualities() {
    use ChordQuality::*;
    for (input, quality) in [
        ("Vmaj", Major),
        ("VMaj", Major),
        ("VMAJ", Major),
        ("VM", Major),
        ("Vmin", Minor),
        ("V-", Minor),
        ("Vdim", Diminished),
        ("V°", Diminished),
        ("Vo", Diminished),
        ("Vaug", Augmented),
        ("V+", Augmented),
        ("Vsus", Sus4),
        ("Vsus2", Sus2),
        ("Vsus4", Sus4),
        ("Vsus9", Sus9),
        ("V5", Power),
        ("V6", Major6),
        ("Vm6", Minor6),
        ("V-6", Minor6),
    ] {
        check(
            input,
            functional(RomanDegree::V, quality, &[], &[], None),
            None,
        );
    }
}

// ─── Functional: extensions ─────────────────────────────────────────────

#[test]
fn functional_extensions() {
    // `Iadd9` — explicit Major + Add9 (no promotion).
    check(
        "Iadd9",
        functional(
            RomanDegree::I,
            ChordQuality::Major,
            &[Extension::Add9],
            &[],
            None,
        ),
        None,
    );
    // `V9` — implicit Major + Ninth → promoted to Dom7+Ninth.
    check(
        "V9",
        functional(
            RomanDegree::V,
            ChordQuality::Dominant7,
            &[Extension::Ninth],
            &[],
            None,
        ),
        None,
    );
    // `V13` — same promotion.
    check(
        "V13",
        functional(
            RomanDegree::V,
            ChordQuality::Dominant7,
            &[Extension::Thirteenth],
            &[],
            None,
        ),
        None,
    );
    // `bVImaj7add9` — explicit Major7 + Add9 (no promotion since
    // Major7 is already a 7th-bearing quality).
    check(
        "bVImaj7add9",
        functional(
            RomanDegree::FlatVI,
            ChordQuality::Major7,
            &[Extension::Add9],
            &[],
            None,
        ),
        None,
    );
}

// ─── Functional: alterations ────────────────────────────────────────────

#[test]
fn functional_alterations() {
    use Alteration::*;
    for (input, alt) in [
        ("V7b5", Flat5),
        ("V7#5", Sharp5),
        ("V7b9", Flat9),
        ("V7#9", Sharp9),
        ("V7#11", Sharp11),
        ("V7b13", Flat13),
        ("V7no3", NoThird),
        ("V7no5", NoFifth),
    ] {
        check(
            input,
            functional(RomanDegree::V, ChordQuality::Dominant7, &[], &[alt], None),
            None,
        );
    }
}

// ─── Functional: secondary dominants (in_key) ──────────────────────────

#[test]
fn functional_secondary_dominants() {
    // Current key: C major. V/V → V in G Ionian.
    check(
        "V/V",
        functional(
            RomanDegree::V,
            ChordQuality::Major,
            &[],
            &[],
            Some(Scale {
                tonic: PitchClass::G,
                mode: Mode::Ionian,
            }),
        ),
        None,
    );
    // Both-sides Arabic.
    check(
        "5/5",
        functional(
            RomanDegree::V,
            ChordQuality::Major,
            &[],
            &[],
            Some(Scale {
                tonic: PitchClass::G,
                mode: Mode::Ionian,
            }),
        ),
        None,
    );
    // Minor sub-chord → Aeolian.
    check(
        "V/v",
        functional(
            RomanDegree::V,
            ChordQuality::Major,
            &[],
            &[],
            Some(Scale {
                tonic: PitchClass::G,
                mode: Mode::Aeolian,
            }),
        ),
        None,
    );
    // V7/IV → outer Dom7, in_key F Ionian.
    check(
        "V7/IV",
        functional(
            RomanDegree::V,
            ChordQuality::Dominant7,
            &[],
            &[],
            Some(Scale {
                tonic: PitchClass::F,
                mode: Mode::Ionian,
            }),
        ),
        None,
    );
    // vii°7/V → outer Diminished7, in_key G Ionian (the sub-chord's
    // `V` is uppercase Major default → Ionian, NOT the outer quality).
    check(
        "vii°7/V",
        functional(
            RomanDegree::VII,
            ChordQuality::Diminished7,
            &[],
            &[],
            Some(Scale {
                tonic: PitchClass::G,
                mode: Mode::Ionian,
            }),
        ),
        None,
    );
    // ii/bVII → outer minor, in_key Bb Ionian.
    check(
        "ii/bVII",
        functional(
            RomanDegree::II,
            ChordQuality::Minor,
            &[],
            &[],
            Some(Scale {
                tonic: PitchClass::ASharp,
                mode: Mode::Ionian,
            }),
        ),
        None,
    );
    // V/bIII → outer Major, in_key Eb Ionian.
    check(
        "V/bIII",
        functional(
            RomanDegree::V,
            ChordQuality::Major,
            &[],
            &[],
            Some(Scale {
                tonic: PitchClass::DSharp,
                mode: Mode::Ionian,
            }),
        ),
        None,
    );
}

// ─── Functional: absolute bass ──────────────────────────────────────────

#[test]
fn functional_with_absolute_bass() {
    check(
        "V/B",
        functional(RomanDegree::V, ChordQuality::Major, &[], &[], None),
        Some(BassSpec::Absolute(PitchClass::B)),
    );
    check(
        "V/Bb",
        functional(RomanDegree::V, ChordQuality::Major, &[], &[], None),
        Some(BassSpec::Absolute(PitchClass::ASharp)),
    );
    check(
        "Imaj7/E",
        functional(RomanDegree::I, ChordQuality::Major7, &[], &[], None),
        Some(BassSpec::Absolute(PitchClass::E)),
    );
    check(
        "bVI/F#",
        functional(RomanDegree::FlatVI, ChordQuality::Major, &[], &[], None),
        Some(BassSpec::Absolute(PitchClass::FSharp)),
    );
}

// ─── Absolute ───────────────────────────────────────────────────────────

#[test]
fn absolute_basic() {
    check("C", absolute(PitchClass::C, ChordQuality::Major, &[], &[]), None);
    check("Cm", absolute(PitchClass::C, ChordQuality::Minor, &[], &[]), None);
    check(
        "Cmaj7",
        absolute(PitchClass::C, ChordQuality::Major7, &[], &[]),
        None,
    );
    check(
        "F#m7",
        absolute(PitchClass::FSharp, ChordQuality::Minor7, &[], &[]),
        None,
    );
    check(
        "Bbm7b5",
        absolute(PitchClass::ASharp, ChordQuality::HalfDiminished7, &[], &[]),
        None,
    );
    check(
        "G7b9",
        absolute(
            PitchClass::G,
            ChordQuality::Dominant7,
            &[],
            &[Alteration::Flat9],
        ),
        None,
    );
}

#[test]
fn absolute_with_bass() {
    check(
        "Cmaj7/E",
        absolute(PitchClass::C, ChordQuality::Major7, &[], &[]),
        Some(BassSpec::Absolute(PitchClass::E)),
    );
}

#[test]
fn absolute_ext_and_alt() {
    check(
        "Cmaj7add9#11",
        absolute(
            PitchClass::C,
            ChordQuality::Major7,
            &[Extension::Add9],
            &[Alteration::Sharp11],
        ),
        None,
    );
}

// ─── Aliases (parse-only — formatter normalizes) ───────────────────────

#[test]
fn alias_minor_dash() {
    let expected = absolute(PitchClass::C, ChordQuality::Minor, &[], &[]);
    check("Cm", expected.clone(), None);
    check("C-", expected.clone(), None);
    check("Cmin", expected, None);
}

#[test]
fn alias_major7_glyphs() {
    let expected = absolute(PitchClass::C, ChordQuality::Major7, &[], &[]);
    check("Cmaj7", expected.clone(), None);
    check("CM7", expected.clone(), None);
    check("CΔ7", expected.clone(), None);
    check("CΔ", expected, None);
}

#[test]
fn alias_half_diminished_glyphs() {
    let expected = absolute(PitchClass::C, ChordQuality::HalfDiminished7, &[], &[]);
    check("Cm7b5", expected.clone(), None);
    check("Cm7♭5", expected.clone(), None);
    check("Cø", expected.clone(), None);
    check("Cø7", expected, None);
}

#[test]
fn alias_whitespace_ignored() {
    let expected = functional(RomanDegree::V, ChordQuality::Minor7, &[], &[], None);
    check("Vm7", expected.clone(), None);
    check("V m7", expected.clone(), None);
    check(" Vm7 ", expected, None);
}

// ─── Malformed inputs ───────────────────────────────────────────────────

#[test]
fn rejects_empty() {
    check_err("", "empty");
}

#[test]
fn rejects_unknown_lead() {
    check_err("H", "unknown");
}

#[test]
fn rejects_degree_out_of_range() {
    check_err("8", "1–7");
}

#[test]
fn rejects_double_flat() {
    // `bbVI` — second `b` falls into the degree-token slot but isn't
    // a valid degree, producing "expected degree".
    check_err("bbVI", "expected degree");
}

#[test]
fn rejects_empty_slash() {
    check_err("V/", "expected slash content");
}

#[test]
fn rejects_pitch_letter_after_b_in_slash() {
    // `V/bB` — `b` opens a Functional sub-chord, then `B` isn't a
    // Roman/digit so the sub-parser fails on degree.
    check_err("V/bB", "expected degree");
}

#[test]
fn rejects_trailing_garbage() {
    check_err("Vmaj7X", "trailing");
}

#[test]
fn rejects_unknown_add() {
    check_err("Vadd14", "unknown extension");
}

#[test]
fn rejects_slash_bass_with_chord() {
    // After absolute-bass pitch class, no more input allowed.
    check_err("V/Cmaj7", "pitch class only");
}
