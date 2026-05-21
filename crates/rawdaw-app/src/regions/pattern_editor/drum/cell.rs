//! A single step-grid cell.
//!
//! Click an empty cell → insert a `DrumEvent` at the cell's
//! `(voice, time)` and focus it. Click a filled cell → focus its
//! event for inspector edits. The toggle-off-on-click + drag-velocity
//! UX from the design plan defers to a later polish bite (P3.x);
//! click-focus + inspector-driven velocity edits is the v1 shape so
//! the editor and the pitched piano-roll share one focus model.
//!
//! Velocity-derived opacity: a fresh `U7::HALF` event renders at
//! ~50% fill; full-velocity at 100%. Empty cells render dimmed with
//! beat/bar boundary borders so the user can read the grid even
//! without any events.

use rinch::prelude::*;

use rawdaw_model::id::{NoteId, PatternId, VariantId};
use rawdaw_model::pattern::DrumVoice;

use crate::pattern_actions::insert_drum_event;
use crate::regions::pattern_editor::grid::GridSpec;
use crate::state::AppState;
use crate::theme;

use super::helpers::{default_drum_event, step_to_time, voice_full_name};

/// One step cell on the drum grid.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) enum StepCellKind {
    /// No event at this `(voice, step)`. Click to insert.
    #[default]
    Empty,
    /// An event lives here. `velocity` (0..=127) drives the fill
    /// opacity; `note_id_value` lets the click handler focus it.
    Filled { note_id_value: u64, velocity: u8 },
}

/// Per-cell props bag. `key()` derives a stable string for the
/// `for`-loop in `step_grid.rs` so insertion / deletion / velocity
/// changes force remount.
///
/// `Default` is implemented for the Rinch `#[component]` prop
/// requirement; an empty cell at step 0 is the safe placeholder.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct StepCellModel {
    pub step: usize,
    pub beat_boundary: bool,
    pub bar_boundary: bool,
    pub kind: StepCellKind,
}

impl StepCellModel {
    pub(super) fn key(&self) -> String {
        match &self.kind {
            StepCellKind::Empty => format!("e{}", self.step),
            StepCellKind::Filled {
                note_id_value,
                velocity,
            } => format!("n{}-{note_id_value}-{velocity}", self.step),
        }
    }
}

/// One step cell. The component is mounted from `step_grid.rs`'s
/// per-row for-loop; everything it needs flows in through props.
#[component]
pub(super) fn StepCell(
    pattern_id: PatternId,
    voice: DrumVoice,
    cell: StepCellModel,
    /// Cell color in `#RRGGBB`. Falls back per the parent pattern's
    /// overlay color so the user can hue-tag patterns from the
    /// library.
    note_color: String,
) -> NodeHandle {
    let outer = cell_outer_style(&cell, &note_color);
    let title = cell_title(&cell, &voice);
    let voice_for_click = voice.clone();
    let cell_for_click = cell.clone();
    rsx! {
        div {
            style: {outer.clone()},
            title: {title.clone()},
            onclick: move || handle_cell_click(pattern_id, &voice_for_click, &cell_for_click),
        }
    }
}

fn cell_outer_style(cell: &StepCellModel, note_color: &str) -> String {
    let border_color: &str = if cell.bar_boundary {
        theme::TEXT2
    } else if cell.beat_boundary {
        theme::LINE
    } else {
        "rgba(255,255,255,0.04)"
    };
    let (bg, border_extra) = match &cell.kind {
        StepCellKind::Empty => (theme::BG1.to_string(), String::new()),
        StepCellKind::Filled { note_id_value, velocity } => {
            let focused = use_store::<AppState>()
                .focused_pattern_note
                .get()
                .map(|n| n.get())
                == Some(*note_id_value);
            // velocity / 127 → [0, 1] opacity, scaled into [0.30, 0.95]
            // so even quiet hits stay visible. Focused notes get a
            // brighter outline so the user can spot the inspector
            // selection.
            let alpha = 0.30 + (*velocity as f32 / 127.0) * 0.65;
            let alpha = alpha.clamp(0.30, 0.95);
            let fill = crate::parts::rgba(note_color, alpha);
            let outline = if focused {
                format!("box-shadow: inset 0 0 0 1px {};", theme::TEXT1)
            } else {
                String::new()
            };
            (fill, outline)
        }
    };
    format!(
        "width: 28px; height: 28px; \
         box-sizing: border-box; cursor: pointer; \
         background: {bg}; \
         border-left: 1px solid {border_color}; \
         border-top: 1px solid rgba(255,255,255,0.04); \
         {border_extra}",
    )
}

fn cell_title(cell: &StepCellModel, voice: &DrumVoice) -> String {
    let voice_name = voice_full_name(voice);
    match &cell.kind {
        StepCellKind::Empty => format!("{voice_name} · step {} (click to insert)", cell.step + 1),
        StepCellKind::Filled { velocity, .. } => format!(
            "{voice_name} · step {} · velocity {velocity} (click to focus)",
            cell.step + 1,
        ),
    }
}

fn handle_cell_click(pattern_id: PatternId, voice: &DrumVoice, cell: &StepCellModel) {
    match &cell.kind {
        StepCellKind::Empty => insert_at_cell(pattern_id, voice.clone(), cell.step),
        StepCellKind::Filled { note_id_value, .. } => {
            use_store::<AppState>()
                .focused_pattern_note
                .set(Some(NoteId::new(*note_id_value)));
        }
    }
}

fn insert_at_cell(pattern_id: PatternId, voice: DrumVoice, step: usize) {
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let grid = GridSpec::STRAIGHT_SIXTEENTH;
    let time = step_to_time(step, grid);
    let voice_for_edit = voice;
    let captured_id = std::rc::Rc::new(std::cell::Cell::new(None::<NoteId>));
    let captured_id_for_edit = std::rc::Rc::clone(&captured_id);
    if let Err(e) = app.apply_project_edit(move |p| {
        let nid = p.id_allocators.alloc_note();
        captured_id_for_edit.set(Some(nid));
        let event = default_drum_event(nid, time, voice_for_edit.clone(), grid);
        insert_drum_event(p, pattern_id, &variant, event);
    }) {
        eprintln!("drum_editor: insert event failed: {e}");
        return;
    }
    if let Some(nid) = captured_id.get() {
        app.focused_pattern_note.set(Some(nid));
    }
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
