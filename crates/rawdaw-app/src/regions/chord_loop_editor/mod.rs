//! Chord-loop editor — CL2 of `docs/chord-loop-editing-plan.md`.
//!
//! Center-stage editor that mounts inside [`ArrangementSurface`]
//! when [`AppState::selected_chord_loop`] is `Some`. Replaces the
//! TracksPane + Arrangement + Inspector triplet; Library remains
//! visible on the side.
//!
//! ## File layout (per CL0 design decision 11 — split eagerly)
//!
//! - `mod.rs` (this file): the `ChordLoopEditor` top-level
//!   component + header bar (loop name + length + ×).
//! - `timeline.rs`: bar-scaled event-block strip + the
//!   `+ Chord` / `Delete` toolbar.
//! - `event_block.rs`: one chord-event cell — Roman + absolute
//!   labels, focused-state highlight, click-to-focus.
//! - `inspector.rs`: per-event field editor pane —
//!   `RomanDegree` / `ChordQuality` dropdowns, extension /
//!   alteration chips, `BassSpec` + `in_key` + cadence + comment.
//! - `helpers.rs`: pure helpers — `default_chord_event`,
//!   beat-snapping, `insert_chord_event_sorted`.
//!
//! Selection / focus state lives on [`AppState`] rather than as
//! local component props: rinch's `#[component]` macro requires
//! every prop to implement `Default`, and `Signal<Option<usize>>`
//! does not. The shared signal also lets the future CL4 realized
//! strip read focus without further plumbing.
//!
//! Functional events only in CL2. The Absolute toggle + quick-
//! entry shorthand parser land in CL3 per CL0 design decision 3.

use rinch::core::reactive::{untracked, Effect};
use rinch::prelude::*;

use rawdaw_model::id::ChordLoopId;

use crate::chord_loop_actions::rename_chord_loop;
use crate::state::AppState;
use crate::theme;

mod event_block;
mod helpers;
mod inspector;
mod realized_strip;
mod timeline;

pub(crate) use inspector::Inspector as ChordLoopInspector;
pub(crate) use timeline::ChordLoopTimeline;

/// Center-stage chord-loop editor. Mounts inside the arrangement
/// surface when `AppState::selected_chord_loop` is `Some`. The
/// caller (the `ArrangementSurface` component in `app.rs`) does
/// the mount/unmount; this component renders the surface for the
/// currently-selected id.
#[component]
pub fn ChordLoopEditor(id: ChordLoopId) -> NodeHandle {
    let surface_style = format!(
        "flex: 1; min-width: 0; display: flex; flex-direction: column; \
         background: {bg}; min-height: 0;",
        bg = theme::BG0,
    );

    rsx! {
        section { style: {surface_style.clone()},
            EditorHeader { id: id }
            div {
                style: "flex: 1; display: flex; min-height: 0;",
                ChordLoopTimeline { id: id }
                ChordLoopInspector { id: id }
            }
        }
    }
}

/// Header bar — loop name (inline rename), length editor, × close.
#[component]
fn EditorHeader(id: ChordLoopId) -> NodeHandle {
    let name_buffer = Signal::new(String::new());
    let renaming = Signal::new(false);

    // Sync the name buffer from the live project. Same
    // untracked-Effect peek pattern as C4 NameControl + the
    // library's ChordLoopRow.
    let _ = Effect::new(move || {
        let project = use_store::<AppState>().project.get();
        let canonical = project
            .chord_loops
            .get(&id)
            .map(|cl| cl.name.clone())
            .unwrap_or_default();
        let display = untracked(|| name_buffer.get());
        if display == canonical {
            return;
        }
        name_buffer.set(canonical);
    });

    let header_style = format!(
        "display: flex; align-items: center; gap: 12px; \
         padding: 8px 14px; border-bottom: 1px solid {line}; \
         background: {bg1}; min-height: 36px;",
        line = theme::LINE,
        bg1 = theme::BG1,
    );

    rsx! {
        header { style: {header_style.clone()},
            span {
                style: "font-size: 10px; letter-spacing: 0.6px; \
                        text-transform: uppercase; color: rgba(232,234,238,0.42); \
                        font-weight: 600;",
                "Chord Loop"
            }
            if renaming.get() {
                input {
                    r#type: "text",
                    title: "Rename chord loop (Enter to commit, blank to revert)",
                    style: {header_name_input_style()},
                    value: {|| name_buffer.get()},
                    oninput: move |v: String| name_buffer.set(v),
                    onsubmit: move || {
                        commit_name_edit(id, name_buffer, renaming);
                    },
                }
            } else {
                button {
                    r#type: "button",
                    title: "Click to rename",
                    style: {header_name_btn_style()},
                    onclick: move || renaming.set(true),
                    {|| name_buffer.get()}
                }
            }
            LoopLengthControls { id: id }
            div { style: "flex: 1;" }
            button {
                r#type: "button",
                title: "Close (×)",
                style: {close_btn_style()},
                onclick: move || {
                    use_store::<AppState>().select_chord_loop(None);
                },
                "×"
            }
        }
    }
}

fn header_name_btn_style() -> String {
    format!(
        "height: 24px; padding: 0 8px; \
         border-radius: 4px; background: {bg0}; border: 1px solid {line}; \
         color: rgba(232,234,238,0.96); font-size: 14px; font-weight: 600; \
         letter-spacing: -0.1px; cursor: pointer;",
        bg0 = theme::BG0,
        line = theme::LINE,
    )
}

