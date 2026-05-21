//! PitchSpec sub-editors for the per-note inspector.
//!
//! The five `PitchSpec` shapes (Scale / Chord / Absolute / Chromatic /
//! Rest) each get their own sub-editor mounted by `FocusedNoteFields`
//! in the parent shell. `PitchSpecKindGroup` is the kind selector;
//! `OctaveRow` + `AnchoredOctavePickerWrapper` are shared by the
//! Scale and Chord editors.

use rinch::core::reactive::{untracked, Effect};
use rinch::prelude::*;

use rawdaw_model::id::{NoteId, PatternId};
use rawdaw_model::pattern::{OctaveSpec, PitchSpec};
use rawdaw_model::pitch::{Accidental, Octave, PitchClass};
use rawdaw_model::scale::ScaleDegree;

use crate::pattern_actions::update_pitched_event;
use crate::regions::pattern_editor::pitched::helpers::DEFAULT_ANCHOR_OCTAVE;
use crate::state::AppState;
use crate::theme;

use super::octave::OctaveRow;
use super::{current_variant, fetch_event, field_group_style, field_label_style};

// ─── PitchSpec kind discriminator ─────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum PitchSpecKind {
    #[default]
    Scale,
    Chord,
    Absolute,
    Chromatic,
    Rest,
}

impl PitchSpecKind {
    pub(super) fn from_spec(spec: &PitchSpec) -> Self {
        match spec {
            PitchSpec::Scale { .. } => Self::Scale,
            PitchSpec::Chord { .. } => Self::Chord,
            PitchSpec::Absolute { .. } => Self::Absolute,
            PitchSpec::Chromatic { .. } => Self::Chromatic,
            PitchSpec::Rest => Self::Rest,
        }
    }

    pub(super) fn as_code(self) -> u8 {
        match self {
            Self::Scale => 0,
            Self::Chord => 1,
            Self::Absolute => 2,
            Self::Chromatic => 3,
            Self::Rest => 4,
        }
    }
}

fn pitch_spec_options() -> Vec<SelectOption> {
    vec![
        SelectOption::new("scale", "Scale degree"),
        SelectOption::new("chord", "Chord degree"),
        SelectOption::new("absolute", "Absolute pitch"),
        SelectOption::new("chromatic", "Chromatic offset"),
        SelectOption::new("rest", "Rest"),
    ]
}

#[component]
pub(super) fn PitchSpecKindGroup(
    pattern_id: PatternId,
    note_id_value: u64,
    kind_code: u8,
) -> NodeHandle {
    let note_id = NoteId::new(note_id_value);
    let initial_value = match kind_code {
        0 => "scale",
        1 => "chord",
        2 => "absolute",
        3 => "chromatic",
        _ => "rest",
    }
    .to_string();
    rsx! {
        div { style: {field_group_style()},
            span { style: {field_label_style()}, "Spec" }
            Select {
                size: "sm",
                value: {initial_value},
                data: pitch_spec_options(),
                onchange: move |v: String| commit_pitch_spec_kind(pattern_id, note_id, v),
            }
        }
    }
}

fn commit_pitch_spec_kind(pattern_id: PatternId, note_id: NoteId, kind_str: String) {
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        update_pitched_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            let next = match kind_str.as_str() {
                "scale" => PitchSpec::Scale {
                    degree: ScaleDegree::new(1),
                    octave: OctaveSpec::Anchored(DEFAULT_ANCHOR_OCTAVE),
                },
                "chord" => PitchSpec::Chord {
                    degree: rawdaw_model::chord::ChordDegree::new(
                        rawdaw_model::chord::ChordStep::Root,
                    ),
                    octave: OctaveSpec::Anchored(DEFAULT_ANCHOR_OCTAVE),
                },
                "absolute" => {
                    let octave = source_anchor_octave(&ev.spec).unwrap_or(DEFAULT_ANCHOR_OCTAVE);
                    PitchSpec::Absolute {
                        pitch_class: PitchClass::C,
                        octave,
                    }
                }
                "chromatic" => PitchSpec::Chromatic {
                    semitones_from_prev: 0,
                },
                "rest" => PitchSpec::Rest,
                _ => return,
            };
            ev.spec = next;
        });
    }) {
        eprintln!("pattern_editor: spec kind change failed: {e}");
    }
}

