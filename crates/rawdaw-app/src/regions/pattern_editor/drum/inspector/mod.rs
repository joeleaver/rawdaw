//! Per-event inspector for the drum step-grid editor.
//!
//! Parallels `pitched/inspector/` minus the PitchSpec sub-editors.
//! Each drum event carries a voice (which row it lives in), a time +
//! duration, a velocity, an articulation tag, and humanization
//! deltas. The voice selector + four field groups cover the editable
//! surface.
//!
//! Same Effect re-entry safeguard as the pitched inspector: every
//! buffer-sync `Effect` reads its sources through `untracked(...)` so
//! a cascading project flush (e.g. Delete) doesn't re-enter a leaked
//! Effect's RefCell. Remount happens whenever the focused note id
//! changes (driven by the keyed for-loop in `Inspector`), at which
//! point the buffers re-seed from canonical.

mod dynamics;
mod humanization;
mod timing;
mod voice;

use rinch::prelude::*;

use rawdaw_model::id::{NoteId, PatternId, VariantId};
use rawdaw_model::pattern::{DrumEvent, PatternBody};

use crate::pattern_actions::delete_drum_event;
use crate::state::AppState;
use crate::theme;

use self::dynamics::DynamicsGroup;
use self::humanization::HumanizationGroup;
use self::timing::TimingGroup;
use self::voice::VoiceSelectorGroup;

#[component]
pub(super) fn Inspector(id: PatternId) -> NodeHandle {
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
        Some(nid) => format!("p{}n{}", id.get(), nid.get()),
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
    let Some(_event) = fetch_event(pattern_id, nid) else {
        return rsx! { EmptyState { } };
    };
    rsx! {
        FocusedNoteFields { pattern_id: pattern_id, note_id_value: nid.get() }
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
            "No step selected. Click an empty cell to insert a hit, \
             or click a filled cell to edit it."
        }
    }
}

#[component]
fn FocusedNoteFields(pattern_id: PatternId, note_id_value: u64) -> NodeHandle {
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 14px;",
            HeaderRow { pattern_id: pattern_id, note_id_value: note_id_value }
            VoiceSelectorGroup {
                pattern_id: pattern_id,
                note_id_value: note_id_value,
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
                {format!("Step #{note_id_value}")}
            }
            button {
                r#type: "button",
                title: "Delete this step",
                style: {danger_btn_style()},
                onclick: move || delete_focused_step(pattern_id, NoteId::new(note_id_value)),
                "Delete"
            }
        }
    }
}

fn delete_focused_step(pattern_id: PatternId, note_id: NoteId) {
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    // Clear focus BEFORE the project edit. Same reasoning as the
    // pitched inspector's `delete_focused_note`: a cascading project
    // bump while the inspector subtree is still mounted will re-enter
    // leaked Effects mid-flush and panic the runtime. Clearing focus
    // first unmounts the inspector before the project signal fires.
    app.focused_pattern_note.set(None);
    if let Err(e) = app.apply_project_edit(move |p| {
        delete_drum_event(p, pattern_id, &variant, note_id);
    }) {
        eprintln!("drum_editor: delete step failed: {e}");
    }
}

// ─── Shared lookup helpers (used by every sub-editor) ────────────────────

pub(super) fn fetch_event(pattern_id: PatternId, note_id: NoteId) -> Option<DrumEvent> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let pattern = project.patterns.get(&pattern_id)?;
    let body = match &pattern.body {
        PatternBody::Drum(b) => b,
        PatternBody::Pitched(_) => return None,
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

pub(super) fn current_variant(pattern_id: PatternId) -> VariantId {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let pattern_default = project
        .patterns
        .get(&pattern_id)
        .map(|p| p.default_variant.clone())
        .unwrap_or_else(VariantId::main);
    app.focused_variant.get().unwrap_or(pattern_default)
}

// ─── Shared styles ───────────────────────────────────────────────────────

pub(super) fn field_group_style() -> String {
    "display: flex; flex-direction: column; gap: 4px;".to_string()
}

pub(super) fn field_label_style() -> String {
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