fn header_name_input_style() -> String {
    format!(
        "height: 24px; padding: 0 8px; \
         border-radius: 4px; background: {bg0}; border: 1px solid {line}; \
         color: rgba(232,234,238,0.96); font-size: 14px; font-weight: 600; \
         letter-spacing: -0.1px;",
        bg0 = theme::BG0,
        line = theme::LINE,
    )
}

fn close_btn_style() -> String {
    format!(
        "height: 24px; width: 24px; padding: 0; \
         border-radius: 4px; background: transparent; \
         border: 1px solid {line}; color: rgba(232,234,238,0.62); \
         font-size: 16px; line-height: 1; cursor: pointer;",
        line = theme::LINE,
    )
}

/// Length-of-loop bar count editor. Mirrors C4's BpmControls
/// shape: − / + nudges + a numeric text input, all committed
/// through the C2 edit pump.
#[component]
fn LoopLengthControls(id: ChordLoopId) -> NodeHandle {
    let bars_input = Signal::new(String::new());

    let _ = Effect::new(move || {
        let project = use_store::<AppState>().project.get();
        let beats_per_bar = project
            .tempo_map
            .beats_per_bar_at(rawdaw_model::time::MusicalTime::ZERO);
        let canonical = project
            .chord_loops
            .get(&id)
            .map(|cl| helpers::ticks_to_bars(cl.length.as_ticks(), beats_per_bar))
            .unwrap_or(0);
        let typed = untracked(|| bars_input.get());
        if typed.parse::<u32>().ok() == Some(canonical) {
            return;
        }
        bars_input.set(canonical.to_string());
    });

    let input_style = format!(
        "width: 44px; height: 22px; padding: 0 6px; box-sizing: border-box; \
         border-radius: 4px; background: {bg0}; border: 1px solid {line}; \
         color: rgba(232,234,238,0.96); font-size: 12px; \
         font-variant-numeric: tabular-nums; text-align: right;",
        bg0 = theme::BG0,
        line = theme::LINE,
    );
    let nudge_minus = nudge_btn_style();
    let nudge_plus = nudge_minus.clone();

    rsx! {
        div { style: "display: flex; align-items: center; gap: 4px;",
            span {
                style: "font-size: 10px; color: rgba(232,234,238,0.42); \
                        text-transform: uppercase; letter-spacing: 0.6px;",
                "Bars"
            }
            button {
                r#type: "button",
                title: "Shorten by 1 bar",
                style: {nudge_minus.clone()},
                onclick: move || nudge_bars(id, -1),
                "−"
            }
            input {
                r#type: "text",
                style: {input_style.clone()},
                value: {|| bars_input.get()},
                oninput: move |v: String| bars_input.set(v),
                onsubmit: move || {
                    if let Ok(bars) = bars_input.get().trim().parse::<u32>() {
                        let bars = bars.max(1);
                        set_loop_bars(id, bars);
                        bars_input.set(bars.to_string());
                    }
                },
            }
            button {
                r#type: "button",
                title: "Lengthen by 1 bar",
                style: {nudge_plus.clone()},
                onclick: move || nudge_bars(id, 1),
                "+"
            }
        }
    }
}

fn nudge_btn_style() -> String {
    format!(
        "height: 22px; width: 22px; padding: 0; \
         border-radius: 4px; background: {bg0}; border: 1px solid {line}; \
         color: rgba(232,234,238,0.72); font-size: 13px; cursor: pointer; \
         display: inline-flex; align-items: center; justify-content: center;",
        bg0 = theme::BG0,
        line = theme::LINE,
    )
}

fn commit_name_edit(id: ChordLoopId, name_buffer: Signal<String>, renaming: Signal<bool>) {
    let typed = name_buffer.get();
    let trimmed = typed.trim();
    if trimmed.is_empty() {
        let app = use_store::<AppState>();
        let canonical = app
            .project
            .get()
            .chord_loops
            .get(&id)
            .map(|cl| cl.name.clone())
            .unwrap_or_default();
        name_buffer.set(canonical);
        renaming.set(false);
        return;
    }
    let new_name = trimmed.to_string();
    let app = use_store::<AppState>();
    if let Err(e) = app.apply_project_edit(move |p| rename_chord_loop(p, id, new_name.clone())) {
        eprintln!("chord_loop_editor: rename failed: {e}");
    }
    renaming.set(false);
}

/// Apply `delta` bars to the loop's length. Floors at 1 bar so
/// the timeline never collapses to zero width.
fn nudge_bars(id: ChordLoopId, delta: i32) {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let beats_per_bar = project
        .tempo_map
        .beats_per_bar_at(rawdaw_model::time::MusicalTime::ZERO);
    let current = project
        .chord_loops
        .get(&id)
        .map(|cl| helpers::ticks_to_bars(cl.length.as_ticks(), beats_per_bar))
        .unwrap_or(0);
    let next = (current as i32 + delta).max(1) as u32;
    set_loop_bars(id, next);
}

fn set_loop_bars(id: ChordLoopId, bars: u32) {
    let app = use_store::<AppState>();
    if let Err(e) = app.apply_project_edit(move |p| {
        let beats_per_bar = p
            .tempo_map
            .beats_per_bar_at(rawdaw_model::time::MusicalTime::ZERO);
        if let Some(loop_) = p.chord_loops.get_mut(&id) {
            loop_.length = rawdaw_model::time::Duration::bars(bars as i64, beats_per_bar);
        }
    }) {
        eprintln!("chord_loop_editor: set loop length failed: {e}");
    }
}