fn source_anchor_octave(spec: &PitchSpec) -> Option<Octave> {
    match spec {
        PitchSpec::Scale {
            octave: OctaveSpec::Anchored(o),
            ..
        }
        | PitchSpec::Chord {
            octave: OctaveSpec::Anchored(o),
            ..
        } => Some(*o),
        PitchSpec::Absolute { octave, .. } => Some(*octave),
        _ => None,
    }
}

// ─── Scale sub-editor ─────────────────────────────────────────────────────

#[component]
pub(super) fn ScaleSubEditor(pattern_id: PatternId, note_id_value: u64) -> NodeHandle {
    let note_id = NoteId::new(note_id_value);
    let degree_options: Vec<SelectOption> = (1..=7u8)
        .map(|d| SelectOption::new(d.to_string(), format!("{d}")))
        .collect();
    let accidental_options = accidental_options();
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 14px;",
            div { style: {field_group_style()},
                span { style: {field_label_style()}, "Degree" }
                div { style: "display: flex; gap: 6px;",
                    Select {
                        size: "sm",
                        value_fn: move || match fetch_event(pattern_id, note_id) {
                            Some(ev) => match &ev.spec {
                                PitchSpec::Scale { degree, .. } => degree.degree.to_string(),
                                _ => "1".to_string(),
                            },
                            None => "1".to_string(),
                        },
                        data: degree_options,
                        onchange: move |v: String| commit_scale_degree(pattern_id, note_id, v),
                    }
                    Select {
                        size: "sm",
                        value_fn: move || match fetch_event(pattern_id, note_id) {
                            Some(ev) => match &ev.spec {
                                PitchSpec::Scale { degree, .. } => {
                                    accidental_to_str(degree.accidental).to_string()
                                }
                                _ => "natural".to_string(),
                            },
                            None => "natural".to_string(),
                        },
                        data: accidental_options,
                        onchange: move |v: String| commit_scale_accidental(pattern_id, note_id, v),
                    }
                }
            }
            div { style: {field_group_style()},
                span { style: {field_label_style()}, "Octave" }
                OctaveRow { pattern_id: pattern_id, note_id_value: note_id_value }
            }
        }
    }
}

fn commit_scale_degree(pattern_id: PatternId, note_id: NoteId, value: String) {
    let parsed = value.trim().parse::<u8>().ok().filter(|d| (1..=7).contains(d));
    let Some(degree_num) = parsed else { return };
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        update_pitched_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            if let PitchSpec::Scale { degree, .. } = &mut ev.spec {
                degree.degree = degree_num;
            }
        });
    }) {
        eprintln!("pattern_editor: set scale degree failed: {e}");
    }
}

fn commit_scale_accidental(pattern_id: PatternId, note_id: NoteId, value: String) {
    let acc = match value.as_str() {
        "flat" => Accidental::Flat,
        "natural" => Accidental::Natural,
        "sharp" => Accidental::Sharp,
        _ => return,
    };
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        update_pitched_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            if let PitchSpec::Scale { degree, .. } = &mut ev.spec {
                degree.accidental = acc;
            }
        });
    }) {
        eprintln!("pattern_editor: set scale accidental failed: {e}");
    }
}

// ─── Chord sub-editor ─────────────────────────────────────────────────────

#[component]
pub(super) fn ChordSubEditor(pattern_id: PatternId, note_id_value: u64) -> NodeHandle {
    let note_id = NoteId::new(note_id_value);
    let step_options = chord_step_options();
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 14px;",
            div { style: {field_group_style()},
                span { style: {field_label_style()}, "Step" }
                Select {
                    size: "sm",
                    value_fn: move || match fetch_event(pattern_id, note_id) {
                        Some(ev) => match &ev.spec {
                            PitchSpec::Chord { degree, .. } => {
                                chord_step_to_str(degree.step).to_string()
                            }
                            _ => "root".to_string(),
                        },
                        None => "root".to_string(),
                    },
                    data: step_options,
                    onchange: move |v: String| commit_chord_step(pattern_id, note_id, v),
                }
            }
            div { style: {field_group_style()},
                span { style: {field_label_style()}, "Octave" }
                OctaveRow { pattern_id: pattern_id, note_id_value: note_id_value }
            }
        }
    }
}

