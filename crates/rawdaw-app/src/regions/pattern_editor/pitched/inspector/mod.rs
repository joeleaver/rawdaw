//! Per-note inspector for the pitched pattern editor.
//!
//! Reads [`AppState::focused_pattern_note`] reactively; mounts the
//! [`FocusedNoteFields`] subtree when a note is focused, otherwise
//! renders a placeholder. Edits flow through the C2 edit pump via
//! [`crate::pattern_actions::update_pitched_event`].
//!
//! File layout (P2 design plan splits this into sub-files when one
//! file approaches the 700-line cap):
//! - `mod.rs` (this file): inspector shell, focused-note resolver,
//!   PitchSpec kind selector, and per-kind field groups.

use rinch::core::reactive::{untracked, Effect};
use rinch::prelude::*;

use rawdaw_model::id::{NoteId, PatternId, VariantId};
use rawdaw_model::pattern::{
    OctaveSpec, PatternBody, PitchSpec, PitchedEvent, PitchedPatternBody,
};
use rawdaw_model::pitch::{Accidental, Octave, PitchClass, U7};
use rawdaw_model::scale::ScaleDegree;
use rawdaw_model::time::{Duration, MusicalTime, PPQ};

use crate::pattern_actions::{delete_pitched_event, update_pitched_event};
use crate::regions::pattern_editor::pitched::helpers::DEFAULT_ANCHOR_OCTAVE;
use crate::state::AppState;
use crate::theme;

#[component]
pub fn Inspector(id: PatternId) -> NodeHandle {
    let pane_style = format!(
        "width: 320px; flex: 0 0 320px; \
         background: {bg1}; border-left: 1px solid {line}; \
         display: flex; flex-direction: column; \
         padding: 16px; gap: 12px; overflow-y: auto;",
        bg1 = theme::BG1,
        line = theme::LINE,
    );

    rsx! {
        aside { style: {pane_style.clone()},
            for _focus_key in inspector_focus_keys(id) {
                FocusedDispatch { key: _focus_key.clone(), pattern_id: id }
            }
        }
    }
}

fn inspector_focus_keys(id: PatternId) -> Vec<String> {
    let app = use_store::<AppState>();
    let key = match app.focused_pattern_note.get() {
        Some(nid) => {
            // Include the PitchSpec kind in the key so swapping
            // Scale ↔ Chord ↔ Absolute (etc.) forces the inspector
            // subtree to remount and render the matching sub-editor
            // arm. Without this, `match kind_code` in
            // `FocusedNoteFields` is evaluated only at construction
            // and the sub-editor stays on the original kind.
            let kind = fetch_event(id, nid)
                .map(|ev| PitchSpecKind::from_spec(&ev.spec).as_code())
                .unwrap_or(255);
            format!("p{}n{}k{}", id.get(), nid.get(), kind)
        }
        None => format!("p{}-empty", id.get()),
    };
    vec![key]
}

#[component]
fn FocusedDispatch(pattern_id: PatternId) -> NodeHandle {
    let app = use_store::<AppState>();
    let Some(nid) = app.focused_pattern_note.get() else {
        return rsx! { EmptyState { } };
    };
    let Some(event) = fetch_event(pattern_id, nid) else {
        return rsx! { EmptyState { } };
    };
    let kind_code = PitchSpecKind::from_spec(&event.spec).as_code();
    rsx! {
        FocusedNoteFields {
            pattern_id: pattern_id,
            note_id_value: nid.get(),
            kind_code: kind_code,
        }
    }
}

#[component]
fn EmptyState() -> NodeHandle {
    let style = format!(
        "padding: 24px 12px; text-align: center; \
         color: {text2}; font-size: 12px; line-height: 1.5;",
        text2 = theme::TEXT2,
    );
    rsx! {
        div { style: {style.clone()},
            "No note selected. Click a cell on the piano roll to insert one, \
             or click an existing note to edit it."
        }
    }
}

