//! Per-note inspector for the pitched pattern editor.
//!
//! Reads [`AppState::focused_pattern_note`] reactively; mounts the
//! [`FocusedNoteFields`] subtree when a note is focused, otherwise
//! renders a placeholder. Edits flow through the C2 edit pump via
//! [`crate::pattern_actions::update_pitched_event`].
//!
//! Layout (per P2 design plan, split when one file approaches the
//! 700-line cap):
//! - `mod.rs` (this file): inspector shell, focus-key resolver,
//!   `FocusedDispatch`, `FocusedNoteFields`, `HeaderRow`, plus the
//!   shared helpers (`fetch_event`, `current_variant`, style helpers,
//!   `delete_focused_note`) used by every sub-editor file.
//! - `spec.rs`: PitchSpec kind selector + the five sub-editors
//!   (Scale/Chord/Absolute/Chromatic/Rest) + the shared `OctaveRow`.
//! - `timing.rs`: time + duration fields.
//! - `dynamics.rs`: velocity + articulation tag.
//! - `humanization.rs`: humanize timing + velocity offsets.
//!
//! **Buffer-sync Effect contract** (load-bearing across every
//! sub-editor): each text input mirrors the canonical event field via
//! a one-shot `Effect` at mount that reads through `untracked(...)`.
//! Rinch's `Drop for Effect` doesn't auto-dispose, so leaked Effects
//! across an inspector remount would re-enter their RefCells during
//! a cascading project flush (e.g. Delete) and panic the runtime.
//! Untracked reads avoid the re-subscription; the inspector remounts
//! whenever the focused note id or PitchSpec kind changes (driven by
//! the keyed for-loop in `Inspector`), which is when the buffers need
//! to re-seed from canonical.

mod dynamics;
mod humanization;
mod octave;
mod spec;
mod timing;

use rinch::prelude::*;

use rawdaw_model::id::{NoteId, PatternId, VariantId};
use rawdaw_model::pattern::{PatternBody, PitchedEvent};

use crate::pattern_actions::delete_pitched_event;
use crate::state::AppState;
use crate::theme;

use self::dynamics::DynamicsGroup;
use self::humanization::HumanizationGroup;
use self::spec::{
    AbsoluteSubEditor, ChordSubEditor, ChromaticSubEditor, PitchSpecKind, PitchSpecKindGroup,
    RestNoticeRow, ScaleSubEditor,
};
use self::timing::TimingGroup;

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

// ─── Shared lookup helpers (used by every sub-editor) ────────────────────

pub(super) fn fetch_event(pattern_id: PatternId, note_id: NoteId) -> Option<PitchedEvent> {
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
