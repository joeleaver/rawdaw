//! Timing fields (time + duration) for the drum inspector.
//!
//! Same buffer-sync Effect pattern as the pitched inspector: reads
//! are `untracked` so subsequent project bumps don't re-enter the
//! Effect's RefCell during cascading flushes.

use rinch::core::reactive::{untracked, Effect};
use rinch::prelude::*;

use rawdaw_model::id::{NoteId, PatternId};
use rawdaw_model::pattern::{DrumPatternBody, PatternBody};
use rawdaw_model::time::{Duration, MusicalTime, PPQ};

use crate::pattern_actions::update_drum_event;
use crate::state::AppState;

use super::{current_variant, fetch_event, field_group_style, field_label_style};

#[component]
pub(super) fn TimingGroup(pattern_id: PatternId, note_id_value: u64) -> NodeHandle {
    let note_id = NoteId::new(note_id_value);
    let time_buffer = Signal::new(String::new());
    let dur_buffer = Signal::new(String::new());

    let _ = Effect::new(move || {
        let canonical = untracked(|| fetch_event(pattern_id, note_id))
            .map(|e| e.time.as_ticks())
            .unwrap_or(0);
        let typed = untracked(|| time_buffer.get());
        if typed.parse::<i64>().ok() == Some(canonical) {
            return;
        }
        time_buffer.set(canonical.to_string());
    });
    let _ = Effect::new(move || {
        let canonical = untracked(|| fetch_event(pattern_id, note_id))
            .map(|e| e.duration.as_ticks())
            .unwrap_or(0);
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
        update_drum_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            ev.time = MusicalTime::ticks(ticks);
        });
        if let Some(body) = drum_body_mut(p, pattern_id)
            && let Some(events) = body.variants.get_mut(&variant_for_edit)
        {
            events.sort_by_key(|e| e.time.as_ticks());
        }
    }) {
        eprintln!("drum_editor: set time failed: {e}");
    }
}

fn commit_duration(pattern_id: PatternId, note_id: NoteId, ticks: i64) {
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        update_drum_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            ev.duration = Duration::ticks(ticks);
        });
    }) {
        eprintln!("drum_editor: set duration failed: {e}");
    }
}

fn drum_body_mut(
    project: &mut rawdaw_model::project::Project,
    id: PatternId,
) -> Option<&mut DrumPatternBody> {
    match &mut project.patterns.get_mut(&id)?.body {
        PatternBody::Drum(body) => Some(body),
        PatternBody::Pitched(_) => None,
    }
}