#[component]
fn FocusedNoteFields(
    pattern_id: PatternId,
    note_id_value: u64,
    /// `PitchSpecKind` discriminator code (0..=4). Passed as `u8`
    /// rather than the enum because `#[component]` requires every
    /// prop to implement `Default` and the macro's per-arm scrutinee
    /// closure captures by move — a `Copy` type avoids needing
    /// per-arm clones.
    kind_code: u8,
) -> NodeHandle {
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 14px;",
            HeaderRow { pattern_id: pattern_id, note_id_value: note_id_value }
            PitchSpecKindGroup {
                pattern_id: pattern_id,
                note_id_value: note_id_value,
                kind_code: kind_code,
            }
            match kind_code {
                0 => ScaleSubEditor {
                    pattern_id: pattern_id,
                    note_id_value: note_id_value,
                },
                1 => ChordSubEditor {
                    pattern_id: pattern_id,
                    note_id_value: note_id_value,
                },
                2 => AbsoluteSubEditor {
                    pattern_id: pattern_id,
                    note_id_value: note_id_value,
                },
                3 => ChromaticSubEditor {
                    pattern_id: pattern_id,
                    note_id_value: note_id_value,
                },
                _ => RestNoticeRow { },
            }
            TimingGroup { pattern_id: pattern_id, note_id_value: note_id_value }
            DynamicsGroup { pattern_id: pattern_id, note_id_value: note_id_value }
            HumanizationGroup { pattern_id: pattern_id, note_id_value: note_id_value }
        }
    }
}

#[component]
fn HeaderRow(pattern_id: PatternId, note_id_value: u64) -> NodeHandle {
    rsx! {
        div {
            style: "display: flex; align-items: center; justify-content: space-between; \
                    gap: 8px;",
            span {
                style: "font-size: 11px; letter-spacing: 0.4px; \
                        text-transform: uppercase; color: rgba(232,234,238,0.55); \
                        font-weight: 600;",
                {format!("Note #{note_id_value}")}
            }
            button {
                r#type: "button",
                title: "Delete this note",
                style: {danger_btn_style()},
                onclick: move || delete_focused_note(pattern_id, NoteId::new(note_id_value)),
                "Delete"
            }
        }
    }
}

// ─── PitchSpec kind selector ─────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum PitchSpecKind {
    #[default]
    Scale,
    Chord,
    Absolute,
    Chromatic,
    Rest,
}

impl PitchSpecKind {
    fn from_spec(spec: &PitchSpec) -> Self {
        match spec {
            PitchSpec::Scale { .. } => Self::Scale,
            PitchSpec::Chord { .. } => Self::Chord,
            PitchSpec::Absolute { .. } => Self::Absolute,
            PitchSpec::Chromatic { .. } => Self::Chromatic,
            PitchSpec::Rest => Self::Rest,
        }
    }

