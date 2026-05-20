//! Roman degree + Chord quality dropdowns — the two harmony fields
//! every chord event surfaces. Functional events expose both; absolute
//! events only expose quality.

use rinch::prelude::*;

use rawdaw_model::chord::{ChordQuality, ChordSpec, RomanDegree};
use rawdaw_model::id::ChordLoopId;

use super::mutate_event;

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
