//! Mode toggle + Roman degree + Absolute root + Chord quality.
//!
//! Mode toggle swaps the `ChordSpec` variant in-place, preserving
//! `suffix` (quality + extensions + alterations). On
//! Functional→Absolute we resolve the Roman against the project's
//! default key so the user doesn't have to re-pick the root.
//! Absolute→Functional defaults to `RomanDegree::I` (reverse-
//! resolution is ambiguous without more context).

use rinch::prelude::*;

use rawdaw_model::chord::{ChordQuality, ChordSpec, ChordSuffix, RomanDegree};
use rawdaw_model::id::ChordLoopId;
use rawdaw_model::pitch::PitchClass;
use rawdaw_model::realize::resolve::resolve_chord_spec_root;

use crate::state::AppState;

use super::{fetch_event, mutate_event};

// ─── Functional / Absolute mode toggle ───────────────────────────────────

pub(super) fn mode_options() -> Vec<SelectOption> {
    vec![
        SelectOption::new("Functional", "Functional (Roman)"),
        SelectOption::new("Absolute", "Absolute (pitch)"),
    ]
}

pub(super) fn current_mode_str(id: ChordLoopId, idx: usize) -> String {
    match fetch_event(id, idx).map(|ev| ev.chord) {
        Some(ChordSpec::Absolute { .. }) => "Absolute".into(),
        _ => "Functional".into(),
    }
}

pub(super) fn commit_mode(id: ChordLoopId, idx: usize, encoded: String) {
    let target_absolute = matches!(encoded.as_str(), "Absolute");
    let Some(event) = fetch_event(id, idx) else { return };
    let already_absolute = matches!(event.chord, ChordSpec::Absolute { .. });
    if target_absolute == already_absolute {
        return;
    }
    let project = use_store::<AppState>().project.get();
    let key = project.default_key.clone();
    let new_chord = match event.chord {
        ChordSpec::Functional { suffix, .. } if target_absolute => {
            // Use a probe Functional with the original suffix to resolve
            // the root via the realize helper, then drop the Roman.
            let probe = ChordSpec::Functional {
                roman: extract_roman(&event_chord_for_probe(id, idx)),
                suffix: suffix.clone(),
                in_key: extract_in_key(&event_chord_for_probe(id, idx)),
            };
            let root = resolve_chord_spec_root(&probe, &key);
            ChordSpec::Absolute { root, suffix }
        }
        ChordSpec::Absolute { root, suffix } if !target_absolute => ChordSpec::Functional {
            roman: pitch_class_to_roman(root, &key).unwrap_or(RomanDegree::I),
            suffix,
            in_key: None,
        },
        other => other,
    };
    mutate_event(id, idx, move |ev| {
        ev.chord = new_chord.clone();
    });
}

/// Re-read the chord at commit time — the event we matched on may
/// have been mutated between fetch and apply (extension chips fire
/// fast). For the probe path we just want roman + in_key off the
/// current event.
fn event_chord_for_probe(id: ChordLoopId, idx: usize) -> ChordSpec {
    fetch_event(id, idx)
        .map(|ev| ev.chord)
        .unwrap_or_else(|| ChordSpec::Functional {
            roman: RomanDegree::I,
            suffix: ChordSuffix::new(ChordQuality::Major),
            in_key: None,
        })
}

fn extract_roman(spec: &ChordSpec) -> RomanDegree {
    match spec {
        ChordSpec::Functional { roman, .. } => *roman,
        _ => RomanDegree::I,
    }
}

fn extract_in_key(spec: &ChordSpec) -> Option<rawdaw_model::scale::Scale> {
    match spec {
        ChordSpec::Functional { in_key, .. } => in_key.clone(),
        _ => None,
    }
}