fn commit_chord_step(pattern_id: PatternId, note_id: NoteId, value: String) {
    let Some(step) = chord_step_from_str(&value) else { return };
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        update_pitched_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            if let PitchSpec::Chord { degree, .. } = &mut ev.spec {
                degree.step = step;
            }
        });
    }) {
        eprintln!("pattern_editor: set chord step failed: {e}");
    }
}

// ─── Absolute sub-editor ──────────────────────────────────────────────────

#[component]
pub(super) fn AbsoluteSubEditor(pattern_id: PatternId, note_id_value: u64) -> NodeHandle {
    let note_id = NoteId::new(note_id_value);
    let pc_options = pitch_class_options();
    let octave_options: Vec<SelectOption> = (0..=8i8)
        .map(|o| SelectOption::new(o.to_string(), format!("Octave {o}")))
        .collect();
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 14px;",
            div { style: {field_group_style()},
                span { style: {field_label_style()}, "Pitch class" }
                Select {
                    size: "sm",
                    value_fn: move || match fetch_event(pattern_id, note_id) {
                        Some(ev) => match &ev.spec {
                            PitchSpec::Absolute { pitch_class, .. } => {
                                pitch_class_to_str(*pitch_class).to_string()
                            }
                            _ => "C".to_string(),
                        },
                        None => "C".to_string(),
                    },
                    data: pc_options,
                    onchange: move |v: String| commit_absolute_pc(pattern_id, note_id, v),
                }
            }
            div { style: {field_group_style()},
                span { style: {field_label_style()}, "Octave" }
                Select {
                    size: "sm",
                    value_fn: move || match fetch_event(pattern_id, note_id) {
                        Some(ev) => match &ev.spec {
                            PitchSpec::Absolute { octave, .. } => octave.0.to_string(),
                            _ => DEFAULT_ANCHOR_OCTAVE.0.to_string(),
                        },
                        None => DEFAULT_ANCHOR_OCTAVE.0.to_string(),
                    },
                    data: octave_options,
                    onchange: move |v: String| commit_absolute_octave(pattern_id, note_id, v),
                }
            }
        }
    }
}

fn commit_absolute_pc(pattern_id: PatternId, note_id: NoteId, value: String) {
    let Some(pc) = pitch_class_from_str(&value) else { return };
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        update_pitched_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            if let PitchSpec::Absolute { pitch_class, .. } = &mut ev.spec {
                *pitch_class = pc;
            }
        });
    }) {
        eprintln!("pattern_editor: set absolute pc failed: {e}");
    }
}

fn commit_absolute_octave(pattern_id: PatternId, note_id: NoteId, value: String) {
    let Ok(o) = value.trim().parse::<i8>() else { return };
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        update_pitched_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            if let PitchSpec::Absolute { octave, .. } = &mut ev.spec {
                *octave = Octave(o);
            }
        });
    }) {
        eprintln!("pattern_editor: set absolute octave failed: {e}");
    }
}

// ─── Chromatic sub-editor ─────────────────────────────────────────────────

#[component]
pub(super) fn ChromaticSubEditor(pattern_id: PatternId, note_id_value: u64) -> NodeHandle {
    let note_id = NoteId::new(note_id_value);
    let value_buffer = Signal::new(String::new());

    let _ = Effect::new(move || {
        let canonical = current_chromatic_offset(pattern_id, note_id).unwrap_or(0);
        let typed = untracked(|| value_buffer.get());
        if typed.parse::<i8>().ok() == Some(canonical) {
            return;
        }
        value_buffer.set(canonical.to_string());
    });

    rsx! {
        div { style: {field_group_style()},
            span { style: {field_label_style()}, "Semitones from prev" }
            TextInput {
                size: "sm",
                value_fn: move || value_buffer.get(),
                oninput: move |v: String| value_buffer.set(v),
                onsubmit: move || {
                    if let Ok(n) = value_buffer.get().trim().parse::<i8>() {
                        commit_chromatic(pattern_id, note_id, n);
                    }
                },
            }
        }
    }
}

fn commit_chromatic(pattern_id: PatternId, note_id: NoteId, n: i8) {
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        update_pitched_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            if let PitchSpec::Chromatic {
                semitones_from_prev,
            } = &mut ev.spec
            {
                *semitones_from_prev = n;
            }
        });
    }) {
        eprintln!("pattern_editor: set chromatic offset failed: {e}");
    }
}