    fn as_code(self) -> u8 {
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
fn PitchSpecKindGroup(pattern_id: PatternId, note_id_value: u64, kind_code: u8) -> NodeHandle {
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

// ─── Per-spec sub-editors ────────────────────────────────────────────────

#[component]
fn ScaleSubEditor(pattern_id: PatternId, note_id_value: u64) -> NodeHandle {
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

#[component]
fn ChordSubEditor(pattern_id: PatternId, note_id_value: u64) -> NodeHandle {
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

#[component]
fn AbsoluteSubEditor(pattern_id: PatternId, note_id_value: u64) -> NodeHandle {
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

#[component]
fn ChromaticSubEditor(pattern_id: PatternId, note_id_value: u64) -> NodeHandle {
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

#[component]
fn RestNoticeRow() -> NodeHandle {
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

// ─── Octave row (shared by Scale + Chord) ────────────────────────────────

#[component]
fn OctaveRow(pattern_id: PatternId, note_id_value: u64) -> NodeHandle {
    let note_id = NoteId::new(note_id_value);
    let options = octave_spec_options();
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 6px;",
            Select {
                size: "sm",
                value_fn: move || match fetch_event(pattern_id, note_id) {
                    Some(ev) => octave_spec_kind_str(&ev.spec).to_string(),
                    None => "nearest".to_string(),
                },
                data: options,
                onchange: move |v: String| commit_octave_kind(pattern_id, note_id, v),
            }
            for show in octave_anchored_show_keys(pattern_id, note_id) {
                AnchoredOctavePickerWrapper {
                    key: show.clone(),
                    pattern_id: pattern_id,
                    note_id_value: note_id_value,
                    show: show.clone(),
                }
            }
        }
    }
}

fn octave_anchored_show_keys(pattern_id: PatternId, note_id: NoteId) -> Vec<String> {
    let event = fetch_event(pattern_id, note_id);
    let show = matches!(
        event.as_ref().map(|e| &e.spec),
        Some(PitchSpec::Scale {
            octave: OctaveSpec::Anchored(_),
            ..
        }) | Some(PitchSpec::Chord {
            octave: OctaveSpec::Anchored(_),
            ..
        }),
    );
    vec![if show { "y" } else { "n" }.to_string()]
}

#[component]
fn AnchoredOctavePickerWrapper(
    pattern_id: PatternId,
    note_id_value: u64,
    show: String,
) -> NodeHandle {
    if show != "y" {
        return rsx! { div { } };
    }
    let note_id = NoteId::new(note_id_value);
    let options: Vec<SelectOption> = (0..=8i8)
        .map(|o| SelectOption::new(o.to_string(), format!("{o}")))
        .collect();
    rsx! {
        Select {
            size: "sm",
            value_fn: move || {
                match fetch_event(pattern_id, note_id) {
                    Some(ev) => anchored_octave_for_spec(&ev.spec)
                        .unwrap_or(DEFAULT_ANCHOR_OCTAVE)
                        .0
                        .to_string(),
                    None => DEFAULT_ANCHOR_OCTAVE.0.to_string(),
                }
            },
            data: options,
            onchange: move |v: String| commit_anchored_octave(pattern_id, note_id, v),
        }
    }
}

fn anchored_octave_for_spec(spec: &PitchSpec) -> Option<Octave> {
    match spec {
        PitchSpec::Scale {
            octave: OctaveSpec::Anchored(o),
            ..
        }
        | PitchSpec::Chord {
            octave: OctaveSpec::Anchored(o),
            ..
        } => Some(*o),
        _ => None,
    }
}

fn commit_octave_kind(pattern_id: PatternId, note_id: NoteId, value: String) {
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        update_pitched_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            let next_octave = match value.as_str() {
                "nearest" => OctaveSpec::Nearest,
                "anchored" => OctaveSpec::Anchored(
                    anchored_octave_for_spec(&ev.spec).unwrap_or(DEFAULT_ANCHOR_OCTAVE),
                ),
                "up" => OctaveSpec::UpFromPrev,
                "down" => OctaveSpec::DownFromPrev,
                "role" => OctaveSpec::RelativeToRole,
                _ => return,
            };
            match &mut ev.spec {
                PitchSpec::Scale { octave, .. } | PitchSpec::Chord { octave, .. } => {
                    *octave = next_octave;
                }
                _ => {}
            }
        });
    }) {
        eprintln!("pattern_editor: set octave kind failed: {e}");
    }
}

fn commit_anchored_octave(pattern_id: PatternId, note_id: NoteId, value: String) {
    let Ok(o) = value.trim().parse::<i8>() else { return };
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        update_pitched_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            match &mut ev.spec {
                PitchSpec::Scale {
                    octave: OctaveSpec::Anchored(existing),
                    ..
                }
                | PitchSpec::Chord {
                    octave: OctaveSpec::Anchored(existing),
                    ..
                } => {
                    *existing = Octave(o);
                }
                _ => {}
            }
        });
    }) {
        eprintln!("pattern_editor: set anchored octave failed: {e}");
    }
}

fn octave_spec_kind_str(spec: &PitchSpec) -> &'static str {
    match spec {
        PitchSpec::Scale { octave, .. } | PitchSpec::Chord { octave, .. } => match octave {
            OctaveSpec::Nearest => "nearest",
            OctaveSpec::Anchored(_) => "anchored",
            OctaveSpec::UpFromPrev => "up",
            OctaveSpec::DownFromPrev => "down",
            OctaveSpec::RelativeToRole => "role",
        },
        _ => "nearest",
    }
}

fn octave_spec_options() -> Vec<SelectOption> {
    vec![
        SelectOption::new("nearest", "Nearest"),
        SelectOption::new("anchored", "Anchored"),
        SelectOption::new("up", "Up from previous"),
        SelectOption::new("down", "Down from previous"),
        SelectOption::new("role", "Track role default"),
    ]
}

// ─── Timing / dynamics / humanization ────────────────────────────────────

