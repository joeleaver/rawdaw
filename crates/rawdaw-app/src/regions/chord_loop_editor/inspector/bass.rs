//! Bass-spec editor — kind dropdown + conditional value sub-editor.
//!
//! All four `BassSpec` variants have first-class editors:
//! - **Inversion**: dropdown (1st/2nd/3rd).
//! - **Absolute**: pitch-class dropdown (12 entries).
//! - **ChordDegree** (added CL3): chord-step dropdown + accidental
//!   dropdown. The shorthand parser doesn't reach this bass kind
//!   (grammar §3); the inspector is the only entry point.
//! - **ScaleDegree** (added CL3): scale-degree dropdown + accidental
//!   dropdown. Also inspector-only.

use rinch::prelude::*;

use rawdaw_model::chord::{BassSpec, ChordDegree, ChordStep};
use rawdaw_model::id::ChordLoopId;
use rawdaw_model::pitch::{Accidental, PitchClass};
use rawdaw_model::scale::ScaleDegree;

use crate::theme;

use super::{fetch_event, mutate_event};

#[component]
pub(super) fn BassEditor(id: ChordLoopId, idx: usize) -> NodeHandle {
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 6px;",
            Select {
                size: "sm",
                value_fn: move || current_bass_kind_str(id, idx),
                data: bass_kind_options(),
                onchange: move |v: String| commit_bass_kind(id, idx, v),
            }
            BassValueEditor { id: id, idx: idx }
        }
    }
}

fn current_bass_kind_str(id: ChordLoopId, idx: usize) -> String {
    bass_kind(&fetch_event(id, idx).and_then(|ev| ev.bass))
}

fn bass_kind(bass: &Option<BassSpec>) -> String {
    match bass {
        None => "None",
        Some(BassSpec::Inversion(_)) => "Inversion",
        Some(BassSpec::ChordDegree(_)) => "ChordDegree",
        Some(BassSpec::ScaleDegree(_)) => "ScaleDegree",
        Some(BassSpec::Absolute(_)) => "Absolute",
    }
    .into()
}

fn bass_kind_options() -> Vec<SelectOption> {
    vec![
        SelectOption::new("None", "Root position (default)"),
        SelectOption::new("Inversion", "Inversion (1st / 2nd / 3rd)"),
        SelectOption::new("ChordDegree", "Chord-tone bass (step + accidental)"),
        SelectOption::new("ScaleDegree", "Scale-tone bass (degree + accidental)"),
        SelectOption::new("Absolute", "Absolute pitch"),
    ]
}

fn commit_bass_kind(id: ChordLoopId, idx: usize, kind: String) {
    let new_bass: Option<BassSpec> = match kind.as_str() {
        "None" => None,
        "Inversion" => Some(BassSpec::Inversion(1)),
        "Absolute" => Some(BassSpec::Absolute(PitchClass::C)),
        "ChordDegree" => Some(BassSpec::ChordDegree(
            rawdaw_model::chord::ChordDegree::new(rawdaw_model::chord::ChordStep::Root),
        )),
        "ScaleDegree" => Some(BassSpec::ScaleDegree(rawdaw_model::scale::ScaleDegree::new(1))),
        _ => return,
    };
    mutate_event(id, idx, move |ev| {
        ev.bass = new_bass.clone();
    });
}

