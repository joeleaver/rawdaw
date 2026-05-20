//! `in_key` (borrowed-key) dropdown, cadence tag dropdown, and
//! free-text comment input.
//!
//! `in_key` is the key-mechanism that handles secondary dominants
//! / modal interchange / brief tonicization per
//! `docs/design/chord-loops.md`. Cadence + Comment live on
//! `ChordEvent.annotation`; v1 surfaces them as display + persist
//! only — drum-fill hints from cadence tags are deferred (CL plan
//! out-of-scope).

use rinch::prelude::*;

use rawdaw_model::chord::{Annotation, CadenceTag, ChordSpec};
use rawdaw_model::id::ChordLoopId;
use rawdaw_model::pitch::PitchClass;
use rawdaw_model::scale::{Mode, Scale};

use super::mutate_event;

// ─── in_key (Borrowed key) ───────────────────────────────────────────────

pub(super) fn in_key_options() -> Vec<SelectOption> {
    use crate::chord_display::pitch_class_name;
    let mut out = vec![SelectOption::new("none", "None (use section key)")];
    for tonic_idx in 0..12_i32 {
        let pc = PitchClass::from_semitones_mod12(tonic_idx);
        let name = pitch_class_name(pc);
        out.push(SelectOption::new(
            format!("{tonic_idx}/major"),
            format!("{name} major"),
        ));
        out.push(SelectOption::new(
            format!("{tonic_idx}/minor"),
            format!("{name} minor"),
        ));
    }
    out
}

pub(super) fn encode_scale(scale: &Scale) -> String {
    let tonic = scale.tonic.semitones_from_c();
    let mode = match scale.mode {
        Mode::Aeolian => "minor",
        _ => "major",
    };
    format!("{tonic}/{mode}")
}

fn decode_scale(s: &str) -> Option<Scale> {
    let (tonic_str, mode_str) = s.split_once('/')?;
    let tonic_idx: i32 = tonic_str.parse().ok()?;
    if !(0..12).contains(&tonic_idx) {
        return None;
    }
    let tonic = PitchClass::from_semitones_mod12(tonic_idx);
    let mode = match mode_str {
        "major" => Mode::Ionian,
        "minor" => Mode::Aeolian,
        _ => return None,
    };
    Some(Scale::new(tonic, mode))
}

pub(super) fn commit_in_key(id: ChordLoopId, idx: usize, encoded: String) {
    let new_value = if encoded == "none" {
        None
    } else if let Some(scale) = decode_scale(&encoded) {
        Some(scale)
    } else {
        return;
    };
    mutate_event(id, idx, move |ev| {
        if let ChordSpec::Functional { in_key, .. } = &mut ev.chord {
            *in_key = new_value.clone();
        }
    });
}

// ─── Cadence ─────────────────────────────────────────────────────────────

pub(super) fn cadence_options() -> Vec<SelectOption> {
    vec![
        SelectOption::new("none", "None"),
        SelectOption::new("PerfectAuthentic", "Perfect Authentic"),
        SelectOption::new("ImperfectAuthentic", "Imperfect Authentic"),
        SelectOption::new("Half", "Half"),
        SelectOption::new("Plagal", "Plagal"),
        SelectOption::new("Deceptive", "Deceptive"),
        SelectOption::new("Phrygian", "Phrygian"),
    ]
}

pub(super) fn encode_cadence_opt(c: Option<CadenceTag>) -> String {
    match c {
        None => "none".into(),
        Some(c) => format!("{c:?}"),
    }
}

fn decode_cadence(s: &str) -> Option<CadenceTag> {
    use CadenceTag::*;
    Some(match s {
        "PerfectAuthentic" => PerfectAuthentic,
        "ImperfectAuthentic" => ImperfectAuthentic,
        "Half" => Half,
        "Plagal" => Plagal,
        "Deceptive" => Deceptive,
        "Phrygian" => Phrygian,
        _ => return None,
    })
}

pub(super) fn commit_cadence(id: ChordLoopId, idx: usize, encoded: String) {
    let new_value = if encoded == "none" {
        None
    } else {
        decode_cadence(&encoded)
    };
    mutate_event(id, idx, move |ev| {
        let ann = ev.annotation.get_or_insert_with(Annotation::default);
        ann.cadence = new_value;
        if ann.cadence.is_none() && ann.comment.is_none() {
            ev.annotation = None;
        }
    });
}

// ─── Comment ─────────────────────────────────────────────────────────────

#[component]
pub(super) fn CommentInput(id: ChordLoopId, idx: usize, seed: String) -> NodeHandle {
    // Use `TextInput` rather than raw `input { onsubmit }`: rinch's
    // html.rs codegen routes `onsubmit` on a raw input to `data-rid`
    // (catch-all) instead of `data-onsubmit`, so the Enter handler
    // would never fire. The `TextInput` component sets the right
    // attribute. Surfaced during CL3 visual verification.
    let buffer = Signal::new(seed);
    rsx! {
        TextInput {
            size: "sm",
            placeholder: "Comment…",
            value_fn: move || buffer.get(),
            oninput: move |v: String| buffer.set(v),
            onsubmit: move || commit_comment(id, idx, buffer.get()),
        }
    }
}

fn commit_comment(id: ChordLoopId, idx: usize, raw: String) {
    let trimmed = raw.trim().to_string();
    mutate_event(id, idx, move |ev| {
        let ann = ev.annotation.get_or_insert_with(Annotation::default);
        ann.comment = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.clone())
        };
        if ann.cadence.is_none() && ann.comment.is_none() {
            ev.annotation = None;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_encode_decode_round_trips() {
        for tonic_idx in 0..12_i32 {
            for mode in [Mode::Ionian, Mode::Aeolian] {
                let pc = PitchClass::from_semitones_mod12(tonic_idx);
                let scale = Scale::new(pc, mode.clone());
                let encoded = encode_scale(&scale);
                assert_eq!(decode_scale(&encoded), Some(scale));
            }
        }
    }

    #[test]
    fn cadence_encode_decode_round_trips() {
        use CadenceTag::*;
        for c in [PerfectAuthentic, ImperfectAuthentic, Half, Plagal, Deceptive, Phrygian] {
            assert_eq!(decode_cadence(&encode_cadence_opt(Some(c))), Some(c));
        }
        assert_eq!(encode_cadence_opt(None), "none");
    }
}