#[component]
fn TimingGroup(pattern_id: PatternId, note_id_value: u64) -> NodeHandle {
    let note_id = NoteId::new(note_id_value);
    let time_buffer = Signal::new(String::new());
    let dur_buffer = Signal::new(String::new());

    let _ = Effect::new(move || {
        let event = fetch_event(pattern_id, note_id);
        let canonical = event.as_ref().map(|e| e.time.as_ticks()).unwrap_or(0);
        let typed = untracked(|| time_buffer.get());
        if typed.parse::<i64>().ok() == Some(canonical) {
            return;
        }
        time_buffer.set(canonical.to_string());
    });
    let _ = Effect::new(move || {
        let event = fetch_event(pattern_id, note_id);
        let canonical = event.as_ref().map(|e| e.duration.as_ticks()).unwrap_or(0);
        let typed = untracked(|| dur_buffer.get());
        if typed.parse::<i64>().ok() == Some(canonical) {
            return;
        }
        dur_buffer.set(canonical.to_string());
    });

    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 14px;",
            div { style: {field_group_style()},
                span { style: {field_label_style()},
                    {format!("Time (ticks; 1 beat = {PPQ})")}
                }
                TextInput {
                    size: "sm",
                    value_fn: move || time_buffer.get(),
                    oninput: move |v: String| time_buffer.set(v),
                    onsubmit: move || {
                        if let Ok(t) = time_buffer.get().trim().parse::<i64>() {
                            commit_time(pattern_id, note_id, t.max(0));
                        }
                    },
                }
            }
            div { style: {field_group_style()},
                span { style: {field_label_style()}, "Duration (ticks)" }
                TextInput {
                    size: "sm",
                    value_fn: move || dur_buffer.get(),
                    oninput: move |v: String| dur_buffer.set(v),
                    onsubmit: move || {
                        if let Ok(d) = dur_buffer.get().trim().parse::<i64>() {
                            commit_duration(pattern_id, note_id, d.max(1));
                        }
                    },
                }
            }
        }
    }
}

fn commit_time(pattern_id: PatternId, note_id: NoteId, ticks: i64) {
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        update_pitched_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            ev.time = MusicalTime::ticks(ticks);
        });
        if let Some(body) = pitched_body_mut(p, pattern_id)
            && let Some(events) = body.variants.get_mut(&variant_for_edit)
        {
            events.sort_by_key(|e| e.time.as_ticks());
        }
    }) {
        eprintln!("pattern_editor: set time failed: {e}");
    }
}

fn pitched_body_mut(
    project: &mut rawdaw_model::project::Project,
    id: PatternId,
) -> Option<&mut PitchedPatternBody> {
    match &mut project.patterns.get_mut(&id)?.body {
        PatternBody::Pitched(body) => Some(body),
        PatternBody::Drum(_) => None,
    }
}

fn commit_duration(pattern_id: PatternId, note_id: NoteId, ticks: i64) {
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        update_pitched_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            ev.duration = Duration::ticks(ticks);
        });
    }) {
        eprintln!("pattern_editor: set duration failed: {e}");
    }
}

#[component]
fn DynamicsGroup(pattern_id: PatternId, note_id_value: u64) -> NodeHandle {
    let note_id = NoteId::new(note_id_value);
    let vel_buffer = Signal::new(String::new());
    let art_buffer = Signal::new(String::new());

    let _ = Effect::new(move || {
        // Read project + focused_variant untracked so this Effect
        // doesn't subscribe to either. It runs once at mount to seed
        // the buffer from canonical, then never re-fires. Subsequent
        // project edits don't ripple back into the buffer (which
        // would feed back into the input's value-display Effect and
        // — when an unrelated edit like Delete causes a cascading
        // flush across every inspector Effect — re-enter the same
        // Effect's RefCell and panic the rinch reactive runtime).
        // The inspector remounts (with fresh buffers) whenever the
        // focused note id or PitchSpec kind changes, so the canonical
        // seed reload happens at the right moments via remount.
        let canonical = untracked(|| fetch_event(pattern_id, note_id))
            .map(|e| e.velocity.get())
            .unwrap_or(64);
        let typed = untracked(|| vel_buffer.get());
        if typed.parse::<u8>().ok() == Some(canonical) {
            return;
        }
        vel_buffer.set(canonical.to_string());
    });
    let _ = Effect::new(move || {
        // Read project + focused_variant untracked so this Effect
        // doesn't subscribe to either. It runs once at mount to seed
        // the buffer from canonical, then never re-fires. Subsequent
        // project edits don't ripple back into the buffer (which
        // would feed back into the input's value-display Effect and
        // — when an unrelated edit like Delete causes a cascading
        // flush across every inspector Effect — re-enter the same
        // Effect's RefCell and panic the rinch reactive runtime).
        // The inspector remounts (with fresh buffers) whenever the
        // focused note id or PitchSpec kind changes, so the canonical
        // seed reload happens at the right moments via remount.
        let canonical = untracked(|| fetch_event(pattern_id, note_id))
            .and_then(|e| e.articulation.map(|a| a.0))
            .unwrap_or_default();
        let typed = untracked(|| art_buffer.get());
        if typed == canonical {
            return;
        }
        art_buffer.set(canonical);
    });

    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 14px;",
            div { style: {field_group_style()},
                span { style: {field_label_style()}, "Velocity (0–127)" }
                TextInput {
                    size: "sm",
                    value_fn: move || vel_buffer.get(),
                    oninput: move |v: String| vel_buffer.set(v),
                    onsubmit: move || {
                        if let Ok(v) = vel_buffer.get().trim().parse::<u8>() {
                            commit_velocity(pattern_id, note_id, v.min(127));
                        }
                    },
                }
            }
            div { style: {field_group_style()},
                span { style: {field_label_style()}, "Articulation tag" }
                TextInput {
                    size: "sm",
                    value_fn: move || art_buffer.get(),
                    oninput: move |v: String| art_buffer.set(v),
                    onsubmit: move || {
                        commit_articulation(pattern_id, note_id, art_buffer.get());
                    },
                }
            }
        }
    }
}

