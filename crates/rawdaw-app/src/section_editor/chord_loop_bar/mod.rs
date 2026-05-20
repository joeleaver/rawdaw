//! Section meta bar's chord-loop strip (CL4 editable + CL4.x
//! right-click split/merge).
//!
//! Renders one cell per bar of the section's duration. Each cell
//! shows the chord active at that bar's downbeat, sourced from the
//! `(BarRange, ChordLoopId)` schedule on `Section.base.chord_loops`
//! (multi-loop ready — bars 1–4 can use loop A and 5–8 loop B). The
//! primary picker is a [`Select`] embedded in each cell; right-
//! clicking opens a [`ContextMenu`] with merge-left / merge-right /
//! clear-range shortcuts.
//!
//! ## Case preservation (UI principle 3)
//!
//! Roman numerals encode quality through case (`I` major, `vi`
//! minor). Don't lowercase, uppercase, or small-caps them.
//! `chord_display::roman_label` returns the canonical case (`I`,
//! `V`, `vi`, `IV`); the cell renders the string verbatim.
//!
//! ## Schedule mutation (CL4 + CL4.x)
//!
//! Single-bar loop changes go through [`schedule::set_loop_for_bar`];
//! the right-click bulk actions route through
//! [`schedule::merge_range_left`], [`schedule::merge_range_right`],
//! and [`schedule::clear_range_at_bar`]. All four are pure functions
//! living in the sibling [`schedule`] module so this file stays
//! UI-focused. The C2 edit pump (`apply_project_edit`) wraps the
//! resulting schedule into a project edit + engine re-arm.
//!
//! ## Sub-bar events (deferred)
//!
//! Cells sample the chord at each bar's downbeat. Chord events can
//! be sub-bar (CL2's timeline supports arbitrary positions); the
//! cell shows whichever event covers `bar * PPQ * 4`. Sub-bar UX
//! lives in the per-loop editor — the section view stays per-bar.

mod schedule;

use rinch::prelude::*;

use rawdaw_model::chord::{ChordEvent, ChordLoop, ChordSpec};
use rawdaw_model::id::{ChordLoopId, SectionId};
use rawdaw_model::scale::Scale;
use rawdaw_model::time::PPQ;

use crate::chord_display::{absolute_label, pitch_class_name, quality_suffix, roman_label};
use crate::parts::rgba;
use crate::state::AppState;
use crate::theme;

use schedule::{clear_range_at_bar, merge_range_left, merge_range_right, set_loop_for_bar};

#[component]
pub fn ChordLoopBar(section_name_key: String, duration_bars: u32) -> NodeHandle {
    // No `overflow: hidden` here — the per-cell `DropdownMenu`'s
    // absolutely-positioned popover (z-index 100) needs to escape
    // the bar's bounding box. Cells round their own corners with
    // `border-radius: 2px` so the jagged-edge issue overflow:hidden
    // was guarding against doesn't apply.
    let outer_style = format!(
        "display: flex; gap: 2px; height: 38px; box-sizing: border-box; \
         border-radius: 4px; \
         background: {bg0}; border: 1px solid {line}; padding: 2px;",
        bg0 = theme::BG0,
        line = theme::LINE,
    );

    // Two clones up front: one for the for-source closure (cells
    // refresh per render), one for each cell's prop. rsx's
    // `for x in expr { ... }` consumes `expr` into a `Fn() -> Vec<T>`,
    // and the body's `section_name_key: ...` prop is captured into
    // each iteration's component prop. They can't share a single
    // String.
    let for_source_key = section_name_key.clone();
    let cell_key = section_name_key;
    rsx! {
        div { style: {outer_style.clone()},
            for cell in build_cells_for_section(for_source_key.clone(), duration_bars) {
                EditableChordCell {
                    key: cell.bar.to_string(),
                    section_name_key: cell_key.clone(),
                    bar: cell.bar,
                    label: cell.label,
                    absolute: cell.absolute,
                    color: cell.color,
                    is_first_of_loop: cell.is_first_of_loop,
                    loop_id_present: cell.loop_id.is_some(),
                }
            }
        }
    }
}

