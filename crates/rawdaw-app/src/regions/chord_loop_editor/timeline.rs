//! Bar-scaled event-block strip for the chord-loop editor.
//!
//! Events render as proportionally-sized blocks; the strip below
//! shows bar lines. Click an event to focus it (drives the
//! inspector pane). `+ Chord` appends a default
//! `Functional { I major }` event; `Delete` removes the focused
//! event.
//!
//! Drag-to-move + resize-handle wiring is deferred to a CL2.x
//! follow-up — the v1 surface uses the toolbar + insert/delete
//! affordances plus the inspector's per-field editors.

use rinch::prelude::*;

use rawdaw_model::chord::ChordSpec;
use rawdaw_model::id::ChordLoopId;
use rawdaw_model::time::{Duration, MusicalTime, PPQ};

use crate::chord_display::{absolute_label, pitch_class_name, quality_suffix, roman_label};
use crate::state::AppState;
use crate::theme;

use super::event_block::EventBlock;
use super::helpers::{
    default_chord_event, insert_chord_event_sorted, snap_ticks_to_beat, ticks_to_bars,
};
use super::realized_strip::RealizedStrip;

#[component]
pub(crate) fn ChordLoopTimeline(id: ChordLoopId) -> NodeHandle {
    let surface_style = format!(
        "flex: 1; min-width: 0; padding: 16px; \
         background: {bg}; \
         display: flex; flex-direction: column; gap: 10px; \
         overflow-y: auto;",
        bg = theme::BG0,
    );

    rsx! {
        div { style: {surface_style.clone()},
            ToolbarRow { id: id }
            EventStrip { id: id }
            RealizedStrip { id: id }
            BarRuler { id: id }
        }
    }
}

#[component]
fn ToolbarRow(id: ChordLoopId) -> NodeHandle {
    let btn_style = format!(
        "height: 26px; padding: 0 10px; \
         border-radius: 4px; background: {bg1}; border: 1px solid {line}; \
         color: rgba(232,234,238,0.96); font-size: 12px; cursor: pointer;",
        bg1 = theme::BG1,
        line = theme::LINE,
    );
    let danger_style = btn_style.clone();

    rsx! {
        div { style: "display: flex; align-items: center; gap: 8px;",
            button {
                r#type: "button",
                title: "Append a default chord event",
                style: {btn_style.clone()},
                onclick: move || append_event_action(id),
                "+ Chord"
            }
            button {
                r#type: "button",
                title: "Delete focused chord event",
                style: {danger_style.clone()},
                onclick: move || delete_focused_action(id),
                "Delete"
            }
            div { style: "flex: 1;" }
            span {
                style: "font-size: 11px; color: rgba(232,234,238,0.42); \
                        font-feature-settings: \"tnum\" 1;",
                {|| event_count_label(id)}
            }
        }
    }
}

fn event_count_label(id: ChordLoopId) -> String {
    let project = use_store::<AppState>().project.get();
    let count = project
        .chord_loops
        .get(&id)
        .map(|cl| cl.events.len())
        .unwrap_or(0);
    if count == 1 {
        "1 chord".to_string()
    } else {
        format!("{count} chords")
    }
}

#[component]
fn EventStrip(id: ChordLoopId) -> NodeHandle {
    let strip_style = format!(
        "display: flex; align-items: stretch; \
         min-height: 56px; padding: 6px; \
         background: {bg1}; border: 1px solid {line}; border-radius: 4px;",
        bg1 = theme::BG1,
        line = theme::LINE,
    );

    rsx! {
        div { style: {strip_style.clone()},
            for cell in build_event_cells(id) {
                EventBlock {
                    key: cell.idx.to_string(),
                    idx: cell.idx,
                    loop_id: id,
                    duration_ticks: cell.duration_ticks,
                    roman: cell.roman,
                    absolute: cell.absolute,
                    width_frac: cell.width_frac,
                    color: cell.color.clone(),
                }
            }
            // Trailing "click to add" target fills any leftover loop
            // duration not covered by events.
            div {
                style: {|| trailing_filler_style(id)},
                onclick: move || append_event_action(id),
            }
        }
    }
}

#[derive(Clone, PartialEq)]
struct EventCell {
    idx: usize,
    duration_ticks: i64,
    roman: String,
    absolute: String,
    width_frac: f32,
    color: String,
}

fn build_event_cells(id: ChordLoopId) -> Vec<EventCell> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let overlay = app.overlay.get();
    let Some(loop_) = project.chord_loops.get(&id) else {
        return Vec::new();
    };
    let total_ticks = loop_.length.as_ticks().max(1) as f32;
    let scale = loop_
        .key
        .clone()
        .unwrap_or_else(|| project.default_key.clone());
    let color = overlay
        .chord_loop_color
        .get(&id)
        .cloned()
        .unwrap_or_else(|| theme::TEXT2.to_string());

    loop_
        .events
        .iter()
        .enumerate()
        .map(|(idx, ev)| {
            let (roman, absolute) = match &ev.chord {
                ChordSpec::Functional { roman, suffix, in_key } => {
                    let s = in_key.as_ref().unwrap_or(&scale);
                    (
                        roman_label(*roman, &suffix.quality),
                        absolute_label(*roman, &suffix.quality, s),
                    )
                }
                ChordSpec::Absolute { root, suffix } => (
                    String::new(),
                    format!(
                        "{}{}",
                        pitch_class_name(*root),
                        quality_suffix(&suffix.quality),
                    ),
                ),
            };
            let width = ev.duration.as_ticks() as f32 / total_ticks;
            EventCell {
                idx,
                duration_ticks: ev.duration.as_ticks(),
                roman,
                absolute,
                width_frac: width.clamp(0.0, 1.0),
                color: color.clone(),
            }
        })
        .collect()
}

