//! Dynamics fields (velocity + articulation tag) for the per-note
//! inspector. Each input mirrors the canonical event field via a
//! one-shot buffer-sync `Effect` at mount; reads are `untracked` so
//! subsequent project bumps don't re-enter the Effect's RefCell during
//! cascading flushes (the inspector remounts on focus/spec changes —
//! see the shell module's doc-comment).

use rinch::core::reactive::{untracked, Effect};
use rinch::prelude::*;

use rawdaw_model::id::{NoteId, PatternId};
use rawdaw_model::pitch::U7;

use crate::pattern_actions::update_pitched_event;
use crate::state::AppState;

use super::{current_variant, fetch_event, field_group_style, field_label_style};

#[component]
pub(super) fn DynamicsGroup(pattern_id: PatternId, note_id_value: u64) -> NodeHandle {
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