// ─── Cell building ──────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
struct ChordCellData {
    bar: u32,
    label: String,
    absolute: String,
    color: String,
    is_first_of_loop: bool,
    loop_id: Option<ChordLoopId>,
}

fn build_cells_for_section(section_name_key: String, duration_bars: u32) -> Vec<ChordCellData> {
    if duration_bars == 0 {
        return Vec::new();
    }
    let app = use_store::<AppState>();
    let project = app.project.get();
    let overlay = app.overlay.get();
    let Some(section) = project.sections.values().find(|s| s.name == section_name_key) else {
        return Vec::new();
    };
    let schedule = &section.base.chord_loops;
    let project_key = &project.default_key;

    let mut cells = Vec::with_capacity(duration_bars as usize);
    let mut prev_loop_id: Option<ChordLoopId> = None;

    for bar in 0..duration_bars {
        let covering = schedule.iter().find(|(range, _)| range.contains(bar));
        let cell = match covering {
            Some((range, loop_id)) => {
                let loop_data = project.chord_loops.get(loop_id);
                let color = overlay
                    .chord_loop_color
                    .get(loop_id)
                    .cloned()
                    .unwrap_or_else(|| theme::TEXT2.to_string());
                let (label, absolute) = loop_data
                    .map(|cl| chord_label_at_bar(cl, bar, range.start, project_key))
                    .unwrap_or_else(|| ("?".into(), String::new()));
                ChordCellData {
                    bar,
                    label,
                    absolute,
                    color,
                    is_first_of_loop: prev_loop_id != Some(*loop_id) || bar == range.start,
                    loop_id: Some(*loop_id),
                }
            }
            None => ChordCellData {
                bar,
                label: "—".into(),
                absolute: String::new(),
                color: theme::TEXT2.to_string(),
                is_first_of_loop: false,
                loop_id: None,
            },
        };
        prev_loop_id = covering.map(|(_, id)| *id);
        cells.push(cell);
    }

    cells
}

/// Resolve the chord active at section bar `section_bar` for a loop
/// anchored at `range_start`. Wraps the loop's events modulo the
/// loop length so multi-iteration sections still pick the right
/// event. Uses i64 throughout to match `MusicalTime::as_ticks`'s
/// signed return; tick values are non-negative in practice.
fn chord_label_at_bar(
    loop_: &ChordLoop,
    section_bar: u32,
    range_start: u32,
    project_key: &Scale,
) -> (String, String) {
    let scale = loop_.key.clone().unwrap_or_else(|| project_key.clone());
    let loop_length_ticks = loop_.length.as_ticks().max(1);
    let ticks_per_bar = PPQ * 4;
    let bars_into_range = section_bar.saturating_sub(range_start) as i64;
    let target_ticks = (bars_into_range * ticks_per_bar).rem_euclid(loop_length_ticks);
    let event = active_event_at(loop_, target_ticks);
    match event {
        Some(ev) => render_event_labels(&ev.chord, &scale),
        None => ("—".into(), String::new()),
    }
}

/// Find the chord event whose `[time, time + duration)` covers
/// `target_ticks`. Falls back to the last event starting at or
/// before the target (a chord "holds" until the next event).
fn active_event_at(loop_: &ChordLoop, target_ticks: i64) -> Option<&ChordEvent> {
    let mut covering: Option<&ChordEvent> = None;
    for ev in &loop_.events {
        let start = ev.time.as_ticks();
        let end = start + ev.duration.as_ticks();
        if start <= target_ticks && target_ticks < end {
            covering = Some(ev);
        }
    }
    covering.or_else(|| {
        loop_
            .events
            .iter()
            .filter(|ev| ev.time.as_ticks() <= target_ticks)
            .max_by_key(|ev| ev.time.as_ticks())
    })
}