fn trailing_filler_style(id: ChordLoopId) -> String {
    let project = use_store::<AppState>().project.get();
    let frac = project
        .chord_loops
        .get(&id)
        .map(|cl| {
            let used: i64 = cl.events.iter().map(|e| e.duration.as_ticks()).sum();
            let total = cl.length.as_ticks().max(1);
            ((total - used) as f32 / total as f32).clamp(0.0, 1.0)
        })
        .unwrap_or(0.0);
    let pct = (frac * 100.0).max(0.0);
    format!(
        "flex: 0 0 {pct}%; min-height: 100%; \
         border: 1px dashed {line}; border-radius: 3px; \
         opacity: 0.5; cursor: pointer;",
        line = theme::LINE,
    )
}

#[component]
fn BarRuler(id: ChordLoopId) -> NodeHandle {
    rsx! {
        div {
            style: "display: flex; height: 18px;",
            for tick in build_bar_ticks(id) {
                div {
                    key: tick.bar.to_string(),
                    style: format!(
                        "flex: 1; min-width: 0; \
                         border-left: 1px solid {line}; \
                         padding-left: 4px; \
                         font-size: 10px; color: rgba(232,234,238,0.42); \
                         font-feature-settings: \"tnum\" 1;",
                        line = theme::LINE,
                    ),
                    {tick.label.clone()}
                }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
struct BarTick {
    bar: u32,
    label: String,
}

fn build_bar_ticks(id: ChordLoopId) -> Vec<BarTick> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let beats_per_bar = project
        .tempo_map
        .beats_per_bar_at(MusicalTime::ZERO);
    let bars = project
        .chord_loops
        .get(&id)
        .map(|cl| ticks_to_bars(cl.length.as_ticks(), beats_per_bar))
        .unwrap_or(0);
    (0..bars)
        .map(|b| BarTick {
            bar: b,
            label: (b + 1).to_string(),
        })
        .collect()
}

/// Insert a default chord event at the next free position
/// (after the last existing event, or at the loop start if empty).
/// Duration defaults to one bar so the inserted block reads
/// clearly on the timeline.
fn append_event_action(id: ChordLoopId) {
    let app = use_store::<AppState>();
    let beats_per_bar = app
        .project
        .get()
        .tempo_map
        .beats_per_bar_at(MusicalTime::ZERO);
    let inserted_idx = std::rc::Rc::new(std::cell::Cell::new(None::<usize>));
    let idx_capture = inserted_idx.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        let Some(loop_) = p.chord_loops.get_mut(&id) else { return };
        let next_start_ticks: i64 = loop_
            .events
            .last()
            .map(|e| e.time.as_ticks() + e.duration.as_ticks())
            .unwrap_or(0);
        let snapped = snap_ticks_to_beat(next_start_ticks);
        let bar_duration = Duration::bars(1, beats_per_bar);
        let remaining = (loop_.length.as_ticks() - snapped).max(0);
        let duration_ticks = bar_duration.as_ticks().min(remaining).max(PPQ);
        let event = default_chord_event(
            MusicalTime::ticks(snapped),
            Duration::ticks(duration_ticks),
        );
        let idx = insert_chord_event_sorted(&mut loop_.events, event);
        idx_capture.set(Some(idx));
    }) {
        eprintln!("chord_loop_editor: append failed: {e}");
        return;
    }
    if let Some(idx) = inserted_idx.get() {
        use_store::<AppState>().focused_chord_event_idx.set(Some(idx));
    }
}

/// Delete the focused chord event from the loop. No-op if nothing
/// is focused or the index has somehow gone stale.
fn delete_focused_action(id: ChordLoopId) {
    let app = use_store::<AppState>();
    let Some(idx) = app.focused_chord_event_idx.get() else { return };
    if let Err(e) = app.apply_project_edit(move |p| {
        if let Some(loop_) = p.chord_loops.get_mut(&id)
            && idx < loop_.events.len()
        {
            loop_.events.remove(idx);
        }
    }) {
        eprintln!("chord_loop_editor: delete failed: {e}");
        return;
    }
    // Adjust focus to a still-valid index.
    let remaining = use_store::<AppState>()
        .project
        .get()
        .chord_loops
        .get(&id)
        .map(|cl| cl.events.len())
        .unwrap_or(0);
    let new_focus = if remaining == 0 {
        None
    } else if idx >= remaining {
        Some(remaining - 1)
    } else {
        Some(idx)
    };
    use_store::<AppState>().focused_chord_event_idx.set(new_focus);
}
