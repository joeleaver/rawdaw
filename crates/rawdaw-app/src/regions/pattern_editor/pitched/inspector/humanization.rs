//! Humanization fields (timing-offset + velocity-offset) for the
//! per-note inspector. Each input mirrors the canonical event field via
//! a one-shot buffer-sync `Effect` at mount; reads are `untracked` so
//! subsequent project bumps don't re-enter the Effect's RefCell during
//! cascading flushes (the inspector remounts on focus/spec changes —
//! see the shell module's doc-comment).

use rinch::core::reactive::{untracked, Effect};
use rinch::prelude::*;

use rawdaw_model::id::{NoteId, PatternId};

use crate::pattern_actions::update_pitched_event;
use crate::state::AppState;

use super::{current_variant, fetch_event, field_group_style, field_label_style};

#[component]
pub(super) fn HumanizationGroup(pattern_id: PatternId, note_id_value: u64) -> NodeHandle {
    let note_id = NoteId::new(note_id_value);
    let timing_buffer = Signal::new(String::new());
    let velocity_buffer = Signal::new(String::new());

    let _ = Effect::new(move || {
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