fn render_event_labels(chord: &ChordSpec, fallback: &Scale) -> (String, String) {
    match chord {
        ChordSpec::Functional { roman, suffix, in_key } => {
            let effective = in_key.as_ref().unwrap_or(fallback);
            (
                roman_label(*roman, &suffix.quality),
                absolute_label(*roman, &suffix.quality, effective),
            )
        }
        ChordSpec::Absolute { root, suffix } => (
            format!(
                "{}{}",
                pitch_class_name(*root),
                quality_suffix(&suffix.quality),
            ),
            String::new(),
        ),
    }
}

// ─── Per-cell component (with DropdownMenu picker) ──────────────────────

#[component]
fn EditableChordCell(
    section_name_key: String,
    bar: u32,
    label: String,
    absolute: String,
    color: String,
    is_first_of_loop: bool,
    loop_id_present: bool,
) -> NodeHandle {
    let bg_fill = rgba(color.as_str(), 0.10);
    let border_soft = rgba(color.as_str(), 0.30);
    let left_border = if is_first_of_loop {
        format!("2px solid {col}", col = color)
    } else {
        format!("1px solid {bc}", bc = border_soft)
    };
    let roman_style = format!(
        "font-family: inherit; font-feature-settings: \"tnum\" 1; \
         font-weight: 600; font-size: 12px; letter-spacing: 0.5px; \
         color: {text}; line-height: 1;",
        text = theme::TEXT0,
    );
    let abs_style = format!(
        "font-size: 9px; color: {text2}; \
         font-feature-settings: \"tnum\" 1; line-height: 1;",
        text2 = theme::TEXT2,
    );

    let _ = loop_id_present; // reserved for "active" highlight (future)
    // CL4 v1: cell IS a Select with the current loop's name visible.
    // The richer Roman+absolute display we had in CL2 lives below
    // the Select as a smaller secondary line. DropdownMenu's
    // absolute-positioned popover doesn't render visibly inside
    // rinch's flex chord-loop bar; Select works because it's the
    // pattern every other inspector dropdown uses.
    //
    // CL4.x adds a right-click ContextMenu on each cell for the
    // common bulk-range actions (merge-left / merge-right / clear
    // the range). ContextMenu portals into the body via
    // `position: fixed`, sidestepping the flex-clipping bug that
    // forced `Select` over `DropdownMenu` for the primary picker.
    let wrapper_style = format!(
        "flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; \
         background: {bg}; border: 1px solid {bc}; border-left: {lb}; \
         border-radius: 2px; box-sizing: border-box; padding: 2px;",
        bg = bg_fill,
        bc = border_soft,
        lb = left_border,
    );
    let chord_line_style = "display: flex; align-items: baseline; gap: 4px; \
         padding: 0 4px; line-height: 1;"
        .to_string();

    // The cell's two interactive surfaces (Select onchange and the
    // three context-menu items) each need their own clone of the
    // section-name key; closures take ownership. One per call site.
    let key_for_select = section_name_key.clone();
    let key_for_merge_left = section_name_key.clone();
    let key_for_merge_right = section_name_key.clone();
    let key_for_clear = section_name_key.clone();
    let key_for_value_fn = section_name_key;

    rsx! {
        ContextMenu {
            ContextMenuTarget {
                div { style: {wrapper_style.clone()},
                    Select {
                        size: "sm",
                        value_fn: move || {
                            current_loop_id_str_for_bar(key_for_value_fn.clone(), bar)
                        },
                        data: loop_picker_select_options(),
                        onchange: move |v: String| {
                            commit_loop_for_bar(
                                key_for_select.clone(),
                                bar,
                                decode_loop_id_opt(&v),
                            );
                        },
                    }
                    div { style: {chord_line_style.clone()},
                        span { style: {roman_style.clone()}, {label.clone()} }
                        span { style: {abs_style.clone()}, {absolute.clone()} }
                    }
                }
            }
            ContextMenuDropdown {
                DropdownMenuItem {
                    onclick: move || commit_merge_left(key_for_merge_left.clone(), bar),
                    "Merge with left"
                }
                DropdownMenuItem {
                    onclick: move || commit_merge_right(key_for_merge_right.clone(), bar),
                    "Merge with right"
                }
                DropdownMenuDivider {}
                DropdownMenuItem {
                    onclick: move || commit_clear_range(key_for_clear.clone(), bar),
                    "Clear this range"
                }
            }
        }
    }
}

