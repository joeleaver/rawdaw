//! Piano-roll grid surface.
//!
//! Rows are scale-degree positions across the project's default
//! scale (7 degrees × 2 octaves anchored at
//! [`helpers::DEFAULT_ANCHOR_OCTAVE`]); columns are grid steps (1/16
//! straight in v1, configurable in a later polish pass).
//!
//! **Visible-on-grid contract.** Only notes with `PitchSpec::Scale {
//! octave: OctaveSpec::Anchored(o) }` whose `(degree, octave)` lands
//! inside the visible window render on the grid as `NoteBlock`s.
//! Click-to-insert produces this shape by default (P2 design
//! decision 3 + 13). Other PitchSpec/OctaveSpec combinations on
//! existing events are still realized by the engine and editable via
//! the inspector; they simply don't anchor to a row in v1's grid.
//!
//! **Interactions:**
//! - Click an empty cell → insert a default note at that
//!   `(degree, octave, time)` and focus it.
//! - Click a note → focus it (drives the inspector).

use rinch::prelude::*;

use rawdaw_model::chord::{ChordQuality, ChordSuffix};
use rawdaw_model::id::{NoteId, PatternId, VariantId};
use rawdaw_model::pattern::{OctaveSpec, PatternBody, PitchSpec, PitchedEvent};
use rawdaw_model::pitch::{Octave, PitchClass};
use rawdaw_model::realize::resolve::resolve_chord_degree;
use rawdaw_model::scale::Scale;
use rawdaw_model::time::MusicalTime;

use crate::chord_display::pitch_class_name;
use crate::parts::rgba;
use crate::pattern_actions::insert_pitched_event;
use crate::regions::pattern_editor::pitched::helpers::{
    default_pitched_event, snap_time_to_grid, GridSpec, DEFAULT_ANCHOR_OCTAVE, DEFAULT_PITCH_ROWS,
};
use crate::state::AppState;
use crate::theme;

use super::{FALLBACK_NOTE_COLOR, ROW_HEIGHT_PX};

/// Number of visible octaves: anchor-1, anchor, anchor+1. Three
/// octaves keeps the fixture patterns visible (bass-main uses
/// `Chord{Nearest}` notes that preview at the anchor octave;
/// lead-main has one `Anchored(5)` event that lands in the
/// anchor+1 octave when anchor=4 — but with anchor=3, that event
/// previews off-grid and is summarized in the off-grid banner).
const VISIBLE_OCTAVES: usize = 3;

/// Number of pitch rows visible at once. 7 diatonic degrees per
/// octave × `VISIBLE_OCTAVES` octaves. Wider chromatic / 25-row
/// view is a future polish item.
const VISIBLE_ROWS: usize = VISIBLE_OCTAVES * 7;

const _: () = assert!(VISIBLE_ROWS <= DEFAULT_PITCH_ROWS);

#[component]
pub fn PianoRoll(id: PatternId) -> NodeHandle {
    let surface_style = format!(
        "display: flex; flex-direction: column; \
         border: 1px solid {line}; border-radius: 4px; \
         background: {bg1}; overflow: auto; min-height: 0; flex: 1;",
        line = theme::LINE,
        bg1 = theme::BG1,
    );

    rsx! {
        div { style: {surface_style.clone()},
            for row_data in build_grid_row_headers(id) {
                PianoRollRow {
                    key: row_data.row.to_string(),
                    pattern_id: id,
                    row: row_data.row,
                    degree: row_data.degree,
                    octave: row_data.octave.0,
                    label: row_data.label.clone(),
                    is_tonic: row_data.is_tonic,
                    note_color: row_data.note_color.clone(),
                }
            }
        }
    }
}

fn pattern_note_color(pattern_id: PatternId) -> String {
    let app = use_store::<AppState>();
    app.overlay
        .get()
        .pattern_color
        .get(&pattern_id)
        .cloned()
        .unwrap_or_else(|| FALLBACK_NOTE_COLOR.to_string())
}