/// Bass value editor — the kind-specific surface beneath the bass
/// kind dropdown. The `match` lives inside `rsx!` so the macro
/// wraps it in an Effect (rsx Rule 14): when the bass kind changes
/// via [`commit_bass_kind`] the right arm remounts surgically.
/// `value_fn` reads inside each arm keep number/pitch-class
/// displays reactive within a kind.
#[component]
fn BassValueEditor(id: ChordLoopId, idx: usize) -> NodeHandle {
    rsx! {
        match current_bass_variant(id, idx) {
            BassVariant::None => span {
                style: bass_note_style(),
                "Bass plays the chord's root."
            },
            BassVariant::Inversion => Select {
                size: "sm",
                value_fn: move || current_inversion_str(id, idx),
                data: inversion_options(),
                onchange: move |v: String| commit_inversion(id, idx, v),
            },
            BassVariant::Absolute => Select {
                size: "sm",
                value_fn: move || current_absolute_bass_str(id, idx),
                data: pitch_class_options(),
                onchange: move |v: String| commit_absolute_bass(id, idx, v),
            },
            BassVariant::ChordDegree => ChordDegreeEditor { id: id, idx: idx },
            BassVariant::ScaleDegree => ScaleDegreeEditor { id: id, idx: idx },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BassVariant {
    None,
    Inversion,
    Absolute,
    ChordDegree,
    ScaleDegree,
}

fn current_bass_variant(id: ChordLoopId, idx: usize) -> BassVariant {
    match fetch_event(id, idx).and_then(|ev| ev.bass) {
        None => BassVariant::None,
        Some(BassSpec::Inversion(_)) => BassVariant::Inversion,
        Some(BassSpec::Absolute(_)) => BassVariant::Absolute,
        Some(BassSpec::ChordDegree(_)) => BassVariant::ChordDegree,
        Some(BassSpec::ScaleDegree(_)) => BassVariant::ScaleDegree,
    }
}

fn current_inversion_str(id: ChordLoopId, idx: usize) -> String {
    match fetch_event(id, idx).and_then(|ev| ev.bass) {
        Some(BassSpec::Inversion(n)) => n.to_string(),
        _ => "1".into(),
    }
}

fn current_absolute_bass_str(id: ChordLoopId, idx: usize) -> String {
    match fetch_event(id, idx).and_then(|ev| ev.bass) {
        Some(BassSpec::Absolute(pc)) => encode_pitch_class(pc),
        _ => "0".into(),
    }
}

fn bass_note_style() -> String {
    format!(
        "font-size: 11px; color: {text2};",
        text2 = theme::TEXT2,
    )
}

fn inversion_options() -> Vec<SelectOption> {
    vec![
        SelectOption::new("1", "1st inversion (3rd in bass)"),
        SelectOption::new("2", "2nd inversion (5th in bass)"),
        SelectOption::new("3", "3rd inversion (7th in bass)"),
    ]
}

fn commit_inversion(id: ChordLoopId, idx: usize, encoded: String) {
    let Ok(n) = encoded.parse::<u8>() else { return };
    if !(1..=3).contains(&n) {
        return;
    }
    mutate_event(id, idx, move |ev| {
        ev.bass = Some(BassSpec::Inversion(n));
    });
}

fn pitch_class_options() -> Vec<SelectOption> {
    use PitchClass::*;
    let entries = [
        (C, "C"), (CSharp, "C#"), (D, "D"), (DSharp, "D#"), (E, "E"),
        (F, "F"), (FSharp, "F#"), (G, "G"), (GSharp, "G#"), (A, "A"),
        (ASharp, "A#"), (B, "B"),
    ];
    entries
        .iter()
        .map(|(pc, label)| SelectOption::new(encode_pitch_class(*pc), *label))
        .collect()
}

fn encode_pitch_class(pc: PitchClass) -> String {
    format!("{}", pc.semitones_from_c())
}

fn decode_pitch_class(s: &str) -> Option<PitchClass> {
    let n: i32 = s.parse().ok()?;
    if !(0..12).contains(&n) {
        return None;
    }
    Some(PitchClass::from_semitones_mod12(n))
}

fn commit_absolute_bass(id: ChordLoopId, idx: usize, encoded: String) {
    let Some(pc) = decode_pitch_class(&encoded) else { return };
    mutate_event(id, idx, move |ev| {
        ev.bass = Some(BassSpec::Absolute(pc));
    });
}

// ─── ChordDegree sub-editor ──────────────────────────────────────────────

#[component]
fn ChordDegreeEditor(id: ChordLoopId, idx: usize) -> NodeHandle {
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 4px;",
            Select {
                size: "sm",
                value_fn: move || current_chord_step_str(id, idx),
                data: chord_step_options(),
                onchange: move |v: String| commit_chord_step(id, idx, v),
            }
            Select {
                size: "sm",
                value_fn: move || current_chord_degree_accidental_str(id, idx),
                data: accidental_options(),
                onchange: move |v: String| commit_chord_degree_accidental(id, idx, v),
            }
        }
    }
}

fn current_chord_step_str(id: ChordLoopId, idx: usize) -> String {
    let cd = fetch_chord_degree(id, idx).unwrap_or_else(|| ChordDegree::new(ChordStep::Root));
    encode_chord_step(cd.step)
}

fn current_chord_degree_accidental_str(id: ChordLoopId, idx: usize) -> String {
    let cd = fetch_chord_degree(id, idx).unwrap_or_else(|| ChordDegree::new(ChordStep::Root));
    encode_accidental(cd.accidental)
}

fn fetch_chord_degree(id: ChordLoopId, idx: usize) -> Option<ChordDegree> {
    fetch_event(id, idx).and_then(|ev| match ev.bass {
        Some(BassSpec::ChordDegree(cd)) => Some(cd),
        _ => None,
    })
}

fn chord_step_options() -> Vec<SelectOption> {
    use ChordStep::*;
    let entries: &[(ChordStep, &str)] = &[
        (Root, "Root"),
        (Second, "2nd"),
        (Third, "3rd"),
        (Fourth, "4th"),
        (Fifth, "5th"),
        (Sixth, "6th"),
        (Seventh, "7th"),
        (Ninth, "9th"),
        (Eleventh, "11th"),
        (Thirteenth, "13th"),
    ];
    entries
        .iter()
        .map(|(step, label)| SelectOption::new(encode_chord_step(*step), *label))
        .collect()
}

fn encode_chord_step(s: ChordStep) -> String {
    format!("{s:?}")
}

fn decode_chord_step(s: &str) -> Option<ChordStep> {
    use ChordStep::*;
    Some(match s {
        "Root" => Root,
        "Second" => Second,
        "Third" => Third,
        "Fourth" => Fourth,
        "Fifth" => Fifth,
        "Sixth" => Sixth,
        "Seventh" => Seventh,
        "Ninth" => Ninth,
        "Eleventh" => Eleventh,
        "Thirteenth" => Thirteenth,
        _ => return None,
    })
}

fn commit_chord_step(id: ChordLoopId, idx: usize, encoded: String) {
    let Some(new_step) = decode_chord_step(&encoded) else { return };
    mutate_event(id, idx, move |ev| {
        if let Some(BassSpec::ChordDegree(cd)) = &mut ev.bass {
            cd.step = new_step;
        }
    });
}

fn commit_chord_degree_accidental(id: ChordLoopId, idx: usize, encoded: String) {
    let Some(acc) = decode_accidental(&encoded) else { return };
    mutate_event(id, idx, move |ev| {
        if let Some(BassSpec::ChordDegree(cd)) = &mut ev.bass {
            cd.accidental = acc;
        }
    });
}

// ─── ScaleDegree sub-editor ──────────────────────────────────────────────

#[component]
fn ScaleDegreeEditor(id: ChordLoopId, idx: usize) -> NodeHandle {
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 4px;",
            Select {
                size: "sm",
                value_fn: move || current_scale_degree_num_str(id, idx),
                data: scale_degree_options(),
                onchange: move |v: String| commit_scale_degree_num(id, idx, v),
            }
            Select {
                size: "sm",
                value_fn: move || current_scale_degree_accidental_str(id, idx),
                data: accidental_options(),
                onchange: move |v: String| commit_scale_degree_accidental(id, idx, v),
            }
        }
    }
}