fn current_loop_id_str_for_bar(section_name_key: String, bar: u32) -> String {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let Some(section) = project.sections.values().find(|s| s.name == section_name_key) else {
        return "none".into();
    };
    match section
        .base
        .chord_loops
        .iter()
        .find(|(range, _)| range.contains(bar))
    {
        Some((_, id)) => encode_loop_id(*id),
        None => "none".into(),
    }
}

fn encode_loop_id(id: ChordLoopId) -> String {
    format!("{}", id.0)
}

fn decode_loop_id_opt(s: &str) -> Option<ChordLoopId> {
    if s == "none" {
        return None;
    }
    s.parse::<u64>().ok().map(ChordLoopId::new)
}

fn loop_picker_select_options() -> Vec<SelectOption> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let mut out = vec![SelectOption::new("none", "(uncover this bar)")];
    for cl in project.chord_loops.values() {
        out.push(SelectOption::new(encode_loop_id(cl.id), cl.name.clone()));
    }
    out
}

fn commit_loop_for_bar(section_name_key: String, bar: u32, new_loop: Option<ChordLoopId>) {
    let app = use_store::<AppState>();
    let Some(section_id) = section_id_by_name(&app, &section_name_key) else {
        return;
    };
    if let Err(e) = app.apply_project_edit(move |p| {
        if let Some(section) = p.sections.get_mut(&section_id) {
            section.base.chord_loops =
                set_loop_for_bar(section.base.chord_loops.clone(), bar, new_loop);
        }
    }) {
        eprintln!("chord_loop_bar: schedule edit failed: {e}");
    }
}

/// CL4.x context-menu handler: extend the left-adjacent loop over
/// the entire range containing `bar`. No-op against the model when
/// the helper says so (uncovered, no left neighbor, same loop).
fn commit_merge_left(section_name_key: String, bar: u32) {
    let app = use_store::<AppState>();
    let Some(section_id) = section_id_by_name(&app, &section_name_key) else {
        return;
    };
    if let Err(e) = app.apply_project_edit(move |p| {
        if let Some(section) = p.sections.get_mut(&section_id) {
            section.base.chord_loops =
                merge_range_left(section.base.chord_loops.clone(), bar);
        }
    }) {
        eprintln!("chord_loop_bar: merge-left failed: {e}");
    }
}

/// Symmetric counterpart to [`commit_merge_left`].
fn commit_merge_right(section_name_key: String, bar: u32) {
    let app = use_store::<AppState>();
    let Some(section_id) = section_id_by_name(&app, &section_name_key) else {
        return;
    };
    if let Err(e) = app.apply_project_edit(move |p| {
        if let Some(section) = p.sections.get_mut(&section_id) {
            section.base.chord_loops =
                merge_range_right(section.base.chord_loops.clone(), bar);
        }
    }) {
        eprintln!("chord_loop_bar: merge-right failed: {e}");
    }
}

/// CL4.x context-menu handler: uncover the entire range containing
/// `bar`. Stronger than `commit_loop_for_bar(None)` which only
/// removes the single-bar slice.
fn commit_clear_range(section_name_key: String, bar: u32) {
    let app = use_store::<AppState>();
    let Some(section_id) = section_id_by_name(&app, &section_name_key) else {
        return;
    };
    if let Err(e) = app.apply_project_edit(move |p| {
        if let Some(section) = p.sections.get_mut(&section_id) {
            section.base.chord_loops =
                clear_range_at_bar(section.base.chord_loops.clone(), bar);
        }
    }) {
        eprintln!("chord_loop_bar: clear-range failed: {e}");
    }
}

fn section_id_by_name(app: &AppState, name: &str) -> Option<SectionId> {
    app.project
        .get()
        .sections
        .iter()
        .find(|(_, s)| s.name == name)
        .map(|(id, _)| *id)
}