/// Headers (no per-cell vec). Cells get re-derived inside each row
/// component so the `for cell in ...` Fn-closure builds fresh on
/// every Effect re-run.
fn build_grid_row_headers(id: PatternId) -> Vec<GridRowHeader> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let Some(pattern) = project.patterns.get(&id) else {
        return Vec::new();
    };
    if !matches!(pattern.body, PatternBody::Pitched(_)) {
        return Vec::new();
    }
    let scale = project.default_key.clone();
    let note_color = pattern_note_color(id);
    (0..VISIBLE_ROWS)
        .map(|row| {
            let (degree, octave) = row_to_degree_octave(row);
            GridRowHeader {
                row,
                degree,
                octave,
                label: row_label(degree, octave, &scale),
                is_tonic: degree == 1,
                note_color: note_color.clone(),
            }
        })
        .collect()
}

#[derive(Clone, PartialEq)]
struct GridRowHeader {
    row: usize,
    degree: u8,
    octave: Octave,
    label: String,
    is_tonic: bool,
    note_color: String,
}

// ─── Row model ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Default)]
pub(super) enum RowCell {
    #[default]
    None,
    Empty { col: usize, beat_boundary: bool, bar_boundary: bool },
    Note { col: usize, note_id_value: u64, span_cols: usize },
}

impl RowCell {
    fn key(&self) -> String {
        match self {
            RowCell::None => "x".to_string(),
            RowCell::Empty { col, .. } => format!("e{col}"),
            RowCell::Note { col, note_id_value, .. } => format!("n{col}-{note_id_value}"),
        }
    }
}