fn current_scale_degree_num_str(id: ChordLoopId, idx: usize) -> String {
    let sd = fetch_scale_degree(id, idx).unwrap_or_else(|| ScaleDegree::new(1));
    sd.degree.to_string()
}

fn current_scale_degree_accidental_str(id: ChordLoopId, idx: usize) -> String {
    let sd = fetch_scale_degree(id, idx).unwrap_or_else(|| ScaleDegree::new(1));
    encode_accidental(sd.accidental)
}

fn fetch_scale_degree(id: ChordLoopId, idx: usize) -> Option<ScaleDegree> {
    fetch_event(id, idx).and_then(|ev| match ev.bass {
        Some(BassSpec::ScaleDegree(sd)) => Some(sd),
        _ => None,
    })
}

fn scale_degree_options() -> Vec<SelectOption> {
    // 1..=7 covers diatonic and most non-pentatonic modes. Pentatonic
    // (5 degrees) accepts 1..=5; the parser/realizer enforces bounds.
    (1u8..=7)
        .map(|n| SelectOption::new(n.to_string(), format!("Degree {n}")))
        .collect()
}

fn commit_scale_degree_num(id: ChordLoopId, idx: usize, encoded: String) {
    let Ok(n) = encoded.parse::<u8>() else { return };
    if !(1..=12).contains(&n) {
        return;
    }
    mutate_event(id, idx, move |ev| {
        if let Some(BassSpec::ScaleDegree(sd)) = &mut ev.bass {
            sd.degree = n;
        }
    });
}