fn commit_velocity(pattern_id: PatternId, note_id: NoteId, v: u8) {
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        update_pitched_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            ev.velocity = U7::clamp(v);
        });
    }) {
        eprintln!("pattern_editor: set velocity failed: {e}");
    }
}

fn commit_articulation(pattern_id: PatternId, note_id: NoteId, tag: String) {
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let variant_for_edit = variant.clone();
    let trimmed = tag.trim().to_string();
    if let Err(e) = app.apply_project_edit(move |p| {
        update_pitched_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            if trimmed.is_empty() {
                ev.articulation = None;
            } else {
                ev.articulation = Some(rawdaw_model::pattern::ArticulationTag::new(trimmed.clone()));
            }
        });
    }) {
        eprintln!("pattern_editor: set articulation failed: {e}");
    }
}

#[component]
fn HumanizationGroup(pattern_id: PatternId, note_id_value: u64) -> NodeHandle {
    let note_id = NoteId::new(note_id_value);
    let timing_buffer = Signal::new(String::new());
    let velocity_buffer = Signal::new(String::new());

    let _ = Effect::new(move || {
        // Read project + focused_variant untracked so this Effect
        // doesn't subscribe to either. It runs once at mount to seed
        // the buffer from canonical, then never re-fires. Subsequent
        // project edits don't ripple back into the buffer (which
        // would feed back into the input's value-display Effect and
        // — when an unrelated edit like Delete causes a cascading
        // flush across every inspector Effect — re-enter the same
        // Effect's RefCell and panic the rinch reactive runtime).
        // The inspector remounts (with fresh buffers) whenever the
        // focused note id or PitchSpec kind changes, so the canonical
        // seed reload happens at the right moments via remount.
        let canonical = untracked(|| fetch_event(pattern_id, note_id))
            .map(|e| e.humanization.timing_offset_ticks)
            .unwrap_or(0);
        let typed = untracked(|| timing_buffer.get());
        if typed.parse::<i32>().ok() == Some(canonical) {
            return;
        }
        timing_buffer.set(canonical.to_string());
    });
    let _ = Effect::new(move || {
        // Read project + focused_variant untracked so this Effect
        // doesn't subscribe to either. It runs once at mount to seed
        // the buffer from canonical, then never re-fires. Subsequent
        // project edits don't ripple back into the buffer (which
        // would feed back into the input's value-display Effect and
        // — when an unrelated edit like Delete causes a cascading
        // flush across every inspector Effect — re-enter the same
        // Effect's RefCell and panic the rinch reactive runtime).
        // The inspector remounts (with fresh buffers) whenever the
        // focused note id or PitchSpec kind changes, so the canonical
        // seed reload happens at the right moments via remount.
        let canonical = untracked(|| fetch_event(pattern_id, note_id))
            .map(|e| e.humanization.velocity_offset)
            .unwrap_or(0);
        let typed = untracked(|| velocity_buffer.get());
        if typed.parse::<i16>().ok() == Some(canonical) {
            return;
        }
        velocity_buffer.set(canonical.to_string());
    });

    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 14px;",
            div { style: {field_group_style()},
                span { style: {field_label_style()}, "Humanize timing (ticks)" }
                TextInput {
                    size: "sm",
                    value_fn: move || timing_buffer.get(),
                    oninput: move |v: String| timing_buffer.set(v),
                    onsubmit: move || {
                        if let Ok(t) = timing_buffer.get().trim().parse::<i32>() {
                            commit_humanize_timing(pattern_id, note_id, t);
                        }
                    },
                }
            }
            div { style: {field_group_style()},
                span { style: {field_label_style()}, "Humanize velocity (U7 ± offset)" }
                TextInput {
                    size: "sm",
                    value_fn: move || velocity_buffer.get(),
                    oninput: move |v: String| velocity_buffer.set(v),
                    onsubmit: move || {
                        if let Ok(v) = velocity_buffer.get().trim().parse::<i16>() {
                            commit_humanize_velocity(pattern_id, note_id, v);
                        }
                    },
                }
            }
        }
    }
}