fn filter_events_for_row(
    events: &[PitchedEvent],
    scale: &Scale,
    degree: u8,
    octave: Octave,
) -> Vec<PitchedEvent> {
    let preview = preview_chord_for(scale);
    events
        .iter()
        .filter(|e| {
            preview_row_for_event(e, scale, &preview)
                .map(|(d, o)| d == degree && o == octave)
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

/// Translate a `PitchedEvent` to a `(degree, octave)` row position
/// for *preview* rendering on the grid. The preview chord is `I` in
/// the project's default key (P2 design decision 4); voice-leading
/// octave-specs (Nearest / UpFromPrev / …) fall back to the anchor
/// octave so the user sees a stable placeholder. Returns `None` for:
/// - `Rest` / `Chromatic` events (no spatial position).
/// - `Chord` events whose chord-step doesn't resolve in the
///   preview chord (e.g. Eleventh in a triad).
/// - `Absolute` / `Chord` events whose realized pitch class doesn't
///   match any diatonic step of the project scale (a sharp pitch
///   in a major scale, etc.).
pub(super) fn preview_row_for_event(
    event: &PitchedEvent,
    scale: &Scale,
    preview: &PreviewChord,
) -> Option<(u8, Octave)> {
    match &event.spec {
        PitchSpec::Scale { degree, octave } => {
            let oct = octave_from_spec(octave);
            Some((degree.degree, oct))
        }
        PitchSpec::Chord { degree, octave } => {
            let pc = resolve_chord_degree(*degree, preview.root, &preview.suffix)?;
            let scale_degree = pitch_class_to_scale_degree(pc, scale)?;
            let oct = octave_from_spec(octave);
            Some((scale_degree, oct))
        }
        PitchSpec::Absolute { pitch_class, octave } => {
            let scale_degree = pitch_class_to_scale_degree(*pitch_class, scale)?;
            Some((scale_degree, *octave))
        }
        PitchSpec::Chromatic { .. } | PitchSpec::Rest => None,
    }
}

fn octave_from_spec(spec: &OctaveSpec) -> Octave {
    match spec {
        OctaveSpec::Anchored(o) => *o,
        OctaveSpec::Nearest
        | OctaveSpec::UpFromPrev
        | OctaveSpec::DownFromPrev
        | OctaveSpec::RelativeToRole => DEFAULT_ANCHOR_OCTAVE,
    }
}

/// Find the scale degree whose interval matches `pc` in `scale`.
/// Returns `None` for a pitch class that isn't diatonic in the
/// scale (the event still realizes through the engine but doesn't
/// have a preview row in the degree grid).
fn pitch_class_to_scale_degree(pc: PitchClass, scale: &Scale) -> Option<u8> {
    let target = ((pc.semitones_from_c() as i32 - scale.tonic.semitones_from_c() as i32)
        .rem_euclid(12)) as u8;
    scale
        .mode
        .intervals()
        .iter()
        .position(|&i| i == target)
        .map(|idx| (idx + 1) as u8)
}

/// Preview chord used to resolve `PitchSpec::Chord` events to a row.
/// Major `I` in major-leaning modes, minor `i` in minor-leaning
/// modes. Mirrors the realized-strip's preview helper but lives
/// here so the piano-roll doesn't import from a sibling.
pub(super) struct PreviewChord {
    pub root: PitchClass,
    pub suffix: ChordSuffix,
}

pub(super) fn preview_chord_for(scale: &Scale) -> PreviewChord {
    use rawdaw_model::scale::Mode::*;
    let quality = match scale.mode {
        Ionian | Lydian | Mixolydian | MajorPentatonic | WholeTone | Chromatic => {
            ChordQuality::Major
        }
        Aeolian | Dorian | Phrygian | Locrian | HarmonicMinor | MelodicMinor
        | PhrygianDominant | Altered | MinorPentatonic | Blues => ChordQuality::Minor,
        Lydian7 => ChordQuality::Dominant7,
        Custom { .. } => ChordQuality::Major,
    };
    PreviewChord {
        root: scale.tonic,
        suffix: ChordSuffix::new(quality),
    }
}

/// Returns `(degree, octave)` for a 0-indexed visible row. Row 0
/// is the highest pitch (degree 7 in `anchor+1`); the bottom-most
/// row is degree 1 in `anchor-(VISIBLE_OCTAVES-2)`. Three-octave
/// default: `anchor+1`, `anchor`, `anchor-1`.
fn row_to_degree_octave(row: usize) -> (u8, Octave) {
    let octave_idx = (row / 7) as i8;
    // Top octave_idx=0 → octave_above = VISIBLE_OCTAVES-2 (= 1 with
    // 3 octaves visible); bottom octave_idx=VISIBLE_OCTAVES-1 →
    // octave_above = -(VISIBLE_OCTAVES-2) (= -1 with 3 octaves).
    let octave_above = (VISIBLE_OCTAVES as i8 - 2) - octave_idx;
    let row_within = row % 7;
    let degree = (7 - row_within) as u8;
    (degree, Octave(DEFAULT_ANCHOR_OCTAVE.0 + octave_above))
}

fn row_label(degree: u8, octave: Octave, scale: &Scale) -> String {
    let pc = scale_degree_to_pc(degree, scale);
    format!("{} · {}{}", degree, pitch_class_name(pc), octave.0)
}

fn scale_degree_to_pc(degree: u8, scale: &Scale) -> rawdaw_model::pitch::PitchClass {
    let intervals = scale.mode.intervals();
    let idx = ((degree as usize).saturating_sub(1)) % intervals.len();
    let semis = scale.tonic.semitones_from_c() as i32 + intervals[idx] as i32;
    rawdaw_model::pitch::PitchClass::from_semitones_mod12(semis)
}

fn total_cols_for_length(length_ticks: i64, grid: GridSpec) -> usize {
    let step = grid.step_ticks().max(1);
    ((length_ticks / step).max(1)) as usize
}

/// Walk `events` in time order, emitting one `RowCell::Empty` per
/// uncovered column and one `RowCell::Note { span_cols }` per event
/// (spanning up to the next event's start or `total_cols`). Notes
/// whose `time` lands past `total_cols` are dropped — the pattern's
/// length defines what realizes (P2 design decision 11).
fn build_row_cells(events: &[PitchedEvent], total_cols: usize, grid: GridSpec) -> Vec<RowCell> {
    let mut cells = Vec::new();
    let step = grid.step_ticks().max(1);

    let mut sorted: Vec<&PitchedEvent> = events.iter().collect();
    sorted.sort_by_key(|e| e.time.as_ticks());

    let mut col = 0;
    let mut iter = sorted.into_iter();
    let mut next_event = iter.next();

    while col < total_cols {
        if let Some(ev) = next_event {
            let ev_col = (ev.time.as_ticks() / step).max(0) as usize;
            if ev_col >= total_cols {
                next_event = None;
                continue;
            }
            if ev_col == col {
                let raw_span = ((ev.duration.as_ticks() / step).max(1)) as usize;
                let span = raw_span.min(total_cols - col);
                cells.push(RowCell::Note {
                    col,
                    note_id_value: ev.note_id.get(),
                    span_cols: span,
                });
                col += span;
                next_event = iter.next();
                continue;
            }
        }
        cells.push(RowCell::Empty {
            col,
            beat_boundary: col % 4 == 0,
            bar_boundary: col % 16 == 0,
        });
        col += 1;
    }
    cells
}

// ─── Row component ───────────────────────────────────────────────────────

#[component]
fn PianoRollRow(
    pattern_id: PatternId,
    row: usize,
    degree: u8,
    /// Raw octave number (e.g. `3` for `Octave(3)`). Passed as `i8`
    /// because `Octave` doesn't impl `Default`, which the
    /// `#[component]` macro requires on every prop type.
    octave: i8,
    label: String,
    is_tonic: bool,
    note_color: String,
) -> NodeHandle {
    let _ = row;
    let row_style = format!(
        "display: flex; align-items: stretch; \
         height: {h}px; border-bottom: 1px solid {line};",
        h = ROW_HEIGHT_PX,
        line = theme::LINE,
    );
    let label_style = format!(
        "flex: 0 0 64px; \
         display: flex; align-items: center; padding: 0 8px; \
         font-size: 10px; font-variant-numeric: tabular-nums; \
         color: rgba(232,234,238,0.62); \
         background: {bg}; border-right: 1px solid {line}; \
         font-weight: {weight};",
        bg = if is_tonic { theme::BG0 } else { theme::BG1 },
        line = theme::LINE,
        weight = if is_tonic { 600 } else { 400 },
    );

    rsx! {
        div { style: {row_style.clone()},
            div { style: {label_style.clone()}, {label.clone()} }
            div {
                style: "flex: 1; display: flex; align-items: stretch; min-width: 0;",
                for cell in build_cells_for_row(pattern_id, degree, octave) {
                    CellSlot {
                        key: cell.key(),
                        pattern_id: pattern_id,
                        degree: degree,
                        octave: octave,
                        is_tonic: is_tonic,
                        cell: cell.clone(),
                        note_color: note_color.clone(),
                    }
                }
            }
        }
    }
}

fn build_cells_for_row(pattern_id: PatternId, degree: u8, octave: i8) -> Vec<RowCell> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let Some(pattern) = project.patterns.get(&pattern_id) else {
        return Vec::new();
    };
    let body = match &pattern.body {
        PatternBody::Pitched(b) => b,
        PatternBody::Drum(_) => return Vec::new(),
    };
    let variant = app
        .focused_variant
        .get()
        .filter(|v| body.variants.contains_key(v))
        .unwrap_or_else(|| pattern.default_variant.clone());
    let events = body.variants.get(&variant).cloned().unwrap_or_default();
    let grid = GridSpec::STRAIGHT_SIXTEENTH;
    let total_cols = total_cols_for_length(body.metadata.length.as_ticks(), grid);
    let scale = project.default_key.clone();
    let events_for_row = filter_events_for_row(&events, &scale, degree, Octave(octave));
    build_row_cells(&events_for_row, total_cols, grid)
}

#[component]
fn CellSlot(
    pattern_id: PatternId,
    degree: u8,
    octave: i8,
    is_tonic: bool,
    cell: RowCell,
    note_color: String,
) -> NodeHandle {
    // Pre-compute every per-prop value so the rsx macro's wrapping
    // closures don't all need to capture `cell` by move (which would
    // conflict on the second prop). The captured-by-onclick clone is
    // separate so the move-closure can own its own copy.
    let outer = cell_outer_style(&cell, is_tonic, &note_color);
    let title = cell_title(&cell, pattern_id, degree, octave);
    let inner_style = cell_inner_style(&cell, &note_color);
    let inner_text = cell_inner_text(&cell).to_string();
    let cell_for_click = cell.clone();
    rsx! {
        div {
            style: {outer.clone()},
            title: {title.clone()},
            onclick: move || handle_cell_click(&cell_for_click, pattern_id, degree, octave),
            span {
                style: {inner_style.clone()},
                {inner_text.clone()}
            }
        }
    }
}

fn cell_outer_style(cell: &RowCell, is_tonic: bool, note_color: &str) -> String {
    match cell {
        RowCell::None => String::new(),
        RowCell::Empty {
            beat_boundary,
            bar_boundary,
            ..
        } => {
            let border_color: &str = if *bar_boundary {
                theme::TEXT2
            } else if *beat_boundary {
                theme::LINE
            } else {
                "rgba(255,255,255,0.04)"
            };
            let bg: &str = if is_tonic { theme::BG0 } else { theme::BG1 };
            format!(
                "flex: 1 1 0; min-width: 0; cursor: pointer; \
                 background: {bg}; \
                 border-left: 1px solid {border_color}; \
                 box-sizing: border-box;",
            )
        }
        RowCell::Note {
            note_id_value,
            span_cols,
            ..
        } => {
            let span = (*span_cols).max(1);
            note_outer_style(*note_id_value, span, note_color)
        }
    }
}

fn note_outer_style(note_id_value: u64, span: usize, color: &str) -> String {
    let focused = use_store::<AppState>()
        .focused_pattern_note
        .get()
        .map(|n| n.get())
        == Some(note_id_value);
    // High-contrast fills: filled chip with a darker border when
    // unfocused; saturated fill + bright outline when focused. The
    // hex `color` flows through `parts::rgba` which expects
    // `#RRGGBB` — a previous iteration passed `theme::TEXT2` (an
    // rgba string) here, which silently parsed to black and made
    // notes invisible against the BG1 panel.
    let bg = if focused {
        rgba(color, 0.90)
    } else {
        rgba(color, 0.65)
    };
    let border = if focused {
        format!("2px solid {color}")
    } else {
        format!("1px solid {}", rgba(color, 0.85))
    };
    format!(
        "flex: {span} 1 0; min-width: 0; \
         background: {bg}; border-radius: 3px; \
         border: {border}; \
         box-sizing: border-box; cursor: pointer; \
         display: flex; align-items: center; padding: 0 6px;",
    )
}

fn cell_inner_style(cell: &RowCell, _note_color: &str) -> String {
    match cell {
        RowCell::Note { .. } => format!(
            // Dark text on the saturated note fill for contrast.
            "font-size: 11px; font-weight: 700; color: {text}; \
             white-space: nowrap; overflow: hidden; text-overflow: ellipsis;",
            text = theme::BG0,
        ),
        _ => "display: none;".to_string(),
    }
}

fn cell_inner_text(cell: &RowCell) -> &'static str {
    match cell {
        RowCell::Note { .. } => "·",
        _ => "",
    }
}

fn cell_title(cell: &RowCell, _pattern_id: PatternId, degree: u8, octave: i8) -> String {
    match cell {
        RowCell::None => String::new(),
        RowCell::Empty { col, .. } => {
            format!("Insert at degree {degree}, octave {octave}, col {col}")
        }
        RowCell::Note { note_id_value, .. } => format!("Note #{note_id_value}"),
    }
}

fn handle_cell_click(cell: &RowCell, pattern_id: PatternId, degree: u8, octave: i8) {
    match cell {
        RowCell::None => {}
        RowCell::Empty { col, .. } => {
            insert_at_cell(pattern_id, degree, Octave(octave), *col);
        }
        RowCell::Note { note_id_value, .. } => {
            use_store::<AppState>()
                .focused_pattern_note
                .set(Some(NoteId::new(*note_id_value)));
        }
    }
}

fn insert_at_cell(pattern_id: PatternId, degree: u8, octave: Octave, col: usize) {
    let app = use_store::<AppState>();
    let grid = GridSpec::STRAIGHT_SIXTEENTH;
    let step = grid.step_ticks();
    let time = snap_time_to_grid(MusicalTime::ticks(col as i64 * step), grid);
    let variant = app
        .focused_variant
        .get()
        .unwrap_or_else(|| pattern_default_variant(pattern_id));

    let inserted = std::rc::Rc::new(std::cell::Cell::new(None::<NoteId>));
    let inserted_capture = std::rc::Rc::clone(&inserted);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |project| {
        let note_id = project.id_allocators.alloc_note();
        let mut event = default_pitched_event(note_id, time, grid);
        event.spec = PitchSpec::Scale {
            degree: rawdaw_model::scale::ScaleDegree::new(degree),
            octave: OctaveSpec::Anchored(octave),
        };
        if insert_pitched_event(project, pattern_id, &variant_for_edit, event).is_some() {
            inserted_capture.set(Some(note_id));
        }
    }) {
        eprintln!("pattern_editor: insert note failed: {e}");
        return;
    }
    if let Some(nid) = inserted.take() {
        app.focused_pattern_note.set(Some(nid));
    }
}