fn commit_scale_degree_accidental(id: ChordLoopId, idx: usize, encoded: String) {
    let Some(acc) = decode_accidental(&encoded) else { return };
    mutate_event(id, idx, move |ev| {
        if let Some(BassSpec::ScaleDegree(sd)) = &mut ev.bass {
            sd.accidental = acc;
        }
    });
}

// ─── Shared accidental helpers ───────────────────────────────────────────

fn accidental_options() -> Vec<SelectOption> {
    use Accidental::*;
    let entries: &[(Accidental, &str)] = &[
        (Natural, "Natural"),
        (Flat, "♭ Flat"),
        (Sharp, "♯ Sharp"),
        (DoubleFlat, "𝄫 Double Flat"),
        (DoubleSharp, "𝄪 Double Sharp"),
    ];
    entries
        .iter()
        .map(|(a, label)| SelectOption::new(encode_accidental(*a), *label))
        .collect()
}

fn encode_accidental(a: Accidental) -> String {
    format!("{a:?}")
}

fn decode_accidental(s: &str) -> Option<Accidental> {
    use Accidental::*;
    Some(match s {
        "Natural" => Natural,
        "Flat" => Flat,
        "Sharp" => Sharp,
        "DoubleFlat" => DoubleFlat,
        "DoubleSharp" => DoubleSharp,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_pitch_class_rejects_out_of_range() {
        assert_eq!(decode_pitch_class("0"), Some(PitchClass::C));
        assert_eq!(decode_pitch_class("11"), Some(PitchClass::B));
        assert_eq!(decode_pitch_class("12"), None);
        assert_eq!(decode_pitch_class("-1"), None);
        assert_eq!(decode_pitch_class("abc"), None);
    }

    #[test]
    fn chord_step_encode_decode_round_trips() {
        use ChordStep::*;
        for s in [
            Root, Second, Third, Fourth, Fifth, Sixth, Seventh, Ninth, Eleventh,
            Thirteenth,
        ] {
            assert_eq!(decode_chord_step(&encode_chord_step(s)), Some(s));
        }
    }

    #[test]
    fn accidental_encode_decode_round_trips() {
        use Accidental::*;
        for a in [Natural, Flat, Sharp, DoubleFlat, DoubleSharp] {
            assert_eq!(decode_accidental(&encode_accidental(a)), Some(a));
        }
    }
}