fn commit_humanize_timing(pattern_id: PatternId, note_id: NoteId, ticks: i32) {
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        update_pitched_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            ev.humanization.timing_offset_ticks = ticks;
        });
    }) {
        eprintln!("pattern_editor: set humanize timing failed: {e}");
    }
}

fn commit_humanize_velocity(pattern_id: PatternId, note_id: NoteId, offset: i16) {
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        update_pitched_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            ev.humanization.velocity_offset = offset;
        });
    }) {
        eprintln!("pattern_editor: set humanize velocity failed: {e}");
    }
}

// ─── Action helpers ──────────────────────────────────────────────────────

fn delete_focused_note(pattern_id: PatternId, note_id: NoteId) {
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    // Clear focus BEFORE the project edit. If we delete first, the
    // project Signal bumps with the event gone, then every per-note
    // Effect in the inspector (velocity buffer, duration buffer,
    // …) re-fires reading a now-deleted event — and the for-loop
    // source closure that drives the keyed inspector remount also
    // depends on project, so the inspector subtree starts unmounting
    // mid-flush. Rinch's reactive runtime panics with `RefCell
    // already borrowed` when an Effect's body is re-entered during
    // its own teardown. Clearing focus first unmounts the inspector
    // (disposing the per-note Effects) before the project edit, so
    // no stale Effects are pending when the project Signal bumps.
    app.focused_pattern_note.set(None);
    if let Err(e) = app.apply_project_edit(move |p| {
        delete_pitched_event(p, pattern_id, &variant, note_id);
    }) {
        eprintln!("pattern_editor: delete note failed: {e}");
    }
}

// ─── Lookup helpers ──────────────────────────────────────────────────────

fn fetch_event(pattern_id: PatternId, note_id: NoteId) -> Option<PitchedEvent> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let pattern = project.patterns.get(&pattern_id)?;
    let body = match &pattern.body {
        PatternBody::Pitched(b) => b,
        PatternBody::Drum(_) => return None,
    };
    let variant = app
        .focused_variant
        .get()
        .filter(|v| body.variants.contains_key(v))
        .unwrap_or_else(|| pattern.default_variant.clone());
    body.variants
        .get(&variant)?
        .iter()
        .find(|e| e.note_id == note_id)
        .cloned()
}

fn current_variant(pattern_id: PatternId) -> VariantId {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let pattern_default = project
        .patterns
        .get(&pattern_id)
        .map(|p| p.default_variant.clone())
        .unwrap_or_else(VariantId::main);
    app.focused_variant.get().unwrap_or(pattern_default)
}

// ─── Option helpers ──────────────────────────────────────────────────────

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

// ─── Styles ──────────────────────────────────────────────────────────────

fn field_group_style() -> String {
    "display: flex; flex-direction: column; gap: 4px;".to_string()
}

fn field_label_style() -> String {
    "font-size: 10px; letter-spacing: 0.6px; text-transform: uppercase; \
         color: rgba(232,234,238,0.42); font-weight: 600;"
        .to_string()
}

fn danger_btn_style() -> String {
    format!(
        "height: 22px; padding: 0 10px; \
         border-radius: 4px; background: transparent; \
         border: 1px solid {line}; color: rgba(255,150,150,0.85); \
         font-size: 11px; cursor: pointer;",
        line = theme::LINE,
    )
}