fn pattern_default_variant(id: PatternId) -> VariantId {
    use_store::<AppState>()
        .project
        .get()
        .patterns
        .get(&id)
        .map(|p| p.default_variant.clone())
        .unwrap_or_else(VariantId::main)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawdaw_model::pattern::EventHumanization;
    use rawdaw_model::pitch::U7;
    use rawdaw_model::scale::ScaleDegree;
    use rawdaw_model::time::Duration;

    fn anchored_scale_event(id: u64, time_ticks: i64, degree: u8, octave: i8) -> PitchedEvent {
        PitchedEvent {
            note_id: NoteId::new(id),
            time: MusicalTime::ticks(time_ticks),
            duration: Duration::ticks(240),
            velocity: U7::HALF,
            articulation: None,
            humanization: EventHumanization::default(),
            spec: PitchSpec::Scale {
                degree: ScaleDegree::new(degree),
                octave: OctaveSpec::Anchored(Octave(octave)),
            },
        }
    }

    #[test]
    fn row_to_degree_octave_covers_visible_octaves() {
        let anchor = DEFAULT_ANCHOR_OCTAVE.0;
        for row in 0..VISIBLE_ROWS {
            let (degree, octave) = row_to_degree_octave(row);
            assert!((1..=7).contains(&degree));
            let span = VISIBLE_OCTAVES as i8 - 1;
            let half = span / 2;
            assert!(
                octave.0 >= anchor - half && octave.0 <= anchor + (span - half),
                "row {row} octave {} outside expected range",
                octave.0,
            );
        }
        // Top row = degree 7 in anchor+1. Bottom = degree 1 in
        // anchor-1 (with 3 visible octaves).
        let top_oct = Octave(DEFAULT_ANCHOR_OCTAVE.0 + 1);
        let bot_oct = Octave(DEFAULT_ANCHOR_OCTAVE.0 - 1);
        assert_eq!(row_to_degree_octave(0), (7, top_oct));
        assert_eq!(row_to_degree_octave(6), (1, top_oct));
        assert_eq!(row_to_degree_octave(7), (7, DEFAULT_ANCHOR_OCTAVE));
        assert_eq!(row_to_degree_octave(13), (1, DEFAULT_ANCHOR_OCTAVE));
        assert_eq!(row_to_degree_octave(14), (7, bot_oct));
        assert_eq!(row_to_degree_octave(20), (1, bot_oct));
    }

    #[test]
    fn preview_row_handles_scale_anchored_and_nearest() {
        let scale = Scale::major(PitchClass::C);
        let preview = preview_chord_for(&scale);
        let anchored = anchored_scale_event(1, 0, 5, 3);
        assert_eq!(
            preview_row_for_event(&anchored, &scale, &preview),
            Some((5, Octave(3))),
        );
        // Nearest octave-spec falls back to the anchor octave for
        // preview purposes (P2 design decision 4 + 13).
        let mut nearest = anchored.clone();
        nearest.spec = PitchSpec::Scale {
            degree: ScaleDegree::new(5),
            octave: OctaveSpec::Nearest,
        };
        assert_eq!(
            preview_row_for_event(&nearest, &scale, &preview),
            Some((5, DEFAULT_ANCHOR_OCTAVE)),
        );
    }

    #[test]
    fn preview_row_resolves_chord_under_tonic_chord() {
        // `Chord { Root }` under preview `I` in C major resolves to
        // pitch C → scale degree 1.
        let scale = Scale::major(PitchClass::C);
        let preview = preview_chord_for(&scale);
        let mut ev = anchored_scale_event(1, 0, 1, 3);
        ev.spec = PitchSpec::Chord {
            degree: rawdaw_model::chord::ChordDegree::new(
                rawdaw_model::chord::ChordStep::Root,
            ),
            octave: OctaveSpec::Nearest,
        };
        assert_eq!(
            preview_row_for_event(&ev, &scale, &preview),
            Some((1, DEFAULT_ANCHOR_OCTAVE)),
        );
        // `Chord { Fifth }` → pitch G → scale degree 5.
        ev.spec = PitchSpec::Chord {
            degree: rawdaw_model::chord::ChordDegree::new(
                rawdaw_model::chord::ChordStep::Fifth,
            ),
            octave: OctaveSpec::Nearest,
        };
        assert_eq!(
            preview_row_for_event(&ev, &scale, &preview),
            Some((5, DEFAULT_ANCHOR_OCTAVE)),
        );
    }

    #[test]
    fn preview_row_returns_none_for_rest_and_chromatic() {
        let scale = Scale::major(PitchClass::C);
        let preview = preview_chord_for(&scale);
        let mut rest = anchored_scale_event(1, 0, 1, 3);
        rest.spec = PitchSpec::Rest;
        assert_eq!(preview_row_for_event(&rest, &scale, &preview), None);

        let mut chromatic = anchored_scale_event(2, 0, 1, 3);
        chromatic.spec = PitchSpec::Chromatic {
            semitones_from_prev: 3,
        };
        assert_eq!(preview_row_for_event(&chromatic, &scale, &preview), None);
    }

    #[test]
    fn preview_row_returns_none_for_non_diatonic_pitch_class() {
        // C# is not diatonic in C major; an Absolute C# event has
        // no preview row in the degree grid (still realizes through
        // the engine, just doesn't anchor on the grid).
        let scale = Scale::major(PitchClass::C);
        let preview = preview_chord_for(&scale);
        let ev = PitchedEvent::absolute(
            NoteId::new(1),
            MusicalTime::ZERO,
            rawdaw_model::time::Duration::beats(1),
            rawdaw_model::pitch::U7::HALF,
            PitchClass::CSharp,
            Octave(3),
        );
        assert_eq!(preview_row_for_event(&ev, &scale, &preview), None);
    }

    #[test]
    fn build_row_cells_fills_empties_between_notes() {
        let grid = GridSpec::STRAIGHT_SIXTEENTH;
        let step = grid.step_ticks();
        let events = vec![
            anchored_scale_event(1, 0, 5, 3),
            {
                let mut e = anchored_scale_event(2, step * 4, 5, 3);
                e.duration = Duration::ticks(step * 2);
                e
            },
        ];
        let cells = build_row_cells(&events, 8, grid);
        assert_eq!(cells.len(), 1 + 3 + 1 + 2);
        match &cells[0] {
            RowCell::Note { note_id_value, span_cols, .. } => {
                assert_eq!(*note_id_value, 1);
                assert_eq!(*span_cols, 1);
            }
            _ => panic!("expected note at col 0"),
        }
        match &cells[4] {
            RowCell::Note { note_id_value, span_cols, .. } => {
                assert_eq!(*note_id_value, 2);
                assert_eq!(*span_cols, 2);
            }
            _ => panic!("expected note at col 4"),
        }
    }

    #[test]
    fn build_row_cells_clamps_note_span_to_remaining_cols() {
        let grid = GridSpec::STRAIGHT_SIXTEENTH;
        let step = grid.step_ticks();
        let mut ev = anchored_scale_event(1, step * 6, 5, 3);
        ev.duration = Duration::ticks(step * 10);
        let cells = build_row_cells(&[ev], 8, grid);
        assert_eq!(cells.len(), 7);
        match &cells[6] {
            RowCell::Note { span_cols, .. } => assert_eq!(*span_cols, 2),
            _ => panic!("expected note at col 6"),
        }
    }

    #[test]
    fn build_row_cells_drops_notes_past_pattern_length() {
        let grid = GridSpec::STRAIGHT_SIXTEENTH;
        let step = grid.step_ticks();
        let ev = anchored_scale_event(1, step * 100, 5, 3);
        let cells = build_row_cells(&[ev], 4, grid);
        assert_eq!(cells.len(), 4);
        assert!(cells.iter().all(|c| matches!(c, RowCell::Empty { .. })));
    }

    #[test]
    fn total_cols_for_length_round_count() {
        let grid = GridSpec::STRAIGHT_SIXTEENTH;
        let step = grid.step_ticks();
        assert_eq!(total_cols_for_length(15360, grid), 64);
        assert_eq!(total_cols_for_length(step / 2, grid), 1);
    }
}
