//! Drum step-grid surface.
//!
//! Rows are `DrumPatternMetadata.voices` in **reversed** declaration
//! order: per the P3 design plan, `voices[0]` is the *bottom* row of
//! the grid (matching DAW convention). Columns are integer step
//! indices derived from `pattern.length` ÷ `grid.step_ticks()`.
//!
//! The grid reads the project reactively via for-loop sources; the
//! cells are rendered by [`super::cell::StepCell`].

use rinch::prelude::*;

use rawdaw_model::id::PatternId;
use rawdaw_model::pattern::{DrumEvent, DrumVoice, PatternBody};

use crate::regions::pattern_editor::grid::GridSpec;
use crate::state::AppState;
use crate::theme;

use super::cell::{StepCell, StepCellKind, StepCellModel};
use super::helpers::{event_at_step, total_steps_for_length, voice_short_label};

/// Pattern-overlay fallback for the cell fill color. Reused from the
/// pitched piano-roll's convention — accent so empty patterns still
/// render visible cells if the user hasn't picked a palette swatch.
const FALLBACK_CELL_COLOR: &str = theme::ACCENT;

#[component]
pub(super) fn StepGrid(id: PatternId) -> NodeHandle {
    let surface_style = format!(
        "display: flex; flex-direction: column; \
         border: 1px solid {line}; border-radius: 4px; \
         background: {bg1}; overflow: auto; min-height: 0; flex: 1;",
        line = theme::LINE,
        bg1 = theme::BG1,
    );

    rsx! {
        div { style: {surface_style.clone()},
            for row in build_voice_rows(id) {
                VoiceRow {
                    key: row.row_key.clone(),
                    pattern_id: id,
                    voice: row.voice.clone(),
                    voice_label: row.voice_label.clone(),
                    note_color: row.note_color.clone(),
                }
            }
        }
    }
}

/// Per-row props bag. Constructed once per `build_voice_rows` call;
/// then propagated through `VoiceRow`'s props so the row component
/// doesn't need to re-fetch the voice list on its own.
#[derive(Clone, PartialEq)]
struct VoiceRowModel {
    voice: DrumVoice,
    voice_label: String,
    /// Stable key for the row's keyed for-loop slot. Encodes the
    /// voice so add/remove/reorder remount cleanly; falls back to
    /// row index for the (degenerate) case of duplicate voice keys
    /// (shouldn't happen — voice list is dedup-on-insert).
    row_key: String,
    note_color: String,
}

fn build_voice_rows(id: PatternId) -> Vec<VoiceRowModel> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let Some(pattern) = project.patterns.get(&id) else {
        return Vec::new();
    };
    let body = match &pattern.body {
        PatternBody::Drum(b) => b,
        PatternBody::Pitched(_) => return Vec::new(),
    };
    let note_color = pattern_cell_color(id);
    // Reverse declaration order: voices[0] is the bottom-most row, so
    // the rendered list starts with the last voice (top) and ends
    // with voices[0] (bottom). Matches Reason / FL Studio convention.
    body.metadata
        .voices
        .iter()
        .enumerate()
        .rev()
        .map(|(idx, voice)| VoiceRowModel {
            voice: voice.clone(),
            voice_label: voice_short_label(voice),
            row_key: format!("v{idx}-{}", voice_short_label(voice)),
            note_color: note_color.clone(),
        })
        .collect()
}

fn pattern_cell_color(pattern_id: PatternId) -> String {
    use_store::<AppState>()
        .overlay
        .get()
        .pattern_color
        .get(&pattern_id)
        .cloned()
        .unwrap_or_else(|| FALLBACK_CELL_COLOR.to_string())
}

#[component]
fn VoiceRow(
    pattern_id: PatternId,
    voice: DrumVoice,
    voice_label: String,
    note_color: String,
) -> NodeHandle {
    let row_style = "display: flex; align-items: stretch; height: 28px; \
                     border-top: 1px solid rgba(255,255,255,0.04);"
        .to_string();
    let label_style = format!(
        "width: 36px; flex: 0 0 36px; \
         display: flex; align-items: center; justify-content: center; \
         font-size: 11px; font-weight: 600; \
         color: rgba(232,234,238,0.72); background: {bg0}; \
         border-right: 1px solid {line};",
        bg0 = theme::BG0,
        line = theme::LINE,
    );
    // Each closure inside the rsx! macro captures by move, so all
    // shared values get an `Rc<T>` per consumer (see the variant-tab
    // pattern in regions/pattern_editor/mod.rs for prior art). The
    // voice + label are read on every cell iteration plus the row-
    // label render.
    let voice = std::rc::Rc::new(voice);
    let voice_for_source = std::rc::Rc::clone(&voice);
    let voice_for_cell = std::rc::Rc::clone(&voice);
    let label = std::rc::Rc::new(voice_label);
    let label_for_title = std::rc::Rc::clone(&label);
    let label_for_text = std::rc::Rc::clone(&label);
    let color = std::rc::Rc::new(note_color);
    let color_for_cell = std::rc::Rc::clone(&color);
    rsx! {
        div { style: {row_style.clone()},
            div {
                style: {label_style.clone()},
                title: {(*label_for_title).clone()},
                {(*label_for_text).clone()}
            }
            for cell in build_cells_for_voice(pattern_id, (*voice_for_source).clone()) {
                StepCell {
                    key: cell.key(),
                    pattern_id: pattern_id,
                    voice: (*voice_for_cell).clone(),
                    cell: cell.clone(),
                    note_color: (*color_for_cell).clone(),
                }
            }
        }
    }
}

fn build_cells_for_voice(pattern_id: PatternId, voice: DrumVoice) -> Vec<StepCellModel> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let Some(pattern) = project.patterns.get(&pattern_id) else {
        return Vec::new();
    };
    let body = match &pattern.body {
        PatternBody::Drum(b) => b,
        PatternBody::Pitched(_) => return Vec::new(),
    };
    let variant = app
        .focused_variant
        .get()
        .filter(|v| body.variants.contains_key(v))
        .unwrap_or_else(|| pattern.default_variant.clone());
    let events: Vec<DrumEvent> = body.variants.get(&variant).cloned().unwrap_or_default();
    let grid = GridSpec::STRAIGHT_SIXTEENTH;
    let total_steps = total_steps_for_length(body.metadata.length, grid);

    // Beat / bar boundaries land at integer multiples of (subdivision /
    // beats_per_bar). In 4/4 at 1/16, that's every 4 steps for beats,
    // every 16 steps for bars.
    let bpb = project.tempo_map.beats_per_bar_at(rawdaw_model::time::MusicalTime::ZERO);
    let steps_per_beat = grid.subdivision as usize / 4;
    let steps_per_bar = steps_per_beat.saturating_mul(bpb as usize);

    (0..total_steps)
        .map(|step| {
            let beat_boundary = steps_per_beat > 0 && step % steps_per_beat == 0;
            let bar_boundary = steps_per_bar > 0 && step % steps_per_bar == 0;
            let kind = match event_at_step(&events, &voice, step, grid) {
                Some(ev) => StepCellKind::Filled {
                    note_id_value: ev.note_id.get(),
                    velocity: ev.velocity.get(),
                },
                None => StepCellKind::Empty,
            };
            StepCellModel {
                step,
                beat_boundary,
                bar_boundary,
                kind,
            }
        })
        .collect()
}