/// Inverse of `resolve_chord_root`: given a pitch class and the
/// current key, find the matching Roman degree. Prefers diatonic
/// (natural) degrees, then flat-of-higher-degree, then sharp-of-
/// lower-degree (matches the formatter's accidental preference).
/// Returns `None` if the pitch class can't be expressed in 7-degree
/// shorthand (only possible with exotic modes — diatonic key
/// signatures with ±1 accidentals cover all 12 pitch classes).
fn pitch_class_to_roman(
    pc: PitchClass,
    key: &rawdaw_model::scale::Scale,
) -> Option<RomanDegree> {
    let diff = (pc.semitones_from_c() as i32 - key.tonic.semitones_from_c() as i32)
        .rem_euclid(12);
    let intervals = key.mode.intervals();
    for (i, st) in intervals.iter().enumerate() {
        if *st as i32 == diff {
            return natural_roman(i + 1);
        }
    }
    for (i, st) in intervals.iter().enumerate() {
        if (*st as i32 - 1).rem_euclid(12) == diff {
            return flat_roman(i + 1);
        }
    }
    for (i, st) in intervals.iter().enumerate() {
        if (*st as i32 + 1).rem_euclid(12) == diff {
            return sharp_roman(i + 1);
        }
    }
    None
}

fn natural_roman(degree: usize) -> Option<RomanDegree> {
    use RomanDegree::*;
    Some(match degree {
        1 => I,
        2 => II,
        3 => III,
        4 => IV,
        5 => V,
        6 => VI,
        7 => VII,
        _ => return None,
    })
}

fn flat_roman(degree: usize) -> Option<RomanDegree> {
    use RomanDegree::*;
    Some(match degree {
        2 => FlatII,
        3 => FlatIII,
        5 => FlatV,
        6 => FlatVI,
        7 => FlatVII,
        _ => return None,
    })
}

fn sharp_roman(degree: usize) -> Option<RomanDegree> {
    use RomanDegree::*;
    Some(match degree {
        1 => SharpI,
        2 => SharpII,
        4 => SharpIV,
        5 => SharpV,
        6 => SharpVI,
        _ => return None,
    })
}

// ─── Absolute root pitch-class dropdown ──────────────────────────────────

pub(super) fn pitch_class_options() -> Vec<SelectOption> {
    use PitchClass::*;
    let entries = [
        (C, "C"),
        (CSharp, "C#"),
        (D, "D"),
        (DSharp, "D#"),
        (E, "E"),
        (F, "F"),
        (FSharp, "F#"),
        (G, "G"),
        (GSharp, "G#"),
        (A, "A"),
        (ASharp, "A#"),
        (B, "B"),
    ];
    entries
        .iter()
        .map(|(pc, label)| SelectOption::new(encode_pitch_class(*pc), *label))
        .collect()
}

pub(super) fn encode_pitch_class(pc: PitchClass) -> String {
    format!("{}", pc.semitones_from_c())
}

fn decode_pitch_class(s: &str) -> Option<PitchClass> {
    let n: i32 = s.parse().ok()?;
    if !(0..12).contains(&n) {
        return None;
    }
    Some(PitchClass::from_semitones_mod12(n))
}

pub(super) fn current_absolute_root_str(id: ChordLoopId, idx: usize) -> String {
    match fetch_event(id, idx).map(|ev| ev.chord) {
        Some(ChordSpec::Absolute { root, .. }) => encode_pitch_class(root),
        _ => "0".into(),
    }
}

pub(super) fn commit_absolute_root(id: ChordLoopId, idx: usize, encoded: String) {
    let Some(pc) = decode_pitch_class(&encoded) else { return };
    mutate_event(id, idx, move |ev| {
        if let ChordSpec::Absolute { root, .. } = &mut ev.chord {
            *root = pc;
        }
    });
}

// ─── Roman + Quality (existing) ──────────────────────────────────────────

pub(super) fn roman_options() -> Vec<SelectOption> {
    use RomanDegree::*;
    let entries = [
        I, II, III, IV, V, VI, VII, FlatII, FlatIII, FlatV, FlatVI, FlatVII, SharpI, SharpII,
        SharpIV, SharpV, SharpVI,
    ];
    entries
        .iter()
        .map(|r| SelectOption::new(encode_roman(*r), roman_label(*r)))
        .collect()
}