fn current_chromatic_offset(pattern_id: PatternId, note_id: NoteId) -> Option<i8> {
    let event = untracked(|| fetch_event(pattern_id, note_id))?;
    match event.spec {
        PitchSpec::Chromatic {
            semitones_from_prev,
        } => Some(semitones_from_prev),
        _ => None,
    }
}

// ─── Rest notice ──────────────────────────────────────────────────────────

#[component]
pub(super) fn RestNoticeRow() -> NodeHandle {
    let style = format!(
        "padding: 8px 10px; border: 1px dashed {line}; border-radius: 4px; \
         color: rgba(232,234,238,0.55); font-size: 11px; line-height: 1.5;",
        line = theme::LINE,
    );
    rsx! {
        div { style: {style.clone()},
            "Rest — no pitch realized. Time and duration still participate in the pattern grid."
        }
    }
}

// ─── Option helpers ───────────────────────────────────────────────────────

fn accidental_options() -> Vec<SelectOption> {
    vec![
        SelectOption::new("flat", "♭"),
        SelectOption::new("natural", "♮"),
        SelectOption::new("sharp", "♯"),
    ]
}

fn accidental_to_str(a: Accidental) -> &'static str {
    match a {
        Accidental::DoubleFlat | Accidental::Flat => "flat",
        Accidental::Natural => "natural",
        Accidental::Sharp | Accidental::DoubleSharp => "sharp",
    }
}

fn chord_step_options() -> Vec<SelectOption> {
    vec![
        SelectOption::new("root", "Root"),
        SelectOption::new("second", "Second"),
        SelectOption::new("third", "Third"),
        SelectOption::new("fourth", "Fourth"),
        SelectOption::new("fifth", "Fifth"),
        SelectOption::new("sixth", "Sixth"),
        SelectOption::new("seventh", "Seventh"),
        SelectOption::new("ninth", "Ninth"),
        SelectOption::new("eleventh", "Eleventh"),
        SelectOption::new("thirteenth", "Thirteenth"),
    ]
}

fn chord_step_to_str(step: rawdaw_model::chord::ChordStep) -> &'static str {
    use rawdaw_model::chord::ChordStep::*;
    match step {
        Root => "root",
        Second => "second",
        Third => "third",
        Fourth => "fourth",
        Fifth => "fifth",
        Sixth => "sixth",
        Seventh => "seventh",
        Ninth => "ninth",
        Eleventh => "eleventh",
        Thirteenth => "thirteenth",
    }
}

fn chord_step_from_str(s: &str) -> Option<rawdaw_model::chord::ChordStep> {
    use rawdaw_model::chord::ChordStep::*;
    Some(match s {
        "root" => Root,
        "second" => Second,
        "third" => Third,
        "fourth" => Fourth,
        "fifth" => Fifth,
        "sixth" => Sixth,
        "seventh" => Seventh,
        "ninth" => Ninth,
        "eleventh" => Eleventh,
        "thirteenth" => Thirteenth,
        _ => return None,
    })
}

fn pitch_class_options() -> Vec<SelectOption> {
    [
        PitchClass::C,
        PitchClass::CSharp,
        PitchClass::D,
        PitchClass::DSharp,
        PitchClass::E,
        PitchClass::F,
        PitchClass::FSharp,
        PitchClass::G,
        PitchClass::GSharp,
        PitchClass::A,
        PitchClass::ASharp,
        PitchClass::B,
    ]
    .iter()
    .map(|pc| {
        let name = crate::chord_display::pitch_class_name(*pc);
        SelectOption::new(pitch_class_to_str(*pc), name)
    })
    .collect()
}

fn pitch_class_to_str(pc: PitchClass) -> &'static str {
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

fn pitch_class_from_str(s: &str) -> Option<PitchClass> {
    Some(match s {
        "C" => PitchClass::C,
        "C#" => PitchClass::CSharp,
        "D" => PitchClass::D,
        "D#" => PitchClass::DSharp,
        "E" => PitchClass::E,
        "F" => PitchClass::F,
        "F#" => PitchClass::FSharp,
        "G" => PitchClass::G,
        "G#" => PitchClass::GSharp,
        "A" => PitchClass::A,
        "A#" => PitchClass::ASharp,
        "B" => PitchClass::B,
        _ => return None,
    })
}
