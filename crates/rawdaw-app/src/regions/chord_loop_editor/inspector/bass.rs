//! Bass-spec editor — kind dropdown + conditional value sub-editor.
//!
//! CL2 supports Inversion (1st/2nd/3rd) + Absolute pitch fully;
//! ChordDegree + ScaleDegree sub-types render a deferral banner
//! until CL3 ships their richer sub-editors alongside the
//! shorthand parser.

use rinch::prelude::*;

use rawdaw_model::chord::BassSpec;
use rawdaw_model::id::ChordLoopId;
use rawdaw_model::pitch::PitchClass;

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
        SelectOption::new("ChordDegree", "Chord-tone bass (CL3 expands)"),
        SelectOption::new("ScaleDegree", "Scale-tone bass (CL3 expands)"),
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

#[component]
fn BassValueEditor(id: ChordLoopId, idx: usize) -> NodeHandle {
    let bass = fetch_event(id, idx).and_then(|ev| ev.bass);
    match bass {
        Some(BassSpec::Inversion(n)) => rsx! {
            Select {
                size: "sm",
                value_fn: move || n.to_string(),
                data: inversion_options(),
                onchange: move |v: String| commit_inversion(id, idx, v),
            }
        },
        Some(BassSpec::Absolute(pc)) => rsx! {
            Select {
                size: "sm",
                value_fn: move || encode_pitch_class(pc),
                data: pitch_class_options(),
                onchange: move |v: String| commit_absolute_bass(id, idx, v),
            }
        },
        Some(BassSpec::ChordDegree(_)) | Some(BassSpec::ScaleDegree(_)) => {
            let style = format!(
                "font-size: 11px; color: {text2}; line-height: 1.4;",
                text2 = theme::TEXT2,
            );
            rsx! {
                span { style: {style.clone()},
                    "Detailed bass-degree editor lands in CL3."
                }
            }
        }
        None => {
            let style = format!(
                "font-size: 11px; color: {text2};",
                text2 = theme::TEXT2,
            );
            rsx! {
                span { style: {style.clone()},
                    "Bass plays the chord's root."
                }
            }
        }
    }
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
}