pub(super) fn encode_roman(r: RomanDegree) -> String {
    format!("{r:?}")
}

pub(super) fn decode_roman(s: &str) -> Option<RomanDegree> {
    use RomanDegree::*;
    Some(match s {
        "I" => I, "II" => II, "III" => III, "IV" => IV, "V" => V, "VI" => VI, "VII" => VII,
        "FlatII" => FlatII, "FlatIII" => FlatIII, "FlatV" => FlatV, "FlatVI" => FlatVI,
        "FlatVII" => FlatVII, "SharpI" => SharpI, "SharpII" => SharpII, "SharpIV" => SharpIV,
        "SharpV" => SharpV, "SharpVI" => SharpVI,
        _ => return None,
    })
}

fn roman_label(r: RomanDegree) -> String {
    use RomanDegree::*;
    match r {
        I => "I", II => "II", III => "III", IV => "IV", V => "V", VI => "VI", VII => "VII",
        FlatII => "♭II", FlatIII => "♭III", FlatV => "♭V", FlatVI => "♭VI", FlatVII => "♭VII",
        SharpI => "♯I", SharpII => "♯II", SharpIV => "♯IV", SharpV => "♯V", SharpVI => "♯VI",
    }
    .into()
}

pub(super) fn commit_roman(id: ChordLoopId, idx: usize, encoded: String) {
    let Some(new_roman) = decode_roman(&encoded) else { return };
    mutate_event(id, idx, move |ev| {
        if let ChordSpec::Functional { roman, .. } = &mut ev.chord {
            *roman = new_roman;
        }
    });
}

pub(super) fn quality_options() -> Vec<SelectOption> {
    use ChordQuality::*;
    let entries: &[(ChordQuality, &str)] = &[
        (Major, "Major"),
        (Minor, "Minor"),
        (Diminished, "Diminished"),
        (Augmented, "Augmented"),
        (Major7, "Major 7"),
        (Minor7, "Minor 7"),
        (Dominant7, "Dominant 7"),
        (Diminished7, "Diminished 7"),
        (HalfDiminished7, "Half-Diminished 7 (m7♭5)"),
        (MinorMajor7, "Minor-Major 7"),
        (AugmentedDom7, "Augmented Dominant 7"),
        (Major6, "Major 6"),
        (Minor6, "Minor 6"),
        (Sus2, "Sus 2"),
        (Sus4, "Sus 4"),
        (Sus7, "7 Sus 4"),
        (Sus9, "9 Sus 4"),
        (Power, "Power (root + 5)"),
    ];
    entries
        .iter()
        .map(|(q, label)| SelectOption::new(encode_quality(q), *label))
        .collect()
}

pub(super) fn encode_quality(q: &ChordQuality) -> String {
    use ChordQuality::*;
    match q {
        Major => "Major", Minor => "Minor", Diminished => "Diminished", Augmented => "Augmented",
        Major7 => "Major7", Minor7 => "Minor7", Dominant7 => "Dominant7",
        Diminished7 => "Diminished7", HalfDiminished7 => "HalfDiminished7",
        MinorMajor7 => "MinorMajor7", AugmentedDom7 => "AugmentedDom7",
        Major6 => "Major6", Minor6 => "Minor6",
        Sus2 => "Sus2", Sus4 => "Sus4", Sus7 => "Sus7", Sus9 => "Sus9",
        Power => "Power", Custom { .. } => "Custom",
    }
    .to_string()
}

pub(super) fn decode_quality(s: &str) -> Option<ChordQuality> {
    use ChordQuality::*;
    Some(match s {
        "Major" => Major, "Minor" => Minor, "Diminished" => Diminished, "Augmented" => Augmented,
        "Major7" => Major7, "Minor7" => Minor7, "Dominant7" => Dominant7,
        "Diminished7" => Diminished7, "HalfDiminished7" => HalfDiminished7,
        "MinorMajor7" => MinorMajor7, "AugmentedDom7" => AugmentedDom7,
        "Major6" => Major6, "Minor6" => Minor6,
        "Sus2" => Sus2, "Sus4" => Sus4, "Sus7" => Sus7, "Sus9" => Sus9,
        "Power" => Power,
        _ => return None,
    })
}

pub(super) fn commit_quality(id: ChordLoopId, idx: usize, encoded: String) {
    let Some(new_quality) = decode_quality(&encoded) else { return };
    mutate_event(id, idx, move |ev| match &mut ev.chord {
        ChordSpec::Functional { suffix, .. } | ChordSpec::Absolute { suffix, .. } => {
            suffix.quality = new_quality;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawdaw_model::scale::Scale;

    #[test]
    fn pitch_class_to_roman_diatonic_in_c_major() {
        let c_major = Scale::major(PitchClass::C);
        use RomanDegree::*;
        assert_eq!(pitch_class_to_roman(PitchClass::C, &c_major), Some(I));
        assert_eq!(pitch_class_to_roman(PitchClass::D, &c_major), Some(II));
        assert_eq!(pitch_class_to_roman(PitchClass::E, &c_major), Some(III));
        assert_eq!(pitch_class_to_roman(PitchClass::F, &c_major), Some(IV));
        assert_eq!(pitch_class_to_roman(PitchClass::G, &c_major), Some(V));
        assert_eq!(pitch_class_to_roman(PitchClass::A, &c_major), Some(VI));
        assert_eq!(pitch_class_to_roman(PitchClass::B, &c_major), Some(VII));
    }

    #[test]
    fn pitch_class_to_roman_chromatic_in_c_major() {
        let c_major = Scale::major(PitchClass::C);
        use RomanDegree::*;
        // ASharp / Bb is bVII (degree 7 with -1 offset), not #VI.
        assert_eq!(pitch_class_to_roman(PitchClass::ASharp, &c_major), Some(FlatVII));
        // DSharp / Eb is bIII.
        assert_eq!(pitch_class_to_roman(PitchClass::DSharp, &c_major), Some(FlatIII));
        // FSharp is #IV (no flat-V at degree-5 cleanly available — actually
        // FSharp is the tritone, both `#IV` and `bV` are valid; the
        // flat-first preference picks `bV`).
        assert_eq!(pitch_class_to_roman(PitchClass::FSharp, &c_major), Some(FlatV));
    }

    #[test]
    fn pitch_class_to_roman_round_trips_via_resolve() {
        // For each diatonic degree, Functional → Absolute → Functional
        // should round-trip (matches CL3 lossless requirement).
        use rawdaw_model::realize::resolve::resolve_chord_root;
        let c_major = Scale::major(PitchClass::C);
        use RomanDegree::*;
        for r in [I, II, III, IV, V, VI, VII] {
            let pc = resolve_chord_root(r, &c_major);
            assert_eq!(
                pitch_class_to_roman(pc, &c_major),
                Some(r),
                "diatonic round-trip failed for {:?}",
                r
            );
        }
    }

    #[test]
    fn roman_encode_decode_round_trips() {
        use RomanDegree::*;
        for r in [I, II, III, IV, V, VI, VII, FlatII, FlatIII, FlatV, FlatVI, FlatVII,
            SharpI, SharpII, SharpIV, SharpV, SharpVI] {
            assert_eq!(decode_roman(&encode_roman(r)), Some(r));
        }
    }

    #[test]
    fn quality_encode_decode_round_trips_for_named_variants() {
        use ChordQuality::*;
        for q in [Major, Minor, Diminished, Augmented, Major7, Minor7, Dominant7,
            Diminished7, HalfDiminished7, MinorMajor7, AugmentedDom7, Major6, Minor6,
            Sus2, Sus4, Sus7, Sus9, Power] {
            assert_eq!(decode_quality(&encode_quality(&q)), Some(q));
        }
    }

    #[test]
    fn quality_custom_decodes_to_none() {
        let custom = ChordQuality::Custom { intervals: vec![0, 3, 7] };
        assert_eq!(encode_quality(&custom), "Custom");
        assert_eq!(decode_quality("Custom"), None);
    }
}
